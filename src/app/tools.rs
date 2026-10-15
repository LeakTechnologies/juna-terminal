//! App-owned bundled utility setup; no process-wide environment or terminal-state changes.
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
};
const FRONTEND: &[u8] = include_bytes!("../../assets/tools/junafetch-front.bin");
fn hash(bytes: &[u8]) -> Result<[u8; 32], String> {
    use windows::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};
    let mut result = [0; 32];
    // SAFETY: the Windows SHA-256 pseudo-handle is valid; slices live for the call and output is exactly 32 bytes.
    unsafe {
        BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, bytes, &mut result)
            .ok()
            .map_err(|_| "Bundled tool verification failed")?;
    }
    Ok(result)
}
fn install(path: &Path, payload: &[u8]) -> Result<(), String> {
    let valid = (|| {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .ok()?
            .take(payload.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes.len() == payload.len() && hash(&bytes).ok()? == hash(payload).ok()?)
    })()
    .unwrap_or(false);
    if valid {
        return Ok(());
    }
    let parent = path.parent().ok_or("Missing tool directory")?;
    std::fs::create_dir_all(parent).map_err(|_| "Cannot create the user tools directory")?;
    let temp = parent.join(format!(".{}.tmp", crate::settings::new_id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|_| "Cannot stage bundled tool")?;
        file.write_all(payload)
            .map_err(|_| "Cannot write bundled tool")?;
        file.sync_all().map_err(|_| "Cannot flush bundled tool")?;
        drop(file);
        std::fs::rename(&temp, path).map_err(|_| "Cannot replace bundled tool")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
pub fn prepare(directory: &Path, shell: &OsString) -> Result<Vec<(OsString, OsString)>, String> {
    let folder = directory.join("tools").join(env!("CARGO_PKG_VERSION"));
    install(&folder.join("junafetch.exe"), FRONTEND)?;
    install(&folder.join("fetch.exe"), FRONTEND)?;
    let mut paths = vec![folder];
    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        paths.push(directory);
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    let path = std::env::join_paths(paths).map_err(|_| "Cannot build the session tools PATH")?;
    let name = PathBuf::from(shell)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let label = match name.as_str() {
        "powershell" => "Windows PowerShell",
        "pwsh" => "PowerShell",
        "cmd" => "Command Prompt",
        "bash" => "Bash",
        "zsh" => "Zsh",
        "fish" => "Fish",
        "nu" => "Nushell",
        _ => "Custom launch",
    };
    Ok(vec![
        ("PATH".into(), path),
        ("JUNA_VERSION".into(), env!("CARGO_PKG_VERSION").into()),
        ("JUNA_SHELL_LABEL".into(), label.into()),
    ])
}
#[cfg(test)]
mod tests {
    #[test]
    fn conpty_preserves_truecolour_marker_cells() {
        use crate::session::{self, Dimensions, SessionSpec};
        let program = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let script = r#"$e=[char]27; [Console]::Write("${e}]777;JunaPrivate;0123456789abcdef0123456789abcdef${e}\░░░░░${e}]777;JunaPrivate;0${e}\`r`n${e}[38;2;17;34;51m${e}[48;2;68;85;102m░░░░░${e}[0m"); Start-Sleep -Seconds 1"#;
        let spec = SessionSpec {
            program: program.into_os_string(),
            args: vec!["-NoProfile".into(), "-Command".into(), script.into()],
            ..Default::default()
        };
        let mut child = session::spawn(&spec, Dimensions { cols: 80, rows: 10 }).unwrap();
        let mut terminal = crate::vt::TerminalState::new(80, 10);
        let mut buffer = [0; 4096];
        loop {
            let count = child.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            terminal.advance(&buffer[..count]);
        }
        let cells = terminal.snapshot(0).cells;
        let rgb = cells
            .iter()
            .filter(|cell| {
                cell.content.first_scalar() == Some('░' as u32)
                    && cell.fg == crate::vt::Color::Rgb(17, 34, 51)
                    && cell.bg == crate::vt::Color::Rgb(68, 85, 102)
            })
            .count();
        let osc = cells
            .iter()
            .filter(|cell| {
                cell.content.first_scalar() == Some('░' as u32)
                    && terminal.private_token(cell.private_tag)
                        == Some("0123456789abcdef0123456789abcdef")
            })
            .count();
        eprintln!("Synthetic shaded cells retaining OSC metadata: {osc}; retaining truecolour attributes: {rgb}");
        assert_eq!(
            rgb, 5,
            "ConPTY did not preserve synthetic truecolour cell attributes"
        );
    }
    #[test]
    fn both_commands_work_through_conpty_without_installed_fastfetch() {
        use crate::session::{self, Dimensions, SessionSpec};
        let root = std::env::temp_dir().join(format!(
            "Juna tool cache 日本語 {}",
            crate::settings::new_id()
        ));
        let directory = root.join("Juna");
        crate::settings::Store::new(directory.clone())
            .save_settings(&crate::settings::Settings {
                junafetch_profile: "brief".into(),
                junafetch_logo: "none".into(),
                ..Default::default()
            })
            .unwrap();
        let windows = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap();
        let program = windows
            .join("System32/WindowsPowerShell/v1.0/powershell.exe")
            .into_os_string();
        let mut env = super::prepare(&directory, &program).unwrap();
        let path = std::env::join_paths([
            directory.join("tools").join(env!("CARGO_PKG_VERSION")),
            windows.join("System32"),
        ])
        .unwrap();
        env.iter_mut().find(|(key, _)| key == "PATH").unwrap().1 = path;
        env.push(("LOCALAPPDATA".into(), root.clone().into_os_string()));
        let broker = crate::privacy::ipc::Broker::new().unwrap();
        env.extend(broker.register(1).unwrap());
        let script = r#"$a=junafetch --json|ConvertFrom-Json; if($LASTEXITCODE -ne 0 -or $a.profile -ne 'brief' -or $a.fields.Count -ne 8){exit 21}; $b=fetch --profile standard --json|ConvertFrom-Json; if($LASTEXITCODE -ne 0 -or $b.profile -ne 'standard' -or $b.fields.Count -le 8 -or @($b.fields|Where-Object {$_.name -eq 'Local IP' -and ($_.value -eq 'Unavailable' -or $_.value -match '^░+$')}).Count -ne 1){exit 22}; $path=Join-Path $env:LOCALAPPDATA 'Juna/settings.json'; $prefs=Get-Content -Raw $path|ConvertFrom-Json; $prefs.junafetch_profile='detailed'; [IO.File]::WriteAllText($path,($prefs|ConvertTo-Json),[Text.UTF8Encoding]::new($false)); $c=junafetch --json|ConvertFrom-Json; if($LASTEXITCODE -ne 0 -or $c.profile -ne 'detailed' -or @($c.fields|Where-Object name -eq 'BIOS').Count -ne 1){exit 23}; Write-Output 'JUNA_BUNDLED_FETCH_OK'"#;
        let spec = SessionSpec {
            program,
            args: vec![
                "-NoProfile".into(),
                "-Command".into(),
                format!("fetch --profile standard --logo none; {script}").into(),
            ],
            cwd: None,
            env,
        };
        let mut child = session::spawn(
            &spec,
            Dimensions {
                cols: 100,
                rows: 20,
            },
        )
        .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let n = child.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..n]);
            assert!(bytes.len() < 1024 * 1024);
        }
        assert!(String::from_utf8_lossy(&bytes).contains("JUNA_BUNDLED_FETCH_OK"));
        let report = broker
            .poll()
            .expect("interactive fetch registered protected fields");
        assert_eq!(report.tab, 1);
        let output = String::from_utf8_lossy(&bytes);
        let mut terminal = crate::vt::TerminalState::new(100, 20);
        terminal.advance(&bytes);
        let visible = terminal.snapshot(0);
        for (token, value) in &report.fields {
            let [r, g, b, br, bg, bb] = crate::privacy::wire::marker(token).unwrap();
            assert!(
                !output.contains(value),
                "protected value entered terminal output"
            );
            let matches = |cell: &crate::vt::Cell| {
                cell.content.first_scalar() == Some('░' as u32)
                    && cell.fg == crate::vt::Color::Rgb(r, g, b)
                    && cell.bg == crate::vt::Color::Rgb(br, bg, bb)
            };
            let tagged = visible.cells.iter().any(matches)
                || (0..terminal.scrollback_len()).any(|row| {
                    terminal
                        .scrollback_line(row)
                        .is_some_and(|cells| cells.iter().any(matches))
                });
            assert!(
                tagged,
                "ConPTY output lost the association between a protected field and its shaded cells"
            );
        }
        let mut app = super::super::App::new(100, 20);
        app.store = crate::settings::Store::new(root.clone());
        app.settings = crate::settings::Settings::default();
        app.terminal = terminal;
        app.scale = 1.5;
        app.left_inset = 24.;
        app.tabs[0].privacy.accept(report.fields.clone());
        for (token, value) in &report.fields {
            let mut found = None;
            for offset in 0..=app.terminal.scrollback_len() {
                app.offset = offset;
                app.tabs[0].privacy.update(&app.terminal, offset);
                for row in 0..20 {
                    for col in 0..100 {
                        if app.tabs[0].privacy.field_at(row, col) == Some(token.as_str()) {
                            found = Some((row, col));
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            let (row, col) = found.expect("actual ConPTY cells must map to their protected field");
            app.pointer_px = [
                super::super::PAD * app.scale
                    + app.left_inset
                    + (col as f32 + 0.5) * super::super::CELL_W * app.scale,
                (super::super::PAD + super::super::ui::TAB_HEIGHT) * app.scale
                    + (row as f32 + 0.5) * super::super::CELL_H * app.scale,
            ];
            let moved = winit::event::WindowEvent::CursorMoved {
                device_id: winit::event::DeviceId::dummy(),
                position: winit::dpi::PhysicalPosition::new(
                    app.pointer_px[0] as f64,
                    app.pointer_px[1] as f64,
                ),
            };
            app.privacy_event(&winit::event::WindowEvent::ModifiersChanged(
                winit::keyboard::ModifiersState::SHIFT.into(),
            ));
            app.privacy_event(&moved);
            assert!(
                app.privacy_reveal.token.as_deref() == Some(token.as_str()),
                "real shaded cell did not reveal through app input"
            );
            assert!(app.privacy_overlay(2000, 1500).is_some());
            app.selection = Some((
                (app.tabs[0].privacy.top() + row as usize, col),
                (app.tabs[0].privacy.top() + row as usize, col),
            ));
            assert!(!app.selection_text().contains(value));
            app.privacy_event(&winit::event::WindowEvent::ModifiersChanged(
                winit::keyboard::ModifiersState::empty().into(),
            ));
            assert!(app.privacy_overlay(2000, 1500).is_none());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn verified_cache_repairs_corruption_and_keeps_both_aliases() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        super::prepare(&root, &"powershell.exe".into()).unwrap();
        let folder = root.join("tools").join(env!("CARGO_PKG_VERSION"));
        let a = std::fs::read(folder.join("fetch.exe")).unwrap();
        assert_eq!(a, std::fs::read(folder.join("junafetch.exe")).unwrap());
        std::fs::write(folder.join("fetch.exe"), b"bad cache").unwrap();
        super::prepare(&root, &"powershell.exe".into()).unwrap();
        assert_eq!(std::fs::read(folder.join("fetch.exe")).unwrap(), a);
        std::fs::remove_dir_all(root).unwrap();
    }
}
