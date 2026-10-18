//! File drops insert text only; paths never become terminal controls or diagnostics.
use super::*;
use std::path::Path;

pub(super) fn text(path: &Path, shell: &Path) -> Result<String, &'static str> {
    let path = path.to_str().ok_or("Dropped path is not Unicode")?;
    if path.is_empty()
        || path.len() > 128 * 1024
        || path
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}' | '"'))
    {
        return Err("Dropped path contains unsupported characters or exceeds the limit");
    }
    let shell = shell
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let quoted = match shell.as_str() {
        "powershell" | "pwsh" => {
            // PowerShell treats typographic single quotes as delimiters too.
            let mut quoted = String::from("'");
            for ch in path.chars() {
                if matches!(ch, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
                    quoted.push(ch);
                }
                quoted.push(ch);
            }
            quoted.push('\'');
            quoted
        }
        "cmd" => {
            // Percent expansion and optional delayed expansion also occur inside cmd quotes.
            if path.contains(['%', '!']) || path.ends_with('\\') {
                return Err("This path requires manual entry in cmd");
            }
            format!("\"{path}\"")
        }
        _ => path.to_owned(),
    };
    Ok(quoted + " ")
}

#[cfg(windows)]
fn position(window: &Window) -> Option<[f32; 2]> {
    use windows::Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::ScreenToClient,
        UI::WindowsAndMessaging::GetCursorPos,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    let mut point = POINT::default();
    // SAFETY: point is writable, and the borrowed live window handle is used synchronously.
    unsafe {
        GetCursorPos(&mut point).ok()?;
        if !ScreenToClient(HWND(handle.hwnd.get() as *mut _), &mut point).as_bool() {
            return None;
        }
    }
    Some([point.x as f32, point.y as f32])
}
#[cfg(not(windows))]
fn position(_: &Window) -> Option<[f32; 2]> {
    None
}

