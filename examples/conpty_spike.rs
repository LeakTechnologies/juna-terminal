use std::io::Read;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use windows::Win32::System::Console::{GetConsoleMode, CONSOLE_MODE};

const T: u64 = 10000;

enum Outcome {
    Got(String),
    Timeout,
    Err(String),
}

fn with_timeout<F>(label: &str, f: F) -> Outcome
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_millis(T)) {
        Ok(Ok(s)) => Outcome::Got(s),
        Ok(Err(e)) => Outcome::Err(e),
        Err(_) => {
            println!("       !! {label} exceeded {T}ms");
            Outcome::Timeout
        }
    }
}

fn report(tag: &str, expect: Option<&str>, o: Outcome) -> bool {
    match o {
        Outcome::Got(s) => {
            let clean: String = s.chars().filter(|c| *c != '\u{1b}' && *c != '\r').collect();
            let ok = expect.map(|e| clean.contains(e)).unwrap_or(true);
            println!(
                "[{:<4}] {tag:<24} {}",
                if ok { "PASS" } else { "FAIL" },
                truncate(clean.trim(), 260)
            );
            ok
        }
        Outcome::Timeout => {
            println!("[FAIL] {tag:<24} TIMEOUT");
            false
        }
        Outcome::Err(e) => {
            println!("[FAIL] {tag:<24} {e}");
            false
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    let t = s.replace('\n', " | ");
    if t.chars().count() > n {
        format!("{}...", t.chars().take(n).collect::<String>())
    } else {
        t
    }
}

fn host_console_mode() -> Result<u32, String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::core::PCWSTR;

    // SAFETY: AllocConsole fails harmlessly when a console already exists.
    unsafe {
        let _ = windows::Win32::System::Console::AllocConsole();
    }
    let name: Vec<u16> = "CONOUT$".encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: OPEN_EXISTING on CONOUT$ returns an owned handle; it is closed on
    // every exit path below, including the error path.
    let h = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            Some(HANDLE::default()),
        )
    }
    .map_err(|e| format!("CreateFileW(CONOUT$): {e:?}"))?;

    // SAFETY: h is a valid console handle from CreateFileW; CONSOLE_MODE is an
    // owned bitfield and h is closed before this block returns.
    unsafe {
        let mut m = CONSOLE_MODE::default();
        let r = GetConsoleMode(h, &mut m).map_err(|e| format!("GetConsoleMode: {e:?}"));
        let _ = CloseHandle(h);
        r.map(|_| m.0)
    }
}

