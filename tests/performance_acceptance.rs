//! Explicit release-mode timing probes; no terminal contents enter the report.
use juna::{gpu::block_on, render::Renderer, vt::TerminalState};
use std::time::Instant;

fn distribution(mut samples: Vec<f64>) -> serde_json::Value {
    samples.sort_by(f64::total_cmp);
    serde_json::json!({
        "samples": samples.len(),
        "median_us": samples[samples.len() / 2],
        "p95_us": samples[(samples.len() * 95 / 100).min(samples.len() - 1)],
    })
}

#[test]
#[ignore = "Explicit performance baseline; requires release mode and a GPU"]
fn viewport_preparation_baseline() {
    let debug_build = cfg!(debug_assertions);
    assert!(!debug_build, "Use cargo test --release for timing");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let info = adapter.get_info();
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut initialisation = Vec::new();
    for _ in 0..3 {
        let start = Instant::now();
        let atlas = juna::render::Atlas::new(&device, 16.).unwrap();
        std::hint::black_box(&atlas);
        initialisation.push(start.elapsed().as_secs_f64() * 1e6);
    }
    let mut renderer = Renderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        2048,
        (10., 22.),
        8.,
        16.,
    )
    .unwrap();
    let mut workloads = Vec::new();
    for (name, content) in [
        (
            "ascii",
            "The quick brown fox jumps over the lazy dog 0123456789 ",
        ),
        (
            "contrast",
            "\x1b[97;103mLight coloured box with readable text 0123456789 ",
        ),
        ("unicode", "Unicode: e\u{301} 日本語 Ελληνικά Кириллица "),
    ] {
        let mut terminal = TerminalState::new(140, 45);
        let bytes = content.repeat(250).into_bytes();
        let start = Instant::now();
        terminal.advance(&bytes);
        let parse_us = start.elapsed().as_secs_f64() * 1e6;
        let snapshot = terminal.snapshot(0);
        for _ in 0..10 {
            renderer.build(&snapshot, 1440, 1024);
        }
        let mut snapshots = Vec::new();
        let mut frames = Vec::new();
        for _ in 0..200 {
            let start = Instant::now();
            std::hint::black_box(terminal.snapshot(0));
            snapshots.push(start.elapsed().as_secs_f64() * 1e6);
            let start = Instant::now();
            renderer.build(std::hint::black_box(&snapshot), 1440, 1024);
            std::hint::black_box(&renderer);
            frames.push(start.elapsed().as_secs_f64() * 1e6);
        }
        workloads.push(serde_json::json!({
            "name": name, "bytes": bytes.len(), "parse_us": parse_us,
            "snapshot": distribution(snapshots), "build": distribution(frames),
        }));
    }
    let report = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"), "commit": env!("JUNA_BUILD_COMMIT"),
        "adapter": info.name, "backend": format!("{:?}", info.backend),
        "cols": 140, "rows": 45, "font": "IBM Plex Mono", "font_px": 16,
        "atlas_initialisation": distribution(initialisation), "workloads": workloads,
        "limits": "CPU snapshot/build only; excludes GPU submission, presentation and input latency. Parse timings are single exploratory samples. No comparative terminal ranking.",
    });
    let path = std::env::var_os("JUNA_PERF_REPORT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "target/acceptance/performance/render-baseline.json".into());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("{report}");
}
