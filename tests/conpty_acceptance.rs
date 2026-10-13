#![cfg(windows)]

use std::time::{Duration, Instant};

const PROBE: &str = env!("CARGO_BIN_EXE_argv_probe");
const BOUND: Duration = Duration::from_secs(30);

#[test]
fn session_environment_overrides_are_child_only_and_unicode_safe() {
    use juna::session::{self, Dimensions, SessionSpec};
    let before = std::env::var_os("JUNA_ENV_PROBE");
    let spec = SessionSpec {
        program: PROBE.into(),
        args: vec!["--environment".into()],
        cwd: None,
        env: vec![(
            "JUNA_ENV_PROBE".into(),
            "Juna tool path with spaces 日本語".into(),
        )],
    };
    let mut child = session::spawn(&spec, Dimensions::default()).unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let deadline = Instant::now() + BOUND;
    loop {
        assert!(Instant::now() < deadline);
        let n = child.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    assert!(String::from_utf8_lossy(&bytes).contains("ENV=Juna tool path with spaces 日本語"));
    assert_eq!(std::env::var_os("JUNA_ENV_PROBE"), before);
}

fn clean(bytes: &[u8]) -> String {
    bytes
        .iter()
        .copied()
        .filter(|b| *b != 0x1b && *b != b'\r')
        .map(|b| b as char)
        .collect()
}

fn spawn(args: &[&str]) -> conpty_oxide::blocking::Session {
    let mut c = conpty_oxide::blocking::Command::new(PROBE);
    c.args(args);
    c.spawn_with(conpty_oxide::SessionOptions::new())
        .expect("spawn probe session")
}

fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("{pid}")))
        .unwrap_or(false)
}

#[test]
fn quoting_preserves_the_argument_vector_exactly() {
    let tricky = [
        "plain",
        "has space",
        "has\"quote",
        "has'squote",
        "trailing ",
        "back\\slash",
        "",
    ];
    let s = spawn(&tricky);
    let out = s.collect_output().expect("collect");
    let text = clean(out.as_bytes());

    assert!(
        text.contains("COUNT=7"),
        "probe did not report 7 args: {text}"
    );
    for (i, a) in tricky.iter().enumerate() {
        let line = format!("ARG[{i}]={}:{a}", a.len());
        assert!(
            text.contains(&line),
            "argument {i} corrupted.\nwant: {line}\ngot: {text}"
        );
    }
}

#[test]
fn sustained_output_arrives_byte_exact() {
    let n = 5000usize;
    let s = spawn(&[&format!("--sustain={n}")]);
    let start = Instant::now();
    let out = s.collect_output().expect("collect");
    let elapsed = start.elapsed();

    let text = clean(out.as_bytes());
    assert!(
        text.contains(&format!("SUSTAIN_END={n}")),
        "sustained output truncated; last marker missing"
    );
    for i in [0usize, 1, n / 2, n - 1] {
        let line = format!("LINE[{i:06}]=abcdefghijklmnopqrstuvwxyz0123456789");
        assert!(text.contains(&line), "line {i} lost or reordered");
    }
    assert!(elapsed < BOUND, "sustained output took {elapsed:?}");
}

#[test]
fn process_tree_is_reaped_on_teardown() {
    let s = spawn(&["--spawn-grandchild"]);
    let out = s.collect_output().expect("collect");
    let text = clean(out.as_bytes());

    let i = text.find("GRANDCHILD=").expect("no grandchild reported");
    let digits: String = text[i + "GRANDCHILD=".len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let pid: u32 = digits.parse().expect("grandchild pid");

    for _ in 0..40 {
        if !pid_alive(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output();
    panic!("grandchild {pid} survived session teardown");
}

#[test]
fn shutdown_unblocks_the_reader_within_the_bound() {
    let mut c = conpty_oxide::blocking::Command::new("cmd.exe");
    c.args(["/C", "ping", "-n", "120", "127.0.0.1", ">", "nul"]);
    let mut s = c
        .spawn_with(conpty_oxide::SessionOptions::new())
        .expect("spawn");

    std::thread::sleep(Duration::from_millis(400));
    let start = Instant::now();
    s.kill().expect("kill");

    let mut exited = false;
    while start.elapsed() < BOUND {
        if s.try_wait().expect("try_wait").is_some() {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let _out = s.collect_output().expect("collect after kill");
    let elapsed = start.elapsed();

    assert!(exited, "child did not exit within {BOUND:?} after kill");
    assert!(
        elapsed < BOUND,
        "reader did not unblock after kill; took {elapsed:?}. \
         A hang here means shutdown is relying on pipe EOF, which ConPTY does not deliver."
    );
}