impl App {
    pub(super) fn dropped_file(&mut self, path: &Path) {
        let point = self.window.as_ref().and_then(|window| position(window));
        if let Some(point) = point {
            if let Err(message) = self.drop_at(path, point) {
                log::warn!("File drop: {message}");
            }
        }
    }
    pub(super) fn drop_at(&mut self, path: &Path, point: [f32; 2]) -> Result<bool, &'static str> {
        let x = point[0] - PAD * self.scale - self.left_inset;
        let y = point[1] - (PAD + ui::TAB_HEIGHT) * self.scale;
        if self.ui.is_some()
            || self.active_closing
            || self.minimized
            || !x.is_finite()
            || !y.is_finite()
            || x < 0.
            || y < 0.
            || x >= self.cols as f32 * CELL_W * self.scale
            || y >= self.rows as f32 * CELL_H * self.scale
        {
            return Ok(false);
        }
        let Some(runtime) = self.runtime.as_mut().filter(|r| !r.is_closing()) else {
            return Ok(false);
        };
        if !path.is_absolute() {
            return Err("Dropped path must be absolute");
        }
        let shell = self
            .shell_spec
            .as_ref()
            .ok_or("Shell context unavailable")?;
        let shell_path = if self.terminal.alternate_active() {
            Path::new("")
        } else {
            Path::new(&shell.program)
        };
        let text = text(path, shell_path)?;
        let bytes = input::paste(&text, self.terminal.input_modes().bracketed_paste);
        runtime
            .write(&bytes)
            .map_err(|_| "File path could not be queued")?;
        self.offset = 0;
        self.selection = None;
        self.privacy_reveal.hide();
        self.privacy_dirty = true;
        self.git_activity = Instant::now();
        self.dirty = true;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn powershell_parses_smart_quotes_and_metacharacters_as_literal_path_data() {
        let path = Path::new("C:\\A'‘’‚‛ $(not_a_command) & 界.txt");
        let quoted = text(path, Path::new("powershell.exe")).unwrap();
        let script=format!("$value={quoted}; if($value -cne $env:JUNA_QUOTE_EXPECTED){{exit 9}}; [Console]::Write('LITERAL_PATH_OK')");
        let spec = SessionSpec {
            program: "powershell.exe".into(),
            args: vec!["-NoProfile".into(), "-Command".into(), script.into()],
            env: vec![("JUNA_QUOTE_EXPECTED".into(), path.as_os_str().to_owned())],
            ..Default::default()
        };
        let mut runtime = Runtime::spawn(&spec, Dimensions::default()).unwrap();
        let mut output = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            output.extend(runtime.stream.take());
            if runtime.tick().unwrap() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(runtime.exit.unwrap().code(), 0);
        assert!(String::from_utf8_lossy(&output).contains("LITERAL_PATH_OK"));
    }
    #[cfg(windows)]
    #[test]
    fn drop_routes_only_to_live_terminal_and_never_adds_enter() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let path = root.join("O'Brien $value 界.txt");
        let mut spec = SessionSpec {
            program: "powershell.exe".into(),
            args: vec![
                "-NoProfile".into(),
                "-Command".into(),
                "[Console]::InputEncoding=[Text.UTF8Encoding]::new($false); [Console]::WriteLine('DROP_READY'); $line=[Console]::ReadLine(); if($line -cne $env:JUNA_DROP_EXPECTED){$e=$env:JUNA_DROP_EXPECTED;$i=0;while($i -lt [Math]::Min($line.Length,$e.Length) -and $line[$i] -ceq $e[$i]){$i++};$a=if($i -lt $line.Length){[int]$line[$i]}else{-1};$b=if($i -lt $e.Length){[int]$e[$i]}else{-1};[Console]::Write(('DROPCHECK:{0}_{1}_{2}_{3}_{4}_{5}END' -f $line.Length,$e.Length,$i,$a,$b,[Console]::InputEncoding.CodePage));exit 9}; [Console]::Write('DROP_ACK_OK')".into(),
            ],
            ..Default::default()
        };
        spec.env.push((
            "JUNA_DROP_EXPECTED".into(),
            text(&path, Path::new("powershell.exe")).unwrap().into(),
        ));
        let mut app = App::new(80, 5);
        app.store = crate::settings::Store::new(root.clone());
        app.settings = crate::settings::Settings::default();
        app.shell_spec = Some(spec.clone());
        app.runtime = Some(Runtime::spawn(&spec, Dimensions { cols: 80, rows: 5 }).unwrap());
        let ready_deadline = Instant::now() + Duration::from_secs(15);
        let mut ready = Vec::new();
        loop {
            let runtime = app.runtime.as_mut().unwrap();
            ready.extend(runtime.stream.take());
            if String::from_utf8_lossy(&ready).contains("DROP_READY") {
                break;
            }
            assert!(!runtime.tick().unwrap());
            assert!(
                Instant::now() < ready_deadline,
                "drop reader did not become ready"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        app.left_inset = 20.;
        let point = [PAD + app.left_inset + 5., PAD + ui::TAB_HEIGHT + 5.];
        assert!(!app.drop_at(&path, [PAD, point[1]]).unwrap());
        assert!(!app.drop_at(&path, [point[0], 0.]).unwrap());
        assert!(!app.drop_at(&path, [f32::NAN, point[1]]).unwrap());
        app.open_ui(ui::Kind::Appearance);
        assert!(!app.drop_at(&path, point).unwrap());
        app.close_ui();
        app.privacy_dirty = false;
        app.privacy_reveal.token = Some("fixture".into());
        assert!(app.drop_at(&path, point).unwrap());
        assert!(app.privacy_dirty && app.privacy_reveal.token.is_none());
        std::thread::sleep(Duration::from_millis(200));
        assert!(!app.runtime.as_mut().unwrap().tick().unwrap());
        app.runtime.as_mut().unwrap().write(b"\r").unwrap();
        let mut output = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let runtime = app.runtime.as_mut().unwrap();
            output.extend(runtime.stream.take());
            if runtime.tick().unwrap() {
                break;
            }
            assert!(Instant::now() < deadline, "drop fixture exceeded deadline");
            std::thread::sleep(Duration::from_millis(5));
        }
        let diagnostic = String::from_utf8_lossy(&output)
            .split("DROPCHECK:")
            .nth(1)
            .unwrap_or("")
            .chars()
            .take_while(|c| c.is_ascii_digit() || matches!(c, '_' | '-'))
            .collect::<String>();
        assert_eq!(
            app.runtime.as_ref().unwrap().exit.unwrap().code(),
            0,
            "length/first-difference/codepage: {diagnostic}"
        );
        assert!(String::from_utf8_lossy(&output).contains("DROP_ACK_OK"));
        assert!(!app.drop_at(&path, point).unwrap());
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn paths_are_text_without_execution_and_expansion() {
        let path = Path::new("C:\\sample folder\\O'Brien $value 界.txt");
        assert_eq!(
            text(path, Path::new("powershell.exe")).unwrap(),
            "'C:\\sample folder\\O''Brien $value 界.txt' "
        );
        assert_eq!(
            text(Path::new("C:\\a & b.txt"), Path::new("cmd.exe")).unwrap(),
            "\"C:\\a & b.txt\" "
        );
        assert!(text(Path::new("C:\\%TEMP%\\file"), Path::new("cmd.exe")).is_err());
        assert!(text(Path::new("C:\\!variable!"), Path::new("cmd.exe")).is_err());
        for bad in [
            "bad\rpath",
            "bad\npath",
            "bad\x1bpath",
            "bad\u{2028}path",
            "bad\"path",
        ] {
            assert!(text(Path::new(bad), Path::new("pwsh.exe")).is_err());
        }
        let plain = text(Path::new("/tmp/a b"), Path::new("unknown-shell")).unwrap();
        assert_eq!(plain, "/tmp/a b ");
        let one = text(Path::new("/tmp/one"), Path::new("pwsh.exe")).unwrap();
        let two = text(Path::new("/tmp/two"), Path::new("pwsh.exe")).unwrap();
        assert_eq!(one + &two, "'/tmp/one' '/tmp/two' ");
        let pasted = input::paste(&plain, true);
        assert!(pasted.starts_with(b"\x1b[200~"));
        assert!(pasted.ends_with(b"\x1b[201~"));
        assert!(!pasted.contains(&b'\r'));
    }
}
