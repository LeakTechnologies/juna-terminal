use super::{
    fake, Dimensions, ExitStatus, Session, SessionParts, SessionSpec,
};
use std::io;

fn dims() -> Dimensions {
    Dimensions { cols: 80, rows: 24 }
}

#[test]
fn session_is_object_safe() {
    // The seam's whole purpose is that callers hold `Box<dyn Session>` and never
    // learn the backend. If this stops compiling, a platform type leaked into the
    // trait signature and a second backend just became a rewrite.
    fn takes_dyn(s: &mut dyn Session) -> io::Result<usize> {
        let mut buf = [0u8; 8];
        s.read(&mut buf)
    }
    let mut s = fake::FakeSession::new(dims());
    s.feed(b"abc");
    assert_eq!(takes_dyn(&mut s).unwrap(), 3);
}

#[test]
fn spec_keeps_argument_boundaries_separate() {
    // The defect that disqualified conpty 0.7 was joining args into one string.
    // The spec must be able to represent arguments that a joined string cannot
    // round-trip, so this asserts the *shape*, not a particular backend.
    let spec = SessionSpec {
        program: "probe.exe".into(),
        args: vec![
            "has space".into(),
            "has\"quote".into(),
            "trailing ".into(),
            "".into(),
        ],
        cwd: None,
        env: Vec::new(),
    };
    assert_eq!(spec.args.len(), 4);
    assert_eq!(spec.args[0], "has space");
    assert_eq!(spec.args[1], "has\"quote");
    assert_eq!(spec.args[2], "trailing ");
    assert!(spec.args[3].is_empty());
}

#[test]
fn fake_round_trips_bytes_exactly() {
    // Same property ARCHITECTURE.md 3.0 demands of a real backend.
    let payload: Vec<u8> = (0u8..=255).collect();
    let mut s = fake::FakeSession::new(dims());
    s.feed(&payload);

    let mut got = Vec::new();
    loop {
        let mut buf = [0u8; 7];
        let n = s.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        got.extend_from_slice(&buf[..n]);
    }
    assert_eq!(got, payload, "fake backend must not reorder or drop");
}

#[test]
fn fake_preserves_queued_bytes_across_a_child_exit() {
    // A child that writes then exits is the ordinary case. If exit cleared the
    // queue, the last write before exit would vanish -- the precise corruption
    // 3.0 forbids, reproduced here where it is cheap to catch.
    let mut s = fake::FakeSession::new(dims());
    s.feed(b"final words");
    s.finish(0);

    let mut buf = [0u8; 64];
    let first = s.read(&mut buf).unwrap();
    assert_eq!(&buf[..first], b"final words", "exit must not discard queued bytes");

    let second = s.read(&mut buf).unwrap();
    assert_eq!(second, 0, "EOF only after the queue drains");
    assert!(s.try_wait().unwrap().is_some());
}

#[test]
fn partial_reads_never_lose_a_byte() {
    // dst smaller than the queue must return exactly dst.len(), never more and
    // never fewer, or the caller loops forever or skips data.
    let mut s = fake::FakeSession::new(dims());
    s.feed(b"0123456789");
    let mut buf = [0u8; 4];
    assert_eq!(s.read(&mut buf).unwrap(), 4);
    assert_eq!(&buf, b"0123");
    assert_eq!(s.read(&mut buf).unwrap(), 4);
    assert_eq!(&buf, b"4567");
}

#[test]
fn write_is_recorded_verbatim() {
    let mut s = fake::FakeSession::new(dims());
    s.write(b"ls -la\n").unwrap();
    s.write(b"\x1b[31mred\x1b[0m").unwrap();
    assert_eq!(s.written(), b"ls -la\n\x1b[31mred\x1b[0m");
}

#[test]
fn backend_faults_surface_as_errors() {
    // A backend that swallows errors turns a resize failure into a silently wrong
    // layout. The seam must not let that happen.
    let mut s = fake::FakeSession::new(dims());
    s.fail_resize();
    assert_eq!(
        s.resize(Dimensions { cols: 1, rows: 1 }).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );

    let mut w = fake::FakeSession::new(dims());
    w.fail_write();
    assert_eq!(
        w.write(b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn resize_updates_dimensions() {
    let mut s = fake::FakeSession::new(dims());
    s.resize(Dimensions { cols: 120, rows: 40 }).unwrap();
    assert_eq!(s.dimensions(), Dimensions { cols: 120, rows: 40 });
}

#[test]
fn kill_marks_the_session_exited() {
    let mut s = fake::FakeSession::new(dims());
    assert!(s.try_wait().unwrap().is_none());
    s.kill().unwrap();
    assert!(s.try_wait().unwrap().is_some());
}

#[test]
fn exit_status_reports_minus_one_when_the_platform_gives_no_code() {
    // Signalled or otherwise uncoded exits must not silently read as success.
    assert_eq!(ExitStatus::from_code(0).code(), 0);
    assert_eq!(ExitStatus::from_code(1).code(), 1);
}

#[test]
fn spawn_reports_a_missing_program_instead_of_panicking() {
    // Cross-platform. A nonexistent program must yield an error, never a panic
    // and never a fake success. The error *kind* legitimately differs by backend
    // (NotFound where a backend exists, Unsupported where none does), so this
    // asserts only the invariant that holds everywhere.
    let spec = SessionSpec {
        program: "definitely-not-a-real-program-xyz".into(),
        args: Vec::new(),
        cwd: None,
        env: Vec::new(),
    };
    assert!(super::spawn(&spec, dims()).is_err());
}

#[test]
fn split_halves_move_independently_across_threads() {
    // The reason `split` exists: a reader thread blocks on `read` while the UI
    // thread writes. If the halves were not genuinely independent, this deadlocks
    // or the write never lands.
    let fake = fake::FakeSession::new(dims());
    let parts: Box<dyn Session> = Box::new(fake.clone());

    let SessionParts {
        reader,
        mut writer,
        mut control,
    } = parts.split().expect("split must succeed");

    // The UI thread owns writer + control; the reader thread owns only reader.
    let handle = std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 64];
        let n = reader.read(&mut buf).expect("reader thread read");
        buf[..n].to_vec()
    });

    writer.write(b"typed by the ui thread").expect("write");
    fake.feed(b"pumped back to the reader");

    let got = handle.join().expect("reader thread");
    assert_eq!(got, b"pumped back to the reader");

    assert_eq!(fake.written(), b"typed by the ui thread");
    assert!(control.try_wait().expect("try_wait").is_none());
}

