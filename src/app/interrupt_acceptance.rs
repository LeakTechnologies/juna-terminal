//! Optional installed-tool acceptance; no prompts or model requests are submitted.
use super::*;
#[test]
#[ignore = "Installed OpenCode; isolated profile and controlled background-startup Ctrl+Z/Ctrl+C lifecycle"]
fn installed_opencode_startup_interrupts_preserve_the_shell() {
    let executable =
        std::env::var_os("JUNA_OPENCODE_ACCEPTANCE_EXE").expect("Set JUNA_OPENCODE_ACCEPTANCE_EXE");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/acceptance")
        .join(crate::settings::new_id());
    std::fs::create_dir_all(&root).unwrap();
    let mut spec = SessionSpec {
        program: "powershell.exe".into(),
        args: vec![
            "-NoProfile".into(),
            "-NoLogo".into(),
            "-NoExit".into(),
            "-Command".into(),
            "Import-Module PSReadLine -ErrorAction Stop; Set-PSReadLineOption -HistorySaveStyle SaveNothing; function global:prompt {'SHELL_READY> '}".into(),
        ],
        cwd: Some(root.clone()),
        ..Default::default()
    };
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("OPENCODE_")
        {
            spec.env.push((name, "".into()));
        }
    }
    let config = root.join("isolated.json");
    std::fs::write(&config, b"{}").unwrap();
    spec.env
        .push(("OPENCODE_CONFIG".into(), config.into_os_string()));
    spec.env.push((
        "OPENCODE_DB".into(),
        root.join("isolated.db").into_os_string(),
    ));
    for (name, folder) in [
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_STATE_HOME", "state"),
        ("USERPROFILE", "home"),
        ("HOME", "home"),
        ("LOCALAPPDATA", "local"),
        ("APPDATA", "roaming"),
        ("OPENCODE_CONFIG_DIR", "config"),
    ] {
        let path = root.join(folder);
        std::fs::create_dir_all(&path).unwrap();
        spec.env.push((name.into(), path.into_os_string()));
    }
    for name in [
        "OPENCODE_DISABLE_AUTOUPDATE",
        "OPENCODE_DISABLE_PROJECT_CONFIG",
        "OPENCODE_DISABLE_DEFAULT_PLUGINS",
        "OPENCODE_DISABLE_MODELS_FETCH",
        "OPENCODE_DISABLE_LSP_DOWNLOAD",
    ] {
        spec.env.push((name.into(), "true".into()));
    }
    spec.env.push((
        "OPENCODE_CONFIG_CONTENT".into(),
        r#"{"autoupdate":false}"#.into(),
    ));
    for (name, _) in std::env::vars_os() {
        let upper = name.to_string_lossy().to_ascii_uppercase();
        if upper.ends_with("_TOKEN") || upper.ends_with("_API_KEY") || upper.ends_with("_PASSWORD")
        {
            spec.env.push((name, "".into()));
        }
    }
    let mut runtime = Runtime::spawn(
        &spec,
        Dimensions {
            cols: 100,
            rows: 30,
        },
    )
    .unwrap();
    let mut terminal = TerminalState::new(100, 30);

    let entry = std::env::var_os("JUNA_OPENCODE_ACCEPTANCE_SCRIPT").unwrap_or(executable);
    let quoted = super::file_drop::text(
        std::path::Path::new(&entry),
        std::path::Path::new("powershell.exe"),
    )
    .unwrap();
    let command = format!("& {quoted}\r");
    let start = Instant::now();
    let mut action = 0;
    let mut bytes = 0;
    let mut launched = false;
    let mut input_started = None;
    let mut recovered = false;
    let mut observed = String::new();
    loop {
        let output = runtime.stream.take();
        bytes += output.len();
        terminal.advance(&output);
        let responses = terminal.take_responses();
        if !responses.is_empty() && !runtime.is_closing() {
            runtime.write(&responses).unwrap();
        }
        observed.push_str(&String::from_utf8_lossy(&output));
        if observed.len() > 1024 * 1024 {
            observed.drain(..observed.floor_char_boundary(observed.len() - 512 * 1024));
        }
        if runtime.tick().unwrap() {
            break;
        }
        if !launched && observed.contains("SHELL_READY") {
            runtime.write(command.as_bytes()).unwrap();
            launched = true;
            input_started = Some(Instant::now());
            observed.clear();
        }
        if launched && input_started.is_none() && terminal.alternate_active() {
            input_started = Some(Instant::now());
        }
        if let Some(ready) = input_started {
            let elapsed = ready.elapsed();
            let scheduled = [(5, 26), (7, 3), (9, 3)];
            if action < scheduled.len()
                && elapsed >= Duration::from_secs(scheduled[action].0)
                && !runtime.is_closing()
            {
                runtime.write(&[scheduled[action].1]).unwrap();
                action += 1;
            }
            if action == 3 && elapsed >= Duration::from_secs(11) {
                runtime
                    .write(b"[Console]::WriteLine(('OPENCODE_'+'RETURN_OK'))\r")
                    .unwrap();
                action += 1;
            }
        }
        if action == 4 && observed.contains("OPENCODE_RETURN_OK") {
            assert!(!terminal.alternate_active());
            recovered = true;
            runtime.write(b"exit 0\r").unwrap();
            action += 1;
        }
        assert!(
            start.elapsed() < Duration::from_secs(45),
            "OpenCode did not return to its shell after controlled interrupts (launched={launched}, actions={action}, alternate={})",terminal.alternate_active()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    std::fs::write(root.join("lifecycle.txt"),format!("control_actions={action}; output_bytes={bytes}; shell_recovered={recovered}; shell_exit={:?}; transport_faults=0\n",runtime.exit)).unwrap();
    assert!(recovered, "OpenCode did not preserve the interactive shell");
    assert_eq!(runtime.exit.unwrap().code(), 0);
}
