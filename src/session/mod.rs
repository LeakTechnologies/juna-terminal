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
    /// Overrides apply only to this child; the parent environment is unchanged.
    pub env: Vec<(OsString, OsString)>,
}
impl SessionSpec {
    pub fn from_environment() -> io::Result<Self> {
        let home =
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        launch_options(
            std::env::args_os().skip(1),
            std::env::var_os("JUNA_SHELL").unwrap_or_else(|| "powershell.exe".into()),
            std::env::var("JUNA_SHELL_ARGS").ok().as_deref(),
            home,
        )
    }
    /// True when the launch named its own shell. An explicit choice outranks the
    /// stored preference for the lifetime of this process.
    pub fn shell_is_explicit() -> bool {
        std::env::var_os("JUNA_SHELL").is_some()
            || std::env::var_os("JUNA_SHELL_ARGS").is_some()
            || std::env::args_os()
                .skip(1)
                .take_while(|argument| argument != "--")
                .any(|argument| argument == "--shell")
    }
}
fn launch_options(
    arguments: impl IntoIterator<Item = OsString>,
    program: OsString,
    legacy_args: Option<&str>,
    home: Option<PathBuf>,
) -> io::Result<SessionSpec> {
    let mut spec = SessionSpec {
        program,
        args: Vec::new(),
        cwd: None,
        env: Vec::new(),
    };
    let mut command = arguments.into_iter();
    let mut explicit_args = false;
    while let Some(arg) = command.next() {
        if arg == "--shell" {
            spec.program = command.next().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "--shell requires a program")
            })?;
        } else if arg == "--cwd" {
            if spec.cwd.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--cwd may only be supplied once",
                ));
            }
            let cwd = command.next().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "--cwd requires a directory")
            })?;
            let directory = std::fs::canonicalize(PathBuf::from(cwd))
                    .ok().filter(|path| path.is_dir())
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Starting directory is unavailable. Choose an existing folder with --cwd DIRECTORY."))?;
            spec.cwd = Some(shell_directory(directory));
        } else if arg == "--" {
            spec.args = command.collect();
            explicit_args = true;
            break;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: juna [--shell PROGRAM] [--cwd DIRECTORY] [-- ARGS...]",
            ));
        }
    }
    if !explicit_args {
        if let Some(args) = legacy_args {
            spec.args = parse_arguments(args)?;
        }
    }
    if spec.cwd.is_none() && spec.args.is_empty() {
        let directory = home.filter(|path| path.is_absolute() && path.is_dir())
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "User home directory is unavailable. Check your home-folder configuration or launch with --cwd DIRECTORY."))?;
        spec.cwd = Some(shell_directory(directory));
    }
    Ok(spec)
}
/// Use ordinary drive/share syntax after validating a canonical Windows path.
fn shell_directory(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        if let Some(Component::Prefix(prefix)) = path.components().next() {
            let mut plain = match prefix.kind() {
                Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:\\", drive as char)),
                Prefix::VerbatimUNC(server, share) => {
                    let mut name = OsString::from("\\\\");
                    name.push(server);
                    name.push("\\");
                    name.push(share);
                    PathBuf::from(name)
                }
                _ => return path,
            };
            for component in path.components().skip(2) {
                plain.push(component.as_os_str());
            }
            return plain;
        }
    }
    path
}
/// Parse the legacy environment value with Windows quote/backslash rules.
/// CLI arguments after `--` are already an argument vector and bypass this parser.
pub fn parse_arguments(text: &str) -> io::Result<Vec<OsString>> {
    let mut out = Vec::new();
    let mut arg = String::new();
    let mut quoted = false;
    let mut active = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let mut slashes = 1;
            while chars.peek() == Some(&'\\') {
                chars.next();
                slashes += 1;
            }
            if chars.peek() == Some(&'"') {
                arg.extend(std::iter::repeat_n('\\', slashes / 2));
                chars.next();
                if slashes % 2 == 1 {
                    arg.push('"');
                } else {
                    quoted = !quoted;
                }
            } else {
                arg.extend(std::iter::repeat_n('\\', slashes));
            }
            active = true;
        } else if c == '"' {
            quoted = !quoted;
            active = true;
        } else if c.is_whitespace() && !quoted {
            if active {
                out.push(OsString::from(std::mem::take(&mut arg)));
                active = false;
            }
        } else {
            arg.push(c);
            active = true;
        }
    }
    if quoted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unclosed quote in JUNA_SHELL_ARGS",
        ));
    }
    if active {
        out.push(arg.into());
    }
    Ok(out)
}

#[cfg(test)]
mod argument_tests {
    #[test]
    fn home_explicit_directory_and_command_launches_are_distinct() {
        use super::*;
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let home = root.join("home 日本語");
        let chosen = root.join("chosen folder");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&chosen).unwrap();
        let parse = |arguments: Vec<OsString>, legacy: Option<&str>, home: Option<PathBuf>| {
            launch_options(arguments, "configured shell.exe".into(), legacy, home)
        };
        let before = std::env::current_dir().unwrap();
        let normal = parse(vec![], None, Some(home.clone())).unwrap();
        assert_eq!(normal.cwd, Some(home.clone()));
        assert!(normal.args.is_empty());
        let explicit = parse(
            vec!["--cwd".into(), chosen.clone().into_os_string(), "--".into()],
            Some("ignored one-off"),
            None,
        )
        .unwrap();
        assert_eq!(
            std::fs::canonicalize(explicit.cwd.unwrap()).unwrap(),
            std::fs::canonicalize(&chosen).unwrap()
        );
        assert!(explicit.args.is_empty());
        let script = parse(
            vec!["--".into(), "/C".into(), "echo a b".into()],
            None,
            None,
        )
        .unwrap();
        assert!(script.cwd.is_none());
        assert_eq!(
            script.args,
            [OsString::from("/C"), OsString::from("echo a b")]
        );
        assert!(parse(vec![], Some("/C exit"), None).unwrap().cwd.is_none());
        assert!(parse(vec![], None, None).is_err());
        assert!(parse(vec![], None, Some(root.join("missing"))).is_err());
        assert!(parse(vec!["--cwd".into()], None, Some(home.clone())).is_err());
        assert!(parse(
            vec![
                "--cwd".into(),
                chosen.clone().into_os_string(),
                "--cwd".into(),
                home.clone().into_os_string()
            ],
            None,
            None
        )
        .is_err());
        assert!(parse(
            vec!["--cwd".into(), root.join("missing").into_os_string()],
            None,
            Some(home)
        )
        .is_err());
        assert_eq!(std::env::current_dir().unwrap(), before);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn quoted_arguments_and_empty_elements_are_preserved() {
        let args =
            super::parse_arguments(r#"-NoProfile -Command "write 'a b'" "" "C:\with space\\""#)
                .unwrap();
        let args: Vec<_> = args
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "-NoProfile",
                "-Command",
                "write 'a b'",
                "",
                "C:\\with space\\"
            ]
        );
        assert!(super::parse_arguments("\"unclosed").is_err());
    }
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
