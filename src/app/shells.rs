//! Shell preference resolution: a fixed allowlist of known executables.
//!
//! Settings may only name one of `SHELL_CHOICES`. The program path is resolved
//! here, so an imported settings file can never inject an arbitrary executable.

use crate::session::SessionSpec;
use std::path::Path;

pub fn display(choice: &str) -> &'static str {
    match choice {
        "pwsh" => "PowerShell 7",
        "cmd" => "Command Prompt",
        "bash" => "Bash",
        _ => "Windows PowerShell",
    }
}

pub fn program(choice: &str) -> &'static str {
    match choice {
        "pwsh" => "pwsh.exe",
        "cmd" => "cmd.exe",
        "bash" => "bash.exe",
        _ => "powershell.exe",
    }
}

/// Replace the launch program when the base spec carries no explicit arguments.
/// A user-supplied shell argument vector belongs to that shell and is left alone.
pub fn apply(spec: &mut SessionSpec, choice: &str) {
    if spec.args.is_empty() && crate::settings::valid_shell(choice) {
        spec.program = program(choice).into();
    }
}

/// Whether the executable can be found on `PATH`. Used by the settings panel so a
/// missing shell is reported there rather than failing when a tab is opened.
pub fn available(choice: &str) -> bool {
    let name = program(choice);
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| !directory.as_os_str().is_empty() && Path::new(&directory).join(name).is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_map_to_known_programs_and_defaults_fall_back_to_powershell() {
        assert_eq!(program("cmd"), "cmd.exe");
        assert_eq!(program("pwsh"), "pwsh.exe");
        assert_eq!(program("bash"), "bash.exe");
        assert_eq!(program("powershell"), "powershell.exe");
        assert_eq!(program("unexpected"), "powershell.exe");
        assert_eq!(display("cmd"), "Command Prompt");
    }

    #[test]
    fn explicit_arguments_are_not_replaced() {
        let mut spec = SessionSpec {
            program: "custom.exe".into(),
            args: vec!["-NoProfile".into()],
            ..Default::default()
        };
        apply(&mut spec, "cmd");
        assert_eq!(spec.program, "custom.exe");
        spec.args.clear();
        apply(&mut spec, "cmd");
        assert_eq!(spec.program, "cmd.exe");
    }
}
