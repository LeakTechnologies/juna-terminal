//! Headless conformance harness (ARCHITECTURE.md 4.1).
//!
//! Drives a real shell through the whole production data path and renders the
//! resulting screen as text:
//!
//! ```text
//!   session ──read──► reader thread ──► StreamBuffer ──► TerminalState ──► screen text
//!        └────────────write───────────────────────────────────────────^
//! ```
//!
//! Every component here is production code. Nothing is stubbed: the shell is
//! real, the ConPTY backend is real, the backpressure buffer is real, and the
//! engine is the same engine the window will drive. That is the point — this
//! answers "is the engine correct against a real shell?" before any renderer
//! exists to confuse the answer.
//!
//! No window, no GPU, no winit event loop.

use juna::session::{self, Dimensions, SessionControl, SessionSpec};
use juna::transport::{Limits, ReadState, StreamBuffer};
use juna::vt::{CellContent, TerminalState};
use std::sync::Arc;
use std::time::{Duration, Instant};

const READ_CHUNK: usize = 8 * 1022;
const BOUND: Duration = Duration::from_secs(60);

struct Opts {
    shell: String,
    cols: u16,
    rows: u16,
    sends: Vec<String>,
    expect: Vec<String>,
    timeout: Duration,
    quiet: Duration,
    keep_padding: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            // ARCHITECTURE.md 5.1: Windows PowerShell 5.1 is the primary
            // interactive acceptance target.
            shell: "powershell.exe".into(),
            cols: 120,
            rows: 40,
            sends: Vec::new(),
            expect: Vec::new(),
            timeout: BOUND,
            quiet: Duration::from_millis(750),
            keep_padding: false,
        }
    }
}

fn usage() -> String {
    "juna_headless [--shell PATH] [--cols N] [--rows N] [--send TEXT]... \
     [--expect TEXT] [--timeout SECS] [--quiet MS] [--keep-padding] [--help]"
        .into()
}

fn parse() -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = |name: &str| {
            args.next()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match a.as_str() {
            "--shell" => o.shell = next("--shell")?,
            "--cols" => {
                o.cols = next("--cols")?.parse().map_err(|_| "bad --cols")?
            }
            "--rows" => {
                o.rows = next("--rows")?.parse().map_err(|_| "bad --rows")?
            }
            "--send" => o.sends.push(next("--send")?),
            "--expect" => o.expect.push(next("--expect")?),
            "--timeout" => {
                let s: u64 = next("--timeout")?.parse().map_err(|_| "bad --timeout")?;
                o.timeout = Duration::from_secs(s.max(1));
            }
            "--quiet" => {
                let ms: u64 = next("--quiet")?.parse().map_err(|_| "bad --quiet")?;
                o.quiet = Duration::from_millis(ms);
            }
            "--keep-padding" => o.keep_padding = true,
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unknown argument {other}\n\n{}", usage())),
        }
    }
    Ok(o)
}

/// Render one cell as text. Clusters keep every scalar, so a combining sequence
/// survives the round trip instead of collapsing to its base.
fn cell_text(c: &CellContent) -> String {
    match c {
        CellContent::Empty | CellContent::WideCharSpacer => " ".into(),
        CellContent::Char(cp) | CellContent::WideChar(cp) => {
            char::from_u32(*cp).map(String::from).unwrap_or_else(|| "\u{fffd}".into())
        }
        CellContent::Cluster(s) => s.to_string(),
    }
}

fn screen_text(t: &TerminalState, keep_padding: bool) -> String {
    let grid = t.grid();
    let mut out = String::new();
    for row in 0..grid.rows() {
        let line = grid
            .row_slice(row)
            .map(|cells| cells.iter().map(|c| cell_text(&c.content)).collect::<String>())
            .unwrap_or_default();
        if keep_padding {
            out.push_str(&line);
        } else {
            out.push_str(line.trim_end());
        }
        out.push('\n');
    }
    out
}

