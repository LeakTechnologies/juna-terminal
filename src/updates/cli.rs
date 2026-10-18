//! Command-line update actions reuse the same verified worker and installer as Settings.
use std::{
    ffi::OsString,
    io::{self, IsTerminal, Write},
    path::Path,
    time::Duration,
};
pub const HELP: &str = "Juna Terminal
Usage: juna [--shell PROGRAM] [--cwd DIRECTORY] [-- ARGS...]
       juna --version
       juna --check-update [--channel stable|preview]
       juna --update [--channel stable|preview]

--check-update  Check GitHub Releases without downloading or opening a window.
--update        Verify a download, then confirm installation and restart.
--channel       Override the saved update channel for this invocation only.
--help          Show this help.

Inside Juna, --update opens the hosting window's installation confirmation.
Outside Juna, close other Juna windows and answer the text confirmation.
PowerShell users can pipe a GUI executable to Out-Host to wait for its output.";
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Help,
    Check(Option<String>),
    Update(Option<String>),
}
pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Option<Action>, String> {
    let arguments: Vec<_> = arguments.into_iter().collect();
    let prefix: Vec<_> = arguments.iter().take_while(|arg| *arg != "--").collect();
    if !prefix.iter().any(|arg| {
        matches!(
            arg.to_str(),
            Some("--help" | "--check-update" | "--update" | "--channel")
        )
    }) {
        return Ok(None);
    }
    if arguments == [OsString::from("--help")] {
        return Ok(Some(Action::Help));
    }
    let mut update = None;
    let mut channel = None;
    let mut args = arguments.iter();
    while let Some(arg) = args.next() {
        if arg == "--update" || arg == "--check-update" {
            if update.replace(arg == "--update").is_some() {
                return Err("Choose exactly one update action".into());
            }
        } else if arg == "--channel" && channel.is_none() {
            channel = Some(
                args.next()
                    .and_then(|arg| arg.to_str())
                    .filter(|s| matches!(*s, "stable" | "preview"))
                    .ok_or("--channel requires stable or preview")?
                    .to_owned(),
            );
        } else {
            return Err("Invalid update arguments; use juna --help".into());
        }
    }
    match update {
        Some(true) => Ok(Some(Action::Update(channel))),
        Some(false) => Ok(Some(Action::Check(channel))),
        None => Err("--channel requires an update action".into()),
    }
}
fn wait(mut worker: super::Worker) -> Result<super::Outcome, String> {
    let mut shown = 0;
    loop {
        if let Some(result) = worker.poll() {
            return result;
        }
        let bytes = worker.progress.load(std::sync::atomic::Ordering::Relaxed);
        if bytes / (1024 * 1024) > shown {
            shown = bytes / (1024 * 1024);
            println!("Transferred {shown} MiB...");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub fn run(action: Action) -> Result<(), String> {
    if action == Action::Help {
        println!("{HELP}");
        return Ok(());
    }
    let install = matches!(&action, Action::Update(_));
    if install && super::helper::installation_state().is_some() {
        return Err("An update is already running or requires recovery".into());
    }
    let channel = match action {
        Action::Check(channel) | Action::Update(channel) => channel,
        Action::Help => unreachable!(),
    }
    .unwrap_or_else(|| {
        crate::settings::Store::new(crate::settings::application_directory())
            .load()
            .0
            .update_channel
    });
    if install && std::env::var_os("JUNA_PRIVACY_SERVER").is_some() {
        handoff(&channel)?;
        println!("Update request sent to this Juna window. Confirm Install and Restart there after verification.");
        return Ok(());
    }
    let target = std::env::current_exe().map_err(|_| "Installed executable unavailable")?;
    if install && installation_in_use(&target)? {
        return Err("Close other Juna windows before updating, or run juna --update inside the Juna window you want to restart".into());
    }
    println!(
        "Installed: v{} | Channel: {channel}",
        env!("CARGO_PKG_VERSION")
    );
    println!("Checking GitHub Releases...");
    let directory = crate::settings::application_directory();
    let outcome = wait(super::Worker::start(super::Task::Check {
        channel,
        current: env!("CARGO_PKG_VERSION").into(),
        directory: directory.clone(),
        discard: None,
    })?)?;
    let super::Outcome::Checked(candidate) = outcome else {
        return Err("Unexpected update result".into());
    };
    let Some(candidate) = candidate else {
        println!("No newer compatible release is available.");
        return Ok(());
    };
    println!("Available: v{}", candidate.version);
    if !install {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err(
            "Installation requires interactive confirmation. Run juna --update in a terminal."
                .into(),
        );
    }
    println!("Downloading and verifying...");
    let super::Outcome::Downloaded(stage) = wait(super::Worker::start(super::Task::Download {
        candidate,
        directory: directory.clone(),
        discard: None,
    })?)?
    else {
        return Err("Unexpected download result".into());
    };
    let result = (|| {
        print!(
            "Verified v{}. Install and restart Juna? [y/N] ",
            stage.version
        );
        io::stdout()
            .flush()
            .map_err(|_| "Cannot display update confirmation")?;
        let mut answer = String::new();
        // Read a bounded answer even if a console input provider produces an oversized line.
        use io::{BufRead, Read};
        io::stdin()
            .lock()
            .take(32)
            .read_line(&mut answer)
            .map_err(|_| "Cannot read update confirmation")?;
        if !confirmed(&answer) {
            println!("Update cancelled.");
            return Ok(false);
        }
        if installation_in_use(&target)? {
            return Err("Close other Juna windows and retry the update".into());
        }
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute() && path.is_dir())
            .ok_or("Restart home folder unavailable")?;
        let shell = crate::session::SessionSpec {
            program: std::env::var_os("JUNA_SHELL").unwrap_or_else(|| "powershell.exe".into()),
            ..Default::default()
        };
        let plan = super::helper::prepare(&stage, &target, &shell, &home, &directory)?;
        super::helper::launch(&plan)?;
        println!("Installer started. Juna will restart after replacement; failures are reported by the installer.");
        Ok(true)
    })();
    if !matches!(result, Ok(true)) {
        let _ = super::cleanup_stage(&stage.root, &directory);
    }
    result.map(|_| ())
}
fn confirmed(answer: &str) -> bool {
    answer.len() < 32 && matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}
#[cfg(windows)]
fn image_path(pid: u32) -> Result<std::path::PathBuf, String> {
    use windows::{
        core::PWSTR,
        Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };
    let mut path = vec![0u16; 32768];
    let mut length = path.len() as u32;
    // SAFETY: query-only handle is closed on either result; owned UTF-16 storage remains valid for the call.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|_| "Cannot verify Juna process")?;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(handle);
        result.map_err(|_| "Cannot verify Juna executable")?;
    }
    Ok(String::from_utf16(&path[..length as usize])
        .map_err(|_| "Invalid executable path")?
        .into())
}
#[cfg(windows)]
pub fn installation_in_use(target: &Path) -> Result<bool, String> {
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{HWND, LPARAM},
            UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, GetWindowThreadProcessId},
        },
    };
    struct Scan<'a> {
        target: &'a Path,
        busy: bool,
        failed: bool,
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
        // SAFETY: EnumWindows is synchronous; data points to its live, exclusively borrowed Scan.
        let scan = unsafe { &mut *(data.0 as *mut Scan<'_>) };
        let mut title = [0u16; 64];
        let mut pid = 0;
        // SAFETY: hwnd comes from EnumWindows; title and PID buffers remain live through these reads.
        unsafe {
            GetWindowTextW(hwnd, &mut title);
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        let length = title.iter().position(|c| *c == 0).unwrap_or(title.len());
        if pid != std::process::id()
            && crate::window_identity::recognized(&String::from_utf16_lossy(&title[..length]))
        {
            match image_path(pid).and_then(|path| {
                std::fs::canonicalize(path).map_err(|_| "Cannot verify Juna path".into())
            }) {
                Ok(path) => scan.busy |= path == scan.target,
                Err(_) => scan.failed = true,
            }
        }
        BOOL(1)
    }
    let target = std::fs::canonicalize(target).map_err(|_| "Installed executable unavailable")?;
    let mut scan = Scan {
        target: &target,
        busy: false,
        failed: false,
    };
    // SAFETY: callback borrows scan only during synchronous enumeration; no pointer escapes.
    unsafe { EnumWindows(Some(visit), LPARAM((&mut scan as *mut Scan<'_>) as isize)) }
        .map_err(|_| "Cannot check Juna windows")?;
    if scan.failed {
        return Err("Cannot verify another Juna window; close it and retry".into());
    }
    Ok(scan.busy)
}
#[cfg(not(windows))]
pub fn installation_in_use(_: &Path) -> Result<bool, String> {
    Err("Juna updates require Windows".into())
}
#[cfg(windows)]
fn handoff(channel: &str) -> Result<(), String> {
    use crate::privacy::wire::windows_pipe::client;
    use std::time::Instant;
    let unavailable = || {
        "Cannot contact the hosting Juna window. Use Settings > Updates in that window.".to_owned()
    };
    let path = std::env::var("JUNA_PRIVACY_PIPE").map_err(|_| unavailable())?;
    let token = std::env::var("JUNA_PRIVACY_TOKEN").map_err(|_| unavailable())?;
    let pid = std::env::var("JUNA_PRIVACY_SERVER")
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(unavailable)?;
    let exe = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|_| unavailable())?;
    if std::fs::canonicalize(image_path(pid)?).map_err(|_| unavailable())? != exe {
        return Err(unavailable());
    }
    let pipe = client(&path, pid).map_err(|_| unavailable())?;
    request(
        &pipe,
        &token,
        channel,
        Instant::now() + Duration::from_secs(3),
    )
    .map_err(|_| unavailable())
}
#[cfg(windows)]
fn request(
    pipe: &crate::privacy::wire::windows_pipe::Pipe,
    token: &str,
    channel: &str,
    deadline: std::time::Instant,
) -> Result<(), String> {
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid command capability".into());
    }
    let bytes =
        serde_json::to_vec(&serde_json::json!({"token":token,"action":"update","channel":channel}))
            .map_err(|_| "Cannot encode command")?;
    pipe.write_all(&(bytes.len() as u32).to_le_bytes(), None, deadline)?;
    pipe.write_all(&bytes, None, deadline)?;
    let mut prefix = [0; 4];
    pipe.read_exact(&mut prefix, None, deadline)?;
    if u32::from_le_bytes(prefix) != 2 {
        return Err("Command rejected".into());
    }
    let mut ack = [0; 2];
    pipe.read_exact(&mut ack, None, deadline)?;
    if &ack != b"[]" {
        return Err("Command rejected".into());
    }
    pipe.write_all(&[1], None, deadline)
}
#[cfg(not(windows))]
fn handoff(_: &str) -> Result<(), String> {
    Err("Juna updates require Windows".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &[&str]) -> Result<Option<Action>, String> {
        parse(s.iter().map(OsString::from))
    }
    #[test]
    fn actions_are_unambiguous_and_child_arguments_are_untouched() {
        assert_eq!(
            args(&["--check-update"]).unwrap(),
            Some(Action::Check(None))
        );
        assert_eq!(
            args(&["--channel", "preview", "--update"]).unwrap(),
            Some(Action::Update(Some("preview".into())))
        );
        for invalid in [
            vec!["--update", "--check-update"],
            vec!["--update", "--channel", "nightly"],
            vec!["--channel", "stable"],
            vec!["--update", "--shell", "cmd.exe"],
            vec!["--help", "--update"],
            vec!["--update", "--"],
        ] {
            assert!(args(&invalid).is_err());
        }
        assert_eq!(
            args(&["--shell", "cmd.exe", "--", "--update"]).unwrap(),
            None
        );
        assert_eq!(args(&["--version"]).unwrap(), None);
        assert_eq!(args(&["--help"]).unwrap(), Some(Action::Help));
        assert!(!confirmed(""));
        assert!(!confirmed("no"));
        assert!(!confirmed("yes please"));
        assert!(!confirmed(&format!("yes{}", " ".repeat(40))));
        assert!(confirmed(" Y\r\n"));
    }
    #[cfg(windows)]
    #[test]
    fn local_update_request_requires_authentication_and_preserves_the_channel() {
        let broker = crate::privacy::ipc::Broker::new().unwrap();
        let vars = broker.register(9).unwrap();
        let token = vars
            .iter()
            .find(|(k, _)| k == "JUNA_PRIVACY_TOKEN")
            .unwrap()
            .1
            .to_string_lossy();
        let pipe =
            crate::privacy::wire::windows_pipe::client(&broker.path, std::process::id()).unwrap();
        assert!(request(
            &pipe,
            &"0".repeat(64),
            "stable",
            std::time::Instant::now() + Duration::from_secs(3)
        )
        .is_err());
        assert!(broker.poll().is_none());
        drop(pipe);
        let pipe =
            crate::privacy::wire::windows_pipe::client(&broker.path, std::process::id()).unwrap();
        request(
            &pipe,
            &token,
            "stable",
            std::time::Instant::now() + Duration::from_secs(3),
        )
        .unwrap();
        let report = broker.poll().unwrap();
        assert_eq!(report.tab, 9);
        assert_eq!(report.update_channel.as_deref(), Some("stable"));
        assert!(report.fields.is_empty());
    }
}
