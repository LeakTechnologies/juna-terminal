//! Explicit portable-settings allowlist and validated, non-executable JSON transfer.
use crate::{
    settings::{Settings, Store},
    theme::Theme,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
};
const MAX_BYTES: u64 = 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableSettings {
    active_theme_id: String,
    font_family: String,
    font_size: f32,
    minimum_contrast: f32,
    #[serde(default)]
    show_line_numbers: bool,
    #[serde(default)]
    favourite_theme_ids: Vec<String>,
}
impl From<&Settings> for PortableSettings {
    fn from(s: &Settings) -> Self {
        Self {
            active_theme_id: s.active_theme_id.clone(),
            font_family: s.font_family.clone(),
            font_size: s.font_size,
            minimum_contrast: s.minimum_contrast,
            show_line_numbers: s.show_line_numbers,
            favourite_theme_ids: s.favourite_theme_ids.clone(),
        }
    }
}
impl PortableSettings {
    fn settings(&self) -> Settings {
        Settings {
            active_theme_id: self.active_theme_id.clone(),
            font_family: self.font_family.clone(),
            font_size: self.font_size,
            minimum_contrast: self.minimum_contrast,
            show_line_numbers: self.show_line_numbers,
            favourite_theme_ids: self.favourite_theme_ids.clone(),
            ..Default::default()
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    schema_version: u32,
    settings: PortableSettings,
    themes: Vec<Theme>,
}
impl Bundle {
    pub fn capture(settings: &Settings, themes: &[Theme]) -> Self {
        let public_sources: BTreeSet<_> = crate::theme::builtins()
            .into_iter()
            .filter_map(|t| t.source)
            .collect();
        let mut needed: BTreeSet<_> = settings.favourite_theme_ids.iter().collect();
        needed.insert(&settings.active_theme_id);
        Self {
            schema_version: 1,
            settings: settings.into(),
            themes: themes
                .iter()
                .filter(|t| t.id.starts_with("custom-") && needed.contains(&t.id))
                .cloned()
                .map(|mut theme| {
                    if theme
                        .source
                        .as_ref()
                        .is_some_and(|source| !public_sources.contains(source))
                    {
                        theme.source = None;
                    }
                    theme
                })
                .collect(),
        }
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES as usize {
            return Err("Settings import exceeds 1 MiB".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "Invalid settings JSON")?;
        let bundle = if value.get("settings").is_some() {
            serde_json::from_value(value).map_err(|_| "Unsupported settings bundle")?
        } else {
            let settings: Settings =
                serde_json::from_value(value).map_err(|_| "Unsupported settings JSON")?;
            settings.validate()?;
            Self::capture(&settings, &[])
        };
        bundle.validate()?;
        Ok(bundle)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.themes.len() > 128 {
            return Err("Unsupported settings bundle".into());
        }
        self.settings.settings().validate()?;
        let mut ids = BTreeSet::new();
        for theme in &self.themes {
            theme.validate()?;
            if !theme.id.starts_with("custom-") || !ids.insert(&theme.id) {
                return Err("Invalid or duplicate custom themes".into());
            }
        }
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::decode(&bytes)
    }
    pub fn export(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES as usize {
            return Err("Settings export exceeds 1 MiB".into());
        }
        crate::settings::write_json(path, self)
    }
    pub fn prepare(&self, existing: &[Theme]) -> Result<ImportPlan, String> {
        self.validate()?;
        let mut settings = self.settings.settings();
        if !crate::fonts::choices().contains(&settings.font_family) {
            return Err("Imported font is unavailable here; export with a bundled font".into());
        }
        let mut remap = BTreeMap::new();
        let mut themes = Vec::new();
        let mut conflicts = 0;
        for incoming in &self.themes {
            if existing.iter().any(|t| t == incoming) {
                continue;
            }
            if existing.iter().any(|t| t.id == incoming.id) {
                conflicts += 1;
            }
            let mut copied = incoming.clone();
            copied.id = crate::settings::new_id();
            remap.insert(incoming.id.clone(), copied.id.clone());
            themes.push(copied);
        }
        if let Some(id) = remap.get(&settings.active_theme_id) {
            settings.active_theme_id = id.clone();
        }
        for id in &mut settings.favourite_theme_ids {
            if let Some(new) = remap.get(id) {
                *id = new.clone();
            }
        }
        if !existing
            .iter()
            .chain(themes.iter())
            .any(|t| t.id == settings.active_theme_id)
        {
            return Err(
                "Imported active theme is missing; include it in the exported bundle".into(),
            );
        }
        settings.validate()?;
        Ok(ImportPlan {
            settings,
            themes,
            conflicts,
        })
    }
}
pub struct ImportPlan {
    pub settings: Settings,
    pub themes: Vec<Theme>,
    pub conflicts: usize,
}
fn linked(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
impl ImportPlan {
    pub fn apply(&self, store: &Store) -> Result<(), String> {
        let library = crate::theme_library::scan(&store.directory);
        if library.sources.len() + library.errors.len() + self.themes.len() > 128 {
            return Err("Theme import would exceed the discovery limit".into());
        }
        let root = store.directory.join("themes");
        if std::fs::symlink_metadata(&root).is_ok_and(|m| linked(&m)) {
            return Err("Cannot import into a linked theme folder".into());
        }
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
        let mut written: Vec<PathBuf> = Vec::new();
        let result = (|| {
            for theme in &self.themes {
                let family = root.join(crate::theme_library::folder_name(&theme.family_name()));
                if std::fs::symlink_metadata(&family).is_ok_and(|m| linked(&m)) {
                    return Err("Cannot import into a linked family folder".into());
                }
                std::fs::create_dir_all(&family).map_err(|e| e.to_string())?;
                if !family
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .starts_with(&canonical_root)
                {
                    return Err("Theme import must stay in the app folder".into());
                }
                let file = family.join(format!("{}.json", theme.id));
                if file.exists() {
                    return Err("Imported theme filename already exists".into());
                }
                crate::settings::write_json(&file, theme)?;
                written.push(file);
            }
            store.save_settings(&self.settings)
        })();
        if result.is_err() {
            for file in written {
                std::fs::remove_file(file)
                    .map_err(|e| format!("Import failed; staged-theme cleanup also failed: {e}"))?;
            }
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_preference_write_rolls_back_staged_themes_and_export_omits_private_source() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let store = Store::new(root.clone());
        store.save_settings(&Settings::default()).unwrap();
        let mut theme = crate::theme::default_theme();
        theme.id = "custom-rollback".into();
        theme.source = Some("file:///PRIVATE/path?token=secret".into());
        let settings = Settings {
            active_theme_id: theme.id.clone(),
            ..Default::default()
        };
        let bundle = Bundle::capture(&settings, &[theme]);
        let encoded = serde_json::to_vec(&bundle).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("PRIVATE"));
        assert!(!String::from_utf8_lossy(&encoded).contains("secret"));
        let plan = bundle.prepare(&store.load().1).unwrap();
        let file = root.join("settings.json");
        let original = std::fs::read(&file).unwrap();
        let original_permissions = std::fs::metadata(&file).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        std::fs::set_permissions(&file, permissions).unwrap();
        assert!(plan.apply(&store).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), original);
        assert!(!store
            .load()
            .1
            .iter()
            .any(|t| t.id == plan.settings.active_theme_id));
        std::fs::set_permissions(&file, original_permissions).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn portable_round_trip_remaps_conflicts_and_preserves_originals() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let store = Store::new(root.clone());
        let mut original = crate::theme::default_theme();
        original.id = "custom-transfer".into();
        original.name = "Original".into();
        store.save_theme(&original).unwrap();
        let mut incoming = original.clone();
        incoming.name = "Imported".into();
        let settings = Settings {
            active_theme_id: incoming.id.clone(),
            favourite_theme_ids: vec![incoming.id.clone()],
            font_size: 18.,
            ..Default::default()
        };
        let bundle = Bundle::capture(&settings, &[incoming]);
        let encoded = serde_json::to_vec(&bundle).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("history"));
        let plan = Bundle::decode(&encoded)
            .unwrap()
            .prepare(&store.load().1)
            .unwrap();
        assert_eq!(plan.conflicts, 1);
        assert_ne!(plan.settings.active_theme_id, original.id);
        plan.apply(&store).unwrap();
        let (saved, themes, errors) = store.load();
        assert!(errors.is_empty());
        assert_eq!(saved.font_size, 18.);
        assert_eq!(saved.favourite_theme_ids, vec![saved.active_theme_id]);
        assert!(themes.iter().any(|t| t == &original));
        assert!(Bundle::decode(&vec![0; MAX_BYTES as usize + 1]).is_err());
        assert!(Bundle::decode(br#"{"schema_version":99,"settings":{},"themes":[]}"#).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