fn main() {
    let opts = match parse() {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(if msg.starts_with("unknown") { 2 } else { 0 });
        }
    };

    let dims = Dimensions {
        cols: opts.cols,
        rows: opts.rows,
    };
    let spec = SessionSpec {
        program: opts.shell.clone().into(),
        args: Vec::new(),
        cwd: None,
    };

    let session = match session::spawn(&spec, dims) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("spawn {}: {e}", opts.shell);
            std::process::exit(3);
        }
    };
    let pid = session.pid().unwrap_or(0);

    // ARCHITECTURE.md 3: the reader thread moves bytes and holds no engine state.
    let parts = match session.split() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("split: {e}");
            std::process::exit(3);
        }
    };

    let stream = Arc::new(StreamBuffer::new(Limits::default()));
    let pump = stream.clone();
    let reader_thread = std::thread::spawn(move || {
        let mut reader = parts.reader;
        let mut buf = [0u8; READ_CHUNK];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    // Wait *before* accepting more so memory stays bounded. This
                    // is ARCHITECTURE.md 3.0.1: backpressure is applied before the
                    // data is taken, never by discarding it.
                    if pump.wait_for_capacity(n) != ReadState::Open {
                        break;
                    }
                    pump.push(&buf[..n]);
                }
                Err(e) => {
                    eprintln!("reader: {e}");
                    break;
                }
            }
        }
        pump.finish();
    });

    let mut terminal = TerminalState::new(opts.cols, opts.rows);
    let mut writer = parts.writer;
    let mut control = parts.control;

    let start = Instant::now();
    let deadline = start + opts.timeout;
    let mut bytes_seen = 0u64;
    let mut exit: Option<i32> = None;

    // Each input line is sent only after the shell has gone quiet.
    //
    // This is what a real terminal does, and it is not cosmetic. PowerShell needs
    // roughly 1.5s to load profiles and attach its console host; input written at
    // t=0 is discarded before anything is listening. Waiting for quiet rather than
    // matching a prompt string is deliberate: matching a prompt would couple the
    // harness to one shell, which ARCHITECTURE.md 5.1 forbids.
    for line in &opts.sends {
        exit = drain_until_quiet(
            &stream,
            &mut terminal,
            &mut control,
            opts.quiet,
            deadline,
            &mut bytes_seen,
        );
        if exit.is_some() || Instant::now() >= deadline {
            break;
        }
        let mut bytes = line.clone().into_bytes();
        bytes.push(b'\r');
        if let Err(e) = writer.write(&bytes) {
            eprintln!("write {line:?}: {e}");
        }
    }

    // Final collect.
    if exit.is_none() {
        exit = drain_until_quiet(
            &stream,
            &mut terminal,
            &mut control,
            opts.quiet,
            deadline,
            &mut bytes_seen,
        );
    }

    // Never orphan the child. kill() is asynchronous, so poll to confirm.
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

    let text = screen_text(&terminal, opts.keep_padding);
    print!("{text}");

    eprintln!(
        "-- pid={pid} bytes={bytes_seen} exit={exit:?} scrollback={} dropped={} title={:?} elapsed={:?}",
        terminal.scrollback_len(),
        terminal.scrollback_dropped(),
        terminal.title(),
        start.elapsed()
    );

    // Screen alone is not the whole story: content that scrolled off is still in
    // history, and a marker that was written and then scrolled away is a PASS, not
    // a failure. Checking only the visible screen would report that as a miss.
    let history = scrollback_text(&terminal, opts.keep_padding);
    let mut failed = false;
    for want in &opts.expect {
        let on_screen = text.contains(want.as_str());
        let in_history = history.contains(want.as_str());
        if on_screen {
            eprintln!("OK  screen:  {want}");
        } else if in_history {
            eprintln!("OK  history: {want} (scrolled off)");
        } else {
            eprintln!("FAIL: {want:?} is on neither the screen nor scrollback");
            failed = true;
        }
    }
    if failed {
        std::process::exit(1);
    }
}

/// Render the full scrollback, oldest line first, excluding the visible screen.
fn scrollback_text(t: &TerminalState, keep_padding: bool) -> String {
    let mut out = String::new();
    for i in 0..t.scrollback_len() {
        let Some(cells) = t.scrollback_line(i) else { continue };
        let line = cells
            .iter()
            .map(|c| cell_text(&c.content))
            .collect::<String>();
        if keep_padding {
            out.push_str(&line);
        } else {
            out.push_str(line.trim_end());
        }
        out.push('\n');
    }
    out
}

/// Feed everything the child has produced into the engine until it has been quiet
/// for `quiet`, or the child exits, or `deadline` passes.
///
/// Returns the child's exit code if it has exited. Bytes are never discarded; the
/// engine simply is not advanced with a partial stream when the deadline lands,
/// and the caller sees exactly what arrived.
fn drain_until_quiet(
    stream: &Arc<StreamBuffer>,
    terminal: &mut TerminalState,
    control: &mut Box<dyn SessionControl>,
    quiet: Duration,
    deadline: Instant,
    bytes_seen: &mut u64,
) -> Option<i32> {
    let mut last = Instant::now();
    loop {
        let batch = stream.take();
        if !batch.is_empty() {
            *bytes_seen += batch.len() as u64;
            terminal.advance(&batch);
            last = Instant::now();
        }
        if let Some(status) = control.try_wait().expect("try_wait") {
            // Drain what is still buffered before reporting the exit.
            let batch = stream.take();
            if !batch.is_empty() {
                *bytes_seen += batch.len() as u64;
                terminal.advance(&batch);
            }
            return Some(status.code());
        }
        if last.elapsed() >= quiet || Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
}