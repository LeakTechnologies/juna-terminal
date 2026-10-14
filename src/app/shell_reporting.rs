//! Opt-in prompt integration for default PowerShell sessions; no profile writes.
use crate::session::SessionSpec;
pub fn prepare(mut spec: SessionSpec, enabled: bool) -> SessionSpec {
    let name = std::path::Path::new(&spec.program)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if enabled && spec.args.is_empty() && matches!(name.as_str(), "powershell" | "pwsh") {
        const SCRIPT: &str = r#"$global:JunaOriginalPrompt=(Get-Command prompt -CommandType Function -ErrorAction SilentlyContinue).ScriptBlock; function global:prompt { $junaExit=$global:LASTEXITCODE; $junaText=if($global:JunaOriginalPrompt){ & $global:JunaOriginalPrompt }else{'PS> '}; try { $junaLocation=Get-Location; $junaUri=if($junaLocation.Provider.Name -eq 'FileSystem'){([Uri]$junaLocation.ProviderPath).AbsoluteUri}else{'file://unsupported/'}; [Console]::Write(([char]27)+']7;'+$junaUri+([char]7)) }catch{}; $global:LASTEXITCODE=$junaExit; $junaText }"#;
        spec.args = vec![
            "-NoLogo".into(),
            "-NoExit".into(),
            "-Command".into(),
            SCRIPT.into(),
        ];
    }
    spec
}
#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn prompt_preserves_style_exit_code_and_reports_unicode_directory_through_conpty() {
        use crate::session::{self, Dimensions, SessionSpec};
        let root =
            std::env::temp_dir().join(format!("Juna cwd 日本語 {}", crate::settings::new_id()));
        std::fs::create_dir_all(&root).unwrap();
        let program = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe")
            .into_os_string();
        let mut spec = super::prepare(
            SessionSpec {
                program,
                cwd: Some(root.clone()),
                ..Default::default()
            },
            true,
        );
        let script = spec.args.last().unwrap().to_string_lossy().into_owned();
        let bootstrap=format!("function global:prompt {{ 'JUNA_CUSTOM_PROMPT' }}; {script}; $global:LASTEXITCODE=37; $text=prompt; if($text -ne 'JUNA_CUSTOM_PROMPT' -or $global:LASTEXITCODE -ne 37){{exit 9}}; [Console]::Write('JUNA_PROMPT_OK'); exit 0");
        spec.args = vec!["-NoProfile".into(), "-Command".into(), bootstrap.into()];
        let child = session::spawn(
            &spec,
            Dimensions {
                cols: 100,
                rows: 20,
            },
        )
        .unwrap();
        let parts = child.split().unwrap();
        let mut control = parts.control;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            let mut reader = parts.reader;
            let mut buffer = [0; 4096];
            loop {
                let n = reader.read(&mut buffer).unwrap();
                if n == 0 {
                    break;
                }
                output.extend_from_slice(&buffer[..n]);
                assert!(output.len() < 1024 * 1024);
            }
            tx.send(output).unwrap();
        });
        let result = rx.recv_timeout(std::time::Duration::from_secs(15));
        if result.is_err() {
            let _ = control.kill();
        }
        let bytes = result.expect("prompt integration timed out");
        reader.join().unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("JUNA_PROMPT_OK"));
        let mut terminal = crate::vt::TerminalState::new(100, 20);
        terminal.advance(&bytes);
        let cwd = terminal
            .working_directory()
            .1
            .expect("ConPTY did not preserve OSC 7");
        assert_eq!(std::path::Path::new(cwd), root);
        drop(control);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn only_opted_in_default_powershell_is_wrapped() {
        let spec = crate::session::SessionSpec {
            program: "powershell.exe".into(),
            ..Default::default()
        };
        assert!(super::prepare(spec.clone(), false).args.is_empty());
        assert_eq!(super::prepare(spec.clone(), true).args[1], "-NoExit");
        let mut custom = spec;
        custom.args = vec!["-NoProfile".into()];
        assert_eq!(super::prepare(custom.clone(), true), custom);
    }
}
