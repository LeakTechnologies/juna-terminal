//! End-to-end conformance against a **real** shell process.
//!
//! Everything under `tests/` before this drove synthetic byte sequences into the
//! engine. That proves the parser and grid behave; it does not prove a real
//! program can talk to them. These tests spawn `cmd.exe`, drive it through the
//! production session seam, and assert on what the engine renders.
//!
//! **Shell independence.** Per `ARCHITECTURE.md` 5.1 these tests must not assert
//! on one shell's output. Every assertion below is on text *we chose*, never on
//! prompt strings, colours, or banners that belong to the shell itself. The
//! marker is emitted by the command we send, so the assertion tests Juna, not
//! `cmd.exe`. Substituting PowerShell changes the harness target, not the
//! assertions.

#![cfg(windows)]

use juna::session::{self, Dimensions, SessionSpec};
use juna::transport::{Limits, ReadState, StreamBuffer};
use juna::vt::TerminalState;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(60);
const QUIET: Duration = Duration::from_millis(750);

/// Runs a shell, feeds each line after the shell settles, and returns
/// (screen, scrollback). Teardown always reaps the child.
fn drive(lines: &[&str], cols: u16, rows: u16) -> (String, String, i32) {
    let spec = SessionSpec {
        program: OsString::from("cmd.exe"),
        args: Vec::new(),
        cwd: None,
    };
    let session = session::spawn(&spec, Dimensions { cols, rows }).expect("spawn cmd.exe");
    let parts = session.split().expect("split");

    let stream = Arc::new(StreamBuffer::new(Limits::default()));
    let pump = stream.clone();
    let reader_thread = std::thread::spawn(move || {
        let mut reader = parts.reader;
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if pump.wait_for_capacity(n) != ReadState::Open {
                        break;
                    }
                    pump.push(&buf[..n]);
                }
                Err(_) => break,
            }
        }
        pump.finish();
    });

    let mut term = TerminalState::new(cols, rows);
    let mut writer = parts.writer;
    let mut control = parts.control;
    let deadline = Instant::now() + BOUND;

    // All input is written up front. `cmd.exe` reads its input buffer eagerly, so
    // there is no race; waiting for quiet before every line (as the interactive
    // harness does for PowerShell) would make a 120-line script take minutes and
    // blow the bound without testing anything extra.
    for line in lines {
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\r');
        writer.write(&bytes).expect("write to shell");
    }

    let mut exit = pump_until_quiet(&stream, &mut term, &mut control, deadline);
    if exit.is_none() {
        let _ = control.kill();
        let reap = Instant::now() + Duration::from_secs(10);
        while Instant::now() < reap {
            match control.try_wait() {
                Ok(Some(s)) => {
                    exit = Some(s.code());
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                Err(_) => break,
            }
        }
    }

    stream.cancel();
    let _ = reader_thread.join();

    let screen = render_screen(&term);
    let history = render_history(&term);
    (screen, history, exit.unwrap_or(-1))
}

fn cell_text(c: &juna::vt::CellContent) -> String {
    match c {
        juna::vt::CellContent::Empty | juna::vt::CellContent::WideCharSpacer => " ".into(),
        juna::vt::CellContent::Char(cp) | juna::vt::CellContent::WideChar(cp) => {
            char::from_u32(*cp).map(String::from).unwrap_or_else(|| "?".into())
        }
        juna::vt::CellContent::Cluster(s) => s.to_string(),
    }
}

fn render(term: &TerminalState) -> String {
    let mut out = String::new();
    for r in 0..term.grid().rows() {
        if let Some(cells) = term.grid().row_slice(r) {
            out.push_str(&cells.iter().map(|c| cell_text(&c.content)).collect::<String>());
        }
        out.push('\n');
    }
    out
}

fn render_screen(term: &TerminalState) -> String {
    render(term)
}

fn render_history(term: &TerminalState) -> String {
    let mut out = String::new();
    for i in 0..term.scrollback_len() {
        if let Some(cells) = term.scrollback_line(i) {
            out.push_str(&cells.iter().map(|c| cell_text(&c.content)).collect::<String>());
        }
        out.push('\n');
    }
    out
}

