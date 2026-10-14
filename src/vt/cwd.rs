//! Pure OSC 7 local-file URI validation; never resolves or opens a path.
pub fn local_path(uri: &str) -> Option<String> {
    if uri.len() > 8192 {
        return None;
    }
    let uri = uri.strip_prefix("file://")?;
    let (host, path) = uri.split_once('/')?;
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return None;
    }
    let bytes = path.as_bytes();
    let mut decoded = vec![b'/'];
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let text = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            decoded.push(u8::from_str_radix(text, 16).ok()?);
            i += 3;
        } else {
            if matches!(bytes[i], b'?' | b'#' | b'\\') {
                return None;
            }
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    let path = String::from_utf8(decoded).ok()?;
    if path.chars().any(char::is_control) || path.contains('\\') || path.starts_with("//") {
        return None;
    }
    #[cfg(windows)]
    {
        let path = path.strip_prefix('/')?;
        let bytes = path.as_bytes();
        if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1..3] != *b":/" {
            return None;
        }
        Some(path.to_owned())
    }
    #[cfg(not(windows))]
    {
        Some(path)
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn rejects_remote_control_and_malformed_uris() {
        for uri in [
            "file://server/share",
            "file:///C:/a%00",
            "file:///C:/%ff",
            "file:///C:/%x0",
            "file:relative",
            "file:////server/share",
        ] {
            assert!(super::local_path(uri).is_none(), "{uri}");
        }
        assert!(
            super::local_path("file://localhost/C:/with%20space/%E6%97%A5")
                .unwrap()
                .contains("with space/日")
        );
    }
}
