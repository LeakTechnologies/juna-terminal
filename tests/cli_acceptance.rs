#![cfg(windows)]
use std::process::Command;
fn executable() -> std::path::PathBuf {
    let path = std::env::var_os("JUNA_CLI_ACCEPTANCE_EXE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_juna").into());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        juna::release_image::version_strings(&bytes)
            .unwrap()
            .get("ProductVersion")
            .map(String::as_str),
        Some(env!("CARGO_PKG_VERSION"))
    );
    path
}
#[test]
fn help_and_invalid_commands_work_with_redirected_output_without_a_window() {
    let root = std::env::temp_dir().join(juna::settings::new_id());
    let output = Command::new(executable())
        .arg("--help")
        .env("LOCALAPPDATA", &root)
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("--check-update") && text.contains("--update") && text.contains("--channel")
    );
    assert!(!root.exists(), "help created application state");
    let output = Command::new(executable())
        .args(["--update", "--check-update"])
        .env("LOCALAPPDATA", &root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("exactly one"));
    assert!(
        !root.exists(),
        "invalid arguments created application state"
    );
}
#[test]
#[ignore = "Network; actual GitHub check via a window-free redirected command"]
fn check_reports_saved_and_temporary_channels_without_writing_preferences() {
    let root = std::env::temp_dir().join(juna::settings::new_id());
    let store = juna::settings::Store::new(root.join("Juna"));
    store
        .save_settings(&juna::settings::Settings {
            update_channel: "stable".into(),
            ..Default::default()
        })
        .unwrap();
    let before = std::fs::read(store.directory.join("settings.json")).unwrap();
    for (extra, channel) in [
        (vec![], "stable"),
        (vec!["--channel", "preview"], "preview"),
    ] {
        let output = Command::new(executable())
            .arg("--check-update")
            .args(extra)
            .env("LOCALAPPDATA", &root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(&format!("Channel: {channel}")));
        assert!(text.contains("No newer compatible") || text.contains("Available: v"));
        assert_eq!(
            std::fs::read(store.directory.join("settings.json")).unwrap(),
            before
        );
        assert!(!store.directory.join("logs").exists());
        assert!(!store.directory.join("updates").exists());
    }
    std::fs::remove_dir_all(root).unwrap();
}
