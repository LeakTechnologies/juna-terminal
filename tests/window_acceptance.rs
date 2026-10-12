#![cfg(windows)]
//! Opens the actual winit window and presents GPU frames; no UI automation or prompt matching.
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
                "echo WINDOW_ACCEPTANCE & ping -n 3 127.0.0.1 >nul & exit /b 0",
            ],
        ),
        (
            "powershell.exe",
            vec![
                "-NoProfile",
                "-Command",
                "[Console]::Out.Write('WINDOW_ACCEPTANCE'); Start-Sleep -Seconds 2; exit 7",
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
