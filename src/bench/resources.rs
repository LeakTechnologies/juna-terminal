use serde::Serialize;
use std::{mem::size_of, os::windows::io::AsRawHandle, process::Child};
use windows::Win32::{
    Foundation::{FILETIME, HANDLE},
    System::{
        Memory::{
            VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE, MEM_MAPPED,
            MEM_PRIVATE,
        },
        ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
        },
        SystemInformation::{GetSystemInfo, SYSTEM_INFO},
        Threading::GetProcessTimes,
    },
};
#[derive(Serialize)]
pub struct Resources {
    pub private_bytes: usize,
    pub working_set_bytes: usize,
    pub peak_working_set_bytes: usize,
    pub cpu_seconds: f64,
    pub committed_private_bytes: usize,
    pub committed_mapped_bytes: usize,
    pub committed_image_bytes: usize,
    pub private_regions_over_16_mib: usize,
}
pub fn read(child: &Child) -> Result<Resources, String> {
    let handle = HANDLE(child.as_raw_handle());
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: the child owns a live process handle; all output structures are writable and correctly sized.
    unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
        .map_err(|_| "Memory counters unavailable")?;
        GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user)
            .map_err(|_| "CPU counters unavailable")?;
    }
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    let mut result = Resources {
        private_bytes: counters.PrivateUsage,
        working_set_bytes: counters.WorkingSetSize,
        peak_working_set_bytes: counters.PeakWorkingSetSize,
        cpu_seconds: (ticks(kernel) + ticks(user)) as f64 / 10_000_000.,
        committed_private_bytes: 0,
        committed_mapped_bytes: 0,
        committed_image_bytes: 0,
        private_regions_over_16_mib: 0,
    };
    let mut system = SYSTEM_INFO::default();
    // SAFETY: system is a correctly sized writable native structure; no process memory is read.
    unsafe {
        GetSystemInfo(&mut system);
    }
    let maximum = system.lpMaximumApplicationAddress as usize;
    let mut address = system.lpMinimumApplicationAddress as usize;
    // No page contents, addresses or mapped paths enter the report.
    while address <= maximum {
        let mut region = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: queries metadata only for the owned child; region has the documented size and storage.
        let size = unsafe {
            VirtualQueryEx(
                handle,
                Some(address as *const _),
                &mut region,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if size == 0 {
            return Err("Virtual memory classification incomplete".into());
        }
        if region.State == MEM_COMMIT {
            match region.Type {
                MEM_PRIVATE => {
                    result.committed_private_bytes += region.RegionSize;
                    if region.RegionSize >= 16 * 1024 * 1024 {
                        result.private_regions_over_16_mib += 1;
                    }
                }
                MEM_MAPPED => result.committed_mapped_bytes += region.RegionSize,
                MEM_IMAGE => result.committed_image_bytes += region.RegionSize,
                _ => (),
            }
        }
        let next = (region.BaseAddress as usize)
            .checked_add(region.RegionSize)
            .ok_or("Region overflow")?;
        if next <= address {
            return Err("Region query made no progress".into());
        }
        address = next;
    }
    Ok(result)
}