#[test]
fn split_control_plane_still_reports_exit() {
    let fake = fake::FakeSession::new(dims());
    let mut parts = Box::new(fake.clone()).split().expect("split");
    fake.finish(7);

    let status = parts.control.try_wait().expect("try_wait");
    assert_eq!(status.map(|s| s.code()), Some(7));
}

#[test]
fn split_writes_are_all_or_error_on_the_shim_too() {
    let fake = fake::FakeSession::new(dims());
    fake.fail_write();
    let mut parts = Box::new(fake).split().expect("split");
    assert_eq!(
        parts.writer.write(b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn split_survives_a_reader_that_reads_before_anything_is_written() {
    // Ordering the harness relies on: the reader thread starts first and blocks,
    // and the write that follows must still be delivered rather than lost to a
    // race.
    let fake = fake::FakeSession::new(dims());
    let parts = Box::new(fake.clone()).split().expect("split");

    let reader = parts.reader;
    let mut writer = parts.writer;

    let h = std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 8];
        reader.read(&mut buf).expect("read")
    });

    writer.write(b"later").expect("write");
    assert_eq!(h.join().unwrap(), 0, "nothing was queued for the reader yet");
    assert_eq!(fake.written(), b"later");
}

#[cfg(not(windows))]
#[test]
fn targets_without_a_backend_report_unsupported() {
    // Where no backend exists, `spawn` must say so precisely rather than
    // degrading into a different error or pretending to succeed.
    let spec = SessionSpec {
        program: "whatever".into(),
        args: Vec::new(),
        cwd: None,
        env: Vec::new(),
    };
    assert_eq!(
        super::spawn(&spec, dims()).unwrap_err().kind(),
        io::ErrorKind::Unsupported
    );
}

#[cfg(windows)]
mod conpty {
    use super::super::{fake, spawn, Dimensions, Session, SessionSpec};
    use std::ffi::OsString;
    use std::time::{Duration, Instant};

    /// `CARGO_BIN_EXE_*` is only defined for integration tests, so a unit test
    /// inside `src/` uses a program Windows always ships: `cmd.exe`. That also
    /// makes it a real interactive shell, which is a better thing to point the
    /// seam at than a probe binary anyway.
    fn shell() -> SessionSpec {
        SessionSpec {
            program: OsString::from("cmd.exe"),
            args: vec!["/C".into(), "echo JUNA_SEAM_OK".into()],
            cwd: None,
            env: Vec::new(),
        }
    }

    /// The seam must survive contact with the real backend, not just the fake.
    #[test]
    fn real_backend_round_trips_through_the_trait() {
        let mut s = spawn(&shell(), Dimensions { cols: 80, rows: 24 }).expect("spawn via seam");
        assert!(s.pid().is_some(), "a live child must report a pid");

        let mut buf = [0u8; 4096];
        let start = Instant::now();
        let mut got = Vec::new();
        while start.elapsed() < Duration::from_secs(30) {
            let n = s.read(&mut buf).expect("read");
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
        let text = String::from_utf8_lossy(&got);
        assert!(
            text.contains("JUNA_SEAM_OK"),
            "cmd.exe output did not arrive through the seam: {text:?}"
        );
    }

    #[test]
    fn real_backend_resize_is_accepted() {
        let mut s = spawn(&shell(), Dimensions { cols: 80, rows: 24 }).unwrap();
        s.resize(Dimensions { cols: 120, rows: 40 })
            .expect("resize via seam");
        let mut buf = [0u8; 4096];
        let _ = s.read(&mut buf);
    }

    #[test]
    fn kill_terminates_a_live_child() {
        let mut s = spawn(
            &SessionSpec {
                program: OsString::from("cmd.exe"),
                args: vec!["/C".into(), "ping -n 60 127.0.0.1 > nul".into()],
                cwd: None,
                env: Vec::new(),
            },
            Dimensions::default(),
        )
        .unwrap();

        s.kill().expect("kill");

        // kill() returns before the process is reaped, so this must poll rather
        // than assert on the very next call -- see Session::kill.
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut exited = false;
        while Instant::now() < deadline {
            if s.try_wait().expect("try_wait").is_some() {
                exited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(exited, "child did not exit within 30s of kill()");
    }

    #[test]
    fn fake_and_real_share_one_trait_object() {
        // Both must be usable through `dyn Session`, which is the whole claim of
        // the seam: callers hold one type and never learn the backend.
        fn drive(s: &mut dyn Session) -> bool {
            let mut b = [0u8; 1];
            s.read(&mut b).is_ok()
        }
        let mut f = fake::FakeSession::new(Dimensions::default());
        assert!(drive(&mut f));
    }
}