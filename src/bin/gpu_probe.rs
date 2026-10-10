use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

fn block_on<F: Future>(fut: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    let mut fut = Box::pin(fut);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => {
                if Instant::now() > deadline {
                    panic!("block_on exceeded 20s; the future never completed");
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

fn main() {
    println!("=== smith GPU diagnostic (enumeration only) ===");
    println!("environment:");
    for k in [
        "WGPU_BACKEND",
        "WGPU_ADAPTER_NAME",
        "WGPU_POWER_PREFERENCE",
        "WGPU_INSTANCE_BACKEND",
    ] {
        match std::env::var(k) {
            Ok(v) => println!("  {k} = {v}"),
            Err(_) => println!("  {k} = (unset)"),
        }
    }
    println!(
        "  stdout_is_terminal = {}",
        std::io::IsTerminal::is_terminal(&std::io::stdout())
    );

    let instance = wgpu::Instance::default();

    println!("\n=== adapters, Backends::all() ===");
    let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    if adapters.is_empty() {
        println!("  NONE ENUMERATED");
        println!("  NOTE: an empty result is NOT proof the hardware is absent.");
        println!("        Enumeration can fail when stdout is redirected or the");
        println!("        process has no interactive desktop session.");
    }
    for (n, a) in adapters.iter().enumerate() {
        let info = a.get_info();
        println!("\n  [{n}] {}", info.name);
        println!("      backend     = {:?}", info.backend);
        println!("      device_type = {:?}", info.device_type);
        println!("      driver      = {}", info.driver);
        println!("      driver_info = {}", info.driver_info);
        println!("      vendor_id   = {:#06x}", info.vendor);
        println!("      device_id   = {:#06x}", info.device);
        let l = a.limits();
        println!("      max_texture_dimension_2d        = {}", l.max_texture_dimension_2d);
        println!("      max_buffer_size                 = {}", l.max_buffer_size);
        println!("      max_compute_workgroup_storage   = {}", l.max_compute_workgroup_storage_size);
    }

    println!("\n=== per-backend adapter counts ===");
    for (label, backends) in [
        ("dx12", wgpu::Backends::DX12),
        ("vulkan", wgpu::Backends::VULKAN),
        ("gles", wgpu::Backends::GL),
    ] {
        let n = block_on(instance.enumerate_adapters(backends)).len();
        println!("  {label:<7} {n}");
    }

    println!("\nSCOPE: enumeration only. Device request, surface creation, and");
    println!("presentation require an interactive desktop session and remain open (B7).");
}
