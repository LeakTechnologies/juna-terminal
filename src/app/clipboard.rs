//! Unicode clipboard platform access. Never reads terminal engine state.
#[cfg(windows)]
pub fn read() -> Result<String, String> {
    use windows::Win32::{
        Foundation::HGLOBAL,
        System::{DataExchange::*, Memory::*},
    };
    // SAFETY: clipboard ownership is held throughout the lock, bounded read, and unlock.
    unsafe {
        OpenClipboard(None).map_err(|e| e.to_string())?;
        let result = (|| {
            let handle = GetClipboardData(13).map_err(|e| e.to_string())?;
            let global = HGLOBAL(handle.0);
            let size = GlobalSize(global);
            if size > 2 * 1024 * 1024 {
                return Err("Clipboard exceeds the 1 MiB text limit".into());
            }
            let pointer = GlobalLock(global).cast::<u16>();
            if pointer.is_null() {
                return Err("Cannot lock clipboard".into());
            }
            let data = std::slice::from_raw_parts(pointer, size / 2);
            let len = data.iter().position(|v| *v == 0).unwrap_or(data.len());
            let text = String::from_utf16_lossy(&data[..len]);
            let _ = GlobalUnlock(global);
            Ok(text)
        })();
        let _ = CloseClipboard();
        result
    }
}
#[cfg(windows)]
pub fn write(window: &winit::window::Window, text: &str) -> Result<(), String> {
    use windows::Win32::{
        Foundation::{GlobalFree, HANDLE, HWND},
        System::{DataExchange::*, Memory::*},
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let owner = match window.window_handle().map_err(|e| e.to_string())?.as_raw() {
        RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut _),
        _ => return Err("No Windows clipboard owner".into()),
    };
    let data: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // SAFETY: allocated movable memory is filled only within its length. Ownership transfers
    // to Windows only on successful SetClipboardData; every failure frees it and closes the clipboard.
    unsafe {
        OpenClipboard(Some(owner)).map_err(|e| e.to_string())?;
        let result = (|| {
            let global = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2).map_err(|e| e.to_string())?;
            let p = GlobalLock(global).cast::<u16>();
            if p.is_null() {
                let _ = GlobalFree(Some(global));
                return Err("Cannot allocate clipboard text".into());
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
            let _ = GlobalUnlock(global);
            if let Err(e) =
                EmptyClipboard().and_then(|_| SetClipboardData(13, Some(HANDLE(global.0))))
            {
                let _ = GlobalFree(Some(global));
                return Err(e.to_string());
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}
#[cfg(not(windows))]
pub fn read() -> Result<String, String> {
    Err("Clipboard is not available on this platform".into())
}
#[cfg(not(windows))]
pub fn write(_: &winit::window::Window, _: &str) -> Result<(), String> {
    Err("Clipboard is not available on this platform".into())
}

pub fn error_dialog(text: &str) {
    log::error!("{text}");
    #[cfg(windows)]
    {
        use windows::{
            core::{w, PCWSTR},
            Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK},
        };
        let data: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        // SAFETY: both strings are NUL-terminated and live until the modal call returns.
        unsafe {
            MessageBoxW(
                None,
                PCWSTR(data.as_ptr()),
                w!("Juna Terminal"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
