//! Application-owned menus and forms. Pure geometry/data, no session or terminal access.
use crate::{
    render::overlay::{Overlay, Rectangle, Text},
    settings::Settings,
    theme::{Theme, FIELD_NAMES},
};
use winit::keyboard::{Key, NamedKey};
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Copy,
    Paste,
    SelectAll,
    Themes,
    Settings,
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
    Editor,
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
}
pub struct Row {
    pub label: String,
    pub action: Action,
    pub value: Option<String>,
}
impl Ui {
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
        }
    }
    pub fn rows(&self, themes: &[Theme], settings: &Settings) -> Vec<Row> {
        let row = |label: String, action: Action, value: Option<String>| Row {
            label,
            action,
            value,
        };
        match self.kind {
            Kind::Context => [
                Action::Copy,
                Action::Paste,
                Action::SelectAll,
                Action::Themes,
                Action::Settings,
            ]
            .into_iter()
            .zip([
                "Copy   Ctrl+Shift+C",
                "Paste  Ctrl+Shift+V",
                "Select all",
                "Themes...",
                "Settings...",
            ])
            .map(|(a, l)| row(l.into(), a, None))
            .collect(),
            Kind::Themes => {
                let mut rows = vec![row(
                    format!("Search: {}", self.query),
                    Action::Search,
                    Some(self.query.clone()),
                )];
                rows.extend(
                    themes
                        .iter()
                        .filter(|t| t.name.to_lowercase().contains(&self.query.to_lowercase()))
                        .map(|t| {
                            row(
                                format!(
                                    "{} {}{}",
                                    if t.id == settings.active_theme_id {
                                        "[*]"
                                    } else {
                                        "[ ]"
                                    },
                                    t.name,
                                    if t.id.starts_with("custom-") {
                                        " (custom)"
                                    } else {
                                        ""
                                    }
                                ),
                                Action::Pick(t.id.clone()),
                                None,
                            )
                        }),
                );
                rows.push(row(
                    "Create custom theme from current...".into(),
                    Action::Duplicate,
                    None,
                ));
                rows
            }
            Kind::Settings => vec![
                row(
                    format!("Font: {}", settings.font_family),
                    Action::Setting(0),
                    Some(settings.font_family.clone()),
                ),
                row(
                    format!("Font size: {} px", settings.font_size),
                    Action::Setting(1),
                    Some(settings.font_size.to_string()),
                ),
                row(
                    format!("Minimum contrast: {}:1", settings.minimum_contrast),
                    Action::Setting(2),
                    Some(settings.minimum_contrast.to_string()),
                ),
                row(
                    "Edit current theme (presets are duplicated)...".into(),
                    Action::Edit,
                    None,
                ),
                row("Theme selector...".into(), Action::Themes, None),
            ],
            Kind::Editor => {
                let t = self.draft.as_ref().expect("editor draft");
                let mut rows = vec![row(
                    format!("Name: {}", t.name),
                    Action::Name,
                    Some(t.name.clone()),
                )];
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
        let x = self.anchor[0].clamp(0., (width - w).max(0.));
        let y = self.anchor[1].clamp(0., (height - h).max(0.));
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
    }
    pub fn paste_text(&mut self, text: &str) {
        if self.select_all {
            self.buffer.clear();
            self.select_all = false;
        }
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.buffer.len() + c.len_utf8() > 256 {
                break;
            }
            self.buffer.push(c);
        }
        if self.kind == Kind::Themes {
            self.query = self.buffer.clone();
            self.focus = 0;
            self.scroll = 0;
        }
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
            Key::Named(NamedKey::Escape) => return Some(Action::Cancel),
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
        let editable =
            self.kind == Kind::Themes || rows.get(self.focus).is_some_and(|r| r.value.is_some());
        if !editable || control {
            return None;
        }
        if matches!(key, Key::Named(NamedKey::Backspace)) {
            if self.select_all {
                self.buffer.clear();
            } else {
                self.buffer.pop();
            }
            self.select_all = false;
        } else if let Some(text) = text.or_else(|| match key {
            Key::Character(s) => Some(s.as_str()),
            Key::Named(NamedKey::Space) => Some(" "),
            _ => None,
        }) {
            if self.select_all {
                self.buffer.clear();
                self.select_all = false;
            }
            if self.buffer.len() + text.len() <= 256 {
                self.buffer.extend(text.chars().filter(|c| !c.is_control()));
            }
        }
        if self.kind == Kind::Themes {
            self.query = self.buffer.clone();
            self.focus = 0;
            self.scroll = 0;
        }
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
            Kind::Themes => "Themes - live preview",
            Kind::Settings => "Settings - Enter applies",
            Kind::Editor => "Custom theme - live preview",
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
                    "{} > {}",
                    row.label.split(':').next().unwrap_or("Value"),
                    self.buffer
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
                Kind::Themes => "Type to search | Enter selects",
                Kind::Settings => "Enter saves | Esc closes",
                Kind::Editor => "Ctrl+S saves | Esc cancels",
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
    use super::*;
    #[test]
    fn geometry_hit_testing_and_keyboard_never_touch_a_terminal() {
        let mut ui = Ui::new(Kind::Context, [1000., 1000.]);
        let rows = ui.rows(&[], &Settings::default());
        let (rect, visible, _) = ui.geometry(640., 480., 1., rows.len());
        assert!(rect[0] + rect[2] <= 640. && rect[1] + rect[3] <= 480.);
        assert_eq!(visible, 5);
        assert_eq!(
            ui.hit([rect[0] + 20., rect[1] + 40.], 640., 480., 1., rows.len()),
            Some(0)
        );
        assert_eq!(
            ui.key(&Key::Named(NamedKey::Enter), None, false, &rows),
            Some(Action::Copy)
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
        assert_eq!(ui.rows(&themes, &settings).len(), 3);
    }
}
