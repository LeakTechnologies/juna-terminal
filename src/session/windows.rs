use super::{
    Dimensions, ExitStatus, Session, SessionControl, SessionParts, SessionReader, SessionSpec,
    SessionWriter,
};
use std::io::{self, Read, Write};

/// ConPTY-backed `Session`.
///
/// Wraps `conpty-oxide`'s owned halves rather than reaching for raw handles, so
/// this file contains **no `unsafe`**. Handle ownership stays inside the crate
/// we pinned in `DECISIONS.md` 6.3, and every `// SAFETY:` obligation remains
/// theirs.
pub struct ConptySession {
    read: conpty_oxide::blocking::OwnedReadHalf,
    write: conpty_oxide::blocking::OwnedWriteHalf,
    controller: conpty_oxide::PtyController,
    child: conpty_oxide::blocking::Child,
}

fn io_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

fn conpty_size(dims: Dimensions) -> io::Result<conpty_oxide::Size> {
    conpty_oxide::Size::try_new(dims.cols, dims.rows)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))
}

/// Windows reports exit codes as `u32`; our `ExitStatus` is `i32`. A code that
/// does not fit becomes `-1` rather than wrapping to a plausible-looking value,
/// so an unrepresentable exit can never be mistaken for a small success code.
fn exit_code(s: conpty_oxide::ExitStatus) -> i32 {
    i32::try_from(s.code()).unwrap_or(-1)
}

impl ConptySession {
    pub fn spawn(spec: &SessionSpec, dims: Dimensions) -> io::Result<Self> {
        let mut cmd = conpty_oxide::blocking::Command::new(spec.program.as_os_str());

        // Args go through one at a time and are never joined into a single
        // string. Joining is what corrupted arguments containing spaces or
        // quotes in `conpty` 0.7 -- the defect that disqualified it.
        for arg in &spec.args {
            cmd.arg(arg);
        }

        if let Some(dir) = &spec.cwd {
            cmd.current_dir(dir);
        }
        for (key, value) in &spec.env {
            cmd.env(key, value);
        }

        let session = cmd
            .spawn_with(conpty_oxide::SessionOptions::new())
            .map_err(io_err)?;

        session.resize(conpty_size(dims)?).map_err(io_err)?;

        let parts = session.into_parts();
        Ok(Self {
            read: parts.output,
            write: parts.input,
            controller: parts.controller,
            child: parts.child,
        })
    }
}

impl Session for ConptySession {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        // Loop rather than a single `write_all`: the contract is all-or-error, and
        // `write_all` already guarantees that, but the loop keeps the intent
        // visible at the exact place a short write would corrupt the stream.
        self.write.write_all(bytes)?;
        self.write.flush()
    }

    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.read.read(dst)
    }

    fn resize(&mut self, dims: Dimensions) -> io::Result<()> {
        self.controller.resize(conpty_size(dims)?).map_err(io_err)
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self
            .child
            .try_wait()
            .map_err(io_err)?
            .map(|s| ExitStatus::from_code(exit_code(s))))
    }

    /// Terminates the child **and every descendant**, because the backend holds
    /// the session's job object. That is the property `conpty` 0.7 lacked and the
    /// one its replacement was bought for (`DECISIONS.md` 6.5).
    fn kill(&mut self) -> io::Result<()> {
        self.child.kill().map_err(io_err)
    }

    fn pid(&self) -> Option<u32> {
        Some(self.child.id())
    }

    /// ConPTY already hands out independently-owned halves, so the split is a
    /// move rather than a copy. No handle is duplicated and none is shared, which
    /// is why the reader and UI threads need no locking between them.
    fn split(self: Box<Self>) -> io::Result<SessionParts> {
        Ok(SessionParts {
            reader: Box::new(ConptyReader { half: self.read }),
            writer: Box::new(ConptyWriter { half: self.write }),
            control: Box::new(ConptyControl {
                controller: self.controller,
                child: self.child,
            }),
        })
    }
}

struct ConptyReader {
    half: conpty_oxide::blocking::OwnedReadHalf,
}

impl SessionReader for ConptyReader {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.half.read(dst)
    }
}

struct ConptyWriter {
    half: conpty_oxide::blocking::OwnedWriteHalf,
}

impl SessionWriter for ConptyWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        // All-or-error, per Session::write. `write_all` already guarantees this,
        // but the loop keeps the intent visible where a short write would corrupt
        // the byte stream.
        self.half.write_all(bytes)?;
        self.half.flush()
    }
}

struct ConptyControl {
    controller: conpty_oxide::PtyController,
    child: conpty_oxide::blocking::Child,
}

impl SessionControl for ConptyControl {
    fn resize(&mut self, dims: Dimensions) -> io::Result<()> {
        self.controller.resize(conpty_size(dims)?).map_err(io_err)
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self
            .child
            .try_wait()
            .map_err(io_err)?
            .map(|s| ExitStatus::from_code(exit_code(s))))
    }

    fn kill(&mut self) -> io::Result<()> {
        self.child.kill().map_err(io_err)
    }

    fn pid(&self) -> Option<u32> {
        Some(self.child.id())
    }
}

impl std::fmt::Debug for ConptySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConptySession")
            .field("pid", &self.child.id())
            .finish_non_exhaustive()
    }
}
