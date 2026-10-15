//! Protected report fields: secret transport is separate from all terminal/plain/JSON output.
#[path = "../../../src/privacy/wire.rs"]
mod wire;
use crate::presentation::Section;
use serde_json::{json, Value};
pub use wire::MASK;
const FIRST: u32 = 0xF0000;
fn safe(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            !c.is_control() && !(FIRST..FIRST + wire::MAX_FIELDS as u32).contains(&(*c as u32))
        })
        .take(512)
        .collect()
}
pub fn values(data: &Value) -> Vec<(&'static str, String)> {
    let identity = match (std::env::var("USERNAME"), std::env::var("COMPUTERNAME")) {
        (Ok(user), Ok(host)) if !user.is_empty() && !host.is_empty() => {
            safe(&format!("{user}@{host}"))
        }
        _ => "Unavailable".into(),
    };
    let interfaces = data
        .as_array()
        .and_then(|items| {
            items.iter().find(|item| {
                item["type"]
                    .as_str()
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("localip"))
            })
        })
        .and_then(|item| item["result"].as_array());
    let mut addresses = Vec::new();
    let mut macs = Vec::new();
    for interface in interfaces.into_iter().flatten().take(16) {
        for name in ["ipv4", "ipv6"] {
            if let Some(address) = interface[name].as_str().filter(|text| !text.is_empty()) {
                let base = address
                    .split('/')
                    .next()
                    .unwrap_or("")
                    .split('%')
                    .next()
                    .unwrap_or("");
                if base.parse::<std::net::IpAddr>().is_ok() {
                    addresses.push(safe(address));
                }
            }
        }
        if let Some(mac) = interface["mac"].as_str() {
            let parts = mac.split([':', '-']).collect::<Vec<_>>();
            if parts.len() == 6
                && parts
                    .iter()
                    .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                macs.push(safe(mac));
            }
        }
    }
    let list = |items: Vec<String>| {
        if items.is_empty() {
            "Unavailable".into()
        } else {
            safe(&items.join(", "))
        }
    };
    vec![
        ("Identity", identity),
        ("Local IP", list(addresses)),
        ("MAC", list(macs)),
    ]
}
pub fn prepare(
    sections: &mut [Section],
    values: &[(&str, String)],
    mode: &str,
    interactive: bool,
) -> Vec<String> {
    for section in sections.iter_mut() {
        for (_, value) in &mut section.fields {
            *value = safe(value);
        }
    }
    let present = values
        .iter()
        .filter(|(name, value)| {
            value != "Unavailable"
                && sections
                    .iter()
                    .any(|section| section.fields.iter().any(|(label, _)| label == name))
        })
        .collect::<Vec<_>>();
    let tokens = if mode != "off" && interactive && !present.is_empty() {
        register(
            &present
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    for section in sections {
        for (name, output) in &mut section.fields {
            if let Some((_, value)) = values.iter().find(|(label, _)| *label == name) {
                *output = if mode == "off" || value == "Unavailable" {
                    value.clone()
                } else if let Some(index) = present
                    .iter()
                    .position(|(label, _)| *label == name)
                    .filter(|index| *index < tokens.len())
                {
                    char::from_u32(FIRST + index as u32)
                        .unwrap()
                        .to_string()
                        .repeat(5)
                } else {
                    MASK.into()
                };
            }
        }
    }
    tokens
}
pub fn annotate(line: &str, tokens: &[String]) -> String {
    let mut out = String::new();
    for ch in line.chars() {
        if let Some(index) = (ch as u32)
            .checked_sub(FIRST)
            .filter(|index| *index < wire::MAX_FIELDS as u32)
        {
            if let Some(token) = tokens.get(index as usize) {
                out.push_str("\x1b]777;JunaPrivate;");
                out.push_str(token);
                out.push_str("\x1b\\░\x1b]777;JunaPrivate;0\x1b\\");
            } else {
                out.push('░');
            }
        } else {
            out.push(ch);
        }
    }
    out
}
#[cfg(windows)]
fn register(values: &[&str]) -> Result<Vec<String>, String> {
    use std::time::{Duration, Instant};
    let path = std::env::var("JUNA_PRIVACY_PIPE").map_err(|_| "Privacy unavailable")?;
    let token = std::env::var("JUNA_PRIVACY_TOKEN").map_err(|_| "Privacy unavailable")?;
    let server = std::env::var("JUNA_PRIVACY_SERVER")
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or("Privacy unavailable")?;
    if token.len() != 64
        || values.len() > wire::MAX_FIELDS
        || values.iter().any(|value| value.len() > wire::MAX_VALUE)
    {
        return Err("Invalid protected report".into());
    }
    let pipe = wire::windows_pipe::client(&path, server)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let bytes = serde_json::to_vec(&json!({"token": token, "fields": values}))
        .map_err(|_| "Invalid protected report")?;
    if bytes.len() > wire::MAX_MESSAGE {
        return Err("Protected report too large".into());
    }
    pipe.write_all(&(bytes.len() as u32).to_le_bytes(), None, deadline)?;
    pipe.write_all(&bytes, None, deadline)?;
    let mut prefix = [0; 4];
    pipe.read_exact(&mut prefix, None, deadline)?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length > 4096 {
        return Err("Invalid privacy acknowledgement".into());
    }
    let mut bytes = vec![0; length];
    pipe.read_exact(&mut bytes, None, deadline)?;
    let ids: Vec<String> =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid privacy acknowledgement")?;
    if ids.len() != values.len()
        || ids.iter().any(|id| {
            id.len() != 32
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
    {
        return Err("Invalid privacy field tokens".into());
    }
    pipe.write_all(&[1], None, deadline)?;
    Ok(ids)
}
#[cfg(not(windows))]
fn register(_: &[&str]) -> Result<Vec<String>, String> {
    Err("Privacy IPC requires Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_and_json_fields_never_include_protected_values_without_explicit_off() {
        let values = vec![
            ("Identity", "Fixture@TestHost".into()),
            ("Local IP", "192.0.2.55".into()),
            ("MAC", "02:00:00:00:00:01".into()),
        ];
        for mode in ["shift-hover", "hover", "always-hidden"] {
            let mut sections = vec![Section {
                name: "Network",
                fields: values
                    .iter()
                    .map(|(name, _)| (name.to_string(), MASK.into()))
                    .collect(),
            }];
            assert!(prepare(&mut sections, &values, mode, false).is_empty());
            let text = crate::presentation::lines(&sections, 16, true).join("\n");
            assert!(!text.contains("192.0.2.55"));
            assert!(!text.contains("Fixture@TestHost"));
            assert!(!text.contains("02:00"));
            assert_eq!(sections[0].fields[1].1, MASK);
            prepare(&mut sections, &values, "off", false);
            assert_eq!(sections[0].fields[1].1, "192.0.2.55");
        }
    }
    #[test]
    fn protected_layout_tokens_have_five_columns_and_never_escape_annotation() {
        use unicode_width::UnicodeWidthStr;
        let token = "0123456789abcdef0123456789abcdef".to_owned();
        let placeholder = char::from_u32(FIRST).unwrap().to_string().repeat(5);
        assert_eq!(placeholder.width(), MASK.width());
        assert_eq!(MASK.width(), 5);
        for width in [16, 32, 48, 120] {
            let sections = vec![Section {
                name: "Network",
                fields: vec![("Local IP".into(), placeholder.clone())],
            }];
            let output = crate::presentation::lines(&sections, width, true)
                .iter()
                .map(|line| annotate(line, std::slice::from_ref(&token)))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!output.contains(&placeholder));
            assert_eq!(output.matches('░').count(), 5);
            assert_eq!(output.matches(&format!("JunaPrivate;{token}")).count(), 5);
            assert_eq!(output.matches("JunaPrivate;0\x1b\\").count(), 5);
        }
        assert!(safe(&format!("public{placeholder}"))
            .chars()
            .all(|c| (c as u32) < FIRST));
    }
    #[test]
    fn ip_and_mac_values_come_from_detector_data_not_invented_defaults() {
        let data = json!([{"type":"LocalIp", "result":[{"ipv4":"192.0.2.55/24", "ipv6":"2001:db8::55", "mac":"02:00:00:00:00:01"}]}]);
        let found = values(&data);
        assert_eq!(found[1].1, "192.0.2.55/24, 2001:db8::55");
        assert_eq!(found[2].1, "02:00:00:00:00:01");
        let empty = values(&json!([]));
        assert_eq!(empty[1].1, "Unavailable");
        assert_eq!(empty[2].1, "Unavailable");
    }
}
