//! Owned report/preferences and Unicode-aware text layout, separate from detector execution.
use serde_json::Value;
use std::io::Read;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Brief,
    Standard,
    Detailed,
}
impl Profile {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "brief" => Some(Self::Brief),
            "standard" => Some(Self::Standard),
            "detailed" => Some(Self::Detailed),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::Standard => "standard",
            Self::Detailed => "detailed",
        }
    }
}
pub struct Preferences {
    pub privacy_mode: String,
    pub profile: Profile,
    pub logo: String,
    pub theme: String,
    pub font: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            privacy_mode: "shift-hover".into(),
            profile: Profile::Standard,
            logo: "compact".into(),
            theme: "catppuccin-mocha".into(),
            font: "IBM Plex Mono (16px)".into(),
        }
    }
}
pub fn preferences() -> Preferences {
    let result = (|| {
        let path =
            std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Juna/settings.json");
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Some(Preferences::default())
            }
            Err(_) => return None,
        };
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes).ok()?;
        if bytes.len() > 65536 {
            return None;
        }
        let value: Value = serde_json::from_slice(&bytes).ok()?;
        decode_preferences(&value)
    })();
    result.unwrap_or_else(|| {
        eprintln!("junafetch: preferences unavailable; using defaults");
        Preferences::default()
    })
}
fn decode_preferences(value: &Value) -> Option<Preferences> {
    if value["schema_version"].as_u64() != Some(1) {
        return None;
    }
    let profile = Profile::parse(match value.get("junafetch_profile") {
        Some(v) => v.as_str()?,
        None => "standard",
    })?;
    let logo = match value.get("junafetch_logo") {
        Some(v) => v.as_str()?,
        None => "compact",
    };
    if !matches!(logo, "compact" | "full" | "none") {
        return None;
    }
    let theme = value["active_theme_id"]
        .as_str()
        .unwrap_or("catppuccin-mocha");
    let font = value["font_family"].as_str().unwrap_or("IBM Plex Mono");
    let size = value["font_size"].as_f64().unwrap_or(16.);
    if !(8.0..=40.0).contains(&size)
        || [theme, font]
            .iter()
            .any(|v| v.len() > 128 || v.chars().any(char::is_control))
    {
        return None;
    }
    Some(Preferences {
        privacy_mode: value["privacy_mode"]
            .as_str()
            .filter(|mode| matches!(*mode, "shift-hover" | "hover" | "always-hidden" | "off"))
            .unwrap_or("shift-hover")
            .into(),
        profile,
        logo: logo.into(),
        theme: theme.into(),
        font: format!("{font} ({size}px)"),
    })
}
pub struct Section {
    pub name: &'static str,
    pub fields: Vec<(String, String)>,
}
fn result<'a>(data: &'a Value, kind: &str) -> &'a Value {
    data.as_array()
        .and_then(|items| items.iter().find(|item| item["type"] == kind))
        .map(|item| &item["result"])
        .unwrap_or(&Value::Null)
}
fn row(name: &str, value: String) -> (String, String) {
    (name.into(), value)
}
fn frequency(value: &Value) -> String {
    value
        .as_f64()
        .filter(|v| *v > 0.)
        .map(|v| format!("{:.2} GHz", v / 1000.))
        .unwrap_or_else(|| "Unavailable".into())
}
pub fn sections(data: &Value, profile: Profile, prefs: &Preferences) -> Vec<Section> {
    if profile == Profile::Brief {
        return vec![Section {
            name: "Summary",
            fields: super::report(data, false),
        }];
    }
    let brief = super::report(data, false);
    let os = result(data, "OS");
    let kernel = result(data, "Kernel");
    let system = Section {
        name: "System",
        fields: vec![
            row(
                "OS",
                format!(
                    "{} · {}",
                    super::text(os, "prettyName"),
                    super::text(kernel, "architecture")
                ),
            ),
            row(
                "Kernel",
                format!(
                    "{} {}",
                    super::text(kernel, "name"),
                    super::text(kernel, "release")
                ),
            ),
            row("Uptime", brief[5].1.clone()),
            row(
                "Locale",
                result(data, "Locale")
                    .as_str()
                    .map(super::clean)
                    .unwrap_or_else(|| "Unavailable".into()),
            ),
            row("Identity", crate::privacy::MASK.into()),
        ],
    };
    let cpu = result(data, "CPU");
    let clocks = format!("{} (maximum)", frequency(&cpu["frequency"]["max"]));
    let mut hardware = Section {
        name: "Hardware",
        fields: vec![row("CPU", format!("{} · {clocks}", brief[1].1))],
    };
    if let Some(gpus) = result(data, "GPU").as_array() {
        for (i, gpu) in gpus.iter().take(16).enumerate() {
            let name = super::text(gpu, "name");
            let kind = gpu["type"]
                .as_str()
                .filter(|v| matches!(*v, "Discrete" | "Integrated" | "Virtual" | "Software"))
                .unwrap_or("Unclassified");
            let kind = if kind == "Unclassified" && name.to_ascii_lowercase().contains("virtual") {
                "Virtual (reported name)"
            } else {
                kind
            };
            let vram = gpu["memory"]["dedicated"]["total"]
                .as_u64()
                .filter(|v| *v > 0)
                .map(|v| format!(" · {:.1} GiB VRAM", v as f64 / 1073741824.))
                .unwrap_or_default();
            hardware.fields.push(row(
                &format!("GPU {}", i + 1),
                format!(
                    "{name} [{kind}]{vram}{}",
                    gpu["frequency"]
                        .as_f64()
                        .filter(|v| *v > 0.)
                        .map(|v| format!(" · {:.2} GHz", v / 1000.))
                        .unwrap_or_default()
                ),
            ));
            if profile == Profile::Detailed {
                hardware.fields.push(row(
                    &format!("GPU {} driver", i + 1),
                    super::text(gpu, "driver"),
                ));
            }
        }
        if gpus.is_empty() {
            hardware.fields.push(row("GPU", "Unavailable".into()));
        }
    } else {
        hardware.fields.push(row("GPU", "Unavailable".into()));
    }
    if let Some(displays) = result(data, "Display").as_array() {
        for (i, display) in displays.iter().take(16).enumerate() {
            let output = &display["output"];
            let geometry = match (
                output["width"].as_u64(),
                output["height"].as_u64(),
                output["refreshRate"].as_f64(),
            ) {
                (Some(w), Some(h), Some(hz)) => format!("{w}×{h} @ {hz:.0} Hz"),
                _ => "Unavailable".into(),
            };
            let scale = output["dpi"]
                .as_f64()
                .filter(|v| *v > 0.)
                .map(|dpi| format!(" · {:.0}%", dpi / 96. * 100.))
                .unwrap_or_default();
            let physical = &display["physical"];
            let inches = match (physical["width"].as_f64(), physical["height"].as_f64()) {
                (Some(w), Some(h)) if w > 0. && h > 0. => {
                    format!(" · {:.1} in", (w * w + h * h).sqrt() / 25.4)
                }
                _ => String::new(),
            };
            let kind = super::text(display, "type");
            let primary = if display["primary"].as_bool() == Some(true) {
                " · primary"
            } else {
                ""
            };
            hardware.fields.push(row(
                &format!("Display {}", i + 1),
                format!(
                    "{} · {geometry}{scale}{inches} [{kind}]{primary}",
                    super::text(display, "name")
                ),
            ));
        }
        if displays.is_empty() {
            hardware.fields.push(row("Displays", "Unavailable".into()));
        }
    } else {
        hardware.fields.push(row("Displays", "Unavailable".into()));
    }
    hardware.fields.extend([
        row("Memory", super::usage(result(data, "Memory"))),
        row("Swap", super::usage(result(data, "Swap"))),
    ]);
    if profile == Profile::Detailed {
        let bios = result(data, "BIOS");
        let board = result(data, "Board");
        hardware.fields.push(row(
            "Board",
            format!(
                "{} {}",
                super::text(board, "vendor"),
                super::text(board, "name")
            ),
        ));
        hardware.fields.push(row(
            "BIOS",
            format!(
                "{} {}",
                super::text(bios, "vendor"),
                super::text(bios, "version")
            ),
        ));
        if let Some(batteries) = result(data, "Battery").as_array().filter(|v| !v.is_empty()) {
            for (i, battery) in batteries.iter().take(8).enumerate() {
                let charge = battery["capacity"]
                    .as_f64()
                    .map(|v| format!("{v:.0}%"))
                    .unwrap_or_else(|| "Unavailable".into());
                hardware.fields.push(row(
                    &format!("Battery {}", i + 1),
                    format!("{charge} · {}", super::text(battery, "status")),
                ));
            }
        } else {
            hardware.fields.push(row("Battery", "Unavailable".into()));
        }
    }
    let mut storage = Section {
        name: "Storage",
        fields: Vec::new(),
    };
    if let Some(disks) = result(data, "Disk").as_array() {
        for (i, disk) in disks.iter().take(32).enumerate() {
            let mount = disk["mountpoint"].as_str().unwrap_or("");
            let bytes = mount.as_bytes();
            let label = if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                format!("Disk {}:", bytes[0] as char)
            } else {
                format!("Disk {}", i + 1)
            };
            let flags = disk["volumeType"]
                .as_array()
                .map(|types| {
                    types
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|v| matches!(*v, "Regular" | "External" | "ReadOnly" | "Hidden"))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            storage.fields.push(row(
                &label,
                format!(
                    "{} · {}{}",
                    super::usage(&disk["bytes"]),
                    super::text(disk, "filesystem"),
                    if flags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{flags}]")
                    }
                ),
            ));
        }
    }
    if storage.fields.is_empty() {
        storage.fields.push(row("Disks", "Unavailable".into()));
    }
    let wm = result(data, "WM");
    let desktop = Section {
        name: "Desktop",
        fields: vec![
            row(
                "Window mgr",
                format!(
                    "{} {}",
                    super::text(wm, "prettyName"),
                    super::text(wm, "version")
                ),
            ),
            row(
                "WM theme",
                result(data, "WMTheme")
                    .as_str()
                    .map(super::clean)
                    .unwrap_or_else(|| "Unavailable".into()),
            ),
            row("Theme", super::text(result(data, "Theme"), "theme1")),
            row("Icons", super::text(result(data, "Icons"), "icons1")),
            row("Desktop font", super::text(result(data, "Font"), "display")),
            row("Cursor", {
                let cursor = result(data, "Cursor");
                let size = cursor["size"]
                    .as_u64()
                    .map(|size| format!(" ({size}px)"))
                    .unwrap_or_default();
                format!("{}{size}", super::text(cursor, "theme"))
            }),
        ],
    };
    let shell = result(data, "Shell");
    let version = std::env::var("JUNA_SHELL_VERSION")
        .ok()
        .map(|v| super::clean(&v))
        .or_else(|| {
            let name = shell["prettyName"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase();
            let expected = brief[6].1.to_ascii_lowercase();
            if !name.is_empty() && expected.contains(&name) {
                shell["version"]
                    .as_str()
                    .filter(|v| !v.is_empty())
                    .map(super::clean)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "version unavailable".into());
    let inside = std::env::var_os("JUNA_VERSION").is_some();
    let session = Section {
        name: "Session",
        fields: vec![
            row("Shell", format!("{} · {version}", brief[6].1)),
            row(
                "Terminal",
                if inside {
                    format!("Juna {}", brief[7].1)
                } else {
                    "Standalone Junafetch".into()
                },
            ),
            row(
                "Juna theme",
                if inside {
                    super::clean(&prefs.theme)
                } else {
                    "Unavailable outside Juna".into()
                },
            ),
            row(
                "Juna font",
                if inside {
                    super::clean(&prefs.font)
                } else {
                    "Unavailable outside Juna".into()
                },
            ),
        ],
    };
    let network = Section {
        name: "Network",
        fields: vec![
            row("Local IP", crate::privacy::MASK.into()),
            row("MAC", crate::privacy::MASK.into()),
        ],
    };
    vec![system, hardware, storage, desktop, session, network]
}
pub fn wrap(value: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut used = 0;
    for cluster in value.graphemes(true) {
        let cells = cluster.width();
        if used + cells > width && used > 0 {
            lines.push(String::new());
            used = 0;
        }
        if cells <= width {
            lines.last_mut().unwrap().push_str(cluster);
            used += cells;
        }
    }
    lines
}
pub fn lines(sections: &[Section], width: usize, borders: bool) -> Vec<String> {
    let width = width.max(8);
    let mut result = Vec::new();
    for section in sections {
        if !result.is_empty() {
            result.push(String::new());
        }
        if borders {
            let heading = format!(" {} ", section.name);
            result.push(format!(
                "┌{}{}┐",
                heading,
                "─".repeat(width.saturating_sub(heading.width() + 2))
            ));
        } else if section.name != "Summary" {
            result.push(format!("[{}]", section.name));
        }
        let labels = section
            .fields
            .iter()
            .map(|(n, _)| n.width())
            .max()
            .unwrap_or(0)
            .min(16);
        let available = width
            .saturating_sub(labels + if borders { 7 } else { 3 })
            .max(1);
        for (name, value) in &section.fields {
            if width < 32 {
                let inner = width.saturating_sub(if borders { 4 } else { 0 });
                for part in wrap(&format!("{name}: {value}"), inner) {
                    if borders {
                        result.push(format!(
                            "│ {part}{} │",
                            " ".repeat(inner.saturating_sub(part.width()))
                        ));
                    } else {
                        result.push(part);
                    }
                }
                continue;
            }
            for (i, part) in wrap(value, available).into_iter().enumerate() {
                let name = if i == 0 { name.as_str() } else { "" };
                let text = format!(
                    "{name}{} : {part}",
                    " ".repeat(labels.saturating_sub(name.width()))
                );
                if borders {
                    result.push(format!(
                        "│ {text}{} │",
                        " ".repeat(width.saturating_sub(text.width() + 4))
                    ));
                } else {
                    result.push(text);
                }
            }
        }
        if borders {
            result.push(format!("└{}┘", "─".repeat(width - 2)));
        }
    }
    result
}
#[cfg(test)]
mod tests {
    #[test]
    fn report_is_grouped_and_protected_fields_never_enter_output() {
        let data = serde_json::json!([{"type":"OS","result":{"prettyName":"Windows","hostname":"SECRET_HOST"}},{"type":"Disk","result":[{"mountpoint":"C:\\","name":"SECRET_VOLUME","bytes":{"used":10,"total":20},"filesystem":"NTFS"}]},{"type":"LocalIP","result":[{"ipv4":"SECRET_IP"}]}]);
        let sections = super::sections(&data, super::Profile::Standard, &Default::default());
        assert_eq!(
            sections.iter().map(|s| s.name).collect::<Vec<_>>(),
            ["System", "Hardware", "Storage", "Desktop", "Session", "Network"]
        );
        let output = super::lines(&sections, 48, true).join("\n");
        assert!(!output.contains("SECRET"));
        assert!(output.contains(crate::privacy::MASK));
        assert!(output.contains("NTFS"));
        assert_eq!(
            super::sections(&data, super::Profile::Brief, &Default::default())[0]
                .fields
                .len(),
            8
        );
    }
    #[test]
    fn unicode_wrapping_and_borders_fit_the_column_budget() {
        use unicode_width::UnicodeWidthStr;
        let sections = vec![super::Section {
            name: "Hardware",
            fields: vec![("Display".into(), "長い monitor e\u{301} text".into())],
        }];
        for width in [16, 28, 48, 80] {
            for line in super::lines(&sections, width, true) {
                assert!(line.width() <= width, "{line}");
            }
        }
    }
    #[test]
    fn preference_fields_are_bounded_validated_and_older_files_default() {
        let old = serde_json::json!({"schema_version":1});
        assert_eq!(
            super::decode_preferences(&old).unwrap().profile,
            super::Profile::Standard
        );
        let bad = serde_json::json!({"schema_version":1,"junafetch_profile":"run-command"});
        assert!(super::decode_preferences(&bad).is_none());
        let bad = serde_json::json!({"schema_version":1,"font_family":"secret\u{001b}"});
        assert!(super::decode_preferences(&bad).is_none());
    }
}
