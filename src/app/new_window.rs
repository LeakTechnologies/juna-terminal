//! Independent app launches; no shell command reconstruction or engine references.
use crate::session::SessionSpec;
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use winit::keyboard::{Key, ModifiersState};

pub fn shortcut(key: &Key, modifiers: ModifiersState) -> bool {
    modifiers.control_key()
        && modifiers.shift_key()
        && !modifiers.alt_key()
        && !modifiers.super_key()
        && matches!(key, Key::Character(s) if s.eq_ignore_ascii_case("n"))
}
fn local_absolute(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    }
    #[cfg(not(windows))]
    {
        true
    }
}
pub fn command(
    executable: &Path,
    shell: &SessionSpec,
    reported: Option<&str>,
    launch: &Path,
) -> io::Result<Command> {
    if !executable.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Application path is not absolute",
        ));
    }
    let reported = reported.map(PathBuf::from).filter(|p| local_absolute(p));
    let cwd = reported.as_deref().unwrap_or(launch);
    if !local_absolute(cwd) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Launch folder is not local and absolute",
        ));
    }
    let mut command = Command::new(executable);
    // An empty argument vector after -- bypasses inherited JUNA_SHELL_ARGS as well.
    command
        .arg("--shell")
        .arg(&shell.program)
        .arg("--")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_remove("JUNA_SHELL_ARGS");
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("JUNA_ACCEPT_") || name.starts_with("JUNA_STARTUP_") {
            command.env_remove(key);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command)
}
pub fn diagnostic_file(directory: &Path) -> io::Result<(PathBuf, std::fs::File)> {
    let folder = directory.join("logs");
    std::fs::create_dir_all(&folder)?;
    let path = folder.join(format!(
        "juna-{}-{}.log",
        std::process::id(),
        crate::settings::new_id()
    ));
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    Ok((path, file))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modifiers_launch_arguments_and_logs_preserve_independence() {
        let key = Key::Character("N".into());
        let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
        assert!(shortcut(&key, mods));
        for bad in [
            ModifiersState::CONTROL,
            mods | ModifiersState::ALT,
            mods | ModifiersState::SUPER,
        ] {
            assert!(!shortcut(&key, bad));
        }
        let root = std::env::temp_dir().join(crate::settings::new_id());
        std::fs::create_dir_all(&root).unwrap();
        let shell = SessionSpec {
            program: "configured shell.exe".into(),
            args: vec!["dangerous one-off".into()],
            cwd: None,
            env: vec![],
        };
        let executable = std::env::current_exe().unwrap();
        let command = command(&executable, &shell, Some("relative"), &root).unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--shell", "configured shell.exe", "--"]
        );
        assert_eq!(command.get_current_dir(), Some(root.as_path()));
        assert!(super::command(Path::new("relative.exe"), &shell, None, &root).is_err());
        let reported = root.join("reported folder 日本語");
        let command = super::command(&executable, &shell, reported.to_str(), &root).unwrap();
        assert_eq!(command.get_current_dir(), Some(reported.as_path()));
        let mut missing = super::command(&root.join("missing.exe"), &shell, None, &root).unwrap();
        assert!(missing.spawn().is_err());
        let (a, mut first) = diagnostic_file(&root).unwrap();
        use std::io::Write;
        first.write_all(b"first").unwrap();
        let (b, mut second) = diagnostic_file(&root).unwrap();
        second.write_all(b"second").unwrap();
        drop((first, second));
        assert_ne!(a, b);
        assert_eq!(std::fs::read(a).unwrap(), b"first");
        assert_eq!(std::fs::read(b).unwrap(), b"second");
        std::fs::remove_dir_all(root).unwrap();
    }
}
