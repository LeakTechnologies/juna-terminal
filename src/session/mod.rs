//! Platform-neutral session boundary (`ARCHITECTURE.md` 3.9).
//!
//! A `Session` is a bidirectional byte pipe to exactly one child process. This
//! module is the **only** place in the project permitted to name a platform
//! API. The VT engine, the layout engine, and the renderer must never see a type
//! declared here, which is what makes a second backend a file added rather than a
//! codebase rewritten.

mod fake;
#[cfg(windows)]
mod windows;

pub use fake::FakeSession;
#[cfg(windows)]
pub use windows::ConptySession;

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

/// What to launch.
///
/// The argument vector is passed through **verbatim**. No quoting, joining, or
/// re-escaping is performed here: that already broke `conpty` 0.7 and is the
/// reason it was rejected (`DECISIONS.md` 6.5). `SessionSpec` holds the argument
/// boundaries, so the backend is responsible only for handing them to the OS.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
}

/// Character-cell dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dimensions {
    pub cols: u16,
    pub rows: u16,
}

impl Default for Dimensions {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

/// How a child ended. `code()` is the exit code, or `-1` when the platform
/// reports an exit without one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitStatus(i32);

impl ExitStatus {
    pub const fn from_code(code: i32) -> Self {
        Self(code)
    }

    pub const fn code(self) -> i32 {
        self.0
    }
}

/// A `Session` split into independently-owned halves.
///
/// This exists because a terminal **cannot** drive a session from one thread.
/// `read` blocks (and ConPTY never delivers EOF), so a single thread that blocks
/// reading cannot also answer input: if the child waits for input before
/// producing output, the loop deadlocks. `ARCHITECTURE.md` 3 already prescribes
/// the fix — reads happen on their own thread — but that is only possible if
/// ownership can be divided. `Session::split` is what divides it.
///
/// Typical ownership: `reader` onto the reader thread, `writer` and `control`
/// onto the UI thread. No handle is shared, so no locking is required.
pub struct SessionParts {
    pub reader: Box<dyn SessionReader>,
    pub writer: Box<dyn SessionWriter>,
    pub control: Box<dyn SessionControl>,
}

/// The blocking read half. Lives on the reader thread and holds **no** engine state.
pub trait SessionReader: Send {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize>;
}

/// The write half. Lives on the UI thread alongside the engine.
pub trait SessionWriter: Send {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
}

/// Resize, exit checking, and termination. Lives on the UI thread.
///
/// `kill` is asynchronous on every current backend; see `Session::kill`.
pub trait SessionControl: Send {
    fn resize(&mut self, dims: Dimensions) -> io::Result<()>;
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    fn kill(&mut self) -> io::Result<()>;
    fn pid(&self) -> Option<u32>;
}

/// A byte-exact bidirectional pipe to one child process.
///
/// `Session` is object-safe and carries no platform types, so callers hold
/// `Box<dyn Session>` and never learn which backend they got.
pub trait Session: Send {
    /// Write **every** byte of `bytes`, or return an error.
    ///
    /// A backend that can only partially write must loop internally rather than
    /// report a short count. `ARCHITECTURE.md` 3.0 forbids dropping bytes, and a
    /// silently short write is worse than a loud failure: the stream loses
    /// alignment and the parser can never resynchronise.
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Read into `dst`, returning the count. `Ok(0)` means **no further bytes will
    /// ever arrive** — it is not "nothing right now".
    ///
    /// This may block, and on Windows it effectively does: ConPTY holds its
    /// output pipe open after the child exits and does not deliver EOF
    /// (`DECISIONS.md` 6.5). A caller that waits for `Ok(0)` in order to notice
    /// exit therefore hangs forever. **Detect exit with `try_wait`, and run reads
    /// on their own thread**, which is what `ARCHITECTURE.md` 3 specifies and what
    /// `split` makes possible.
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize>;

    /// Tell the child its window changed. Failure is reported, not swallowed.
    fn resize(&mut self, dims: Dimensions) -> io::Result<()>;

    /// Non-blocking exit check. `Ok(None)` means still running.
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;

    /// Terminate the child and every descendant.
    ///
    /// **Returns before the process has actually died.** The Windows backend
    /// tears down the session job object and termination proceeds asynchronously,
    /// so `try_wait` may still report `None` on the next line. Callers must poll
    /// `try_wait` to confirm death; they must not treat a successful return as
    /// "the child is gone".
    fn kill(&mut self) -> io::Result<()>;

    /// Child process id, when the backend exposes one.
    fn pid(&self) -> Option<u32>;

    /// Divide ownership so reads and writes can proceed on different threads.
    ///
    /// Required rather than optional: a backend that cannot split cannot be
    /// driven interactively, and a default that quietly returns an error would
    /// surface as a hang at runtime rather than a compile error here.
    fn split(self: Box<Self>) -> io::Result<SessionParts>;
}

/// Spawn a session using this target's backend.
///
/// Returns `Unsupported` on targets that have no backend yet. That is the honest
/// answer for every non-Windows target today: the Unix PTY backend named in
/// `ARCHITECTURE.md` 3.9 is specified but **not implemented**, and this function
/// must not pretend otherwise.
#[cfg(windows)]
pub fn spawn(spec: &SessionSpec, dims: Dimensions) -> io::Result<Box<dyn Session>> {
    ConptySession::spawn(spec, dims).map(|s| Box::new(s) as Box<dyn Session>)
}

#[cfg(not(windows))]
pub fn spawn(spec: &SessionSpec, dims: Dimensions) -> io::Result<Box<dyn Session>> {
    let _ = (spec, dims);
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no session backend for this target yet; see ARCHITECTURE.md 3.9",
    ))
}

#[cfg(test)]
mod tests;