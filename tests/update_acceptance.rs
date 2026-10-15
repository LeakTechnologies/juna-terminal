#![cfg(windows)]
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[test]
fn native_update_identity_rejects_other_versions_and_architectures() {
    let bytes = std::fs::read(env!("CARGO_BIN_EXE_juna")).unwrap();
    juna::release_image::verify_identity(
        &bytes,
        env!("CARGO_PKG_VERSION"),
        env!("JUNA_BUILD_COMMIT"),
    )
    .unwrap();
    assert!(
        juna::release_image::verify_identity(&bytes, "0.0.0", env!("JUNA_BUILD_COMMIT")).is_err()
    );
    let mut wrong = bytes.clone();
    let pe = u32::from_le_bytes(wrong[60..64].try_into().unwrap()) as usize;
    wrong[pe + 4..pe + 6].copy_from_slice(&0x14cu16.to_le_bytes());
    assert!(juna::release_image::verify_identity(
        &wrong,
        env!("CARGO_PKG_VERSION"),
        env!("JUNA_BUILD_COMMIT")
    )
    .is_err());
}
fn old_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/downloads/dev11/unpacked")
}
fn rehearsal_executable() -> PathBuf {
    let path = std::env::var_os("JUNA_UPDATE_REHEARSAL_EXE")
        .map(PathBuf::from)
        .expect("Set JUNA_UPDATE_REHEARSAL_EXE to this source's release build");
    let bytes = std::fs::read(&path).unwrap();
    juna::release_image::verify_identity(
        &bytes,
        env!("CARGO_PKG_VERSION"),
        env!("JUNA_BUILD_COMMIT"),
    )
    .unwrap();
    path
}
fn stage(root: &Path) -> juna::updates::Staged {
    let folder = root.join("Juna/updates").join(juna::settings::new_id());
    std::fs::create_dir_all(folder.join("package/notices")).unwrap();
    let source = std::fs::read(rehearsal_executable()).unwrap();
    std::fs::write(folder.join("package/juna.exe"), &source).unwrap();
    std::fs::write(
        folder.join("package/notices/LICENSE.txt"),
        "New notices fixture",
    )
    .unwrap();
    std::fs::write(
        folder.join("package/RELEASE_NOTES.md"),
        "New release fixture",
    )
    .unwrap();
    juna::updates::Staged {
        version: env!("CARGO_PKG_VERSION").into(),
        commit: env!("JUNA_BUILD_COMMIT").into(),
        exe_hash: juna::updates::hash(&source).unwrap(),
        root: folder,
    }
}
#[test]
#[ignore = "Windows desktop; real locked installation, hidden helper, restart acknowledgement"]
fn locked_old_version_updates_restarts_and_preserves_user_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/acceptance/update-rehearsal")
        .join(juna::settings::new_id());
    let installation = root.join("portable");
    let cache = root.join("cache");
    let home = root.join("home");
    std::fs::create_dir_all(installation.join("notices")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let preferences = cache.join("Juna/settings.json");
    let custom_theme = cache.join("Juna/themes/Custom/update-fixture.json");
    std::fs::create_dir_all(custom_theme.parent().unwrap()).unwrap();
    let settings = juna::settings::Settings {
        font_family: "Fira Code".into(),
        font_size: 18.,
        ..Default::default()
    };
    let preference_bytes = serde_json::to_vec(&settings).unwrap();
    std::fs::write(&preferences, &preference_bytes).unwrap();
    let mut theme = juna::theme::default_theme();
    theme.id = "custom-update-fixture".into();
    theme.name = "Update fixture".into();
    theme.source = None;
    let theme_bytes = serde_json::to_vec(&theme).unwrap();
    std::fs::write(&custom_theme, &theme_bytes).unwrap();
    let target = installation.join("juna.exe");
    std::fs::copy(old_fixture().join("juna.exe"), &target)
        .expect("Download and verify dev11 fixture first");
    std::fs::write(installation.join("notices/old.txt"), "Previous notices").unwrap();
    std::fs::write(installation.join("RELEASE_NOTES.md"), "Previous release").unwrap();
    std::fs::write(installation.join("user-file.txt"), "untouched").unwrap();
    let shell = PathBuf::from(std::env::var_os("WINDIR").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut parent = Command::new(&target)
        .arg("--shell")
        .arg(&shell)
        .args([
            "--",
            "-NoProfile",
            "-Command",
            "Write-Output 'OLD_UPDATE_FIXTURE'; Start-Sleep -Seconds 10; exit 0",
        ])
        .current_dir(&installation)
        .env("LOCALAPPDATA", &cache)
        .env("USERPROFILE", &home)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let staged = stage(&cache);
    let spec = juna::session::SessionSpec {
        program: shell.into_os_string(),
        ..Default::default()
    };
    let plan = juna::updates::helper::prepare_from(
        &staged,
        &target,
        &spec,
        &home,
        &cache.join("Juna"),
        &rehearsal_executable(),
        parent.id(),
    )
    .unwrap();
    let mut helper = Command::new(staged.root.join("helper.exe"))
        .arg("--update-helper")
        .arg(&plan)
        .env("LOCALAPPDATA", &cache)
        .env("USERPROFILE", &home)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(70);
    loop {
        if let Some(status) = helper.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() > deadline {
            let _ = helper.kill();
            let _ = parent.kill();
            panic!("Updater helper timed out");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(parent.wait().unwrap().success());
    let bytes = std::fs::read(&target).unwrap();
    juna::release_image::verify_identity(
        &bytes,
        env!("CARGO_PKG_VERSION"),
        env!("JUNA_BUILD_COMMIT"),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(installation.join("user-file.txt")).unwrap(),
        "untouched"
    );
    assert_eq!(std::fs::read(preferences).unwrap(), preference_bytes);
    assert_eq!(std::fs::read(custom_theme).unwrap(), theme_bytes);
    assert_eq!(
        std::fs::read_to_string(installation.join("notices/LICENSE.txt")).unwrap(),
        "New notices fixture"
    );
    assert!(!installation.join("notices/old.txt").exists());
    assert_eq!(
        std::fs::read_to_string(staged.root.join("restart.ready")).unwrap(),
        format!(
            "{}\n{}",
            env!("CARGO_PKG_VERSION"),
            env!("JUNA_BUILD_COMMIT")
        )
    );
    let pid = std::fs::read_to_string(staged.root.join("restart.pid"))
        .unwrap()
        .parse::<u32>()
        .unwrap();
    close_window(pid);
}
fn close_window(pid: u32) {
    use windows::core::BOOL;
    use windows::Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
        UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
    };
    unsafe extern "system" fn visit(window: HWND, argument: LPARAM) -> BOOL {
        let mut owner = 0u32;
        // SAFETY: owner is writable/live for the synchronous PID query.
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut owner));
        }
        if owner == argument.0 as u32 {
            // SAFETY: posts a close request only to windows owned by our recorded restarted fixture PID; no pointer payload is supplied.
            unsafe {
                let _ = PostMessageW(
                    Some(window),
                    WM_CLOSE,
                    windows::Win32::Foundation::WPARAM(0),
                    LPARAM(0),
                );
            }
        }
        BOOL(1)
    }
    // SAFETY: enumeration is synchronous; the callback treats LPARAM as a numeric PID and retains no window pointer.
    unsafe {
        // SAFETY: holds a synchronization handle for our restarted fixture PID until its windows have closed and the process exits.
        let process = OpenProcess(PROCESS_SYNCHRONIZE, false, pid).unwrap();
        EnumWindows(Some(visit), LPARAM(pid as isize)).unwrap();
        let status = WaitForSingleObject(process, 15000);
        let _ = CloseHandle(process);
        assert_eq!(
            status, WAIT_OBJECT_0,
            "Restarted fixture did not finish closing"
        );
    }
}
#[test]
#[ignore = "Network; actual GitHub release metadata, streaming and verified ZIP staging"]
fn published_release_download_is_verified_without_installed_tools() {
    let root = std::env::temp_dir().join(juna::settings::new_id());
    let mut worker = juna::updates::Worker::start(juna::updates::Task::Check {
        channel: "preview".into(),
        current: "0.1.1-dev10".into(),
        directory: root.clone(),
        discard: None,
    })
    .unwrap();
    let outcome = wait(&mut worker);
    let juna::updates::Outcome::Checked(Some(candidate)) = outcome else {
        panic!("No newer published release");
    };
    let mut worker = juna::updates::Worker::start(juna::updates::Task::Download {
        candidate,
        directory: root.clone(),
        discard: None,
    })
    .unwrap();
    let juna::updates::Outcome::Downloaded(staged) = wait(&mut worker) else {
        panic!("No staged release");
    };
    let data = std::fs::read(staged.root.join("package/juna.exe")).unwrap();
    assert_eq!(juna::updates::hash(&data).unwrap(), staged.exe_hash);
    juna::release_image::verify_identity(&data, &staged.version, &staged.commit).unwrap();
    juna::updates::cleanup_stage(&staged.root, &root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
fn wait(worker: &mut juna::updates::Worker) -> juna::updates::Outcome {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(result) = worker.poll() {
            return result.unwrap();
        }
        if Instant::now() > deadline {
            worker
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
            panic!("Update worker timed out");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
