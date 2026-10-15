//! Restricted release ZIP extraction from PKWARE APPNOTE, with bounded Deflate infrastructure.
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
pub const MAX_ARCHIVE: usize = 128 * 1024 * 1024;
const MAX_EXPANDED: usize = 256 * 1024 * 1024;
fn slice(data: &[u8], at: usize, len: usize) -> Result<&[u8], String> {
    data.get(at..at.checked_add(len).ok_or("ZIP offset overflow")?)
        .ok_or_else(|| "Truncated ZIP".into())
}
fn u16n(data: &[u8], at: usize) -> Result<usize, String> {
    Ok(u16::from_le_bytes(slice(data, at, 2)?.try_into().unwrap()) as usize)
}
fn u32n(data: &[u8], at: usize) -> Result<usize, String> {
    Ok(u32::from_le_bytes(slice(data, at, 4)?.try_into().unwrap()) as usize)
}
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
pub fn safe_name(name: &str) -> bool {
    let file = name.trim_end_matches('/');
    if file.is_empty()
        || name.len() > 1024
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return false;
    }
    if !(file == "juna.exe"
        || file == "RELEASE_NOTES.md"
        || file == "notices"
        || file.starts_with("notices/"))
    {
        return false;
    }
    file.split('/').all(|part| {
        if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' ']) {
            return false;
        }
        let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        !matches!(
            base.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) && !base
            .strip_prefix("COM")
            .or_else(|| base.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
    })
}
struct Entry {
    name: String,
    method: usize,
    flags: usize,
    crc: u32,
    size: usize,
    compressed: usize,
    offset: usize,
}
fn entries(data: &[u8]) -> Result<Vec<Entry>, String> {
    if data.len() > MAX_ARCHIVE || data.len() < 22 {
        return Err("Invalid ZIP size".into());
    }
    let end = (data.len().saturating_sub(65557)..=data.len() - 22)
        .rev()
        .find(|&at| {
            data.get(at..at + 4) == Some(b"PK\x05\x06")
                && u16n(data, at + 20)
                    .ok()
                    .is_some_and(|n| at + 22 + n == data.len())
        })
        .ok_or("ZIP directory missing")?;
    let count = u16n(data, end + 10)?;
    if u16n(data, end + 4)? != 0
        || u16n(data, end + 6)? != 0
        || u16n(data, end + 8)? != count
        || count == 0
        || count > 4096
    {
        return Err("Unsupported ZIP directory".into());
    }
    let start = u32n(data, end + 16)?;
    let length = u32n(data, end + 12)?;
    if start.checked_add(length) != Some(end) {
        return Err("Invalid ZIP directory bounds".into());
    }
    let mut at = start;
    let mut total = 0usize;
    let mut names = BTreeSet::new();
    let mut result = Vec::new();
    for _ in 0..count {
        if slice(data, at, 4)? != b"PK\x01\x02" {
            return Err("Invalid ZIP directory entry".into());
        }
        let flags = u16n(data, at + 8)?;
        let method = u16n(data, at + 10)?;
        let compressed = u32n(data, at + 20)?;
        let size = u32n(data, at + 24)?;
        let name_len = u16n(data, at + 28)?;
        let extra = u16n(data, at + 30)?;
        let comment = u16n(data, at + 32)?;
        let attributes = u32n(data, at + 38)?;
        let offset = u32n(data, at + 42)?;
        let name = std::str::from_utf8(slice(data, at + 46, name_len)?)
            .map_err(|_| "ZIP names must be UTF-8")?
            .to_owned();
        let directory = name.ends_with('/');
        if !safe_name(&name)
            || !names.insert(name.trim_end_matches('/').to_lowercase())
            || !matches!(method, 0 | 8)
            || flags & !0x080e != 0
            || u16n(data, at + 34)? != 0
            || attributes >> 16 & 0xf000 == 0xa000
            || attributes & 0x400 != 0
            || [size, compressed, offset].contains(&(u32::MAX as usize))
            || (directory && size != 0)
            || size > 192 * 1024 * 1024
            || (!directory && name != "juna.exe" && size > 8 * 1024 * 1024)
        {
            return Err("Unsafe or unsupported ZIP entry".into());
        }
        total = total.checked_add(size).ok_or("ZIP size overflow")?;
        if total > MAX_EXPANDED {
            return Err("Expanded update is too large".into());
        }
        result.push(Entry {
            name,
            method,
            flags,
            crc: u32n(data, at + 16)? as u32,
            size,
            compressed,
            offset,
        });
        at = at
            .checked_add(46 + name_len + extra + comment)
            .ok_or("ZIP offset overflow")?;
        if at > end {
            return Err("ZIP entry exceeds directory".into());
        }
    }
    if at != end
        || !result.iter().any(|e| e.name == "juna.exe")
        || !result.iter().any(|e| e.name == "RELEASE_NOTES.md")
        || !result
            .iter()
            .any(|e| e.name.starts_with("notices/") && !e.name.ends_with('/'))
    {
        return Err("Update package is incomplete".into());
    }
    // Validate all local records before writing any archive member.
    let mut intervals = Vec::new();
    for e in &result {
        let local = e.offset;
        if slice(data, local, 4)? != b"PK\x03\x04"
            || u16n(data, local + 6)? != e.flags
            || u16n(data, local + 8)? != e.method
        {
            return Err("ZIP local header differs".into());
        }
        let name_len = u16n(data, local + 26)?;
        let extra = u16n(data, local + 28)?;
        if slice(data, local + 30, name_len)? != e.name.as_bytes() {
            return Err("ZIP local name differs".into());
        }
        if e.flags & 8 == 0
            && (u32n(data, local + 14)? as u32 != e.crc
                || u32n(data, local + 18)? != e.compressed
                || u32n(data, local + 22)? != e.size)
        {
            return Err("ZIP local sizes differ".into());
        }
        let finish = local
            .checked_add(30 + name_len + extra)
            .and_then(|n| n.checked_add(e.compressed))
            .ok_or("ZIP offset overflow")?;
        if finish > start {
            return Err("ZIP data exceeds payload".into());
        }
        intervals.push((local, finish));
    }
    intervals.sort_unstable();
    if intervals.windows(2).any(|w| w[0].1 > w[1].0) {
        return Err("Overlapping ZIP records".into());
    }
    Ok(result)
}
pub fn extract(data: &[u8], destination: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let entries = entries(data)?;
    std::fs::create_dir(destination).map_err(|_| "Cannot create update staging folder")?;
    for entry in entries {
        if cancel.load(Ordering::Relaxed) {
            return Err("Update cancelled".into());
        }
        let path = destination.join(PathBuf::from(entry.name.trim_end_matches('/')));
        if entry.name.ends_with('/') {
            std::fs::create_dir_all(path).map_err(|_| "Cannot stage update directory")?;
            continue;
        }
        std::fs::create_dir_all(path.parent().unwrap())
            .map_err(|_| "Cannot stage update directory")?;
        let start =
            entry.offset + 30 + u16n(data, entry.offset + 26)? + u16n(data, entry.offset + 28)?;
        let payload = slice(data, start, entry.compressed)?;
        let bytes = if entry.method == 0 {
            payload.to_vec()
        } else {
            let mut decoder = flate2::read::DeflateDecoder::new(payload);
            let mut out = Vec::new();
            decoder
                .by_ref()
                .take(entry.size as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|_| "Invalid compressed update")?;
            if decoder.total_in() != entry.compressed as u64 {
                return Err("Invalid compressed update size".into());
            }
            out
        };
        if bytes.len() != entry.size || crc32(&bytes) != entry.crc {
            return Err("Update ZIP checksum mismatch".into());
        }
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|_| "Cannot stage update file")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Cannot write staged update")?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    fn zip(deflate: bool) -> Vec<u8> {
        use std::io::Write;
        let plain = b"123456789";
        let body = if deflate {
            let mut out =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            out.write_all(plain).unwrap();
            out.finish().unwrap()
        } else {
            plain.to_vec()
        };
        let mut data = Vec::new();
        let mut directory = Vec::new();
        for name in ["juna.exe", "RELEASE_NOTES.md", "notices/LICENSE.txt"] {
            let offset = data.len() as u32;
            data.extend(b"PK\x03\x04");
            for value in [20u16, 0x800, if deflate { 8 } else { 0 }, 0, 0] {
                data.extend(value.to_le_bytes());
            }
            for value in [0xcbf43926u32, body.len() as u32, 9] {
                data.extend(value.to_le_bytes());
            }
            data.extend((name.len() as u16).to_le_bytes());
            data.extend(0u16.to_le_bytes());
            data.extend(name.as_bytes());
            data.extend(&body);
            directory.extend(b"PK\x01\x02");
            for value in [20u16, 20, 0x800, if deflate { 8 } else { 0 }, 0, 0] {
                directory.extend(value.to_le_bytes());
            }
            for value in [0xcbf43926u32, body.len() as u32, 9] {
                directory.extend(value.to_le_bytes());
            }
            for value in [name.len() as u16, 0, 0, 0, 0] {
                directory.extend(value.to_le_bytes());
            }
            directory.extend(0u32.to_le_bytes());
            directory.extend(offset.to_le_bytes());
            directory.extend(name.as_bytes());
        }
        let offset = data.len() as u32;
        let size = directory.len() as u32;
        data.extend(directory);
        data.extend(b"PK\x05\x06");
        for value in [0u16, 0, 3, 3] {
            data.extend(value.to_le_bytes());
        }
        data.extend(size.to_le_bytes());
        data.extend(offset.to_le_bytes());
        data.extend(0u16.to_le_bytes());
        data
    }
    #[test]
    fn stored_deflated_corrupt_encrypted_and_cancelled_packages_are_distinct() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        std::fs::create_dir_all(&root).unwrap();
        for deflate in [false, true] {
            let folder = root.join(if deflate { "deflated" } else { "stored" });
            super::extract(
                &zip(deflate),
                &folder,
                &std::sync::atomic::AtomicBool::new(false),
            )
            .unwrap();
            assert_eq!(
                std::fs::read(folder.join("juna.exe")).unwrap(),
                b"123456789"
            );
        }
        let mut corrupt = zip(false);
        corrupt[38] ^= 1;
        assert!(super::extract(
            &corrupt,
            &root.join("corrupt"),
            &std::sync::atomic::AtomicBool::new(false)
        )
        .is_err());
        assert!(!root.join("corrupt/juna.exe").exists());
        let mut encrypted = zip(false);
        encrypted[6] |= 1;
        assert!(super::entries(&encrypted).is_err());
        assert!(super::extract(
            &zip(true),
            &root.join("cancelled"),
            &std::sync::atomic::AtomicBool::new(true)
        )
        .is_err());
        assert!(!root.join("cancelled/juna.exe").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn names_and_malformed_archives_never_escape_staging() {
        for name in [
            "../juna.exe",
            "/juna.exe",
            "C:/juna.exe",
            "notices/../evil",
            "notices/a\\b",
            "notices/CON.txt",
            "notices/a.",
            "notices/x:stream",
            "notices/./x",
            "notices/COM1.txt",
            "notices/COM¹.txt",
            "notices/LPT³.txt",
            "notices/x?y",
            "notices/x*y",
            "notices/x|y",
            "notices/x\"y",
            "settings.json",
        ] {
            assert!(!super::safe_name(name), "{name}");
        }
        assert!(super::safe_name("notices/fonts/FiraCode/OFL.txt"));
        assert_eq!(super::crc32(b"123456789"), 0xcbf43926);
        for length in 0..1024 {
            assert!(super::entries(&vec![0; length]).is_err());
        }
    }
}