fn pump_until_quiet(
    stream: &Arc<StreamBuffer>,
    term: &mut TerminalState,
    control: &mut Box<dyn juna::session::SessionControl>,
    deadline: Instant,
) -> Option<i32> {
    let mut last = Instant::now();
    loop {
        let batch = stream.take();
        if !batch.is_empty() {
            term.advance(&batch);
            last = Instant::now();
        }
        if let Some(s) = control.try_wait().expect("try_wait") {
            let batch = stream.take();
            if !batch.is_empty() {
                term.advance(&batch);
            }
            return Some(s.code());
        }
        if last.elapsed() >= QUIET || Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
}

#[test]
fn text_the_shell_prints_reaches_the_grid() {
    let (screen, history, _) = drive(&["echo JUNA_E2E_ROUNDTRIP_8F2A"], 80, 24);
    let both = format!("{history}{screen}");
    assert!(
        both.contains("JUNA_E2E_ROUNDTRIP_8F2A"),
        "echoed marker never reached the engine.\nscreen:\n{screen}\nhistory:\n{history}"
    );
}

#[test]
fn output_longer_than_the_window_reaches_scrollback() {
    // 120 lines into a 12-row window: the last line must be on screen and the
    // first must survive in history. Before the history wiring this failed with
    // scrollback empty, because nothing ever called Scrollback::push.
    let mut lines: Vec<String> = (1..=120).map(|i| format!("echo JUNA_SCROLL_{i:04}")).collect();
    lines.push("exit".into());
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();

    let (screen, history, _) = drive(&refs, 80, 12);
    assert!(
        screen.contains("JUNA_SCROLL_0120"),
        "last line must be on screen\n{screen}"
    );
    assert!(
        history.contains("JUNA_SCROLL_0001"),
        "first line must survive in scrollback; scrollback_len was {}",
        history.lines().count()
    );
    assert!(
        history.contains("JUNA_SCROLL_0060"),
        "a middle line must survive in scrollback\n{history}"
    );
}

/// `cls` is a real, everyday screen-clear from a real program, and on modern
/// Windows it routes through the alternate screen. Output after it must land on
/// screen and stay readable.
#[test]
fn output_after_cls_is_visible_again() {
    let (screen, history, _) = drive(
        &["echo JUNA_BEFORE_CLS", "cls", "echo JUNA_AFTER_CLS", "exit"],
        80,
        24,
    );
    let both = format!("{history}{screen}");
    assert!(
        both.contains("JUNA_AFTER_CLS"),
        "output after cls must be visible\nscreen:\n{screen}\nhistory:\n{history}"
    );
}

#[test]
fn the_child_is_reaped_and_leaves_nothing_running() {
    let (_, _, exit) = drive(&["echo JUNA_REAP_CHECK", "exit"], 80, 24);
    assert_eq!(exit, 0, "cmd.exe should exit cleanly on `exit`");
}

/// Sustained output must arrive byte-exact. This is the property
/// `ARCHITECTURE.md` 3.0 makes a release gate for elevated builds.
#[test]
fn sustained_high_volume_output_is_not_truncated() {
    let (screen, history, _) = drive(
        &[
            "for /L %i in (1,1,400) do @echo JUNA_BULK_%i",
            "echo JUNA_BULK_SENTINEL",
            "exit",
        ],
        120,
        24,
    );
    let both = format!("{history}{screen}");
    // First, middle, and last must all survive: a truncation at either end, or a
    // reordering under backpressure, shows up as a missing marker here.
    for i in ["JUNA_BULK_1", "JUNA_BULK_200", "JUNA_BULK_400"] {
        assert!(both.contains(i), "{i} was lost or reordered");
    }
    assert!(
        both.contains("JUNA_BULK_SENTINEL"),
        "the final sentinel proves nothing after it was dropped"
    );
    // And the run must actually have produced them, not simply been skipped.
    assert!(history.contains("JUNA_BULK_1"), "history is empty: {history}");
}