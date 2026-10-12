use super::{
    Dimensions, ExitStatus, Session, SessionControl, SessionParts, SessionReader, SessionWriter,
};
use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct Pipe {
    output: VecDeque<u8>,
    input: Vec<u8>,
    status: Option<ExitStatus>,
    dims: Dimensions,
    resize_fault: bool,
    write_fault: bool,
}

/// An in-memory `Session` used to test everything above the backend seam without
/// spawning a process.
///
/// It exists for one reason beyond convenience: on a target with no backend,
/// this is the **only** way to exercise the wiring. Because it is pure `std` it
/// compiles and runs identically on every platform, which is what keeps the seam
/// honest on a machine that has no ConPTY.
///
/// A fake backend that could pass on Windows and fail on Linux — or the reverse —
/// would not be testing the seam.
///
/// The shared `Mutex` guards the *fake pipe only*. `ARCHITECTURE.md` 5 forbids
/// interior mutability for **engine** state, which remains single-owner and
/// value-passed; this is a test double standing in for two OS pipe endpoints,
/// which genuinely are shared resources.
#[derive(Debug, Clone)]
pub struct FakeSession {
    pipe: Arc<Mutex<Pipe>>,
}

impl FakeSession {
    pub fn new(dims: Dimensions) -> Self {
        Self {
            pipe: Arc::new(Mutex::new(Pipe {
                dims,
                ..Pipe::default()
            })),
        }
    }

    /// Queue bytes as though the child had written them. Visible to any handle
    /// sharing this pipe, which is what makes the split testable.
    pub fn feed(&self, bytes: &[u8]) {
        let mut p = self.lock();
        p.output.extend(bytes.iter().copied());
    }

    /// Everything written to the child so far.
    pub fn written(&self) -> Vec<u8> {
        self.lock().input.clone()
    }

    pub fn dimensions(&self) -> Dimensions {
        self.lock().dims
    }

    /// Mark the child exited, so the next `read` drains and then reports EOF.
    pub fn finish(&self, code: i32) {
        self.lock().status = Some(ExitStatus::from_code(code));
    }

    /// Make `resize` fail, so error propagation can be asserted.
    pub fn fail_resize(&self) {
        self.lock().resize_fault = true;
    }

    /// Make `write` fail, so error propagation can be asserted.
    pub fn fail_write(&self) {
        self.lock().write_fault = true;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Pipe> {
        self.pipe.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Session for FakeSession {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut g = self.lock();
        write_into(&mut g, bytes)
    }

    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        let mut g = self.lock();
        read_from(&mut g, dst)
    }

    fn resize(&mut self, dims: Dimensions) -> io::Result<()> {
        let mut g = self.lock();
        resize_pipe(&mut g, dims)
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self.lock().status)
    }

    fn kill(&mut self) -> io::Result<()> {
        self.lock().status.get_or_insert(ExitStatus::from_code(1));
        Ok(())
    }

    fn pid(&self) -> Option<u32> {
        Some(0)
    }

    fn split(self: Box<Self>) -> io::Result<SessionParts> {
        Ok(SessionParts {
            reader: Box::new(FakeReader { pipe: self.pipe.clone() }),
            writer: Box::new(FakeWriter {
                pipe: self.pipe.clone(),
            }),
            control: Box::new(FakeControl { pipe: self.pipe }),
        })
    }
}

fn write_into(p: &mut Pipe, bytes: &[u8]) -> io::Result<()> {
    if p.write_fault {
        return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fake write fault"));
    }
    p.input.extend_from_slice(bytes);
    Ok(())
}

fn read_from(p: &mut Pipe, dst: &mut [u8]) -> io::Result<usize> {
    if dst.is_empty() {
        return Ok(0);
    }
    // Mirror the real backends: drain what is queued, then report EOF. Queued
    // bytes must survive the transition, otherwise a fast child would lose its
    // last write -- the exact bug ARCHITECTURE.md 3.0 forbids.
    let n = p.output.len().min(dst.len());
    for (slot, byte) in dst.iter_mut().zip(p.output.drain(..n)) {
        *slot = byte;
    }
    Ok(n)
}

fn resize_pipe(p: &mut Pipe, dims: Dimensions) -> io::Result<()> {
    if p.resize_fault {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "fake resize fault",
        ));
    }
    p.dims = dims;
    Ok(())
}

struct FakeReader {
    pipe: Arc<Mutex<Pipe>>,
}

impl SessionReader for FakeReader {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        read_from(&mut self.pipe.lock().unwrap_or_else(|e| e.into_inner()), dst)
    }
}

struct FakeWriter {
    pipe: Arc<Mutex<Pipe>>,
}

impl SessionWriter for FakeWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        write_into(&mut self.pipe.lock().unwrap_or_else(|e| e.into_inner()), bytes)
    }
}

struct FakeControl {
    pipe: Arc<Mutex<Pipe>>,
}

impl SessionControl for FakeControl {
    fn resize(&mut self, dims: Dimensions) -> io::Result<()> {
        resize_pipe(&mut self.pipe.lock().unwrap_or_else(|e| e.into_inner()), dims)
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self.pipe.lock().unwrap_or_else(|e| e.into_inner()).status)
    }

    fn kill(&mut self) -> io::Result<()> {
        self.pipe
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .status
            .get_or_insert(ExitStatus::from_code(1));
        Ok(())
    }

    fn pid(&self) -> Option<u32> {
        Some(0)
    }
}