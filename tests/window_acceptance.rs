#![cfg(windows)]
#[test]
#[ignore = "interactive Windows desktop; concurrent tabs keep independent byte streams"]
fn inactive_tabs_drain_output_and_reap_without_closing_other_tabs() {
    use std::process::{Command, Stdio};
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/acceptance/tabs");
    std::fs::create_dir_all(&folder).unwrap();
    let mut child=Command::new(env!("CARGO_BIN_EXE_juna")).args(["--shell","cmd.exe","--","/Q","/C","(for /L %i in (1,1,300) do @echo TAB_ROW%i) & echo TAB_OUTPUT_END & ping -n 5 127.0.0.1 >nul & exit /b 0"])
        .env("LOCALAPPDATA",&folder).env("RUST_LOG","info").env("JUNA_STARTUP_TABS","3").env("JUNA_ACCEPT_MARKER","TAB_OUTPUT_END").stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(40) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("tabs did not reap");
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let log = std::fs::read_to_string(folder.join("Juna/juna.log")).unwrap();
    assert!(log.contains("count=3"), "{log}");
    assert_eq!(log.matches("marker present=true").count(), 3, "{log}");
    assert_eq!(log.matches("exited").count(), 3, "{log}");
}
// Opens the actual winit window and presents GPU frames; no UI automation or prompt matching.
#[test]
#[ignore = "requires an interactive Windows desktop; run explicitly"]
fn real_window_presents_then_closes_on_shell_exit() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    for (shell, args) in [
        (
            "cmd.exe",
            vec![
                "/Q",
                "/C",
                "title SHELL_TITLE & echo WINDOW_ACCEPTANCE & ping -n 3 127.0.0.1 >nul & exit /b 0",
            ],
        ),
        (
            "powershell.exe",
            vec![
                "-NoProfile",
                "-Command",
                "[Console]::Title = 'SHELL_TITLE'; [Console]::Out.Write('WINDOW_ACCEPTANCE'); Start-Sleep -Seconds 2; exit 7",
            ],
        ),
    ] {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/acceptance")
            .join(shell);
        std::fs::create_dir_all(&folder).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_juna"))
            .arg("--shell")
            .arg(shell)
            .arg("--")
            .args(args)
            .env("LOCALAPPDATA", &folder)
            .env("RUST_LOG", "info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("window did not close after {shell} exited");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let log = std::fs::read_to_string(folder.join("Juna/juna.log")).unwrap();
        assert!(log.contains("Shell exited:"), "{log}");
        assert!(log.contains("Taskbar relaunch icon configured"), "{log}");
        assert!(log.contains("Taskbar application ID: LeakTechnologies.Juna.Terminal"), "{log}");
        assert!(log.contains("Window icons verified: small and taskbar"), "{log}");
        assert!(
            log.contains("Window title: \"Juna Terminal\""),
            "Shell output overwrote the application title: {log}"
        );
        assert!(!log.contains("frames=0"), "No frame was presented: {log}");
        assert!(
            log.contains(if shell == "powershell.exe" {
                "ExitStatus(7)"
            } else {
                "ExitStatus(0)"
            }),
            "{log}"
        );
    }
}

#[test]
#[ignore = "interactive Windows desktop; sustained output with an open options overlay"]
fn options_overlays_keep_shell_output_flowing() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    for panel in ["themes", "settings", "fonts"] {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/acceptance")
            .join(format!("menu-{panel}"));
        std::fs::create_dir_all(&folder).unwrap();
        let mut child=Command::new(env!("CARGO_BIN_EXE_juna")).args(["--shell","cmd.exe","--","/Q","/C","(for /L %i in (1,1,300) do @echo ROW%i) & echo MENU_OUTPUT_END & ping -n 3 127.0.0.1 >nul"]).env("LOCALAPPDATA",&folder).env("RUST_LOG","info").env("JUNA_STARTUP_PANEL",panel).env("JUNA_ACCEPT_MARKER","MENU_OUTPUT_END").stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("menu shell did not exit");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let log = std::fs::read_to_string(folder.join("Juna/juna.log")).unwrap();
        assert!(log.contains("Options panel:"), "{log}");
        assert!(log.contains("Acceptance marker present=true"), "{log}");
        assert!(!log.contains("frames=0"), "{log}");
    }
}
