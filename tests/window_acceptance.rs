#![cfg(windows)]
#[test]
#[ignore = "Windows desktop/network; authenticated CLI update handoff preserves the host and saved channel"]
fn command_line_update_handoff_preserves_the_host_and_preferences() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let root = std::env::temp_dir().join(juna::settings::new_id());
    let store = juna::settings::Store::new(root.join("Juna"));
    store
        .save_settings(&juna::settings::Settings {
            update_channel: "preview".into(),
            ..Default::default()
        })
        .unwrap();
    let before = std::fs::read(store.directory.join("settings.json")).unwrap();
    let shell = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let script = r#"$i=New-Object Diagnostics.ProcessStartInfo; $i.FileName=(Get-Command juna -ErrorAction Stop).Source; $i.Arguments='--update --channel stable'; $i.UseShellExecute=$false; $i.RedirectStandardOutput=$true; $i.RedirectStandardError=$true; $i.EnvironmentVariables.Remove('JUNA_PRIVACY_SERVER'); $p=[Diagnostics.Process]::Start($i); $o=$p.StandardOutput.ReadToEnd(); $e=$p.StandardError.ReadToEnd(); $p.WaitForExit(); if($p.ExitCode -ne 1 -or $e -notlike '*Close other Juna windows*'){exit 30}; $i.EnvironmentVariables['JUNA_PRIVACY_SERVER']=$env:JUNA_PRIVACY_SERVER; $p=[Diagnostics.Process]::Start($i); $o=$p.StandardOutput.ReadToEnd(); $e=$p.StandardError.ReadToEnd(); $p.WaitForExit(); if($p.ExitCode -ne 0 -or $o -notlike '*request sent*'){exit 31}; [IO.File]::WriteAllText($env:JUNA_CLI_HANDOFF_FILE,'CLI_HANDOFF_OK'); Write-Output 'CLI_HANDOFF_OK'; Start-Sleep -Seconds 4; exit 0"#;
    let executable = std::env::var_os("JUNA_CLI_ACCEPTANCE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_juna").into());
    let marker = root.join("handoff-result.txt");
    let mut child = Command::new(executable)
        .arg("--shell")
        .arg(shell)
        .args(["--", "-NoProfile", "-Command", script])
        .env("LOCALAPPDATA", &root)
        .env("RUST_LOG", "info")
        .env("JUNA_ACCEPT_MARKER", "CLI_HANDOFF_OK")
        .env("JUNA_CLI_HANDOFF_FILE", &marker)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("CLI handoff window did not exit");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let log = read_log(&root);
    assert!(
        log.contains("Authenticated command-line update request accepted; channel=stable"),
        "{log}"
    );
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "CLI_HANDOFF_OK");
    assert!(log.contains("ExitStatus(0)"), "{log}");
    assert_eq!(
        std::fs::read(store.directory.join("settings.json")).unwrap(),
        before
    );
    std::fs::remove_dir_all(root).unwrap();
}
fn read_log(folder: &std::path::Path) -> String {
    let mut logs = std::fs::read_dir(folder.join("Juna/logs"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
        .collect::<Vec<_>>();
    logs.sort_by_key(|path| std::fs::metadata(path).unwrap().modified().unwrap());
    std::fs::read_to_string(logs.last().expect("No window log")).unwrap()
}

#[test]
#[ignore = "interactive Windows desktop; portable startup home and explicit folder selection"]
fn portable_interactive_launch_starts_at_home_and_preserves_overrides() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/acceptance/start-directory")
        .join(juna::settings::new_id());
    let portable = root.join("portable folder");
    let home = root.join("home 日本語");
    let explicit = root.join("chosen folder");
    for directory in [&portable, &home, &explicit] {
        std::fs::create_dir_all(directory).unwrap();
    }
    for (index, expected) in [&home, &explicit, &portable].iter().enumerate() {
        let cache = root.join(format!("case-{index}"));
        let script = "$actual=(Get-Location).ProviderPath; [IO.File]::WriteAllText($env:JUNA_ACCEPT_OBSERVED_FILE,$actual,[Text.UTF8Encoding]::new($false)); if([IO.Path]::GetFullPath($actual) -ne [IO.Path]::GetFullPath($env:JUNA_ACCEPT_EXPECTED_DIRECTORY)){exit 9}; Write-Output 'START_DIRECTORY_OK'; Start-Sleep -Seconds 1; exit 0";
        let mut command = Command::new(env!("CARGO_BIN_EXE_juna"));
        command
            .args([
                "--shell",
                if index == 2 {
                    "powershell.exe"
                } else {
                    "cmd.exe"
                },
            ])
            .current_dir(&portable)
            .env("USERPROFILE", &home)
            .env("JUNA_ACCEPT_EXPECTED_DIRECTORY", expected)
            .env("JUNA_ACCEPT_OBSERVED_FILE", cache.join("observed.txt"))
            .env("LOCALAPPDATA", &cache)
            .env_remove("JUNA_SHELL_ARGS")
            .env("RUST_LOG", "info")
            .env("JUNA_ACCEPT_MARKER", "START_DIRECTORY_OK")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if index == 1 {
            command.arg("--cwd").arg(&explicit);
        }
        if index == 2 {
            command.args(["--", "-NoProfile", "-Command", script]);
        } else {
            command.env(
                "JUNA_ACCEPT_WINDOW_COMMAND",
                format!("powershell.exe -NoProfile -Command \"{script}\"\rexit /b\r"),
            );
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("Startup directory window timed out");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let log = read_log(&cache);
        let observed = std::fs::read_to_string(cache.join("observed.txt")).unwrap();
        assert_eq!(
            std::fs::canonicalize(observed).unwrap(),
            std::fs::canonicalize(expected).unwrap()
        );
        assert!(log.contains("ExitStatus(0)"), "{log}");
        assert!(log.contains("Acceptance marker present=true"), "{log}");
        assert!(!log.contains("frames=0"), "{log}");
    }
}

#[test]
#[ignore = "interactive Windows desktop; independent process windows and preferences"]
fn independent_new_window_preserves_cwd_preferences_and_parent_session() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/acceptance/new-window")
        .join(juna::settings::new_id());
    let reported = folder.join("reported folder 日本語");
    std::fs::create_dir_all(&reported).unwrap();
    juna::settings::Store::new(folder.join("Juna"))
        .save_settings(&juna::settings::Settings {
            font_family: "Fira Code".into(),
            font_size: 18.,
            ..Default::default()
        })
        .unwrap();
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_juna"));
    let parent_script = "echo PARENT_WINDOW_OK & ping -n 20 127.0.0.1 >nul & exit /b 0";
    let mut parent = Command::new(executable)
        .args(["--shell", "cmd.exe", "--", "/Q", "/C", parent_script])
        .current_dir(&folder)
        .env("LOCALAPPDATA", &folder)
        .env("RUST_LOG", "info")
        .env("JUNA_ACCEPT_MARKER", "PARENT_WINDOW_OK")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let shell = juna::session::SessionSpec {
        program: "cmd.exe".into(),
        args: vec!["/C".into(), "echo REPLAYED_ONE_OFF & exit /b 9".into()],
        cwd: None,
        env: vec![],
    };
    let mut command =
        juna::app::new_window::command(executable, &shell, reported.to_str(), &folder).unwrap();
    command
        .env("LOCALAPPDATA", &folder)
        .env("RUST_LOG", "info")
        .env("JUNA_SHELL_ARGS", "/C exit /b 9")
        .env("JUNA_ACCEPT_WINDOW_CWD", &reported)
        .env("JUNA_ACCEPT_WINDOW_FONT", "Fira Code")
        .env("JUNA_ACCEPT_WINDOW_SIZE", "18")
        .env("JUNA_ACCEPT_MARKER", "CHILD_WINDOW_OK")
        .env(
            "JUNA_ACCEPT_WINDOW_COMMAND",
            "echo CHILD_WINDOW_OK & ping -n 3 127.0.0.1 >nul & exit /b 0\r",
        );
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = parent.kill();
            panic!("New window did not close");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        parent.try_wait().unwrap().is_none(),
        "Closing new window closed the original"
    );
    loop {
        if let Some(status) = parent.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() > deadline {
            let _ = parent.kill();
            panic!("Original window did not finish");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let logs = std::fs::read_dir(folder.join("Juna/logs"))
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(logs.len(), 2);
    let child = logs
        .iter()
        .find(|log| log.contains("Window acceptance"))
        .unwrap();
    assert!(
        child.contains("Window acceptance cwd=true preferences=true"),
        "{child}"
    );
    for log in &logs {
        assert!(log.contains("Acceptance marker present=true"), "{log}");
        assert!(log.contains("ExitStatus(0)"), "{log}");
        assert!(!log.contains("frames=0"), "{log}");
    }
}
#[test]
#[ignore = "interactive Windows desktop; local Git footer and alternate-screen refit"]
fn git_status_tracks_reported_folder_and_hides_in_alternate_screen() {
    use std::process::{Command, Stdio};
    let folder =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/acceptance/git-window");
    let repo = folder.join("repository");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(Command::new("git")
        .current_dir(&repo)
        .args(["init", "-b", "main"])
        .output()
        .unwrap()
        .status
        .success());
    let shell = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let script="[Console]::Write(([char]27)+']7;'+([Uri](Get-Location).ProviderPath).AbsoluteUri+([char]7)); Write-Output 'GIT_WINDOW_OK'; Start-Sleep -Seconds 4; [Console]::Write(([char]27)+'[?1049h'); Start-Sleep -Seconds 1; [Console]::Write(([char]27)+'[?1049l'); Start-Sleep -Seconds 1; exit 0";
    let mut child = Command::new(env!("CARGO_BIN_EXE_juna"))
        .current_dir(&repo)
        .arg("--shell")
        .arg(shell)
        .args(["--", "-NoProfile", "-Command", script])
        .env("LOCALAPPDATA", &folder)
        .env("RUST_LOG", "info")
        .env("JUNA_ACCEPT_GIT_STATUS", "1")
        .env("JUNA_ACCEPT_MARKER", "GIT_WINDOW_OK")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(40) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Git window did not exit");
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let log = read_log(&folder);
    assert!(log.contains("Git acceptance repository=true"), "{log}");
    assert!(log.contains("Git acceptance footer=true"), "{log}");
    assert!(log.contains("Git acceptance footer=false"), "{log}");
    assert!(log.contains("Acceptance marker present=true"), "{log}");
    assert!(!log.contains("frames=0"), "{log}");
}
#[test]
#[ignore = "interactive Windows desktop; bundled tool startup and presentation"]
fn bundled_fetch_runs_in_a_real_window() {
    use std::process::{Command, Stdio};
    let folder =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/acceptance/bundled-fetch");
    std::fs::create_dir_all(&folder).unwrap();
    let system = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("System32");
    let shell = system.join("WindowsPowerShell/v1.0/powershell.exe");
    let script = "$a=junafetch --brief --json | ConvertFrom-Json; if ($LASTEXITCODE -ne 0 -or $a.fields.Count -ne 8) { exit 21 }; fetch; if ($LASTEXITCODE -ne 0) { exit 22 }; Write-Output 'BUNDLED_FETCH_OK'; Start-Sleep -Seconds 2";
    let mut child = Command::new(env!("CARGO_BIN_EXE_juna"))
        .arg("--shell")
        .arg(shell)
        .args(["--", "-NoProfile", "-Command", script])
        .env("PATH", &system)
        .env("LOCALAPPDATA", &folder)
        .env("RUST_LOG", "info")
        .env("JUNA_ACCEPT_MARKER", "BUNDLED_FETCH_OK")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(45) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("bundled fetch window did not exit");
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let log = read_log(&folder);
    assert!(log.contains("Acceptance marker present=true"), "{log}");
    assert!(log.contains("ExitStatus(0)"), "{log}");
    assert!(!log.contains("frames=0"), "{log}");
}
#[test]
#[ignore = "interactive Windows desktop; concurrent tabs keep independent byte streams"]
fn inactive_tabs_drain_output_and_reap_without_closing_other_tabs() {
    use std::process::{Command, Stdio};
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/acceptance/tabs");
    std::fs::create_dir_all(&folder).unwrap();
    juna::settings::Store::new(folder.join("Juna"))
        .save_settings(&juna::settings::Settings {
            show_line_numbers: true,
            ..Default::default()
        })
        .unwrap();
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
    let log = read_log(&folder);
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
        let log = read_log(&folder);
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
        let log = read_log(&folder);
        assert!(log.contains("Options panel:"), "{log}");
        assert!(log.contains("Acceptance marker present=true"), "{log}");
        assert!(!log.contains("frames=0"), "{log}");
    }
}