fn probe_args() -> Vec<String> {
    [
        "plain",
        "has space",
        "has\"quote",
        "has'squote",
        "trailing ",
        "back\\slash",
        "",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn expected_arg_lines() -> String {
    probe_args()
        .iter()
        .enumerate()
        .map(|(i, a)| format!("ARG[{i}]={}:{a}", a.len()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn check_quoting(probe: String) {
    println!("--- CHECK 1: argument preservation (dedicated argv probe) ---");
    let want = expected_arg_lines();

    let p2 = probe.clone();
    report(
        "conpty-oxide quoting",
        Some(&want),
        with_timeout("oxide quoting", move || {
            let mut c = conpty_oxide::blocking::Command::new(&p2);
            c.args(probe_args());
            let s = c
                .spawn_with(conpty_oxide::SessionOptions::new())
                .map_err(|e| format!("spawn: {e:?}"))?;
            let o = s.collect_output().map_err(|e| format!("collect: {e:?}"))?;
            Ok(String::from_utf8_lossy(o.as_bytes()).to_string())
        }),
    );

    let p3 = probe;
    report(
        "conpty 0.7 quoting",
        Some(&want),
        with_timeout("07 quoting", move || {
            let mut c = StdCommand::new(&p3);
            c.args(probe_args());
            let mut p = conpty::Process::spawn(c).map_err(|e| format!("spawn: {e:?}"))?;
            let mut r = p.output().map_err(|e| format!("output: {e:?}"))?;
            let mut b = Vec::new();
            r.read(&mut b).map_err(|e| format!("read: {e}"))?;
            p.exit(0).map_err(|e| format!("exit: {e:?}"))?;
            Ok(String::from_utf8_lossy(&b).to_string())
        }),
    );
}

fn clear_vt_bit() -> Result<u32, String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Console::SetConsoleMode;
    use windows::core::PCWSTR;

    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    let name: Vec<u16> = "CONOUT$".encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: OPEN_EXISTING on CONOUT$ yields an owned handle, closed on all paths.
    let h = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            Some(HANDLE::default()),
        )
    }
    .map_err(|e| format!("CreateFileW: {e:?}"))?;

    // SAFETY: h is a valid console handle; CONSOLE_MODE is owned by this frame
    // and h is closed before returning.
    unsafe {
        let mut m = CONSOLE_MODE::default();
        let r0 = GetConsoleMode(h, &mut m).map_err(|e| format!("GetConsoleMode: {e:?}"));
        if let Ok(()) = r0 {
            m.0 &= !ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            if let Err(e) = SetConsoleMode(h, m) {
                let _ = CloseHandle(h);
                return Err(format!("SetConsoleMode: {e:?}"));
            }
            let cleared = m.0;
            let _ = CloseHandle(h);
            return Ok(cleared);
        }
        let _ = CloseHandle(h);
        r0.map(|_| m.0)
    }
}

fn check_host_console(probe: String) {
    println!("--- CHECK 4: host console mode must not gain the VT bit ---");
    let before = host_console_mode();
    println!("       initial = {before:?}");
    if before.is_err() {
        println!("[SKIP] no host console attachable; cannot run this check here");
        return;
    }
    match clear_vt_bit() {
        Ok(v) => println!("       cleared VT bit, mode now {v:#x}"),
        Err(e) => {
            println!("[SKIP] cannot clear VT bit: {e}");
            return;
        }
    }
    let b0 = host_console_mode().unwrap();

    let p = probe.clone();
    let r1 = with_timeout("oxide mode", move || {
        let mut c = conpty_oxide::blocking::Command::new(&p);
        c.args(["--spawn-grandchild"]);
        let s = c
            .spawn_with(conpty_oxide::SessionOptions::new())
            .map_err(|e| format!("spawn: {e:?}"))?;
        let _ = s.collect_output();
        Ok(String::new())
    });
    if matches!(r1, Outcome::Got(_)) {
        let a = host_console_mode().unwrap();
        println!(
            "[{}] {:<24} before={b0:#x} after={a:#x}",
            if b0 == a { "PASS" } else { "FAIL" },
            "conpty-oxide no VT set"
        );
        if b0 != a {
            // SAFETY: detaching our own probe console; affects only this test process.
unsafe {
    let _ = windows::Win32::System::Console::FreeConsole();
}
        }
    } else {
        println!("[FAIL] conpty-oxide mode         spawn timed out");
    }

    let b0 = match clear_vt_bit() {
        Ok(v) => host_console_mode().unwrap_or(v),
        Err(_) => return,
    };
    let p = probe;
    let r2 = with_timeout("07 mode", move || {
        let mut c = StdCommand::new(&p);
        c.args(["--spawn-grandchild"]);
        let mut pr = conpty::Process::spawn(c).map_err(|e| format!("spawn: {e:?}"))?;
        let mut r = pr.output().map_err(|e| format!("output: {e:?}"))?;
        let _ = drain_until(&mut r, "GRANDCHILD=", 3000);
        pr.exit(0).ok();
        Ok(String::new())
    });
    if matches!(r2, Outcome::Got(_)) {
        let a = host_console_mode().unwrap();
        println!(
            "[{}] {:<24} before={b0:#x} after={a:#x}",
            if b0 == a { "PASS" } else { "FAIL" },
            "conpty 0.7 no VT set"
        );
    } else {
        println!("[FAIL] conpty 0.7 mode           spawn timed out");
    }
}

fn extract_gc(text: &str) -> Option<u32> {
    let i = text.find("GRANDCHILD=")? + "GRANDCHILD=".len();
    let digits: String = text[i..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn drain_until(r: &mut impl Read, want: &str, deadline_ms: u64) -> String {
    let start = std::time::Instant::now();
    let mut acc: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    while (start.elapsed().as_millis() as u64) < deadline_ms {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                acc.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&acc).contains(want) {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&acc).to_string()
}

fn check_process_tree(probe: String) {
    println!("--- CHECK 5: process-tree teardown (grandchild survival) ---");

    let p = probe.clone();
    let gc_oxide = std::sync::Arc::new(AtomicU32::new(0));
    let g1 = gc_oxide.clone();
    report(
        "oxide tree",
        None,
        with_timeout("oxide tree", move || {
            let mut c = conpty_oxide::blocking::Command::new(&p);
            c.args(["--spawn-grandchild"]);
            let s = c
                .spawn_with(conpty_oxide::SessionOptions::new())
                .map_err(|e| format!("spawn: {e:?}"))?;
            let o = s.collect_output().map_err(|e| format!("collect: {e:?}"))?;
            let t = String::from_utf8_lossy(o.as_bytes()).to_string();
            if let Some(g) = extract_gc(&t) {
                g1.store(g, Ordering::SeqCst);
            }
            Ok(t)
        }),
    );

    let p = probe;
    let gc_07 = std::sync::Arc::new(AtomicU32::new(0));
    let h = gc_07.clone();
    report(
        "conpty 0.7 tree",
        None,
        with_timeout("07 tree", move || {
            let mut c = StdCommand::new(&p);
            c.args(["--spawn-grandchild"]);
            let mut pr = conpty::Process::spawn(c).map_err(|e| format!("spawn: {e:?}"))?;
            let mut r = pr.output().map_err(|e| format!("output: {e:?}"))?;
            let t = drain_until(&mut r, "GRANDCHILD=", 3000);
            if let Some(g) = extract_gc(&t) {
                h.store(g, Ordering::SeqCst);
            }
            drop(pr);
            Ok(t)
        }),
    );

    std::thread::sleep(Duration::from_millis(1500));
    for (label, cell) in [("conpty-oxide", &gc_oxide), ("conpty 0.7", &gc_07)] {
        let g = cell.load(Ordering::SeqCst);
        if g == 0 {
            println!("[SKIP] {label:<24} grandchild pid not observed");
            continue;
        }
        let alive = pid_alive(g);
        println!(
            "[{}] {:<24} grandchild pid={g} alive 1.5s after parent teardown: {}",
            if alive { "FAIL" } else { "PASS" },
            format!("{label} grandchild"),
            alive
        );
        if alive {
            let _ = StdCommand::new("taskkill")
                .args(["/F", "/T", "/PID", &g.to_string()])
                .output();
        }
    }
}

fn pid_alive(pid: u32) -> bool {
    StdCommand::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("{pid}")))
        .unwrap_or(false)
}

fn main() {
    let probe = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: conpty_spike <path to argv_probe.exe>");
            eprintln!("build it first: cargo build --bin argv_probe");
            std::process::exit(2);
        }
    };
    if !std::path::Path::new(&probe).exists() {
        eprintln!("probe not found: {probe}");
        std::process::exit(2);
    }
    check_quoting(probe.clone());
    check_host_console(probe.clone());
    check_process_tree(probe);
}