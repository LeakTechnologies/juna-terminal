use super::{Dimensions, ExitStatus, Session};
use std::collections::VecDeque;
use std::io;

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
#[derive(Debug)]
pub struct FakeSession {
    /// Bytes the "child" would send us.
    output: VecDeque<u8>,
    /// Bytes we wrote to the "child", for asserting byte-exactness.
    input: Vec<u8>,
    dims: Dimensions,
    status: Option<ExitStatus>,
    resize_fault: bool,
    write_fault: bool,
}

impl FakeSession {
    pub fn new(dims: Dimensions) -> Self {
        Self {
            output: VecDeque::new(),
            input: Vec::new(),
            dims,
            status: None,
            resize_fault: false,
            write_fault: false,
        }
    }

    /// Queue bytes as though the child had written them.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.output.extend(bytes.iter().copied());
    }

    /// Everything written to the child so far.
    pub fn written(&self) -> &[u8] {
        &self.input
    }

    pub fn dimensions(&self) -> Dimensions {
        self.dims
    }

    /// Mark the child exited, so the next `read` drains and then reports EOF.
    pub fn finish(&mut self, code: i32) {
        self.status = Some(ExitStatus::from_code(code));
    }

    /// Make `resize` fail, so error propagation can be asserted.
    pub fn fail_resize(&mut self) {
        self.resize_fault = true;
    }

    /// Make `write` fail, so error propagation can be asserted.
    pub fn fail_write(&mut self) {
        self.write_fault = true;
    }
}

impl Session for FakeSession {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.write_fault {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fake write fault"));
        }
        self.input.extend_from_slice(bytes);
        Ok(())
    }

    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        if dst.is_empty() {
            return Ok(0);
        }
        // Mirror the real backends: drain what is queued, then report EOF. Queued
        // bytes must survive the transition, otherwise a fast child would lose
        // its last write -- the exact bug ARCHITECTURE.md 3.0 forbids.
        let n = self.output.len().min(dst.len());
        for (slot, byte) in dst.iter_mut().zip(self.output.drain(..n)) {
            *slot = byte;
        }
        Ok(n)
    }

    fn resize(&mut self, dims: Dimensions) -> io::Result<()> {
        if self.resize_fault {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "fake resize fault",
            ));
        }
        self.dims = dims;
        Ok(())
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self.status)
    }

    fn kill(&mut self) -> io::Result<()> {
        self.status.get_or_insert(ExitStatus::from_code(1));
        Ok(())
    }

    fn pid(&self) -> Option<u32> {
        Some(0)
    }
}