//! File I/O for application settings and custom palettes, outside the VT engine.
use crate::theme::{self, Theme};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub active_theme_id: String,
    pub font_family: String,
    pub font_size: f32,
    pub minimum_contrast: f32,
    #[serde(default)]
    pub show_line_numbers: bool,
    #[serde(default = "default_git_status")]
    pub show_git_status: bool,
    #[serde(default)]
    pub report_shell_directory: bool,
    #[serde(default = "default_junafetch_profile")]
    pub junafetch_profile: String,
    #[serde(default = "default_junafetch_logo")]
    pub junafetch_logo: String,
    #[serde(default)]
    pub favourite_theme_ids: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            active_theme_id: "catppuccin-mocha".into(),
            font_family: "IBM Plex Mono".into(),
            font_size: 16.,
            minimum_contrast: 4.5,
            show_line_numbers: false,
            show_git_status: true,
            report_shell_directory: false,
            junafetch_profile: default_junafetch_profile(),
            junafetch_logo: default_junafetch_logo(),
            favourite_theme_ids: Vec::new(),
        }
    }
}
pub fn default_git_status() -> bool {
    true
}
pub fn default_junafetch_profile() -> String {
    "standard".into()
}
pub fn default_junafetch_logo() -> String {
    "compact".into()
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.junafetch_profile.as_str(),
            "brief" | "standard" | "detailed"
        ) || !matches!(self.junafetch_logo.as_str(), "compact" | "full" | "none")
        {
            return Err("Invalid Junafetch preferences".into());
        }
        let unique: std::collections::BTreeSet<_> = self.favourite_theme_ids.iter().collect();
        if self.favourite_theme_ids.len() > 128
            || unique.len() != self.favourite_theme_ids.len()
            || self
                .favourite_theme_ids
                .iter()
                .any(|id| !theme::valid_id(id))
        {
            return Err("Invalid theme favourites".into());
        }
        if self.schema_version != 1 || !theme::valid_id(&self.active_theme_id) {
            return Err("Unsupported or invalid settings".into());
        }
        if !(8.0..=40.0).contains(&self.font_size)
            || !(1.0..=21.0).contains(&self.minimum_contrast)
            || self.font_family.trim().is_empty()
            || self.font_family.len() > 128
            || self.font_family.chars().any(char::is_control)
        {
            return Err("Invalid font or contrast settings".into());
        }
        Ok(())
    }
}
pub fn application_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Juna")
}
pub fn new_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "custom-{n:032x}-{:x}-{:x}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
pub struct Store {
    pub directory: PathBuf,
}
impl Store {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    pub fn load(&self) -> (Settings, Vec<Theme>, Vec<String>) {
        let mut errors = Vec::new();
        let mut settings = Settings::default();
        let file = self.directory.join("settings.json");
        if file.exists() {
            match read_json::<Settings>(&file).and_then(|v| {
                v.validate()?;
                Ok(v)
            }) {
                Ok(v) => settings = v,
                Err(e) => errors.push(e),
            }
        }
        let library = crate::theme_library::scan(&self.directory);
        let themes = library.themes;
        errors.extend(library.errors);
        if themes.iter().all(|t| t.id != settings.active_theme_id) {
            errors.push("Selected theme is unavailable; using Catppuccin Mocha".into());
            settings.active_theme_id = "catppuccin-mocha".into();
        }
        (settings, themes, errors)
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        settings.validate()?;
        write_json(&self.directory.join("settings.json"), settings)
    }
    pub fn save_theme(&self, theme: &Theme) -> Result<(), String> {
        theme.validate()?;
        if !theme.id.starts_with("custom-") {
            return Err("Built-in themes are immutable; duplicate first".into());
        }
        write_json(
            &self
                .directory
                .join("themes")
                .join(crate::theme_library::folder_name(&theme.family_name()))
                .join(format!("{}.json", theme.id)),
            theme,
        )
    }
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err(format!("{} exceeds 64 KiB", path.display()));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.permissions().readonly() => {
            return Err("Target file is read-only".into())
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.to_string()),
        _ => {}
    }
    let directory = path.parent().ok_or("Missing settings directory")?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let temp = directory.join(format!(".{}.tmp", new_id()));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|e: std::io::Error| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_themes_round_trip_and_corruption_does_not_overwrite_source() {
        let dir = std::env::temp_dir().join(new_id());
        let store = Store::new(dir.clone());
        let mut t = theme::default_theme();
        t.id = new_id();
        t.name = "My ../ theme".into();
        store.save_theme(&t).unwrap();
        t.colors.background = "#101018".into();
        store.save_theme(&t).unwrap();
        let settings = Settings {
            active_theme_id: t.id.clone(),
            ..Default::default()
        };
        store.save_settings(&settings).unwrap();
        let (s, themes, errors) = store.load();
        assert!(errors.is_empty());
        assert_eq!(s.active_theme_id, t.id);
        assert_eq!(themes.last().unwrap(), &t);
        let bad = dir.join("themes/bad.json");
        std::fs::write(&bad, b"invalid").unwrap();
        let (_, _, errors) = store.load();
        assert!(!errors.is_empty());
        assert_eq!(std::fs::read(&bad).unwrap(), b"invalid");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_write_preserves_existing_settings() {
        let dir = std::env::temp_dir().join(new_id());
        let store = Store::new(dir.clone());
        store.save_settings(&Settings::default()).unwrap();
        let before = std::fs::read(dir.join("settings.json")).unwrap();
        let invalid = Settings {
            font_size: f32::NAN,
            ..Default::default()
        };
        assert!(store.save_settings(&invalid).is_err());
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), before);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unsupported_and_duplicate_custom_records_report_errors() {
        let dir = std::env::temp_dir().join(new_id());
        let store = Store::new(dir.clone());
        let mut theme = theme::default_theme();
        theme.id = new_id();
        store.save_theme(&theme).unwrap();
        let path = crate::theme_library::scan(&dir).sources[&theme.id].clone();
        std::fs::copy(path, dir.join("themes/duplicate.json")).unwrap();
        let (_, themes, errors) = store.load();
        assert_eq!(themes.len(), 61);
        assert!(!errors.is_empty());
        let bad = dir.join("themes/future.json");
        theme.schema_version = 2;
        std::fs::write(&bad, serde_json::to_vec(&theme).unwrap()).unwrap();
        let (_, _, errors) = store.load();
        assert!(errors.iter().any(|e| e.contains("Unsupported")));
        assert!(bad.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn a_read_only_settings_file_is_not_destroyed() {
        let dir = std::env::temp_dir().join(new_id());
        let store = Store::new(dir.clone());
        store.save_settings(&Settings::default()).unwrap();
        let file = dir.join("settings.json");
        let bytes = std::fs::read(&file).unwrap();
        let original_permissions = std::fs::metadata(&file).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        std::fs::set_permissions(&file, permissions.clone()).unwrap();
        let result = store.save_settings(&Settings {
            font_size: 20.,
            ..Default::default()
        });
        let preserved = std::fs::read(&file).unwrap();
        std::fs::set_permissions(&file, original_permissions).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
        assert!(result.is_err());
        assert_eq!(preserved, bytes);
    }
}
