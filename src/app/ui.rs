//! Application-owned menus and forms. Pure geometry/data, no session or terminal access.
use crate::{
    render::overlay::{Overlay, Rectangle, Text},
    settings::Settings,
    theme::{Theme, FIELD_NAMES},
};
use winit::keyboard::{Key, NamedKey};
pub const TAB_HEIGHT: f32 = 32.;
pub fn numeric_settings(settings: &Settings, index: usize, text: &str) -> Result<Settings, String> {
    let mut result = settings.clone();
    let value = text.trim().to_ascii_lowercase();
    match index {
        1 => {
            let (number, multiplier) = if let Some(s) = value.strip_suffix("pt") {
                (s.trim(), 96. / 72.)
            } else {
                (value.strip_suffix("px").unwrap_or(&value).trim(), 1.)
            };
            result.font_size = number
                .parse::<f32>()
                .map_err(|_| "Enter a size, such as 16px or 12pt")?
                * multiplier;
            if !(8.0..=40.0).contains(&result.font_size) {
                return Err("Use 8–40px or 6–30pt".into());
            }
        }
        2 => {
            result.minimum_contrast = value
                .strip_suffix(":1")
                .unwrap_or(&value)
                .trim()
                .parse()
                .map_err(|_| "Enter a contrast ratio from 1 to 21")?;
        }
        _ => return Err("Unknown numeric setting".into()),
    }
    result.validate()?;
    Ok(result)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TabHit {
    Select(usize),
    New,
}
pub fn tab_hit(
    point: [f32; 2],
    width: f32,
    scale: f32,
    count: usize,
    active: usize,
) -> Option<TabHit> {
    if point[1] < 0. || point[1] >= TAB_HEIGHT * scale {
        return None;
    }
    let plus = 40. * scale;
    if point[0] >= width - plus {
        return Some(TabHit::New);
    }
    let visible = (((width - plus) / (160. * scale)).floor() as usize)
        .max(1)
        .min(count.max(1));
    let start = active
        .saturating_sub(visible - 1)
        .min(count.saturating_sub(visible));
    let step = (width - plus) / visible as f32;
    let index = (point[0].max(0.) / step) as usize + start;
    (index < count).then_some(TabHit::Select(index))
}
pub fn tab_scene(labels: &[String], active: usize, width: f32, scale: f32) -> Overlay {
    let mut out = Overlay::default();
    out.rectangles.push(Rectangle {
        rect: [0., 0., width, TAB_HEIGHT * scale],
        color: [0.06, 0.06, 0.09, 1.],
    });
    let plus = 40. * scale;
    let visible = (((width - plus) / (160. * scale)).floor() as usize)
        .max(1)
        .min(labels.len().max(1));
    let start = active
        .saturating_sub(visible - 1)
        .min(labels.len().saturating_sub(visible));
    let step = (width - plus) / visible as f32;
    for (i, label) in labels.iter().enumerate().skip(start).take(visible) {
        let x = (i - start) as f32 * step;
        if i == active {
            out.rectangles.push(Rectangle {
                rect: [x, 0., step - 2., TAB_HEIGHT * scale],
                color: [0.18, 0.19, 0.28, 1.],
            });
        }
        out.text.push(Text {
            pos: [x + 10. * scale, 5. * scale],
            width: (step - 20. * scale).max(0.),
            value: label.clone(),
            color: [0.9, 0.9, 0.98, 1.],
        });
    }
    out.text.push(Text {
        pos: [width - plus + 10. * scale, 5. * scale],
        width: plus,
        value: "+".into(),
        color: [0.9, 0.9, 0.98, 1.],
    });
    out
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    ApplySetting,
    CancelSetting,
    Custom,
    ToggleFamily(String),
    OpenThemesFolder,
    RemoveTheme,
    RestoreTheme,
    Family,
    ManageTheme(String),
    NewTab,
    CloseTab,
    Copy,
    Paste,
    SelectAll,
    Themes,
    Settings,
    Fonts,
    PickFont(String),
    SaveSettings,
    Resize(i8),
    Pick(String),
    Duplicate,
    Edit,
    Save,
    Cancel,
    Search,
    Setting(usize),
    Color(usize),
    Name,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Context,
    Themes,
    Settings,
    Fonts,
    Editor,
    Number,
    Custom,
    Manage,
}
pub struct Ui {
    pub kind: Kind,
    pub focus: usize,
    pub scroll: usize,
    pub query: String,
    pub buffer: String,
    pub draft: Option<Theme>,
    pub message: String,
    pub anchor: [f32; 2],
    select_all: bool,
    pub number: Option<usize>,
    pub draft_value: String,
    pub prior_settings: Option<Settings>,
    caret: usize,
    pub expanded: std::collections::BTreeSet<String>,
    pub preview_id: Option<String>,
}
pub struct Row {
    pub label: String,
    pub action: Action,
    pub value: Option<String>,
}
impl Ui {
    pub fn set_number_value(&mut self, value: String) {
        self.draft_value = value.clone();
        if self.focus == 0 {
            self.buffer = value;
            self.caret = self.buffer.len();
            self.select_all = true;
        }
    }
    pub fn new(kind: Kind, anchor: [f32; 2]) -> Self {
        Self {
            kind,
            focus: 0,
            scroll: 0,
            query: String::new(),
            buffer: String::new(),
            draft: None,
            message: String::new(),
            anchor,
            select_all: true,
            number: None,
            draft_value: String::new(),
            prior_settings: None,
            caret: 0,
            expanded: std::collections::BTreeSet::new(),
            preview_id: None,
        }
    }
    pub fn rows(&self, themes: &[Theme], settings: &Settings) -> Vec<Row> {
        let row = |label: String, action: Action, value: Option<String>| Row {
            label,
            action,
            value,
        };
        match self.kind {
            Kind::Manage => {
                let id = self
                    .preview_id
                    .clone()
                    .unwrap_or_else(|| settings.active_theme_id.clone());
                vec![
                    row("Use this theme".into(), Action::Pick(id), None),
                    row("Edit theme...".into(), Action::Edit, None),
                    row("Duplicate theme...".into(), Action::Duplicate, None),
                    row("Move file to trash".into(), Action::RemoveTheme, None),
                    row("Back to Custom".into(), Action::Custom, None),
                ]
            }
            Kind::Number => vec![
                row(
                    if self.number == Some(1) {
                        "Size (px or pt)".into()
                    } else {
                        "Contrast ratio".into()
                    },
                    Action::ApplySetting,
                    Some(self.draft_value.clone()),
                ),
                row("Apply".into(), Action::ApplySetting, None),
                row("Cancel".into(), Action::CancelSetting, None),
            ],
            Kind::Context => [
                Action::NewTab,
                Action::CloseTab,
                Action::Copy,
                Action::Paste,
                Action::SelectAll,
                Action::Themes,
                Action::Settings,
            ]
            .into_iter()
            .zip([
                "New tab   Ctrl+Shift+T",
                "Close tab Ctrl+Shift+W",
                "Copy   Ctrl+Shift+C",
                "Paste  Ctrl+Shift+V",
                "Select all",
                "Themes...",
                "Settings...",
            ])
            .map(|(a, l)| row(l.into(), a, None))
            .collect(),
            Kind::Themes | Kind::Custom => {
                let custom = self.kind == Kind::Custom;
                let mut rows = vec![row(
                    format!("Search: {}", self.query),
                    Action::Search,
                    Some(self.query.clone()),
                )];
                if custom {
                    rows.push(row(
                        "Create theme from current...".into(),
                        Action::Duplicate,
                        None,
                    ));
                    rows.push(row(
                        "Open themes folder".into(),
                        Action::OpenThemesFolder,
                        None,
                    ));
                }
                if !custom {
                    rows.push(row(
                        format!(
                            "Custom themes ({})...",
                            themes
                                .iter()
                                .filter(|t| t.id.starts_with("custom-"))
                                .count()
                        ),
                        Action::Custom,
                        None,
                    ));
                }
                let mut groups: std::collections::BTreeMap<String, Vec<&Theme>> =
                    std::collections::BTreeMap::new();
                for theme in themes
                    .iter()
                    .filter(|t| t.id.starts_with("custom-") == custom)
                    .filter(|t| {
                        self.query.is_empty()
                            || t.name.to_lowercase().contains(&self.query.to_lowercase())
                            || t.family_name()
                                .to_lowercase()
                                .contains(&self.query.to_lowercase())
                    })
                {
                    groups.entry(theme.family_name()).or_default().push(theme);
                }
                for (family, variants) in groups {
                    let expanded = self.expanded.contains(&family) || !self.query.is_empty();
                    rows.push(row(
                        format!(
                            "{} {} ({})",
                            if expanded { "[-]" } else { "[+]" },
                            family,
                            variants.len()
                        ),
                        Action::ToggleFamily(family.clone()),
                        None,
                    ));
                    if expanded {
                        for theme in variants {
                            let short = theme
                                .name
                                .strip_prefix(&family)
                                .unwrap_or(&theme.name)
                                .trim();
                            let short = if short.is_empty() { "Default" } else { short };
                            rows.push(row(
                                format!(
                                    "  {} {}",
                                    if theme.id == settings.active_theme_id {
                                        "*"
                                    } else {
                                        " "
                                    },
                                    short
                                ),
                                if custom {
                                    Action::ManageTheme(theme.id.clone())
                                } else {
                                    Action::Pick(theme.id.clone())
                                },
                                None,
                            ));
                        }
                    }
                }
                if custom {
                    rows.push(row(
                        "Restore last removal".into(),
                        Action::RestoreTheme,
                        None,
                    ));
                    rows.push(row("Back to starter themes".into(), Action::Themes, None));
                }
                rows
            }
            Kind::Fonts => {
                let mut rows = vec![row(
                    format!("Search: {}", self.query),
                    Action::Search,
                    Some(self.query.clone()),
                )];
                rows.extend(
                    crate::fonts::choices()
                        .iter()
                        .filter(|name| name.to_lowercase().contains(&self.query.to_lowercase()))
                        .map(|name| {
                            row(
                                format!(
                                    "{} {}",
                                    if name == &settings.font_family {
                                        "[*]"
                                    } else {
                                        "[ ]"
                                    },
                                    name
                                ),
                                Action::PickFont(name.clone()),
                                None,
                            )
                        }),
                );
                rows.push(row("Back to settings".into(), Action::Settings, None));
                rows
            }
            Kind::Settings => vec![
                row(
                    format!("Font: {}", settings.font_family),
                    Action::Fonts,
                    None,
                ),
                row(
                    format!("Font size: {} px", settings.font_size),
                    Action::Setting(1),
                    None,
                ),
                row(
                    format!("Minimum contrast: {}:1", settings.minimum_contrast),
                    Action::Setting(2),
                    None,
                ),
                row("Edit theme...".into(), Action::Edit, None),
                row("Theme selector...".into(), Action::Themes, None),
                row("Save settings  Ctrl+S".into(), Action::SaveSettings, None),
                row("Cancel".into(), Action::Cancel, None),
            ],
            Kind::Editor => {
                let t = self.draft.as_ref().expect("editor draft");
                let mut rows = vec![row(
                    format!("Name: {}", t.name),
                    Action::Name,
                    Some(t.name.clone()),
                )];
                rows.push(row(
                    format!("Family: {}", t.family_name()),
                    Action::Family,
                    Some(t.family_name()),
                ));
                rows.extend(t.fields().into_iter().zip(FIELD_NAMES).enumerate().map(
                    |(i, (v, label))| {
                        row(
                            format!("{label}: {v}"),
                            Action::Color(i),
                            Some(v.to_owned()),
                        )
                    },
                ));
                rows.push(row("Save custom theme  Ctrl+S".into(), Action::Save, None));
                rows.push(row("Cancel".into(), Action::Cancel, None));
                rows
            }
        }
    }
    pub fn geometry(
        &self,
        width: f32,
        height: f32,
        scale: f32,
        len: usize,
    ) -> ([f32; 4], usize, f32) {
        let step = 26. * scale;
        let w = (500. * scale).min((width - 16.).max(1.));
        let visible = (((height - 100. * scale).max(step) / step) as usize)
            .max(1)
            .min(len.max(1));
        let h = (visible as f32 * step + 78. * scale).min(height.max(1.));
        let anchor_x = if self.kind == Kind::Context {
            self.anchor[0]
        } else {
            width - w - 16. * scale
        };
        let x = anchor_x.clamp(0., (width - w).max(0.));
        let anchor_y = if self.kind == Kind::Context {
            self.anchor[1]
        } else {
            (TAB_HEIGHT + 16.) * scale
        };
        let y = anchor_y.clamp(0., (height - h).max(0.));
        ([x, y, w, h], visible, step)
    }
    pub fn ensure_visible(&mut self, visible: usize, len: usize) {
        self.focus = self.focus.min(len.saturating_sub(1));
        if self.focus < self.scroll {
            self.scroll = self.focus;
        } else if self.focus >= self.scroll + visible {
            self.scroll = self.focus + 1 - visible;
        }
        self.scroll = self.scroll.min(len.saturating_sub(visible));
    }
    pub fn hit(
        &self,
        point: [f32; 2],
        width: f32,
        height: f32,
        scale: f32,
        len: usize,
    ) -> Option<usize> {
        let (r, n, step) = self.geometry(width, height, scale, len);
        let y = point[1] - r[1] - 36. * scale;
        if point[0] < r[0] || point[0] >= r[0] + r[2] || y < 0. {
            return None;
        }
        let index = (y / step) as usize;
        if index >= n {
            return None;
        }
        let index = index + self.scroll;
        (index < len).then_some(index)
    }
    pub fn focus_row(&mut self, index: usize, rows: &[Row]) {
        self.focus = index.min(rows.len().saturating_sub(1));
        self.buffer = rows
            .get(self.focus)
            .and_then(|r| r.value.clone())
            .unwrap_or_default();
        self.select_all = true;
        self.caret = self.buffer.len();
    }
    pub fn paste_text(&mut self, text: &str) {
        if self.select_all {
            self.buffer.clear();
            self.caret = 0;
            self.select_all = false;
        }
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            if self.buffer.len() + ch.len_utf8() > 256 {
                break;
            }
            self.buffer.insert(self.caret, ch);
            self.caret += ch.len_utf8();
        }
        self.sync_text();
    }
    fn sync_text(&mut self) {
        if self.kind == Kind::Number && self.focus == 0 {
            self.draft_value = self.buffer.clone();
        }
        if matches!(self.kind, Kind::Themes | Kind::Custom | Kind::Fonts) {
            self.query = self.buffer.clone();
            self.focus = 0;
            self.scroll = 0;
        }
    }
    pub fn number(index: usize, settings: &Settings, anchor: [f32; 2]) -> Self {
        let mut ui = Self::new(Kind::Number, anchor);
        ui.number = Some(index);
        ui.prior_settings = Some(settings.clone());
        ui.draft_value = if index == 1 {
            settings.font_size.to_string()
        } else {
            settings.minimum_contrast.to_string()
        };
        ui.buffer = ui.draft_value.clone();
        ui.caret = ui.buffer.len();
        ui
    }
    /// Consumes all keyboard events while open. Returned actions are UI-only.
    pub fn key(
        &mut self,
        key: &Key,
        text: Option<&str>,
        control: bool,
        rows: &[Row],
    ) -> Option<Action> {
        match key {
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight)
                if rows
                    .get(self.focus)
                    .is_some_and(|r| matches!(r.action, Action::ToggleFamily(_))) =>
            {
                if let Some(Action::ToggleFamily(family)) = rows.get(self.focus).map(|r| &r.action)
                {
                    let open = self.expanded.contains(family);
                    if (matches!(key, Key::Named(NamedKey::ArrowRight)) && !open)
                        || (matches!(key, Key::Named(NamedKey::ArrowLeft)) && open)
                    {
                        return Some(Action::ToggleFamily(family.clone()));
                    }
                }
                return None;
            }
            Key::Named(NamedKey::Space)
                if rows.get(self.focus).is_some_and(|row| row.value.is_none()) =>
            {
                return rows.get(self.focus).map(|row| row.action.clone())
            }
            Key::Character(text)
                if text == " " && rows.get(self.focus).is_some_and(|row| row.value.is_none()) =>
            {
                return rows.get(self.focus).map(|row| row.action.clone())
            }
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight)
                if self.kind == Kind::Settings
                    && rows
                        .get(self.focus)
                        .is_some_and(|r| r.action == Action::Setting(1)) =>
            {
                return Some(Action::Resize(
                    if matches!(key, Key::Named(NamedKey::ArrowRight)) {
                        1
                    } else {
                        -1
                    },
                ));
            }
            Key::Named(NamedKey::Escape) => {
                return Some(if self.kind == Kind::Number {
                    Action::CancelSetting
                } else {
                    Action::Cancel
                })
            }
            Key::Named(NamedKey::ArrowDown | NamedKey::Tab) => {
                self.focus_row((self.focus + 1) % rows.len().max(1), rows);
                return None;
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.focus_row(self.focus.saturating_sub(1), rows);
                return None;
            }
            Key::Named(NamedKey::Enter) => return rows.get(self.focus).map(|r| r.action.clone()),
            Key::Character(s)
                if control
                    && s.eq_ignore_ascii_case("s")
                    && matches!(self.kind, Kind::Settings | Kind::Fonts) =>
            {
                return Some(Action::SaveSettings);
            }
            Key::Character(s)
                if control && s.eq_ignore_ascii_case("s") && self.kind == Kind::Editor =>
            {
                return Some(Action::Save)
            }
            Key::Character(s) if control && s.eq_ignore_ascii_case("a") => {
                self.select_all = true;
                return None;
            }
            _ => {}
        }
        let editable = matches!(self.kind, Kind::Themes | Kind::Custom | Kind::Fonts)
            || rows.get(self.focus).is_some_and(|r| r.value.is_some());
        if !editable || control {
            return None;
        }
        match key {
            Key::Named(NamedKey::ArrowLeft) => {
                self.caret = self.buffer[..self.caret]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                self.select_all = false;
            }
            Key::Named(NamedKey::ArrowRight) => {
                if let Some(ch) = self.buffer[self.caret..].chars().next() {
                    self.caret += ch.len_utf8();
                }
                self.select_all = false;
            }
            Key::Named(NamedKey::Home) => {
                self.caret = 0;
                self.select_all = false;
            }
            Key::Named(NamedKey::End) => {
                self.caret = self.buffer.len();
                self.select_all = false;
            }
            Key::Named(NamedKey::Backspace) => {
                if self.select_all {
                    self.buffer.clear();
                    self.caret = 0;
                } else if self.caret > 0 {
                    let at = self.buffer[..self.caret]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.buffer.drain(at..self.caret);
                    self.caret = at;
                }
                self.select_all = false;
            }
            Key::Named(NamedKey::Delete) => {
                if self.select_all {
                    self.buffer.clear();
                    self.caret = 0;
                } else if let Some(ch) = self.buffer[self.caret..].chars().next() {
                    self.buffer.drain(self.caret..self.caret + ch.len_utf8());
                }
                self.select_all = false;
            }
            _ => {
                if let Some(text) = text.or_else(|| match key {
                    Key::Character(s) => Some(s.as_str()),
                    Key::Named(NamedKey::Space) => Some(" "),
                    _ => None,
                }) {
                    self.paste_text(text);
                }
            }
        }
        self.sync_text();
        None
    }
    pub fn scene(&self, rows: &[Row], width: f32, height: f32, scale: f32) -> Overlay {
        let (r, n, step) = self.geometry(width, height, scale, rows.len());
        let mut out = Overlay::default();
        out.rectangles.push(Rectangle {
            rect: [r[0] + 3. * scale, r[1] + 3. * scale, r[2], r[3]],
            color: [0., 0., 0., 0.7],
        });
        out.rectangles.push(Rectangle {
            rect: r,
            color: [0.075, 0.075, 0.115, 1.],
        });
        let title = match self.kind {
            Kind::Context => "Juna",
            Kind::Themes => "Theme families - live preview",
            Kind::Custom => "Custom themes - live preview",
            Kind::Fonts => "Fonts - live preview",
            Kind::Settings => "Settings - live preview",
            Kind::Editor => "Custom theme - live preview",
            Kind::Number => {
                if self.number == Some(1) {
                    "Font size - live preview"
                } else {
                    "Text contrast - live preview"
                }
            }
            Kind::Manage => "Manage custom theme",
        };
        out.text.push(Text {
            pos: [r[0] + 12. * scale, r[1] + 8. * scale],
            width: r[2] - 24. * scale,
            value: title.into(),
            color: [0.9, 0.9, 0.98, 1.],
        });
        for (i, row) in rows.iter().enumerate().skip(self.scroll).take(n) {
            let y = r[1] + 36. * scale + (i - self.scroll) as f32 * step;
            let selected = i == self.focus;
            if selected {
                out.rectangles.push(Rectangle {
                    rect: [r[0] + 6. * scale, y, r[2] - 12. * scale, step],
                    color: [0.20, 0.22, 0.34, 1.],
                });
            }
            let value = if selected && row.value.is_some() && self.kind != Kind::Themes {
                format!(
                    "{} > {}|{}",
                    row.label.split(':').next().unwrap_or("Value"),
                    &self.buffer[..self.caret],
                    &self.buffer[self.caret..]
                )
            } else {
                row.label.clone()
            };
            out.text.push(Text {
                pos: [r[0] + 12. * scale, y + 2. * scale],
                width: r[2] - 24. * scale,
                value,
                color: if selected {
                    [1., 1., 1., 1.]
                } else {
                    [0.78, 0.8, 0.9, 1.]
                },
            });
        }
        let footer = if self.message.is_empty() {
            match self.kind {
                Kind::Context => "Enter selects | Esc closes",
                Kind::Themes | Kind::Custom => "Search | Space expands | Enter selects",
                Kind::Manage => "Enter selects | Esc closes",
                Kind::Settings => "Left/Right size | Ctrl+S saves | Esc cancels",
                Kind::Fonts => "Search | Enter picks | Esc cancels",
                Kind::Editor => "Ctrl+S saves | Esc cancels",
                Kind::Number => "Enter applies | Esc returns",
            }
        } else {
            &self.message
        };
        out.text.push(Text {
            pos: [r[0] + 12. * scale, r[1] + r[3] - 28. * scale],
            width: r[2] - 24. * scale,
            value: footer.to_owned(),
            color: [0.85, 0.82, 0.7, 1.],
        });
        out
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn numeric_prompts_parse_points_edit_at_caret_and_activate_from_space() {
        let settings = Settings::default();
        assert!(
            (numeric_settings(&settings, 1, "16pt").unwrap().font_size - 21.333334).abs() < 0.001
        );
        for bad in ["NaN", "0", "100pt", "invalid"] {
            assert!(numeric_settings(&settings, 1, bad).is_err());
        }
        let mut ui = Ui::new(Kind::Settings, [0., 0.]);
        let rows = ui.rows(&[], &settings);
        ui.focus_row(1, &rows);
        assert_eq!(
            ui.key(&Key::Named(NamedKey::Space), None, false, &rows),
            Some(Action::Setting(1))
        );
        let mut ui = Ui::number(1, &settings, [0., 0.]);
        let rows = ui.rows(&[], &settings);
        ui.key(&Key::Named(NamedKey::End), None, false, &rows);
        ui.key(&Key::Named(NamedKey::ArrowLeft), None, false, &rows);
        ui.key(&Key::Character("2".into()), Some("2"), false, &rows);
        assert_eq!(ui.draft_value, "126");
        ui.key(&Key::Named(NamedKey::Backspace), None, false, &rows);
        assert_eq!(ui.draft_value, "16");
        assert_eq!(
            ui.key(&Key::Named(NamedKey::Escape), None, false, &rows),
            Some(Action::CancelSetting)
        );
    }
    #[test]
    fn starter_families_are_collapsed_and_custom_records_have_their_own_menu() {
        let mut themes = crate::theme::builtins();
        let mut custom = themes[0].clone();
        custom.id = "custom-example".into();
        custom.name = "My Mocha".into();
        themes.push(custom);
        let settings = Settings::default();
        let mut ui = Ui::new(Kind::Themes, [0., 0.]);
        let rows = ui.rows(&themes, &settings);
        assert!(!rows.iter().any(|r| matches!(r.action, Action::Pick(_))));
        assert_eq!(rows[1].action, Action::Custom);
        ui.expanded.insert("Catppuccin".into());
        let rows = ui.rows(&themes, &settings);
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.action, Action::Pick(_)))
                .count(),
            4
        );
        assert!(!rows
            .iter()
            .any(|r| r.action == Action::Pick("custom-example".into())));
        ui.kind = Kind::Custom;
        let rows = ui.rows(&themes, &settings);
        assert!(rows
            .iter()
            .any(|r| r.action == Action::ManageTheme("custom-example".into())));
    }
    #[test]
    fn tab_strip_keeps_the_active_tab_reachable_and_hits_plus() {
        for active in 0..16 {
            let labels: Vec<String> = (0..16).map(|i| format!("Tab {i}")).collect();
            let scene = tab_scene(&labels, active, 800., 1.);
            assert!(scene
                .text
                .iter()
                .any(|t| t.value == format!("Tab {active}")));
            for text in &scene.text {
                if text.value == format!("Tab {active}") {
                    assert_eq!(
                        tab_hit([text.pos[0], 16.], 800., 1., 16, active),
                        Some(TabHit::Select(active))
                    );
                }
            }
            assert_eq!(
                tab_hit([790., 16.], 800., 1., 16, active),
                Some(TabHit::New)
            );
        }
        assert_eq!(tab_hit([10., 40.], 800., 1., 1, 0), None);
    }
    #[test]
    fn font_search_and_size_arrow_actions() {
        use super::*;
        let settings = Settings::default();
        let mut ui = Ui::new(Kind::Fonts, [0., 0.]);
        ui.paste_text("JetBrains");
        let rows = ui.rows(&[], &settings);
        assert!(rows
            .iter()
            .any(|r| r.action == Action::PickFont("JetBrains Mono".into())));
        assert!(!rows
            .iter()
            .any(|r| r.action == Action::PickFont("Fira Code".into())));
        let mut ui = Ui::new(Kind::Settings, [0., 0.]);
        let rows = ui.rows(&[], &settings);
        ui.focus_row(1, &rows);
        assert_eq!(
            ui.key(&Key::Named(NamedKey::ArrowRight), None, false, &rows),
            Some(Action::Resize(1))
        );
        assert_eq!(
            ui.key(&Key::Named(NamedKey::ArrowLeft), None, false, &rows),
            Some(Action::Resize(-1))
        );
    }
    use super::*;
    #[test]
    fn geometry_hit_testing_and_keyboard_never_touch_a_terminal() {
        for kind in [Kind::Settings, Kind::Fonts, Kind::Themes, Kind::Editor] {
            let panel = Ui::new(kind, [16., 16.]);
            let (rect, _, _) = panel.geometry(1200., 800., 1., 8);
            assert_eq!(rect[0] + rect[2], 1184.);
            let (resized, _, _) = panel.geometry(1400., 800., 1., 8);
            assert_eq!(resized[0] + resized[2], 1384.);
        }
        let mut ui = Ui::new(Kind::Context, [1000., 1000.]);
        let rows = ui.rows(&[], &Settings::default());
        let (rect, visible, _) = ui.geometry(640., 480., 1., rows.len());
        assert!(rect[0] + rect[2] <= 640. && rect[1] + rect[3] <= 480.);
        assert_eq!(visible, 7);
        assert_eq!(
            ui.hit([rect[0] + 20., rect[1] + 40.], 640., 480., 1., rows.len()),
            Some(0)
        );
        assert_eq!(
            ui.key(&Key::Named(NamedKey::Enter), None, false, &rows),
            Some(Action::NewTab)
        );
        assert_eq!(
            ui.key(&Key::Named(NamedKey::Escape), None, false, &rows),
            Some(Action::Cancel)
        );
    }
    #[test]
    fn theme_search_and_invalid_editor_text_stay_in_ui() {
        let mut ui = Ui::new(Kind::Themes, [0., 0.]);
        let themes = crate::theme::builtins();
        let settings = Settings::default();
        let rows = ui.rows(&themes, &settings);
        ui.key(&Key::Character("nord".into()), Some("nord"), false, &rows);
        assert_eq!(ui.query, "nord");
        assert!(ui
            .rows(&themes, &settings)
            .iter()
            .any(|row| row.action == Action::Pick("nord".into())));
    }
}
