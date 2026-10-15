//! Validate release resources using Juna's shared original PE reader.
use juna::release_image::{bytes, u16_at, u32_at, Image};
use std::{fs, path::Path};
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
