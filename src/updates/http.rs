//! Synchronous HTTPS owned by one bounded I/O worker. No TLS bypass or credential logging.
#[cfg(windows)]
use std::sync::atomic::Ordering;
use std::{
    io::Write,
    sync::atomic::{AtomicBool, AtomicU64},
};
pub fn get(
    url: &str,
    out: &mut impl Write,
    limit: u64,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (url, out, limit, cancel, progress);
        Err("Updates require Windows x64".into())
    }
    #[cfg(windows)]
    {
        windows_get(url, out, limit, cancel, progress)
    }
}
#[cfg(any(windows, test))]
fn address(url: &str) -> Option<(&str, &str)> {
    let (host, path) = url.strip_prefix("https://")?.split_once('/')?;
    if !matches!(
        host,
        "api.github.com"
            | "github.com"
            | "release-assets.githubusercontent.com"
            | "objects.githubusercontent.com"
    ) || url.len() > 16384
        || url.chars().any(char::is_control)
        || url.contains('#')
    {
        return None;
    }
    Some((host, path))
}
#[cfg(windows)]
fn windows_get(
    url: &str,
    out: &mut impl Write,
    limit: u64,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<(), String> {
    use std::ffi::c_void;
    use windows::{
        core::{w, PCWSTR},
        Win32::Networking::WinHttp::*,
    };
    struct Handle(*mut c_void);
    impl Handle {
        fn new(raw: *mut c_void) -> Result<Self, String> {
            if raw.is_null() {
                Err("Cannot initialise HTTPS request".into())
            } else {
                Ok(Self(raw))
            }
        }
    }
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: this guard uniquely owns a valid WinHTTP handle on its creating thread.
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
    let (host, path) = address(url).ok_or("Update URL is not an approved HTTPS origin")?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Update cancelled".into());
    }
    let host = host.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let path = format!("/{path}")
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let agent = format!("Juna/{}", env!("CARGO_PKG_VERSION"))
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: synchronous handles and all UTF-16 inputs are owned/live until each call returns; no callback or external pointer is retained.
    let (session, connection, request) = unsafe {
        let session = Handle::new(WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 10000, 10000, 10000, 15000)
            .map_err(|_| "Cannot set HTTPS timeouts")?;
        let connection = Handle::new(WinHttpConnect(session.0, PCWSTR(host.as_ptr()), 443, 0))?;
        let request = Handle::new(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        WinHttpSetOption(
            Some(request.0),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP.to_le_bytes()),
        )
        .map_err(|_| "Cannot enforce HTTPS redirects")?;
        WinHttpSetOption(
            Some(request.0),
            WINHTTP_OPTION_AUTOLOGON_POLICY,
            Some(&WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH.to_le_bytes()),
        )
        .map_err(|_| "Cannot disable automatic HTTP credentials")?;
        let headers = "Accept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2026-03-10\r\n"
            .encode_utf16()
            .collect::<Vec<_>>();
        WinHttpSendRequest(request.0, Some(&headers), None, 0, 0, 0)
            .map_err(|_| "HTTPS connection failed; check your network")?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            .map_err(|_| "HTTPS response timed out or failed")?;
        (session, connection, request)
    };
    let _lifetime = (&session, &connection);
    let mut status = 0u32;
    let mut size = 4u32;
    let mut index = 0u32;
    // SAFETY: numeric header output points to a live four-byte u32 and the request remains open on this thread.
    unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut size,
            &mut index,
        )
        .map_err(|_| "HTTP status unavailable")?;
    }
    if status != 200 {
        return Err(match status {
            403 | 429 => "GitHub rate limit or access refusal; try later",
            404 => "Release asset is unavailable",
            _ => "GitHub request failed",
        }
        .into());
    }
    let mut final_url = vec![0u16; 16385];
    let mut bytes = (final_url.len() * 2) as u32;
    // SAFETY: final_url is writable/live and byte length matches its allocation; no output is used unless the synchronous call succeeds.
    unsafe {
        WinHttpQueryOption(
            request.0,
            WINHTTP_OPTION_URL,
            Some(final_url.as_mut_ptr().cast()),
            &mut bytes,
        )
        .map_err(|_| "Cannot verify HTTPS redirect origin")?;
    }
    let final_url = String::from_utf16(&final_url[..(bytes as usize / 2).min(final_url.len())])
        .map_err(|_| "Invalid redirect origin")?;
    if address(final_url.trim_end_matches('\0')).is_none() {
        return Err("Update redirect origin is not approved".into());
    }
    let mut buffer = [0u8; 32768];
    let mut total = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Update cancelled".into());
        }
        let mut read = 0u32;
        // SAFETY: buffer/read are owned writable storage; request is live, synchronous and used by this thread only.
        unsafe {
            WinHttpReadData(
                request.0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut read,
            )
            .map_err(|_| "Download stalled or connection failed")?;
        }
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or("Download size overflow")?;
        if total > limit {
            return Err("Download exceeds the allowed size".into());
        }
        out.write_all(&buffer[..read as usize])
            .map_err(|_| "Cannot write update download")?;
        progress.store(total, Ordering::Relaxed);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Update cancelled".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    #[cfg(windows)]
    fn cancellation_before_connection_writes_no_bytes() {
        let mut bytes = Vec::new();
        let error = super::get(
            "https://api.github.com/repos/LeakTechnologies/juna-terminal/releases",
            &mut bytes,
            1,
            &std::sync::atomic::AtomicBool::new(true),
            &std::sync::atomic::AtomicU64::new(0),
        )
        .unwrap_err();
        assert_eq!(error, "Update cancelled");
        assert!(bytes.is_empty());
    }
    #[test]
    fn https_origins_are_restricted() {
        for invalid in [
            "http://github.com/x",
            "https://github.com.evil/x",
            "https://user@github.com/x",
            "https://localhost/x",
            "https://github.com:443/x",
            "https://github.com/x#fragment",
        ] {
            assert!(super::address(invalid).is_none());
        }
        assert!(
            super::address("https://release-assets.githubusercontent.com/x?signature=x").is_some()
        );
    }
}
