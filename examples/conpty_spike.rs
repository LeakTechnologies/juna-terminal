use std::io::Read;
use std::process::Command as StdCommand;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const T: u64 = 8000;

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
        let r = f();
        let _ = tx.send(r);
    });
    match rx.recv_timeout(Duration::from_millis(T)) {
        Ok(Ok(s)) => Outcome::Got(s),
        Ok(Err(e)) => Outcome::Err(e),
        Err(_) => {
            println!("[HANG] {label} did not finish within {T}ms");
            Outcome::Timeout
        }
    }
}

fn report(tag: &str, expect: Option<&str>, o: Outcome) {
    match o {
        Outcome::Got(s) => {
            let clean: String = s.chars().filter(|c| *c != '\u{1b}' && *c != '\r').collect();
            let ok = expect.map(|e| clean.contains(e)).unwrap_or(true);
            println!("[{:<4}] {tag:<26} {}", if ok { "PASS" } else { "FAIL" }, truncate(&clean, 220));
        }
        Outcome::Timeout => println!("[FAIL] {tag:<26} TIMEOUT after {T}ms (blocking read/close)"),
        Outcome::Err(e) => println!("[FAIL] {tag:<26} {e}"),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}...", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

fn probe_args() -> Vec<String> {
    vec![
        "-NoProfile", "-Command", "$args -join '|'",
        "a b", "c\"d", "e'f",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn check_quoting() {
    println!("--- CHECK 1: argument quoting ---");
    let expected = "a b|c\"d|e'f";
    report(
        "conpty 0.7",
        Some(expected),
        with_timeout("conpty 0.7 quoting", move || {
            let mut c = StdCommand::new("powershell.exe");
            c.args(probe_args());
            let mut p = conpty::Process::spawn(c).map_err(|e| format!("spawn: {e:?}"))?;
            let mut r = p.output().map_err(|e| format!("output: {e:?}"))?;
            let mut b = Vec::new();
            r.read_to_end(&mut b).map_err(|e| format!("read: {e}"))?;
            p.exit(0).ok();
            Ok(String::from_utf8_lossy(&b).to_string())
        }),
    );
    report(
        "conpty-oxide",
        Some(expected),
        with_timeout("conpty-oxide quoting", move || {
            let mut c = conpty_oxide::blocking::Command::new("powershell.exe");
            c.args(probe_args());
            let s = c
                .spawn_with(conpty_oxide::SessionOptions::new())
                .map_err(|e| format!("spawn: {e:?}"))?;
            let o = s.collect_output().map_err(|e| format!("collect: {e:?}"))?;
            Ok(String::from_utf8_lossy(o.as_bytes()).to_string())
        }),
    );
}

fn check_roundtrip() {
    println!("--- CHECK 2: output round-trip ---");
    let marker = "SMITH_RT_9F3A";
    report(
        "conpty 0.7",
        Some(marker),
        with_timeout("conpty 0.7 rt", move || {
            let mut c = StdCommand::new("cmd.exe");
            c.args(["/C", "echo", marker]);
            let mut p = conpty::Process::spawn(c).map_err(|e| format!("spawn: {e:?}"))?;
            let mut r = p.output().map_err(|e| format!("output: {e:?}"))?;
            let mut b = Vec::new();
            r.read_to_end(&mut b).map_err(|e| format!("read: {e}"))?;
            p.exit(0).ok();
            Ok(String::from_utf8_lossy(&b).to_string())
        }),
    );
    report(
        "conpty-oxide",
        Some(marker),
        with_timeout("conpty-oxide rt", move || {
            let mut c = conpty_oxide::blocking::Command::new("cmd.exe");
            c.args(["/C", "echo", marker]);
            let s = c
                .spawn_with(conpty_oxide::SessionOptions::new())
                .map_err(|e| format!("spawn: {e:?}"))?;
            let o = s.collect_output().map_err(|e| format!("collect: {e:?}"))?;
            Ok(String::from_utf8_lossy(o.as_bytes()).to_string())
        }),
    );
}

fn check_resize() {
    println!("--- CHECK 3: resize (120x40 then 80x25) ---");
    report(
        "conpty-oxide",
        None,
        with_timeout("conpty-oxide resize", move || {
            let mut c = conpty_oxide::blocking::Command::new("cmd.exe");
            c.args(["/C", "ping", "-n", "4", "127.0.0.1", ">", "nul"]);
            let mut s = c
                .spawn_with(conpty_oxide::SessionOptions::new())
                .map_err(|e| format!("spawn: {e:?}"))?;
            s.resize(conpty_oxide::Size::try_new(120, 40).map_err(|e| e.to_string())?)
                .map_err(|e| format!("resize1: {e:?}"))?;
            s.resize(conpty_oxide::Size::try_new(80, 25).map_err(|e| e.to_string())?)
                .map_err(|e| format!("resize2: {e:?}"))?;
            let z = s.size();
            s.kill().map_err(|e| format!("kill: {e:?}"))?;
            Ok(format!("final size {}x{}", z.cols(), z.rows()))
        }),
    );
    report(
        "conpty 0.7",
        None,
        with_timeout("conpty 0.7 resize", move || {
            let mut c = StdCommand::new("cmd.exe");
            c.args(["/C", "ping", "-n", "4", "127.0.0.1", ">", "nul"]);
            let mut p = conpty::Process::spawn(c).map_err(|e| format!("spawn: {e:?}"))?;
            p.resize(120, 40).map_err(|e| format!("resize1: {e:?}"))?;
            p.resize(80, 25).map_err(|e| format!("resize2: {e:?}"))?;
            p.exit(0).map_err(|e| format!("exit: {e:?}"))?;
            Ok("resized and exited".into())
        }),
    );
}

fn check_kill_and_orphan() {
    println!("--- CHECK 4: terminate + drop-without-kill ---");
    report(
        "conpty-oxide kill",
        None,
        with_timeout("conpty-oxide kill", move || {
            let mut c = conpty_oxide::blocking::Command::new("cmd.exe");
            c.args(["/C", "ping", "-n", "30", "127.0.0.1", ">", "nul"]);
            let mut s = c
                .spawn_with(conpty_oxide::SessionOptions::new())
                .map_err(|e| format!("spawn: {e:?}"))?;
            let t = Instant::now();
            s.kill().map_err(|e| format!("kill: {e:?}"))?;
            let gone = s.try_wait().map_err(|e| format!("try_wait: {e:?}"))?;
            Ok(format!("kill took {:?}, try_wait={:?}", t.elapsed(), gone))
        }),
    );

    let pid = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let p2 = pid.clone();
    let r = with_timeout("conpty 0.7 drop", move || {
        let mut c = StdCommand::new("cmd.exe");
        c.args(["/C", "ping", "-n", "30", "127.0.0.1", ">", "nul"]);
        let p = conpty::Process::spawn(c).map_err(|e| format!("spawn: {e:?}"))?;
        p2.store(p.pid(), std::sync::atomic::Ordering::SeqCst);
        drop(p);
        Ok("dropped without terminate".into())
    });
    report("conpty 0.7 drop", None, r);

    let orphan_pid = pid.load(std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(1200));
    let survived = tasklist_contains(orphan_pid);
    println!(
        "[{}] {:<26} pid={orphan_pid} still running 1.2s after Drop: {}",
        if survived { "FAIL" } else { "PASS" },
        "conpty 0.7 orphan",
        survived
    );
    if survived {
        let _ = StdCommand::new("taskkill")
            .args(["/F", "/PID", &orphan_pid.to_string()])
            .output();
    }
}

fn tasklist_contains(pid: u32) -> bool {
    StdCommand::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("{pid}")))
        .unwrap_or(false)
}

fn main() {
    check_quoting();
    check_roundtrip();
    check_resize();
    check_kill_and_orphan();
    println!("--- CHECK 5 host-console-mode: NOT IMPLEMENTED (needs windows crate) ---");
}