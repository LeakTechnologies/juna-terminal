//! Session lifecycle owns byte transport, never terminal state.
use crate::session::{self, Dimensions, ExitStatus, SessionControl, SessionSpec};
use crate::transport::{Limits, ReadState, StreamBuffer};
use std::{
    io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub struct Runtime {
    pub stream: Arc<StreamBuffer>,
    pub control: Box<dyn SessionControl>,
    writer: Option<mpsc::Sender<Vec<u8>>>,
    writer_thread: Option<JoinHandle<()>>,
    pending_input: Arc<AtomicUsize>,
    reader: Option<JoinHandle<()>>,
    faults: mpsc::Receiver<String>,
    closing: Option<Instant>,
    killed: bool,
    pub exit: Option<ExitStatus>,
}
impl Runtime {
    pub fn spawn(spec: &SessionSpec, dims: Dimensions) -> io::Result<Self> {
        let parts = session::spawn(spec, dims)?.split()?;
        let stream = Arc::new(StreamBuffer::new(Limits::default()));
        let pump = stream.clone();
        let (fault_tx, faults) = mpsc::channel();
        let writer_faults = fault_tx.clone();
        let (writer_tx, writer_rx) = mpsc::channel::<Vec<u8>>();
        let pending_input = Arc::new(AtomicUsize::new(0));
        let count = pending_input.clone();
        let writer_thread = std::thread::Builder::new()
            .name("juna-writer".into())
            .spawn(move || {
                let mut writer = parts.writer;
                while let Ok(bytes) = writer_rx.recv() {
                    let result = writer.write(&bytes);
                    count.fetch_sub(bytes.len(), Ordering::Relaxed);
                    if let Err(e) = result {
                        let _ = writer_faults.send(format!("session write: {e}"));
                        break;
                    }
                }
            })?;
        let mut control = parts.control;
        let reader = std::thread::Builder::new()
            .name("juna-reader".into())
            .spawn(move || {
                let mut reader = parts.reader;
                let mut bytes = [0; 8192];
                loop {
                    if pump.wait_for_capacity(bytes.len()) != ReadState::Open {
                        break;
                    }
                    match reader.read(&mut bytes) {
                        Ok(0) => break,
                        Ok(n) => pump.push(&bytes[..n]),
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) => {
                            let _ = fault_tx.send(format!("session read: {e}"));
                            break;
                        }
                    }
                }
                pump.finish();
            });
        let reader = match reader {
            Ok(r) => r,
            Err(e) => {
                let _ = control.kill();
                return Err(e);
            }
        };
        Ok(Self {
            stream,
            control,
            writer: Some(writer_tx),
            writer_thread: Some(writer_thread),
            pending_input,
            reader: Some(reader),
            faults,
            closing: None,
            killed: false,
            exit: None,
        })
    }
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let Some(w) = self.writer.as_ref() else {
            return Err(io::ErrorKind::BrokenPipe.into());
        };
        // Rejection happens before acceptance, is explicit, and never drops queued bytes.
        const LIMIT: usize = 8 * 1024 * 1024;
        if self
            .pending_input
            .load(Ordering::Relaxed)
            .saturating_add(bytes.len())
            > LIMIT
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "input queue exceeds 8 MiB; child is not reading",
            ));
        }
        self.pending_input.fetch_add(bytes.len(), Ordering::Relaxed);
        if w.send(bytes.to_vec()).is_err() {
            self.pending_input.fetch_sub(bytes.len(), Ordering::Relaxed);
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        Ok(())
    }
    pub fn request_close(&mut self) {
        if self.closing.is_none() {
            self.closing = Some(Instant::now());
            self.writer.take();
        }
    }
    /// Called after each bounded output drain. Returns true only after the reader is joined.
    pub fn tick(&mut self) -> io::Result<bool> {
        if let Ok(fault) = self.faults.try_recv() {
            self.request_close();
            return Err(io::Error::other(fault));
        }
        if self.exit.is_none() {
            self.exit = self.control.try_wait()?;
        }
        if self.exit.is_some() {
            self.request_close();
        }
        if self.stream.is_finished() && self.exit.is_none() {
            self.request_close();
        }
        if let Some(start) = self.closing {
            if self.exit.is_none() && !self.killed && start.elapsed() >= Duration::from_secs(2) {
                self.control.kill()?;
                self.killed = true;
            }
            if self.exit.is_some() && self.stream.depth() == 0 {
                if self.reader.as_ref().is_some_and(JoinHandle::is_finished) {
                    self.reader
                        .take()
                        .unwrap()
                        .join()
                        .map_err(|_| io::Error::other("reader panicked"))?;
                }
                if self
                    .writer_thread
                    .as_ref()
                    .is_some_and(JoinHandle::is_finished)
                {
                    self.writer_thread
                        .take()
                        .unwrap()
                        .join()
                        .map_err(|_| io::Error::other("writer panicked"))?;
                }
                if self.reader.is_none() && self.writer_thread.is_none() {
                    return Ok(true);
                }
            }
            if start.elapsed() > Duration::from_secs(10) {
                return Err(io::Error::other("session shutdown exceeded 10 seconds"));
            }
        }
        Ok(false)
    }
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn an_already_joined_reader_does_not_block_the_writer_join() {
        use crate::session::Session;
        let parts = Box::new(crate::session::FakeSession::new(Dimensions::default()))
            .split()
            .unwrap();
        let (_, faults) = mpsc::channel();
        let stream = Arc::new(StreamBuffer::new(Limits::default()));
        stream.finish();
        let writer_thread = std::thread::spawn(|| {});
        while !writer_thread.is_finished() {
            std::thread::yield_now();
        }
        let mut runtime = Runtime {
            stream,
            control: parts.control,
            writer: None,
            writer_thread: Some(writer_thread),
            pending_input: Arc::new(AtomicUsize::new(0)),
            reader: None,
            faults,
            closing: Some(Instant::now()),
            killed: false,
            exit: Some(ExitStatus::from_code(0)),
        };
        assert!(runtime.tick().unwrap());
        assert!(runtime.writer_thread.is_none());
    }
    #[test]
    fn exits_and_close_join_threads_without_losing_tail() {
        for (shell, args) in [
            ("cmd.exe", vec!["/Q", "/C", "echo JUNA_TAIL_MARKER"]),
            (
                "powershell.exe",
                vec![
                    "-NoProfile",
                    "-Command",
                    "[Console]::Out.Write('JUNA_TAIL_MARKER'); exit 7",
                ],
            ),
        ] {
            let spec = SessionSpec {
                program: shell.into(),
                args: args.into_iter().map(Into::into).collect(),
                cwd: None,
            };
            let mut runtime = Runtime::spawn(&spec, Dimensions { cols: 80, rows: 24 }).unwrap();
            let mut bytes = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                bytes.extend(runtime.stream.take_up_to(4096));
                if runtime.tick().unwrap() {
                    break;
                }
                assert!(Instant::now() < deadline, "shutdown exceeded deadline");
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(String::from_utf8_lossy(&bytes).contains("JUNA_TAIL_MARKER"));
            assert!(runtime.reader.is_none());
            assert!(runtime.writer_thread.is_none());
            if shell == "powershell.exe" {
                assert_eq!(runtime.exit.unwrap().code(), 7);
            }
        }
        let mut runtime = Runtime::spawn(
            &SessionSpec {
                program: "cmd.exe".into(),
                args: Vec::new(),
                cwd: None,
            },
            Dimensions::default(),
        )
        .unwrap();
        runtime.request_close();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            runtime.stream.take();
            if runtime.tick().unwrap() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.writer.take();
        if self.exit.is_none() {
            let _ = self.control.kill();
        }
        self.stream.cancel();
    }
}
