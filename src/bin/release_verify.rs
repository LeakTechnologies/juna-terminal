//! Read-only PE/resource checks for release assets, based on Microsoft's PE format.
use std::{fs, path::Path};
fn bytes(data: &[u8], offset: usize, size: usize) -> Result<&[u8], String> {
    data.get(offset..offset.checked_add(size).ok_or("Offset overflow")?)
        .ok_or_else(|| "Truncated image".into())
}
fn u16_at(data: &[u8], offset: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        bytes(data, offset, 2)?.try_into().unwrap(),
    ))
}
fn u32_at(data: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes(data, offset, 4)?.try_into().unwrap(),
    ))
}
struct Image<'a> {
    data: &'a [u8],
    sections: usize,
    count: usize,
    root: usize,
}
impl<'a> Image<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, String> {
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
    fn resource(&self, kind: u32, id: u32) -> Result<&[u8], String> {
        let kind = self.entry(0, Some(kind), true)?;
        let name = self.entry(kind, Some(id), true)?;
        let entry = self.root + self.entry(name, None, false)?;
        let offset = self.rva(u32_at(self.data, entry)?)?;
        bytes(self.data, offset, u32_at(self.data, entry + 4)? as usize)
    }
}
fn verify(path: &Path) -> Result<(), String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    let image = Image::parse(&data)?;
    let ico = include_bytes!("../../assets/juna.ico");
    let group = image.resource(14, 1)?;
    let count = u16_at(group, 4)?;
    if count != u16_at(ico, 4)? || count != 7 {
        return Err("Incorrect icon count".into());
    }
    for index in 0..count as usize {
        let original = bytes(ico, 6 + index * 16, 16)?;
        let entry = bytes(group, 6 + index * 14, 14)?;
        let icon = image.resource(3, u16_at(entry, 12)? as u32)?;
        if entry[..12] != original[..12]
            || icon
                != bytes(
                    ico,
                    u32_at(original, 12)? as usize,
                    u32_at(original, 8)? as usize,
                )?
        {
            return Err("Embedded icon differs from approved artwork".into());
        }
    }
    if image.resource(16, 1)?.is_empty() {
        return Err("Version resource missing".into());
    }
    println!("Verified Windows x64, native version resource and all seven approved icon images");
    Ok(())
}
fn main() {
    let result = std::env::args_os()
        .nth(1)
        .ok_or_else(|| "usage: release_verify EXE".into())
        .and_then(|path| verify(Path::new(&path)));
    if let Err(error) = result {
        eprintln!("release_verify: {error}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn truncated_images_return_errors() {
        for size in 0..256 {
            assert!(super::Image::parse(&vec![0; size]).is_err());
        }
    }
}
