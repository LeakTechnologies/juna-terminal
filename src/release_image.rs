//! Read-only PE/resource checks for release assets, based on Microsoft's PE format.

pub fn bytes(data: &[u8], offset: usize, size: usize) -> Result<&[u8], String> {
    data.get(offset..offset.checked_add(size).ok_or("Offset overflow")?)
        .ok_or_else(|| "Truncated image".into())
}
pub fn u16_at(data: &[u8], offset: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        bytes(data, offset, 2)?.try_into().unwrap(),
    ))
}
pub fn u32_at(data: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes(data, offset, 4)?.try_into().unwrap(),
    ))
}
pub struct Image<'a> {
    data: &'a [u8],
    sections: usize,
    count: usize,
    root: usize,
}
impl<'a> Image<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, String> {
        if bytes(data, 0, 2)? != b"MZ" {
            return Err("Not a Windows executable".into());
        }
        let pe = u32_at(data, 60)? as usize;
        if bytes(data, pe, 4)? != b"PE\0\0" || u16_at(data, pe + 4)? != 0x8664 {
            return Err("Expected Windows x64".into());
        }
        let optional = pe + 24;
        if u16_at(data, optional)? != 0x20b {
            return Err("Expected PE32+".into());
        }
        let mut image = Self {
            data,
            sections: optional + u16_at(data, pe + 20)? as usize,
            count: u16_at(data, pe + 6)? as usize,
            root: 0,
        };
        if image.count > 96 {
            return Err("Too many PE sections".into());
        }
        image.root = image.rva(u32_at(data, optional + 128)?)?;
        Ok(image)
    }
    fn rva(&self, address: u32) -> Result<usize, String> {
        for index in 0..self.count {
            let section = self.sections + index * 40;
            let start = u32_at(self.data, section + 12)?;
            let size = u32_at(self.data, section + 16)?;
            if address >= start && address - start < size {
                let offset = (u32_at(self.data, section + 20)? as usize)
                    .checked_add((address - start) as usize)
                    .ok_or("Offset overflow")?;
                bytes(self.data, offset, 1)?;
                return Ok(offset);
            }
        }
        Err("Resource RVA is outside file sections".into())
    }
    fn entry(&self, directory: usize, id: Option<u32>, subtree: bool) -> Result<usize, String> {
        let directory = self.root.checked_add(directory).ok_or("Offset overflow")?;
        let count = usize::from(u16_at(self.data, directory + 12)?)
            + usize::from(u16_at(self.data, directory + 14)?);
        if count > 1024 {
            return Err("Too many resource entries".into());
        }
        for index in 0..count {
            let entry = directory + 16 + index * 8;
            let name = u32_at(self.data, entry)?;
            let target = u32_at(self.data, entry + 4)?;
            if id.is_none_or(|id| name == id) && (target & 0x80000000 != 0) == subtree {
                return Ok((target & 0x7fffffff) as usize);
            }
        }
        Err("Required native resource is missing".into())
    }
    pub fn resource(&self, kind: u32, id: u32) -> Result<&[u8], String> {
        let kind = self.entry(0, Some(kind), true)?;
        let name = self.entry(kind, Some(id), true)?;
        let entry = self.root + self.entry(name, None, false)?;
        let offset = self.rva(u32_at(self.data, entry)?)?;
        bytes(self.data, offset, u32_at(self.data, entry + 4)? as usize)
    }
}

pub fn version_strings(data: &[u8]) -> Result<std::collections::BTreeMap<String, String>, String> {
    fn visit(
        data: &[u8],
        start: usize,
        end: usize,
        depth: usize,
        remaining: &mut usize,
        out: &mut std::collections::BTreeMap<String, String>,
    ) -> Result<(), String> {
        if depth > 8 || *remaining == 0 {
            return Err("Excessive version resource nesting".into());
        }
        *remaining -= 1;
        let size = u16_at(data, start)? as usize;
        if size < 6 || start.checked_add(size).is_none_or(|n| n > end) {
            return Err("Invalid version block".into());
        }
        let end = start + size;
        let count = u16_at(data, start + 2)? as usize;
        let text = u16_at(data, start + 4)? == 1;
        let mut at = start + 6;
        let mut key = Vec::new();
        loop {
            if at + 2 > end {
                return Err("Unterminated version key".into());
            }
            let value = u16_at(data, at)?;
            at += 2;
            if value == 0 {
                break;
            }
            key.push(value);
        }
        let key = String::from_utf16(&key).map_err(|_| "Invalid version key")?;
        at = (at + 3) & !3;
        let value_bytes = count
            .checked_mul(if text { 2 } else { 1 })
            .ok_or("Version size overflow")?;
        if at.checked_add(value_bytes).is_none_or(|n| n > end) {
            return Err("Invalid version value".into());
        }
        if text && count > 0 {
            let words = bytes(data, at, value_bytes)?
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<_>>();
            let value = String::from_utf16(&words)
                .map_err(|_| "Invalid version text")?
                .trim_end_matches('\0')
                .to_owned();
            if out.insert(key, value).is_some() {
                return Err("Duplicate version string".into());
            }
        }
        at = (at + value_bytes + 3) & !3;
        while at < end {
            let child = u16_at(data, at)? as usize;
            if child == 0 {
                return Err("Empty version child".into());
            }
            visit(data, at, end, depth + 1, remaining, out)?;
            at = (at + child + 3) & !3;
        }
        Ok(())
    }
    let image = Image::parse(data)?;
    let version = image.resource(16, 1)?;
    let mut strings = std::collections::BTreeMap::new();
    visit(version, 0, version.len(), 0, &mut 1024, &mut strings)?;
    Ok(strings)
}
pub fn verify_identity(data: &[u8], version: &str, commit: &str) -> Result<(), String> {
    let strings = version_strings(data)?;
    if strings.get("ProductName").map(String::as_str) != Some("Juna Terminal")
        || strings.get("ProductVersion").map(String::as_str) != Some(version)
        || strings.get("Comments") != Some(&format!("Source commit: {commit}"))
    {
        return Err("Executable version/source identity mismatch".into());
    }
    Ok(())
}
