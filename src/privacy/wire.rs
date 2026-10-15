//! Bounded local IPC infrastructure shared with the bundled frontend; no terminal semantics.
pub const MASK: &str = "░░░░░";
pub const MAX_MESSAGE: usize = 65536;
pub const MAX_FIELDS: usize = 32;
pub const MAX_VALUE: usize = 2048;

#[cfg(windows)]
pub mod windows_pipe {
    use std::time::{Duration, Instant};
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{CloseHandle, ERROR_IO_PENDING, ERROR_PIPE_BUSY, HANDLE, WAIT_OBJECT_0},
            Storage::FileSystem::{
                CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_MODE,
                OPEN_EXISTING,
            },
            System::{
                Pipes::{GetNamedPipeServerProcessId, WaitNamedPipeW},
                Threading::{CreateEventW, ResetEvent, WaitForMultipleObjects},
                IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            },
        },
    };
    pub struct Handle(pub HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: uniquely owns a successful Win32 handle; no pending operation outlives its guard.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    pub struct Pipe {
        pub handle: Handle,
        event: Handle,
    }
    impl Pipe {
        pub fn new(handle: Handle) -> Result<Self, String> {
            // SAFETY: creates a private unnamed manual-reset event without inherited handles.
            let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
                .map(Handle)
                .map_err(|_| "Cannot create privacy I/O event")?;
            Ok(Self { handle, event })
        }
        pub fn operation(&self) -> OVERLAPPED {
            // SAFETY: this event belongs exclusively to this pipe; previous I/O has completed.
            unsafe {
                let _ = ResetEvent(self.event.0);
            }
            OVERLAPPED {
                hEvent: self.event.0,
                ..Default::default()
            }
        }
        pub fn finish(
            &self,
            operation: &mut OVERLAPPED,
            started: windows::core::Result<()>,
            stop: Option<HANDLE>,
            timeout: u32,
        ) -> Result<u32, String> {
            if let Err(error) = started {
                if error.code() != ERROR_IO_PENDING.to_hresult() {
                    return Err("Privacy pipe operation failed".into());
                }
            }
            let mut events = vec![self.event.0];
            if let Some(stop) = stop {
                events.push(stop);
            }
            // SAFETY: event handles and OVERLAPPED storage remain live through completion/cancellation.
            unsafe {
                let completed = WaitForMultipleObjects(&events, false, timeout) == WAIT_OBJECT_0;
                if !completed {
                    let _ = CancelIoEx(self.handle.0, Some(operation));
                }
                let mut bytes = 0;
                // Always join cancelled I/O before releasing its buffers or OVERLAPPED storage.
                let result = GetOverlappedResult(self.handle.0, operation, &mut bytes, true);
                if !completed {
                    return Err("Privacy operation cancelled or timed out".into());
                }
                result.map_err(|_| "Privacy pipe operation failed")?;
                Ok(bytes)
            }
        }
        pub fn read_exact(
            &self,
            bytes: &mut [u8],
            stop: Option<HANDLE>,
            deadline: Instant,
        ) -> Result<(), String> {
            let mut offset = 0;
            while offset < bytes.len() {
                let timeout = remaining(deadline)?;
                let mut operation = self.operation();
                // SAFETY: writable buffer/OVERLAPPED stay live until finish joins I/O, including cancellation.
                let started = unsafe {
                    ReadFile(
                        self.handle.0,
                        Some(&mut bytes[offset..]),
                        None,
                        Some(&mut operation),
                    )
                };
                let count = self.finish(&mut operation, started, stop, timeout)? as usize;
                if count == 0 {
                    return Err("Privacy pipe closed".into());
                }
                offset += count;
            }
            Ok(())
        }
        pub fn write_all(
            &self,
            bytes: &[u8],
            stop: Option<HANDLE>,
            deadline: Instant,
        ) -> Result<(), String> {
            let mut offset = 0;
            while offset < bytes.len() {
                let timeout = remaining(deadline)?;
                let mut operation = self.operation();
                // SAFETY: read-only buffer/OVERLAPPED stay live until finish joins I/O, including cancellation.
                let started = unsafe {
                    WriteFile(
                        self.handle.0,
                        Some(&bytes[offset..]),
                        None,
                        Some(&mut operation),
                    )
                };
                let count = self.finish(&mut operation, started, stop, timeout)? as usize;
                if count == 0 {
                    return Err("Privacy pipe closed".into());
                }
                offset += count;
            }
            Ok(())
        }
    }
    fn remaining(deadline: Instant) -> Result<u32, String> {
        let duration = deadline.saturating_duration_since(Instant::now());
        if duration.is_zero() {
            return Err("Privacy message timed out".into());
        }
        Ok(duration.as_millis().clamp(1, 3000) as u32)
    }
    pub fn client(path: &str, expected_server: u32) -> Result<Pipe, String> {
        if !path.starts_with(r"\\.\pipe\JunaPrivacy-")
            || path.len() > 160
            || path.chars().any(char::is_control)
        {
            return Err("Invalid privacy endpoint".into());
        }
        let name = path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let deadline = Instant::now() + Duration::from_secs(1);
        let handle = loop {
            // SAFETY: owned terminated local pipe name; noninherited overlapped handle, no executable loading.
            match unsafe {
                CreateFileW(
                    PCWSTR(name.as_ptr()),
                    0xC0000000,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    None,
                )
            } {
                Ok(handle) => break Handle(handle),
                Err(error)
                    if error.code() == ERROR_PIPE_BUSY.to_hresult()
                        && Instant::now() < deadline =>
                {
                    // SAFETY: same local name; bounded wait permits another tab's report to finish.
                    unsafe {
                        let _ = WaitNamedPipeW(PCWSTR(name.as_ptr()), 100);
                    }
                }
                Err(_) => return Err("Privacy endpoint unavailable".into()),
            }
        };
        let mut server = 0;
        // SAFETY: handle is an open pipe and server is live writable PID storage.
        unsafe { GetNamedPipeServerProcessId(handle.0, &mut server) }
            .map_err(|_| "Privacy server identity unavailable")?;
        if server != expected_server || server == 0 {
            return Err("Privacy server identity mismatch".into());
        }
        Pipe::new(handle)
    }
}
