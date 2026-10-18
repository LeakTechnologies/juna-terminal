mod capture;
mod clock;
mod fixture;
mod platform;
mod report;
mod resources;
use serde::Serialize;
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use windows::Win32::Foundation::HWND;
const LIMIT: Duration = Duration::from_secs(30);
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        // This path was freshly created inside the new, canonicalised output directory.
        // It contains only this run's synthetic sessions, including their isolated app logs.
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("juna-bench: synthetic staging cleanup failed; inspect the selected output directory");
            }
        }
    }
}
pub(super) fn publish(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("pending");
    std::fs::write(&temp, bytes)
        .and_then(|_| std::fs::rename(temp, path))
        .map_err(|_| "Synthetic coordination write failed".into())
}
pub(super) fn wait_file(path: &Path, timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    while !path.is_file() {
        if start.elapsed() > timeout {
            return Err("Synthetic coordination timed out".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}
fn bounded_read(path: &Path) -> Result<String, String> {
    let mut value = String::new();
    std::fs::File::open(path)
        .map_err(|_| "Coordination read failed")?
        .take(4097)
        .read_to_string(&mut value)
        .map_err(|_| "Invalid coordination text")?;
    if value.len() > 4096 {
        return Err("Coordination record too large".into());
    }
    Ok(value)
}
#[derive(Serialize)]
struct Trial {
    workload: String,
    client_width: i32,
    client_height: i32,
    warm_fixture_startup_ms: f64,
    idle: resources::Resources,
    after: resources::Resources,
    parent_cpu_delta_seconds: f64,
    elapsed_seconds: f64,
    producer_bytes: Option<usize>,
    response_proxy_ms: Option<report::Summary>,
    child_received_ms: Option<report::Summary>,
    performance_counter_hz: i64,
}
#[derive(Serialize)]
struct Report {
    schema_version: u32,
    benchmark_version: String,
    benchmark_build_base_commit: String,
    benchmark_sources_sha256: String,
    target_sha256: String,
    target_version: String,
    profile: String,
    summaries: Vec<Aggregate>,
    limitations: Vec<String>,
    trials: Vec<Trial>,
}
#[derive(Serialize)]
struct Aggregate {
    workload: String,
    startup_ms: report::Summary,
    idle_private_mib: report::Summary,
    after_private_mib: report::Summary,
    parent_cpu_seconds: report::Summary,
    elapsed_seconds: report::Summary,
    response_proxy_ms: Option<report::Summary>,
}
fn aggregates(trials: &[Trial]) -> Result<Vec<Aggregate>, String> {
    ["idle", "ascii", "unicode", "colour", "flood"]
        .into_iter()
        .map(|mode| {
            let selected: Vec<_> = trials.iter().filter(|t| t.workload == mode).collect();
            let timings: Vec<_> = selected
                .iter()
                .filter_map(|t| t.response_proxy_ms.as_ref())
                .flat_map(|s| s.samples.iter().copied())
                .collect();
            Ok(Aggregate {
                workload: mode.into(),
                startup_ms: report::summary(
                    selected.iter().map(|t| t.warm_fixture_startup_ms).collect(),
                )?,
                idle_private_mib: report::summary(
                    selected
                        .iter()
                        .map(|t| t.idle.private_bytes as f64 / 1_048_576.)
                        .collect(),
                )?,
                after_private_mib: report::summary(
                    selected
                        .iter()
                        .map(|t| t.after.private_bytes as f64 / 1_048_576.)
                        .collect(),
                )?,
                parent_cpu_seconds: report::summary(
                    selected
                        .iter()
                        .map(|t| t.parent_cpu_delta_seconds)
                        .collect(),
                )?,
                elapsed_seconds: report::summary(
                    selected.iter().map(|t| t.elapsed_seconds).collect(),
                )?,
                response_proxy_ms: if timings.is_empty() {
                    None
                } else {
                    Some(report::summary(timings)?)
                },
            })
        })
        .collect()
}
struct Options {
    juna: PathBuf,
    out: PathBuf,
    full: bool,
}
fn options(args: &[OsString]) -> Result<Options, String> {
    let (mut juna, mut out, mut full, mut profile) = (None, None, false, false);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = iter.next().ok_or("Each option requires a value")?;
        match arg.to_str() {
            Some("--juna") if juna.is_none() => juna = Some(PathBuf::from(value)),
            Some("--out") if out.is_none() => out = Some(PathBuf::from(value)),
            Some("--profile") if !profile => {
                profile = true;
                full = match value.to_str() {
                    Some("quick") => false,
                    Some("full") => true,
                    _ => return Err("Profile must be quick or full".into()),
                };
            }
            _ => return Err("Unknown or repeated option".into()),
        }
    }
    Ok(Options {
        juna: juna.ok_or("--juna is required")?,
        out: out.ok_or("--out is required")?,
        full,
    })
}
fn trial(
    exe: &Path,
    base: &Path,
    mode: &str,
    samples: u32,
    blocks: usize,
) -> Result<Trial, String> {
    let root = base.join(juna::settings::new_id());
    std::fs::create_dir(&root).map_err(|_| "Fixture staging failed")?;
    let directory = root.join("Juna");
    std::fs::create_dir(&directory).map_err(|_| "Fixture settings failed")?;
    let settings = juna::settings::Settings {
        show_git_status: false,
        ..Default::default()
    };
    std::fs::write(
        directory.join("settings.json"),
        serde_json::to_vec(&settings).map_err(|_| "Settings encoding failed")?,
    )
    .map_err(|_| "Fixture settings failed")?;
    let start = Instant::now();
    let child = Command::new(exe)
        .arg("--shell")
        .arg(std::env::current_exe().map_err(|_| "Benchmark unavailable")?)
        .arg("--")
        .arg("--fixture")
        .arg(&root)
        .arg(mode)
        .arg(samples.to_string())
        .arg(blocks.to_string())
        .env("LOCALAPPDATA", &root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Juna launch failed")?;
    let mut app = platform::Owned {
        child,
        hwnd: HWND::default(),
    };
    app.find_window()?;
    let mut rect = windows::Win32::Foundation::RECT::default();
    // SAFETY: the freshly owned window is live and rect is a writable native rectangle.
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetClientRect(app.hwnd, &mut rect)
            .map_err(|_| "Viewport query failed")?;
    }
    wait_file(&root.join("ready"), LIMIT)?;
    let startup = start.elapsed().as_secs_f64() * 1000.;
    let mut observer = platform::Observer::new(&app, &root)?;
    observer.expect(0)?;
    std::thread::sleep(Duration::from_millis(500));
    let idle = resources::read(&app.child)?;
    let begin = Instant::now();
    let frequency = clock::frequency()?;
    publish(&root.join("go"), b"go")?;
    let (mut times, mut receipts) = (Vec::new(), Vec::new());
    let producer_bytes = if matches!(mode, "ascii" | "unicode" | "colour") {
        observer.expect(1)?;
        wait_file(&root.join("complete"), LIMIT)?;
        Some(
            bounded_read(&root.join("bytes"))?
                .parse::<usize>()
                .map_err(|_| "Invalid producer count")?,
        )
    } else {
        if mode == "flood" {
            std::thread::sleep(Duration::from_millis(250));
        }
        for epoch in 1..=samples {
            observer.request(epoch)?;
            let injection = clock::ticks()?;
            platform::key(&app, false)?;
            wait_file(&root.join(format!("received-{epoch}")), LIMIT)?;
            let received = bounded_read(&root.join(format!("received-{epoch}")))?
                .parse()
                .map_err(|_| "Invalid child receipt counter")?;
            receipts.push(clock::milliseconds(injection, received, frequency)?);
            let observed = observer.observed(epoch)?;
            // Reject an incidental/stale colour detected before the fixture actually received this key.
            clock::milliseconds(received, observed, frequency)?;
            times.push(clock::milliseconds(injection, observed, frequency)?);
            platform::key(&app, true)?;
            std::thread::sleep(Duration::from_millis(30));
        }
        None
    };
    let elapsed = begin.elapsed().as_secs_f64();
    let after = resources::read(&app.child)?;
    let cpu = (after.cpu_seconds - idle.cpu_seconds).max(0.);
    observer.finish()?;
    publish(&root.join("stop"), b"stop")?;
    app.finish()?;
    drop(app);
    std::fs::remove_dir_all(&root).map_err(|_| "Synthetic cleanup failed")?;
    Ok(Trial {
        workload: mode.into(),
        client_width: rect.right - rect.left,
        client_height: rect.bottom - rect.top,
        warm_fixture_startup_ms: startup,
        idle,
        after,
        parent_cpu_delta_seconds: cpu,
        elapsed_seconds: elapsed,
        producer_bytes,
        response_proxy_ms: if times.is_empty() {
            None
        } else {
            Some(report::summary(times)?)
        },
        child_received_ms: if receipts.is_empty() {
            None
        } else {
            Some(report::summary(receipts)?)
        },
        performance_counter_hz: frequency,
    })
}
pub fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!("Juna benchmark (Windows x64)\n  juna-bench --juna <juna.exe> --out <new-directory> --profile quick|full\nFresh synthetic windows only. Keep desktop settings fixed; avoid competing workloads. Local report.json and report.html. Quick: 3 trials, 5 response samples; full: 5 trials, 30 samples. No physical latency or GPU memory claims.");
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--version" {
        println!(
            "juna-bench {} {}",
            env!("CARGO_PKG_VERSION"),
            env!("JUNA_BUILD_COMMIT")
        );
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--fixture") {
        if args.len() != 5 {
            return Err("Invalid fixture arguments".into());
        }
        return fixture::run(
            Path::new(&args[1]),
            args[2].to_str().ok_or("Invalid mode")?,
            args[3]
                .to_str()
                .ok_or("Invalid samples")?
                .parse()
                .map_err(|_| "Invalid samples")?,
            args[4]
                .to_str()
                .ok_or("Invalid blocks")?
                .parse()
                .map_err(|_| "Invalid blocks")?,
        );
    }
    if args.first().is_some_and(|a| a == "--observe") {
        if args.len() != 4 {
            return Err("Invalid capture arguments".into());
        }
        let pid = args[1]
            .to_str()
            .ok_or("Invalid PID")?
            .parse()
            .map_err(|_| "Invalid PID")?;
        let hwnd = args[2]
            .to_str()
            .ok_or("Invalid HWND")?
            .parse::<usize>()
            .map_err(|_| "Invalid HWND")?;
        let root = Path::new(&args[3]);
        let result = platform::observe(pid, HWND(hwnd as *mut _), root);
        if let Err(error) = &result {
            let _ = publish(&root.join("observer-error"), error.as_bytes());
        }
        return result;
    }
    let options = options(&args)?;
    let metadata = std::fs::metadata(&options.juna).map_err(|_| "Target unavailable")?;
    if !metadata.is_file() || metadata.len() > 128 * 1024 * 1024 {
        return Err("Target must be an executable no larger than 128 MiB".into());
    }
    let image = std::fs::read(&options.juna).map_err(|_| "Executable read failed")?;
    let identity = juna::release_image::version_strings(&image)
        .map_err(|_| "Target is not a versioned Windows executable")?;
    if identity.get("ProductName").map(String::as_str) != Some("Juna Terminal") {
        return Err("This adapter requires Juna Terminal".into());
    }
    let hash = juna::updates::hash(&image)?;
    drop(image);
    std::fs::create_dir(&options.out)
        .map_err(|_| "Output directory must be new and its parent must exist")?;
    // The synthetic child starts in Juna's home directory, so coordination paths must be absolute.
    let output = std::fs::canonicalize(&options.out).map_err(|_| "Output directory unavailable")?;
    let scratch = output.join("synthetic");
    std::fs::create_dir(&scratch).map_err(|_| "Scratch directory failed")?;
    let _scratch = Scratch(scratch.clone());
    let (repeats, samples, blocks) = if options.full {
        (5, 30, 128)
    } else {
        (3, 5, 32)
    };
    let mut trials = Vec::new();
    println!("Warming up one isolated window...");
    trial(&options.juna, &scratch, "idle", 1, blocks)?;
    for mode in ["idle", "ascii", "unicode", "colour", "flood"] {
        for repeat in 0..repeats {
            println!("Measuring {mode} ({}/{repeats})...", repeat + 1);
            trials.push(trial(&options.juna, &scratch, mode, samples, blocks)?);
        }
    }
    std::fs::remove_dir(&scratch).map_err(|_| "Scratch cleanup failed")?;
    let summaries = aggregates(&trials)?;
    let source = [
        include_bytes!("mod.rs").as_slice(),
        include_bytes!("capture.rs").as_slice(),
        include_bytes!("clock.rs").as_slice(),
        include_bytes!("fixture.rs").as_slice(),
        include_bytes!("platform.rs").as_slice(),
        include_bytes!("report.rs").as_slice(),
        include_bytes!("resources.rs").as_slice(),
        include_bytes!("../bin/juna-bench.rs").as_slice(),
    ]
    .concat();
    let source_hash = juna::updates::hash(&source)?;
    let report=Report{schema_version:1,benchmark_version:env!("CARGO_PKG_VERSION").into(),benchmark_build_base_commit:env!("JUNA_BUILD_COMMIT").into(),benchmark_sources_sha256:source_hash,target_sha256:hash,target_version:identity.get("ProductVersion").cloned().unwrap_or_else(||"unknown".into()),profile:if options.full{"full"}else{"quick"}.into(),summaries,limitations:vec![
        "Windows x64 Juna adapter only; cross-terminal equivalence is not yet established.".into(),
        "Input timing is posted A key -> unique colour observed through window capture plus local coordination I/O/polling; not physical keyboard/display latency. Child receipt is timestamped before output with the same-machine performance counter; cross-process ordering uncertainty is one tick. Only deltas are exported.".into(),
        "Fresh isolated settings: IBM Plex Mono 16, Catppuccin Mocha, Git bar disabled; default window dimensions and host DPI. Keep desktop/GPU/power settings fixed. No private path/transcript/pixel export.".into(),
        "One warm-up excluded. Executable hashing warms file cache. Startup ends at synthetic child ready, not first paint. Memory is parent-process only; excludes capture helper, shell and GPU. Region categories are not allocation call-stack attribution.".into(),
        "Output bytes are producer bytes, not a byte-exact ConPTY transport counter. Final marker proves final response observed, not every intermediate frame. Trials include coordination overhead; small-sample p95 is exploratory.".into(),
        "True Ctrl+C response, scroll latency, multiple tabs, battery/power and PresentMon/WPR comparisons remain unmeasured.".into(),
    ],trials};
    let json = serde_json::to_string_pretty(&report).map_err(|_| "Report encoding failed")?;
    publish(&options.out.join("report.json"), json.as_bytes())?;
    publish(
        &options.out.join("report.html"),
        report::html(&json).as_bytes(),
    )?;
    println!("Reports written. This baseline does not imply a performance improvement.");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_rejects_unknown_duplicate_and_missing() {
        for args in [
            vec!["--juna"],
            vec!["--bad", "x"],
            vec!["--juna", "a", "--juna", "b", "--out", "c"],
            vec!["--juna", "a", "--out", "b", "--profile", "fast"],
        ] {
            assert!(options(&args.into_iter().map(OsString::from).collect::<Vec<_>>()).is_err());
        }
    }
    #[test]
    fn deadlines_fail_without_creating_a_success_record() {
        let root = std::env::temp_dir().join(format!("juna-bench-{}", juna::settings::new_id()));
        std::fs::create_dir(&root).unwrap();
        let cleanup = Scratch(root.clone());
        let start = Instant::now();
        assert!(wait_file(&root.join("absent"), Duration::from_millis(10)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(!root.join("report.json").exists());
        assert!(platform::observe(0, HWND::default(), &root).is_err());
        assert!(fixture::run(&root, "unknown", 1, 1).is_err());
        assert!(fixture::run(&root, "idle", 31, 1).is_err());
        drop(cleanup);
        assert!(!root.exists());
    }
}
