//! File selection and JSON I/O worker; it never receives terminal state.
use crate::settings_transfer::Bundle;
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
};
pub enum Outcome {
    Imported(Box<Bundle>),
    Exported,
}
pub type ResultChannel = mpsc::Receiver<Result<Option<Outcome>, String>>;
pub fn start(
    window: Arc<winit::window::Window>,
    export: Option<Bundle>,
) -> Result<ResultChannel, String> {
    #[cfg(windows)]
    let owner = {
        use winit::raw_window_handle::HasWindowHandle;
        match window.window_handle().map_err(|e| e.to_string())?.as_raw() {
            winit::raw_window_handle::RawWindowHandle::Win32(handle) => handle.hwnd.get() as usize,
            _ => return Err("Native file selection is unavailable".into()),
        }
    };
    #[cfg(not(windows))]
    let owner = 0;
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("juna-settings-file".into())
        .spawn(move || {
            let _keep_window_alive = window;
            let result = (|| {
                let Some(path) = pick(owner, export.is_some())? else {
                    return Ok(None);
                };
                if let Some(bundle) = export {
                    bundle.export(&path)?;
                    Ok(Some(Outcome::Exported))
                } else {
                    Ok(Some(Outcome::Imported(Box::new(Bundle::read(&path)?))))
                }
            })();
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
    Ok(rx)
}
#[cfg(not(windows))]
fn pick(_: usize, _: bool) -> Result<Option<PathBuf>, String> {
    Err("File selection requires Windows".into())
}
#[cfg(windows)]
fn pick(owner: usize, export: bool) -> Result<Option<PathBuf>, String> {
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::{
            Foundation::HWND,
            System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
            UI::Controls::Dialogs::*,
        },
    };
    // SAFETY: this new worker owns its COM apartment; no engine/window state is accessed through COM.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: matches the successful initialization above on this same worker thread.
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    let mut filename = vec![0u16; 32768];
    if export {
        for (i, ch) in "Juna-settings.json".encode_utf16().enumerate() {
            filename[i] = ch;
        }
    }
    let filter: Vec<u16> = "JSON settings\0*.json\0All files\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = format!(
        "{} Juna settings\0",
        if export { "Export" } else { "Import" }
    )
    .encode_utf16()
    .collect();
    let extension: Vec<u16> = "json\0".encode_utf16().collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner as *mut _),
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(filename.as_mut_ptr()),
        nMaxFile: filename.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        lpstrDefExt: PCWSTR(extension.as_ptr()),
        Flags: OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if export {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            },
        ..Default::default()
    };
    // SAFETY: all UTF-16 buffers are owned, terminated, and live for the synchronous call;
    // the owner window is retained by the worker, and filename has nMaxFile writable elements.
    let accepted = unsafe {
        if export {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        }
    }
    .as_bool();
    if !accepted {
        // SAFETY: reads only this thread's result of the immediately preceding dialog call.
        let error = unsafe { CommDlgExtendedError() };
        return if error.0 == 0 {
            Ok(None)
        } else {
            Err(format!("File dialog failed ({})", error.0))
        };
    }
    use std::os::windows::ffi::OsStringExt;
    let length = filename
        .iter()
        .position(|&ch| ch == 0)
        .ok_or("Invalid file selection")?;
    Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
        &filename[..length],
    ))))
}
