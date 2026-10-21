//! Application-owned menus and forms. Pure geometry/data, no session or terminal access.
use crate::{
    render::overlay::{Overlay, Rectangle, Text},
    settings::Settings,
    theme::{Theme, FIELD_NAMES},
};
use winit::keyboard::{Key, NamedKey};
pub const TAB_HEIGHT: f32 = 32.;
/// Pointer travel before a tab press becomes a drag rather than a click.
pub const TAB_DRAG_THRESHOLD: f32 = 4.;
pub const STATUS_HEIGHT: f32 = 26.;
/// Total duration of the attention flash before it settles to a steady accent.
pub const ATTENTION_FLASH_MS: u64 = 1200;
/// Half-period of the flash: background toggles every this many milliseconds.
pub const ATTENTION_FLASH_PERIOD_MS: u64 = 400;
pub fn status_scene(label: &str, width: f32, height: f32, scale: f32, theme: &Theme) -> Overlay {
    let color = |value: &str| {
        let c = crate::theme::rgb(value).unwrap();
        [c[0], c[1], c[2], 1.]
    };
    let y = (height - STATUS_HEIGHT * scale).max(0.);
    Overlay {
        rectangles: vec![Rectangle {
            rect: [0., y, width, STATUS_HEIGHT * scale],
            color: color(&theme.colors.background),
        }],
        text: vec![Text {
            pos: [8. * scale, y + 3. * scale],
            width: (width - 16. * scale).max(0.),
            value: label.into(),
            color: color(&theme.colors.foreground),
        }],
    }
}
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
/// Shared strip geometry used by hit-testing, rendering and drag reordering.
pub struct Strip {
    pub start: usize,
    pub visible: usize,
    pub step: f32,
    pub plus: f32,
}
/// `start` overrides the window origin while a tab is being dragged so hidden
/// tabs can be reached by scrolling at the strip edges.
pub fn strip(width: f32, scale: f32, count: usize, active: usize, start: Option<usize>) -> Strip {
    let plus = 40. * scale;
    let visible = (((width - plus) / (160. * scale)).floor() as usize)
        .max(1)
        .min(count.max(1));
    let start = start
        .unwrap_or_else(|| active.saturating_sub(visible - 1))
        .min(count.saturating_sub(visible));
    Strip {
        start,
        visible,
        step: (width - plus) / visible as f32,
        plus,
    }
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
    let strip = strip(width, scale, count, active, None);
    if point[0] >= width - strip.plus {
        return Some(TabHit::New);
    }
    let index = (point[0].max(0.) / strip.step) as usize + strip.start;
    (index < count).then_some(TabHit::Select(index))
}
/// Insertion gap for a pointer position, in pre-removal indexing. The right half
/// of a tab's slot targets the gap after it.
pub fn drop_gap(point_x: f32, strip: &Strip, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let slot = (point_x.max(0.) / strip.step) as usize;
    let index = (strip.start + slot).min(count - 1);
    let offset = point_x - (index.saturating_sub(strip.start)) as f32 * strip.step;
    let gap = if offset > strip.step / 2. {
        index + 1
    } else {
        index
    };
    gap.min(count)
}
/// Advance the drag window one tab when the pointer reaches a strip edge.
pub fn drag_start(point_x: f32, width: f32, scale: f32, strip: &Strip, count: usize) -> usize {
    let edge = 24. * scale;
    if point_x < edge && strip.start > 0 {
        strip.start - 1
    } else if point_x > width - strip.plus - edge
        && strip.start + strip.visible < count
    {
        strip.start + 1
    } else {
        strip.start
    }
}
/// Accent used by drag feedback. Fixed rather than themed so the gesture reads
/// the same in every theme, including low-contrast light ones.
pub const DRAG_ACCENT: [f32; 4] = [0.42, 0.55, 0.95, 1.];
/// Translucent fill marking the slot the dragged tab will occupy.
pub const DRAG_BAND_ALPHA: f32 = 0.18;
/// Outline thickness of the dragged tab, in logical pixels.
pub const DRAG_OUTLINE: f32 = 1.5;
/// Drag feedback state: the strip window origin, the resolved insertion gap and
/// the pointer position that positions the dragged-tab outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragView {
    pub start: usize,
    pub gap: usize,
    pub pointer_x: f32,
}
/// Outline of the tab being dragged, centred on the pointer and kept inside the
/// strip so it never draws over the new-tab button.
pub fn ghost_rect(pointer_x: f32, width: f32, scale: f32, strip: &Strip) -> [f32; 4] {
    let w = (strip.step - 2.).max(1.);
    let room = (width - strip.plus - w).max(0.);
    [
        (pointer_x - w / 2.).clamp(0., room),
        0.,
        w,
        TAB_HEIGHT * scale,
    ]
}
/// Destination band and its insertion marker, both derived from the same gap so
/// the highlight always matches where the reorder will insert.
pub fn drop_rects(gap: usize, strip: &Strip, width: f32, scale: f32) -> ([f32; 4], [f32; 4]) {
    let slot = (strip.step - 2.).max(1.);
    let area = (width - strip.plus).max(0.);
    let line = if gap <= strip.start {
        0.
    } else if gap >= strip.start + strip.visible {
        area
    } else {
        (gap - strip.start) as f32 * strip.step
    };
    // The tab lands in the slot after the insertion line; at the far end that
    // slot is off-strip, so the band moves to the left of the line.
    let band_x = if gap >= strip.start + strip.visible {
        (line - slot).max(0.)
    } else {
        line.min((area - slot).max(0.))
    };
    (
        [band_x, 0., slot.min(area), TAB_HEIGHT * scale],
        [(line - scale).max(0.), 0., 2. * scale, TAB_HEIGHT * scale],
    )
}
/// `drag` carries the window origin, insertion gap and pointer while a drag is
/// active.
pub fn tab_scene(
    labels: &[String],
    active: usize,
    attention: &[bool],
    flash_on: bool,
    width: f32,
    scale: f32,
    drag: Option<DragView>,
) -> Overlay {
    let mut out = Overlay::default();
    out.rectangles.push(Rectangle {
        rect: [0., 0., width, TAB_HEIGHT * scale],
        color: [0.06, 0.06, 0.09, 1.],
    });
    let strip = strip(
        width,
        scale,
        labels.len(),
        active,
        drag.map(|drag| drag.start),
    );
    let Strip {
        start,
        visible,
        step,
        plus,
    } = strip;
    for (i, label) in labels.iter().enumerate().skip(start).take(visible) {
        let x = (i - start) as f32 * step;
        let needs_attention = attention.get(i).copied().unwrap_or(false);
        if i == active {
            out.rectangles.push(Rectangle {
                rect: [x, 0., step - 2., TAB_HEIGHT * scale],
                color: [0.18, 0.19, 0.28, 1.],
            });
        } else if needs_attention && flash_on {
            out.rectangles.push(Rectangle {
                rect: [x, 0., step - 2., TAB_HEIGHT * scale],
                color: [0.34, 0.28, 0.12, 1.],
            });
        }
        let color = if needs_attention && i != active {
            [1.0, 0.84, 0.36, 1.]
        } else {
            [0.9, 0.9, 0.98, 1.]
        };
        out.text.push(Text {
            pos: [x + 10. * scale, 5. * scale],
            width: (step - 20. * scale).max(0.),
            value: label.clone(),
            color,
        });
    }
    if let Some(drag) = drag {
        let (band, marker) = drop_rects(drag.gap, &strip, width, scale);
        let mut accent = DRAG_ACCENT;
        accent[3] = DRAG_BAND_ALPHA;
        out.rectangles.push(Rectangle {
            rect: band,
            color: accent,
        });
        out.rectangles.push(Rectangle {
            rect: marker,
            color: DRAG_ACCENT,
        });
        // Outline last so it reads above the band and the tab fills beneath it.
        let ghost = ghost_rect(drag.pointer_x, width, scale, &strip);
        let [x, y, w, h] = ghost;
        let edge = (DRAG_OUTLINE * scale).max(1.);
        for rect in [
            [x, y, w, edge],
            [x, y + h - edge, w, edge],
            [x, y, edge, h],
            [x + w - edge, y, edge, h],
        ] {
            out.rectangles.push(Rectangle {
                rect,
                color: DRAG_ACCENT,
            });
        }
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
    Privacy,
    PrivacyMode(String),
    Updates,
    UpdateChannel(String),
    CheckUpdate,
    DownloadUpdate,
    InstallUpdate,
    ConfirmUpdate,
    CancelUpdate,
    ImportSettings,
    ExportSettings,
    ApplyImport,
    ExpandAll,
    CollapseAll,
    ToggleFavourite(String),
    ToggleLineNumbers,
    ToggleGitStatus,
    ToggleDirectoryReporting,
    RefreshGitStatus,
    Shells,
    PickShell(String),
    Appearance,
    TerminalSettings,
    Tools,
    ImportExport,
    Junafetch,
    FetchProfiles,
    FetchArtwork,
    FetchProfile(String),
    FetchLogo(String),
    CloseTargetTab,
    RenameTab,
    SaveTabName,
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
    Privacy,
    Updates,
    ConfirmUpdate,
    Appearance,
    TerminalSettings,
    Shells,
    Tools,
    ImportExport,
    Junafetch,
    FetchProfiles,
    FetchArtwork,
    ImportReview,
    ConfirmClose,
    TabContext,
    Rename,
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
    pub summary: Vec<String>,
    wheel_remainder: f32,
    pub theme: Option<Theme>,
    pub star_focused: bool,
    pub target_tab: Option<u64>,
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
    pub favourite: bool,
    pub label: String,
    pub action: Action,
    pub value: Option<String>,
}
pub fn theme_id(row: &Row) -> Option<&str> {
    match &row.action {
        Action::Pick(id) | Action::ManageTheme(id) => Some(id),
        _ => None,
    }
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
            summary: Vec::new(),
            wheel_remainder: 0.,
            theme: None,
            star_focused: false,
            target_tab: None,
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
            favourite: false,
            label,
            action,
            value,
        };
        let mut result: Vec<Row> = match self.kind {
            Kind::ImportReview => {
                let mut rows: Vec<_> = self
                    .summary
                    .iter()
                    .map(|text| row(text.clone(), Action::Search, None))
                    .collect();
                rows.push(row(
                    "Apply imported settings".into(),
                    Action::ApplyImport,
                    None,
                ));
                rows.push(row("Cancel".into(), Action::Cancel, None));
                rows
            }
            Kind::ConfirmUpdate => vec![
                row("✓ Install and restart".into(), Action::ConfirmUpdate, None),
                row("✕ Cancel".into(), Action::Updates, None),
                row(
                    "Close other Juna windows first".into(),
                    Action::Updates,
                    None,
                ),
            ],
            Kind::Privacy => {
                let mut rows = [
                    ("shift-hover", "Mask · Shift+hover reveal"),
                    ("hover", "Mask · hover reveal"),
                    ("always-hidden", "Always mask · Shift override"),
                    ("off", "Censorship off · next report"),
                ]
                .into_iter()
                .map(|(mode, label)| {
                    row(
                        format!(
                            "[{}] {label}",
                            if settings.privacy_mode == mode {
                                "x"
                            } else {
                                " "
                            }
                        ),
                        Action::PrivacyMode(mode.into()),
                        None,
                    )
                })
                .collect::<Vec<_>>();
                rows.push(row("Junafetch fields only".into(), Action::Privacy, None));
                rows.push(row(
                    "Off permits output/history disclosure".into(),
                    Action::Privacy,
                    None,
                ));
                rows.push(row("Back to settings".into(), Action::Settings, None));
                rows
            }
            Kind::Updates => {
                let mut rows = self
                    .summary
                    .iter()
                    .map(|line| row(line.clone(), Action::Updates, None))
                    .collect::<Vec<_>>();
                rows.extend([
                    row(
                        format!(
                            "[{}] Stable releases",
                            if settings.update_channel == "stable" {
                                "x"
                            } else {
                                " "
                            }
                        ),
                        Action::UpdateChannel("stable".into()),
                        None,
                    ),
                    row(
                        format!(
                            "[{}] Preview releases",
                            if settings.update_channel == "preview" {
                                "x"
                            } else {
                                " "
                            }
                        ),
                        Action::UpdateChannel("preview".into()),
                        None,
                    ),
                    row("Check for update".into(), Action::CheckUpdate, None),
                    row("Download update".into(), Action::DownloadUpdate, None),
                    row("Install and restart...".into(), Action::InstallUpdate, None),
                    row("Cancel download/check".into(), Action::CancelUpdate, None),
                    row("Back to settings".into(), Action::Settings, None),
                ]);
                rows
            }
            Kind::ConfirmClose => vec![
                row("✓ Close tab".into(), Action::CloseTargetTab, None),
                row("✕ Cancel".into(), Action::Cancel, None),
            ],
            Kind::TabContext => {
                let mut rows = vec![row("New tab   Ctrl+Shift+T".into(), Action::NewTab, None)];
                if self.target_tab.is_some() {
                    rows.push(row("Rename tab...".into(), Action::RenameTab, None));
                    rows.push(row("Close tab".into(), Action::CloseTargetTab, None));
                }
                rows
            }
            Kind::Rename => vec![
                row(
                    "Tab name".into(),
                    Action::SaveTabName,
                    Some(self.draft_value.clone()),
                ),
                row("Save".into(), Action::SaveTabName, None),
                row("Cancel".into(), Action::Cancel, None),
            ],
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
                rows.push(row("Expand all families".into(), Action::ExpandAll, None));
                rows.push(row(
                    "Collapse all families".into(),
                    Action::CollapseAll,
                    None,
                ));
                let mut groups: std::collections::BTreeMap<String, Vec<&Theme>> =
                    std::collections::BTreeMap::new();
                if !custom {
                    let favourites: Vec<_> = themes
                        .iter()
                        .filter(|t| settings.favourite_theme_ids.contains(&t.id))
                        .filter(|t| {
                            self.query.is_empty()
                                || t.name.to_lowercase().contains(&self.query.to_lowercase())
                                || t.family_name()
                                    .to_lowercase()
                                    .contains(&self.query.to_lowercase())
                        })
                        .collect();
                    groups.insert("\0favourites".into(), favourites);
                }
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
                            if family == "\0favourites" {
                                "Favourites"
                            } else {
                                &family
                            },
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
                                        ">"
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
                rows.push(row("Back to appearance".into(), Action::Appearance, None));
                rows
            }
            Kind::Settings => vec![
                row("Appearance...".into(), Action::Appearance, None),
                row("Terminal...".into(), Action::TerminalSettings, None),
                row("Tools...".into(), Action::Tools, None),
                row("Import / Export...".into(), Action::ImportExport, None),
                row("Updates...".into(), Action::Updates, None),
                row("Privacy...".into(), Action::Privacy, None),
                row("Close".into(), Action::Cancel, None),
            ],
            Kind::Tools => vec![
                row("Junafetch...".into(), Action::Junafetch, None),
                row("Back to settings".into(), Action::Settings, None),
            ],
            Kind::ImportExport => vec![
                row("Import settings...".into(), Action::ImportSettings, None),
                row("Export settings...".into(), Action::ExportSettings, None),
                row("Back to settings".into(), Action::Settings, None),
            ],
            Kind::Junafetch => vec![
                row(
                    format!("Report: {}...", settings.junafetch_profile),
                    Action::FetchProfiles,
                    None,
                ),
                row(
                    format!("Artwork: {}...", settings.junafetch_logo),
                    Action::FetchArtwork,
                    None,
                ),
                row("Back to tools".into(), Action::Tools, None),
            ],
            Kind::FetchProfiles => ["brief", "standard", "detailed"]
                .into_iter()
                .map(|name| {
                    row(
                        format!(
                            "[{}] {}",
                            if settings.junafetch_profile == name {
                                "x"
                            } else {
                                " "
                            },
                            match name {
                                "brief" => "Brief — eight stats",
                                "standard" => "Standard — grouped report",
                                _ => "Detailed — additional hardware",
                            }
                        ),
                        Action::FetchProfile(name.into()),
                        None,
                    )
                })
                .chain([row("Back to Junafetch".into(), Action::Junafetch, None)])
                .collect(),
            Kind::FetchArtwork => ["compact", "full", "none"]
                .into_iter()
                .map(|name| {
                    row(
                        format!(
                            "[{}] {}",
                            if settings.junafetch_logo == name {
                                "x"
                            } else {
                                " "
                            },
                            name
                        ),
                        Action::FetchLogo(name.into()),
                        None,
                    )
                })
                .chain([row("Back to Junafetch".into(), Action::Junafetch, None)])
                .collect(),
            Kind::TerminalSettings => vec![
                row(
                    format!(
                        "[{}] Show line numbers",
                        if settings.show_line_numbers { "x" } else { " " }
                    ),
                    Action::ToggleLineNumbers,
                    None,
                ),
                row(
                    format!(
                        "[{}] Show Git status",
                        if settings.show_git_status { "x" } else { " " }
                    ),
                    Action::ToggleGitStatus,
                    None,
                ),
                row(
                    format!(
                        "[{}] Track current folder in tab name",
                        if settings.report_shell_directory {
                            "x"
                        } else {
                            " "
                        }
                    ),
                    Action::ToggleDirectoryReporting,
                    None,
                ),
                row(
                    format!("Shell (new tabs): {}", super::shells::display(&settings.shell)),
                    Action::Shells,
                    None,
                ),
                row(
                    "Refresh local Git status".into(),
                    Action::RefreshGitStatus,
                    None,
                ),
                row("Back to settings".into(), Action::Settings, None),
            ],
            Kind::Shells => crate::settings::SHELL_CHOICES
                .iter()
                .map(|name| {
                    row(
                        format!(
                            "[{}] {}{}",
                            if settings.shell == *name { "x" } else { " " },
                            super::shells::display(name),
                            if super::shells::available(name) {
                                ""
                            } else {
                                " (not found)"
                            }
                        ),
                        Action::PickShell((*name).into()),
                        None,
                    )
                })
                .chain([row(
                    "Back to terminal".into(),
                    Action::TerminalSettings,
                    None,
                )])
                .collect(),
            Kind::Appearance => vec![
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
                row("Back to settings".into(), Action::Settings, None),
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
        };
        for row in &mut result {
            row.favourite = theme_id(row)
                .is_some_and(|id| settings.favourite_theme_ids.iter().any(|saved| saved == id));
        }
        result
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
        let anchor_x = if matches!(self.kind, Kind::Context | Kind::TabContext) {
            self.anchor[0]
        } else {
            width - w - 16. * scale
        };
        let x = anchor_x.clamp(0., (width - w).max(0.));
        let anchor_y = if matches!(self.kind, Kind::Context | Kind::TabContext) {
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
        self.star_focused = false;
        self.focus = index.min(rows.len().saturating_sub(1));
        self.buffer = rows
            .get(self.focus)
            .and_then(|r| r.value.clone())
            .unwrap_or_default();
        self.select_all = true;
        self.caret = self.buffer.len();
    }
    pub fn scroll_rows(&mut self, amount: f32, visible: usize, rows: &[Row]) {
        if !amount.is_finite() {
            return;
        }
        self.wheel_remainder += amount;
        let steps = self.wheel_remainder.trunc().clamp(-128., 128.) as isize;
        self.wheel_remainder -= steps as f32;
        let limit = rows.len().saturating_sub(visible);
        self.scroll = self.scroll.saturating_add_signed(steps).min(limit);
        if self.scroll == 0 && amount < 0. || self.scroll == limit && amount > 0. {
            self.wheel_remainder = 0.;
        }
        let last = (self.scroll + visible.saturating_sub(1)).min(rows.len().saturating_sub(1));
        if self.focus < self.scroll || self.focus > last {
            self.focus_row(self.focus.clamp(self.scroll, last.max(self.scroll)), rows);
        }
    }
    pub fn star_hit(
        &self,
        point: [f32; 2],
        width: f32,
        height: f32,
        scale: f32,
        rows: &[Row],
    ) -> Option<String> {
        let index = self.hit(point, width, height, scale, rows.len())?;
        let (rect, _, _) = self.geometry(width, height, scale, rows.len());
        (point[0] >= rect[0] + rect[2] - 40. * scale)
            .then(|| theme_id(&rows[index]).map(str::to_string))
            .flatten()
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
        if matches!(self.kind, Kind::Number | Kind::Rename) && self.focus == 0 {
            self.draft_value = self.buffer.clone();
        }
        if matches!(self.kind, Kind::Themes | Kind::Custom | Kind::Fonts) {
            self.star_focused = false;
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
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowLeft)
                if rows.get(self.focus).and_then(theme_id).is_some() =>
            {
                self.star_focused = matches!(key, Key::Named(NamedKey::ArrowRight));
                return None;
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) if self.star_focused => {
                return rows
                    .get(self.focus)
                    .and_then(theme_id)
                    .map(|id| Action::ToggleFavourite(id.into()));
            }
            Key::Character(value) if value == " " && self.star_focused => {
                return rows
                    .get(self.focus)
                    .and_then(theme_id)
                    .map(|id| Action::ToggleFavourite(id.into()));
            }
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
                if self.kind == Kind::Appearance
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
                return Some(match self.kind {
                    Kind::Privacy | Kind::Updates => Action::Settings,
                    Kind::ConfirmUpdate => Action::Updates,
                    Kind::Number => Action::CancelSetting,
                    Kind::Appearance
                    | Kind::TerminalSettings
                    | Kind::Tools
                    | Kind::ImportExport => Action::Settings,
                    Kind::Junafetch => Action::Tools,
                    Kind::FetchProfiles | Kind::FetchArtwork => Action::Junafetch,
                    Kind::Fonts => Action::Appearance,
                    _ => Action::Cancel,
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
                    && matches!(self.kind, Kind::Settings | Kind::Appearance | Kind::Fonts) =>
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
        let theme = self
            .theme
            .clone()
            .unwrap_or_else(crate::theme::default_theme);
        let confirmation = matches!(self.kind, Kind::ConfirmClose | Kind::ConfirmUpdate);
        let bg = if confirmation {
            crate::theme::rgb(&theme.colors.background).unwrap()
        } else {
            [0.075, 0.075, 0.115]
        };
        let highlight = if confirmation {
            crate::theme::rgb(&theme.colors.selection_background).unwrap()
        } else {
            [0.20, 0.22, 0.34]
        };
        let readable = |fg, bg| {
            let c = crate::render::contrast::foreground(fg, bg, 4.5);
            [c[0], c[1], c[2], 1.]
        };
        let (r, n, step) = self.geometry(width, height, scale, rows.len());
        let mut out = Overlay::default();
        out.rectangles.push(Rectangle {
            rect: [r[0] + 3. * scale, r[1] + 3. * scale, r[2], r[3]],
            color: [0., 0., 0., 0.7],
        });
        out.rectangles.push(Rectangle {
            rect: r,
            color: [bg[0], bg[1], bg[2], 1.],
        });
        let title = match self.kind {
            Kind::Privacy => "Privacy - saved automatically",
            Kind::Updates => "Updates",
            Kind::ConfirmUpdate => "Install update and restart?",
            Kind::ImportReview => "Review settings import",
            Kind::ConfirmClose => "Close tab?",
            Kind::TabContext => "Tabs",
            Kind::Rename => "Rename tab",
            Kind::Context => "Juna",
            Kind::Themes => "Theme families - live preview",
            Kind::Custom => "Custom themes - live preview",
            Kind::Fonts => "Fonts - live preview",
            Kind::Settings => "Settings - saved automatically",
            Kind::Appearance => "Appearance - live preview",
            Kind::TerminalSettings => "Terminal - saved automatically",
            Kind::Shells => "Shell - applies to new tabs",
            Kind::Tools => "Tools",
            Kind::ImportExport => "Import / Export",
            Kind::Junafetch => "Junafetch - next command",
            Kind::FetchProfiles => "Junafetch report detail",
            Kind::FetchArtwork => "Junafetch artwork",
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
            color: readable(
                if confirmation {
                    crate::theme::rgb(&theme.colors.foreground).unwrap()
                } else {
                    [0.9, 0.9, 0.98]
                },
                bg,
            ),
        });
        for (i, row) in rows.iter().enumerate().skip(self.scroll).take(n) {
            let y = r[1] + 36. * scale + (i - self.scroll) as f32 * step;
            let selected = i == self.focus;
            if selected {
                out.rectangles.push(Rectangle {
                    rect: [r[0] + 6. * scale, y, r[2] - 12. * scale, step],
                    color: [highlight[0], highlight[1], highlight[2], 1.],
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
                width: r[2]
                    - if theme_id(row).is_some() {
                        60. * scale
                    } else {
                        24. * scale
                    },
                value,
                color: if confirmation {
                    let index =
                        if matches!(row.action, Action::CloseTargetTab | Action::ConfirmUpdate) {
                            2
                        } else {
                            1
                        };
                    readable(
                        crate::theme::rgb(&theme.colors.ansi[index]).unwrap(),
                        if selected { highlight } else { bg },
                    )
                } else if selected {
                    [1., 1., 1., 1.]
                } else {
                    [0.78, 0.8, 0.9, 1.]
                },
            });
            if theme_id(row).is_some() {
                if selected && self.star_focused {
                    out.rectangles.push(Rectangle {
                        rect: [r[0] + r[2] - 38. * scale, y, 30. * scale, step],
                        color: [0.3, 0.3, 0.4, 1.],
                    });
                }
                out.text.push(Text {
                    pos: [r[0] + r[2] - 35. * scale, y + 2. * scale],
                    width: 26. * scale,
                    value: if row.favourite { "★" } else { "☆" }.into(),
                    color: [1., 0.83, 0.3, 1.],
                });
            }
        }
        let footer = if self.message.is_empty() {
            match self.kind {
                Kind::Context | Kind::TabContext | Kind::ConfirmClose | Kind::ConfirmUpdate => {
                    "Enter selects | Esc closes"
                }
                Kind::Rename => "Enter saves | Esc cancels",
                Kind::ImportReview => "Review first | Apply replaces preferences",
                Kind::Themes | Kind::Custom => {
                    if self.star_focused {
                        "Favourite / Remove favourite | Space toggles"
                    } else {
                        "Enter selects | Right: favourite star"
                    }
                }
                Kind::Manage => "Enter selects | Esc closes",
                Kind::Appearance => "Left/Right size | Enter edits | Esc back",
                Kind::Privacy => "Masked reports copy as masks",
                Kind::Junafetch => "Protected values: Settings > Privacy",
                Kind::Settings
                | Kind::TerminalSettings
                | Kind::Shells
                | Kind::Tools
                | Kind::ImportExport
                | Kind::FetchProfiles
                | Kind::FetchArtwork => "Enter selects | Esc back",
                Kind::Updates => "Downloads verified before install",
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
            color: readable(
                if confirmation {
                    crate::theme::rgb(&theme.colors.foreground).unwrap()
                } else {
                    [0.85, 0.82, 0.7]
                },
                bg,
            ),
        });
        out
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn grouped_settings_keep_root_short_and_keyboard_returns_to_parent() {
        let settings = Settings::default();
        let rows = Ui::new(Kind::Settings, [0.; 2]).rows(&[], &settings);
        assert_eq!(rows.len(), 7);
        assert!(rows.iter().all(|row| !matches!(
            row.action,
            Action::ToggleLineNumbers | Action::Setting(_) | Action::FetchProfile(_)
        )));
        let mut profiles = Ui::new(Kind::FetchProfiles, [0.; 2]);
        let rows = profiles.rows(&[], &settings);
        assert_eq!(
            profiles.key(&Key::Named(NamedKey::Enter), None, false, &rows),
            Some(Action::FetchProfile("brief".into()))
        );
        assert_eq!(
            profiles.key(&Key::Named(NamedKey::Escape), None, false, &rows),
            Some(Action::Junafetch)
        );
        let mut tools = Ui::new(Kind::Tools, [0.; 2]);
        let rows = tools.rows(&[], &settings);
        assert_eq!(
            tools.key(&Key::Named(NamedKey::Escape), None, false, &rows),
            Some(Action::Settings)
        );
        let (rect, _, _) = profiles.geometry(1024., 700., 1., 4);
        assert_eq!(
            profiles.hit([rect[0] + 16., rect[1] + 40.], 1024., 700., 1., 4),
            Some(0)
        );
    }
    #[test]
    fn wheel_scrolls_viewport_immediately_and_accumulates_pixels() {
        use super::*;
        let mut panel = Ui::new(Kind::Themes, [0.; 2]);
        panel.expanded.insert("Catppuccin".into());
        let rows = panel.rows(&crate::theme::builtins(), &Settings::default());
        panel.scroll_rows(3., 8, &rows);
        assert_eq!(panel.scroll, 3);
        panel.scroll_rows(0.4, 8, &rows);
        assert_eq!(panel.scroll, 3);
        panel.scroll_rows(0.7, 8, &rows);
        assert_eq!(panel.scroll, 4);
        panel.scroll_rows(-128., 8, &rows);
        assert_eq!(panel.scroll, 0);
    }
    #[test]
    fn confirmation_icons_follow_theme_with_readable_contrast() {
        use super::*;
        for theme in crate::theme::builtins() {
            let mut panel = Ui::new(Kind::ConfirmClose, [0.; 2]);
            panel.theme = Some(theme.clone());
            let rows = panel.rows(&[], &Settings::default());
            panel.focus_row(1, &rows);
            let scene = panel.scene(&rows, 1024., 512., 1.);
            assert_eq!(scene.text[0].value, "Close tab?");
            assert!(scene.text[1].value.starts_with('✓'));
            assert!(scene.text[2].value.starts_with('✕'));
            for (i, text) in scene.text[1..3].iter().enumerate() {
                let bg = crate::theme::rgb(if i == 0 {
                    &theme.colors.background
                } else {
                    &theme.colors.selection_background
                })
                .unwrap();
                assert!(
                    crate::render::contrast::ratio(
                        [text.color[0], text.color[1], text.color[2]],
                        bg
                    ) >= 4.499
                );
            }
        }
    }
    #[test]
    fn favourite_star_keyboard_and_saved_section_are_distinct_from_selection() {
        use super::*;
        let settings = Settings {
            favourite_theme_ids: vec!["nord".into(), "custom-missing".into()],
            ..Default::default()
        };
        let mut panel = Ui::new(Kind::Themes, [0.; 2]);
        panel.expanded.insert("\0favourites".into());
        let rows = panel.rows(&crate::theme::builtins(), &settings);
        let index = rows
            .iter()
            .position(|r| theme_id(r) == Some("nord"))
            .unwrap();
        assert!(rows[index].favourite);
        panel.focus_row(index, &rows);
        assert_eq!(
            panel.key(&Key::Named(NamedKey::ArrowRight), None, false, &rows),
            None
        );
        assert_eq!(
            panel.key(&Key::Named(NamedKey::Space), None, false, &rows),
            Some(Action::ToggleFavourite("nord".into()))
        );
        panel.key(&Key::Named(NamedKey::ArrowLeft), None, false, &rows);
        assert_eq!(
            panel.key(&Key::Named(NamedKey::Enter), None, false, &rows),
            Some(Action::Pick("nord".into()))
        );
    }
    #[test]
    fn tab_menu_and_terminal_menu_have_separate_actions() {
        use super::*;
        let settings = Settings::default();
        let body = Ui::new(Kind::Context, [0.; 2]).rows(&[], &settings);
        assert!(!body.iter().any(|r| matches!(
            r.action,
            Action::NewTab | Action::CloseTab | Action::CloseTargetTab | Action::RenameTab
        )));
        assert!(!body.iter().any(|r| r.action == Action::ToggleLineNumbers));
        assert!(Ui::new(Kind::TerminalSettings, [0.; 2])
            .rows(&[], &settings)
            .iter()
            .any(|r| r.action == Action::ToggleLineNumbers));
        let mut tabs = Ui::new(Kind::TabContext, [0.; 2]);
        assert_eq!(tabs.rows(&[], &settings).len(), 1);
        tabs.target_tab = Some(7);
        let rows = tabs.rows(&[], &settings);
        assert_eq!(rows.len(), 3);
        assert!(!rows
            .iter()
            .any(|r| matches!(r.action, Action::Themes | Action::Settings | Action::Copy)));
    }
    #[test]
    fn numeric_prompts_parse_points_edit_at_caret_and_activate_from_space() {
        let settings = Settings::default();
        assert!(
            (numeric_settings(&settings, 1, "16pt").unwrap().font_size - 21.333334).abs() < 0.001
        );
        for bad in ["NaN", "0", "100pt", "invalid"] {
            assert!(numeric_settings(&settings, 1, bad).is_err());
        }
        let mut ui = Ui::new(Kind::Appearance, [0., 0.]);
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
            let scene = tab_scene(&labels, active, &[], false, 800., 1., None);
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
    fn drag_gap_and_scroll_follow_the_shared_strip_geometry() {
        let geometry = strip(800., 1., 16, 0, Some(0));
        assert_eq!(geometry.visible, 4);
        assert_eq!(geometry.step, 190.);
        assert_eq!(drop_gap(10., &geometry, 16), 0);
        assert_eq!(drop_gap(50., &geometry, 16), 0);
        assert_eq!(drop_gap(100., &geometry, 16), 1);
        assert_eq!(drop_gap(150., &geometry, 16), 1);
        assert_eq!(drop_gap(400., &geometry, 16), 2);
        assert_eq!(drop_gap(760., &geometry, 16), 4);
        // Edge positions scroll the window one tab at a time.
        assert_eq!(drag_start(5., 800., 1., &geometry, 16), 0);
        assert_eq!(drag_start(790., 800., 1., &geometry, 16), 1);
        let held = strip(800., 1., 16, 0, Some(2));
        assert_eq!(drag_start(5., 800., 1., &held, 16), 1);
        let scene = tab_scene(
            &["a".into(), "b".into(), "c".into()],
            0,
            &[],
            false,
            800.,
            1.,
            Some(DragView {
                start: 0,
                gap: 2,
                pointer_x: 100.,
            }),
        );
        // Strip background, active-tab fill, destination band, insertion marker
        // and the four edges of the dragged-tab outline.
        assert_eq!(scene.rectangles.len(), 8);
        assert!(scene
            .rectangles
            .iter()
            .any(|r| r.color[3] == DRAG_BAND_ALPHA));
    }
    #[test]
    fn dragged_tab_outline_follows_the_pointer_within_the_strip() {
        let geometry = strip(800., 1., 16, 0, Some(0));
        // Centred on the pointer, clamped to the strip at both edges.
        assert_eq!(ghost_rect(200., 800., 1., &geometry), [106., 0., 188., 32.]);
        assert_eq!(ghost_rect(-40., 800., 1., &geometry), [0., 0., 188., 32.]);
        assert_eq!(ghost_rect(900., 800., 1., &geometry), [572., 0., 188., 32.]);
        let two = strip(800., 1., 2, 0, Some(0));
        let scene = tab_scene(
            &["a".into(), "b".into()],
            0,
            &[],
            false,
            800.,
            1.,
            Some(DragView {
                start: 0,
                gap: 2,
                pointer_x: 900.,
            }),
        );
        let ghost = ghost_rect(900., 800., 1., &two);
        assert!(scene
            .rectangles
            .iter()
            .any(|r| r.rect == [ghost[0], 0., ghost[2], DRAG_OUTLINE]));
    }
    #[test]
    fn destination_band_sits_where_the_tab_will_land() {
        let geometry = strip(800., 1., 16, 0, Some(0));
        let (band, marker) = drop_rects(2, &geometry, 800., 1.);
        assert_eq!(band, [380., 0., 188., 32.]);
        assert_eq!(marker, [379., 0., 2., 32.]);
        // Far end: the slot is off-strip, so the band moves left of the marker.
        let (band, marker) = drop_rects(16, &geometry, 800., 1.);
        assert_eq!(band, [572., 0., 188., 32.]);
        assert_eq!(marker, [759., 0., 2., 32.]);
        // Near end: the band starts at the strip origin.
        let (band, marker) = drop_rects(0, &geometry, 800., 1.);
        assert_eq!(band, [0., 0., 188., 32.]);
        assert_eq!(marker, [0., 0., 2., 32.]);
        // No drag leaves the strip exactly as it was.
        let idle = tab_scene(&["a".into()], 0, &[], false, 800., 1., None);
        assert_eq!(idle.rectangles.len(), 2);
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
        let mut ui = Ui::new(Kind::Appearance, [0., 0.]);
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
        assert!(ui
            .rows(&themes, &settings)
            .iter()
            .any(|row| row.action == Action::Pick("nord".into())));
    }
}
