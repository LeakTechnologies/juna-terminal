//! Juna-owned lean presentation over the separately maintained Fastfetch detector.
mod presentation;
mod privacy;
use serde_json::{json, Value};
use std::{
    io::{self, IsTerminal, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
const DETECTOR: &[u8] = include_bytes!("../assets/detector.bin");
const COMPACT: &str = include_str!("../assets/logo-compact.txt");
const FULL: &str = include_str!("../assets/logo-full.txt");
#[cfg(windows)]
fn hash(bytes: &[u8]) -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};
    let mut digest = [0u8; 32];
    // SAFETY: the Windows SHA-256 pseudo-handle is valid; input/output are owned live slices and output is exactly 32 bytes.
    unsafe {
        BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, bytes, &mut digest)
            .ok()
            .map_err(|_| "Payload hash failed")?;
    }
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}
#[cfg(not(windows))]
fn hash(_: &[u8]) -> Result<String, String> {
    Err("This bundled detector supports Windows x64".into())
}
fn detector() -> Result<PathBuf, String> {
    let digest = hash(DETECTOR)?;
    let base = std::env::var_os("LOCALAPPDATA").ok_or("User tool cache is unavailable")?;
    let folder = PathBuf::from(base)
        .join("Juna/tools/native")
        .join(&digest[..16]);
    std::fs::create_dir_all(&folder).map_err(|_| "Cannot create the user tool cache")?;
    let file = folder.join("detector.exe");
    let valid = (|| {
        let mut bytes = Vec::new();
        std::fs::File::open(&file)
            .ok()?
            .take(DETECTOR.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes.len() == DETECTOR.len() && hash(&bytes).ok()? == digest)
    })()
    .unwrap_or(false);
    if !valid {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Tool cache clock is unavailable")?
            .as_nanos();
        let temp = folder.join(format!(".detector-{}-{nonce}.tmp", std::process::id()));
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| "Cannot stage the bundled detector")?;
        out.write_all(DETECTOR)
            .map_err(|_| "Cannot write the bundled detector")?;
        out.sync_all()
            .map_err(|_| "Cannot flush the bundled detector")?;
        drop(out);
        if std::fs::rename(&temp, &file).is_err() {
            let _ = std::fs::remove_file(&temp);
            return Err("Cannot install the bundled detector".into());
        }
    }
    Ok(file)
}
#[cfg(windows)]
fn width() -> usize {
    use windows::Win32::System::Console::{
        GetConsoleScreenBufferInfo, GetStdHandle, CONSOLE_SCREEN_BUFFER_INFO, STD_OUTPUT_HANDLE,
    };
    let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
    // SAFETY: standard handle lookup has no pointer input, and info is writable/live for the synchronous query.
    unsafe {
        GetStdHandle(STD_OUTPUT_HANDLE)
            .and_then(|h| GetConsoleScreenBufferInfo(h, &mut info))
            .map(|_| {
                (i32::from(info.srWindow.Right) - i32::from(info.srWindow.Left) + 1).clamp(20, 512)
                    as usize
            })
            .unwrap_or(80)
    }
}
#[cfg(not(windows))]
fn width() -> usize {
    80
}
fn detect(details: bool) -> Result<Value, String> {
    let detector = detector()?;
    let disk = if !details {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let drive = if drive.len() == 2
            && drive.as_bytes()[0].is_ascii_alphabetic()
            && drive.ends_with(':')
        {
            drive
        } else {
            "C:".into()
        };
        json!({"type":"Disk","folders":format!("{drive}\\")})
    } else {
        json!("Disk")
    };
    let mut modules = vec![
        json!("OS"),
        json!("Kernel"),
        json!("CPU"),
        json!("GPU"),
        json!("Memory"),
        disk,
        json!("Uptime"),
    ];
    if details {
        modules.push(json!({"type":"LocalIp", "showIpv4":true, "showIpv6":true, "showMac":true}));
        modules.extend(
            [
                "Shell", "Display", "WM", "WMTheme", "Theme", "Icons", "Font", "Cursor",
                "Terminal", "Swap", "Locale", "Bios", "Board", "Battery",
            ]
            .into_iter()
            .map(|module| json!(module)),
        );
    }
    let profile = json!({"modules":modules});
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Profile clock unavailable")?
        .as_nanos();
    let path = detector
        .parent()
        .ok_or("Detector folder unavailable")?
        .join(format!(".report-{}-{nonce}.json", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "Cannot stage the lean report profile")?;
    file.write_all(&serde_json::to_vec(&profile).map_err(|_| "Cannot encode report profile")?)
        .map_err(|_| "Cannot write report profile")?;
    drop(file);
    struct Profile(PathBuf);
    impl Drop for Profile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _profile = Profile(path.clone());
    let mut command = Command::new(detector);
    command
        .arg("--config")
        .arg(&path)
        .args(["--format", "json", "--logo", "none"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Bundled detector could not start")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Detector output is unavailable")?;
    let (tx, rx) = mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = tx.send(result);
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let received = loop {
        if let Ok(result) = rx.try_recv() {
            break result.map_err(|_| "Detector output read failed");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Detector timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let bytes = received?;
    if bytes.len() > 1024 * 1024 {
        let _ = child.kill();
    }
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| "Detector exit status unavailable")?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Detector shutdown timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let _ = reader.join();
    if bytes.len() > 1024 * 1024 || !status.success() {
        return Err("Detector did not return a complete report".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Detector returned invalid JSON".into())
}
fn text(value: &Value, key: &str) -> String {
    value[key]
        .as_str()
        .map(clean)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unavailable".into())
}
fn clean(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .take(160)
        .collect()
}
fn usage(result: &Value) -> String {
    match (result["used"].as_u64(), result["total"].as_u64()) {
        (Some(used), Some(total)) if total > 0 => format!(
            "{:.1} / {:.1} GiB ({:.0}%)",
            used as f64 / 1073741824.,
            total as f64 / 1073741824.,
            used as f64 / total as f64 * 100.
        ),
        _ => "Unavailable".into(),
    }
}
fn report(data: &Value, details: bool) -> Vec<(String, String)> {
    let result = |kind: &str| {
        data.as_array()
            .and_then(|items| items.iter().find(|item| item["type"] == kind))
            .map(|item| &item["result"])
            .unwrap_or(&Value::Null)
    };
    let cpu = result("CPU");
    let mut os = text(result("OS"), "prettyName");
    if let Some(build) = result("Kernel")["release"].as_str() {
        os.push_str(&format!(" (build {})", clean(build)));
    }
    let mut rows = vec![
        ("OS".into(), os),
        (
            "CPU".into(),
            format!(
                "{} ({} cores / {} threads)",
                text(cpu, "cpu"),
                cpu["cores"]["physical"]
                    .as_u64()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into()),
                cpu["cores"]["logical"]
                    .as_u64()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into())
            ),
        ),
    ];
    let gpus = result("GPU").as_array().cloned().unwrap_or_default();
    let physical = gpus
        .iter()
        .any(|g| matches!(g["type"].as_str(), Some("Discrete" | "Integrated")));
    let gpu: Vec<_> = gpus
        .iter()
        .filter(|g| {
            details || !physical || matches!(g["type"].as_str(), Some("Discrete" | "Integrated"))
        })
        .map(|g| {
            let name = text(g, "name");
            match g["memory"]["dedicated"]["total"]
                .as_u64()
                .filter(|&n| n > 0)
            {
                Some(n) => format!("{name} ({:.1} GiB VRAM)", n as f64 / 1073741824.),
                None => name,
            }
        })
        .collect();
    rows.push((
        "GPU".into(),
        if gpu.is_empty() {
            "Unavailable".into()
        } else {
            gpu.join("; ")
        },
    ));
    rows.push(("Memory".into(), usage(result("Memory"))));
    let disks = result("Disk").as_array().cloned().unwrap_or_default();
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let disk = disks
        .iter()
        .find(|disk| {
            disk["mountpoint"]
                .as_str()
                .is_some_and(|path| path.trim_end_matches('\\').eq_ignore_ascii_case(&drive))
        })
        .map(|disk| usage(&disk["bytes"]))
        .unwrap_or_else(|| "Unavailable".into());
    rows.push(("System disk".into(), disk));
    let uptime = result("Uptime")["uptime"]
        .as_u64()
        .map(|ms| {
            let minutes = ms / 60000;
            format!(
                "{}d {}h {}m",
                minutes / 1440,
                minutes / 60 % 24,
                minutes % 60
            )
        })
        .unwrap_or_else(|| "Unavailable".into());
    rows.push(("Uptime".into(), uptime));
    rows.push((
        "Session shell".into(),
        std::env::var("JUNA_SHELL_LABEL")
            .map(|v| clean(&v))
            .unwrap_or_else(|_| "Unavailable outside Juna".into()),
    ));
    rows.push((
        "Juna".into(),
        std::env::var("JUNA_VERSION")
            .map(|v| clean(&v))
            .unwrap_or_else(|_| "Standalone Junafetch".into()),
    ));
    if details {
        for (index, disk) in disks.iter().enumerate().skip(1) {
            rows.push((format!("Other disk {}", index), usage(&disk["bytes"])));
        }
        rows.push((
            "Privacy".into(),
            "Personal/network identifiers omitted".into(),
        ));
    }
    rows
}
fn run() -> Result<(), String> {
    use presentation::Profile;
    use unicode_width::UnicodeWidthStr;
    let prefs = presentation::preferences();
    let mut profile = prefs.profile;
    let mut logo = prefs.logo.clone();
    let mut json_output = false;
    let mut plain = false;
    let mut privacy_mode = prefs.privacy_mode.clone();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--brief" => profile = Profile::Brief,
            "--details" => profile = Profile::Detailed,
            "--profile" => {
                profile = Profile::parse(
                    &args
                        .next()
                        .ok_or("--profile requires brief, standard or detailed")?,
                )
                .ok_or("Use brief, standard or detailed")?
            }
            "--json" => json_output = true,
            "--plain" => plain = true,
            "--logo" => {
                logo = args.next().ok_or("--logo requires compact, full or none")?;
                if !matches!(logo.as_str(), "compact" | "full" | "none") {
                    return Err("Use compact, full or none for --logo".into());
                }
            }
            "--version" => {
                println!(
                    "junafetch {} (Fastfetch detector 2.69.0)",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            "--help" | "-h" => {
                println!("junafetch / fetch [--brief | --details | --profile brief|standard|detailed] [--json] [--plain] [--logo compact|full|none] [--privacy shift-hover|hover|always-hidden|off]\nGrouped local stats; saved choices apply on each run. Protected fields use shaded masks. Juna defaults to deliberate Shift+hover reveal; plain/JSON output stays masked unless censorship is explicitly off.");
                return Ok(());
            }
            "--privacy" => {
                privacy_mode = args
                    .next()
                    .filter(|mode| {
                        matches!(
                            mode.as_str(),
                            "shift-hover" | "hover" | "always-hidden" | "off"
                        )
                    })
                    .ok_or("Invalid privacy mode")?;
            }
            _ => return Err("Unknown option; use --help".into()),
        }
    }
    let data = detect(profile != Profile::Brief)?;
    let mut sections = presentation::sections(&data, profile, &prefs);
    let terminal = io::stdout().is_terminal();
    let values = privacy::values(&data);
    let protected = privacy::prepare(
        &mut sections,
        &values,
        &privacy_mode,
        terminal && !plain && !json_output,
    );
    if json_output {
        let fields: Vec<_> = sections
            .iter()
            .flat_map(|section| {
                section.fields.iter().map(
                    move |(name, value)| json!({"section":section.name,"name":name,"value":value}),
                )
            })
            .collect();
        // Keep mask characters readable to older PowerShell JSON pipelines as well as UTF-8 consumers.
        println!(
            "{}",
            json!({"schema_version":2,"profile":profile.name(),"fields":fields})
                .to_string()
                .replace('░', "\\u2591")
        );
        return Ok(());
    }
    let columns = width().max(16);
    let art: Vec<_> = if terminal && !plain && logo != "none" {
        (if logo == "full" { FULL } else { COMPACT })
            .trim_end()
            .lines()
            .collect()
    } else {
        Vec::new()
    };
    let logo_width = art.iter().map(|line| line.width()).max().unwrap_or(0);
    let art = if logo_width <= columns {
        art
    } else {
        Vec::new()
    };
    let side = !art.is_empty() && columns >= logo_width + 4 + 32;
    let report_width = if side {
        columns - logo_width - 4
    } else {
        columns
    };
    let borders = terminal && !plain && profile != Profile::Brief;
    let formatted = presentation::lines(&sections, report_width, borders);
    let colors = terminal && !plain;
    let colored = |line: &str| {
        let annotated = privacy::annotate(line, &protected);
        if colors && (line.starts_with('┌') || line.starts_with('└') || line.starts_with('[')) {
            format!("\x1b[36m{annotated}\x1b[0m")
        } else {
            annotated
        }
    };
    let mut out = io::stdout().lock();
    if side {
        for index in 0..art.len().max(formatted.len()) {
            let logo = art.get(index).copied().unwrap_or("");
            let pad = " ".repeat(logo_width.saturating_sub(logo.width()));
            let report = colored(formatted.get(index).map(String::as_str).unwrap_or(""));
            if colors {
                writeln!(out, "\x1b[36m{logo}{pad}\x1b[0m    {report}")
            } else {
                writeln!(out, "{logo}{pad}    {report}")
            }
            .map_err(|_| "Output failed")?;
        }
    } else {
        for line in art {
            if colors {
                writeln!(out, "\x1b[36m{line}\x1b[0m")
            } else {
                writeln!(out, "{line}")
            }
            .map_err(|_| "Output failed")?;
        }
        for line in formatted {
            writeln!(out, "{}", colored(&line)).map_err(|_| "Output failed")?;
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("junafetch: {error}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn report_omits_identifiers_and_prefers_physical_gpu() {
        let data = serde_json::json!([{"type":"OS","result":{"prettyName":"Windows","hostname":"secret-host"}},{"type":"GPU","result":[{"name":"Virtual","type":null},{"name":"Physical","type":"Discrete"}]}]);
        let rows = super::report(&data, false);
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[2].1, "Physical");
        assert!(!format!("{rows:?}").contains("secret-host"));
    }
}
