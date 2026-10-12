//! Juna-owned portable palette data. This module performs no I/O and knows no VT state.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection_background: String,
    pub selection_foreground: String,
    pub ansi: [String; 16],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub colors: Colors,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
pub fn rgb(text: &str) -> Result<[f32; 3], String> {
    if text.len() != 7
        || !text.starts_with('#')
        || !text[1..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err(format!("Expected #RRGGBB, received {text:?}"));
    }
    Ok([
        u8::from_str_radix(&text[1..3], 16).unwrap() as f32 / 255.,
        u8::from_str_radix(&text[3..5], 16).unwrap() as f32 / 255.,
        u8::from_str_radix(&text[5..7], 16).unwrap() as f32 / 255.,
    ])
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}
impl Theme {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .source
            .as_ref()
            .is_some_and(|s| s.len() > 2048 || s.chars().any(char::is_control))
        {
            return Err("Invalid source metadata".into());
        }
        if self.schema_version != 1 {
            return Err("Unsupported theme schema version".into());
        }
        if !valid_id(&self.id) {
            return Err("Invalid theme ID".into());
        }
        if self.name.trim().is_empty()
            || self.name.chars().count() > 64
            || self.name.chars().any(char::is_control)
        {
            return Err("Theme name must contain 1–64 printable characters".into());
        }
        for c in self.fields() {
            rgb(c)?;
        }
        Ok(())
    }
    pub fn fields(&self) -> Vec<&str> {
        [
            &self.colors.foreground,
            &self.colors.background,
            &self.colors.cursor,
            &self.colors.selection_background,
            &self.colors.selection_foreground,
        ]
        .into_iter()
        .chain(self.colors.ansi.iter())
        .map(String::as_str)
        .collect()
    }
    pub fn set_color(&mut self, index: usize, value: String) -> Result<(), String> {
        rgb(&value)?;
        match index {
            0 => self.colors.foreground = value,
            1 => self.colors.background = value,
            2 => self.colors.cursor = value,
            3 => self.colors.selection_background = value,
            4 => self.colors.selection_foreground = value,
            5..=20 => self.colors.ansi[index - 5] = value,
            _ => return Err("Invalid color field".into()),
        }
        Ok(())
    }
}
pub fn builtins() -> Vec<Theme> {
    let data: Vec<Theme> = serde_json::from_str(include_str!("../assets/themes.json"))
        .expect("built-in palettes must parse");
    for t in &data {
        t.validate().expect("built-in palettes must validate");
    }
    data
}
pub fn default_theme() -> Theme {
    builtins().remove(0)
}
pub const FIELD_NAMES: [&str; 21] = [
    "Foreground",
    "Background",
    "Cursor",
    "Selection background",
    "Selection text",
    "ANSI black",
    "ANSI red",
    "ANSI green",
    "ANSI yellow",
    "ANSI blue",
    "ANSI magenta",
    "ANSI cyan",
    "ANSI white",
    "Bright black",
    "Bright red",
    "Bright green",
    "Bright yellow",
    "Bright blue",
    "Bright magenta",
    "Bright cyan",
    "Bright white",
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twenty_valid_unique_presets_and_dark_mocha() {
        let themes = builtins();
        assert_eq!(themes.len(), 20);
        let ids: std::collections::HashSet<_> = themes.iter().map(|t| &t.id).collect();
        assert_eq!(ids.len(), 20);
        assert_eq!(themes[0].id, "catppuccin-mocha");
        assert_eq!(themes[0].colors.background.to_lowercase(), "#1e1e2e");
    }
    #[test]
    fn malformed_colors_names_and_paths_are_rejected() {
        assert!(rgb("#ffffff").is_ok());
        for bad in ["#gg0000", "#000", "#éé000", "white"] {
            assert!(rgb(bad).is_err());
        }
        for id in ["../x", "x/y", "x:y", ""] {
            assert!(!valid_id(id));
        }
        let mut theme = default_theme();
        theme.schema_version = 2;
        assert!(theme.validate().is_err());
    }
}
