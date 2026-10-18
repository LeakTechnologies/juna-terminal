//! Pure keyboard encoding using xterm's PC-style key sequences.
use crate::vt::InputModes;
use winit::keyboard::{Key, ModifiersState, NamedKey};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    Step(i8),
    Reset,
}
pub fn zoom(
    key: &Key,
    physical: winit::keyboard::PhysicalKey,
    modifiers: ModifiersState,
) -> Option<Zoom> {
    use winit::keyboard::{KeyCode, PhysicalKey};
    if !modifiers.control_key() || modifiers.alt_key() || modifiers.super_key() {
        return None;
    }
    match key {
        Key::Character(s) if s == "+" || s == "=" => Some(Zoom::Step(1)),
        Key::Character(s) if s == "-" => Some(Zoom::Step(-1)),
        Key::Character(s) if s == "0" => Some(Zoom::Reset),
        _ => match physical {
            PhysicalKey::Code(KeyCode::NumpadAdd) => Some(Zoom::Step(1)),
            PhysicalKey::Code(KeyCode::NumpadSubtract) => Some(Zoom::Step(-1)),
            _ => None,
        },
    }
}

pub fn encode(
    key: &Key,
    text: Option<&str>,
    modifiers: ModifiersState,
    modes: InputModes,
) -> Option<Vec<u8>> {
    let shift = modifiers.shift_key();
    let alt = modifiers.alt_key();
    let ctrl = modifiers.control_key();
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
    let mut out = match key {
        Key::Character(s) => {
            // Produced non-ASCII text wins for AltGr; Ctrl+Alt is not blindly a shortcut.
            let produced = text.unwrap_or(s);
            if ctrl
                && !(alt
                    && produced
                        .chars()
                        .any(|c| !c.is_ascii() || "@€{}[]\\|~".contains(c)))
            {
                let c = s.chars().next()?.to_ascii_uppercase();
                let code = match c {
                    'A'..='Z' => c as u8 - b'A' + 1,
                    '@' | ' ' | '2' => 0,
                    '[' | '3' => 27,
                    '\\' | '4' => 28,
                    ']' | '5' => 29,
                    '^' | '6' => 30,
                    '_' | '7' => 31,
                    '?' | '8' => 127,
                    _ => return None,
                };
                vec![code]
            } else {
                produced.as_bytes().to_vec()
            }
        }
        Key::Named(NamedKey::Space) => vec![if ctrl { 0 } else { b' ' }],
        Key::Named(NamedKey::Enter) => vec![b'\r'],
        Key::Named(NamedKey::Backspace) => vec![if ctrl { 8 } else { 127 }],
        Key::Named(NamedKey::Tab) => {
            if shift {
                b"\x1b[Z".to_vec()
            } else {
                vec![b'\t']
            }
        }
        Key::Named(NamedKey::Escape) => vec![27],
        Key::Named(n) => {
            let final_byte = match n {
                NamedKey::ArrowUp => 'A',
                NamedKey::ArrowDown => 'B',
                NamedKey::ArrowRight => 'C',
                NamedKey::ArrowLeft => 'D',
                NamedKey::Home => 'H',
                NamedKey::End => 'F',
                NamedKey::F1 => 'P',
                NamedKey::F2 => 'Q',
                NamedKey::F3 => 'R',
                NamedKey::F4 => 'S',
                _ => '\0',
            };
            if final_byte != '\0' {
                if modifier > 1 {
                    format!("\x1b[1;{modifier}{final_byte}").into_bytes()
                } else {
                    format!(
                        "\x1b{}{final_byte}",
                        if modes.application_cursor
                            || matches!(
                                n,
                                NamedKey::F1 | NamedKey::F2 | NamedKey::F3 | NamedKey::F4
                            )
                        {
                            'O'
                        } else {
                            '['
                        }
                    )
                    .into_bytes()
                }
            } else {
                let number = match n {
                    NamedKey::Insert => 2,
                    NamedKey::Delete => 3,
                    NamedKey::PageUp => 5,
                    NamedKey::PageDown => 6,
                    NamedKey::F5 => 15,
                    NamedKey::F6 => 17,
                    NamedKey::F7 => 18,
                    NamedKey::F8 => 19,
                    NamedKey::F9 => 20,
                    NamedKey::F10 => 21,
                    NamedKey::F11 => 23,
                    NamedKey::F12 => 24,
                    _ => return None,
                };
                if modifier > 1 {
                    format!("\x1b[{number};{modifier}~").into_bytes()
                } else {
                    format!("\x1b[{number}~").into_bytes()
                }
            }
        }
        _ => return None,
    };
    if alt
        && !ctrl
        && !matches!(
            key,
            Key::Named(
                NamedKey::ArrowUp
                    | NamedKey::ArrowDown
                    | NamedKey::ArrowLeft
                    | NamedKey::ArrowRight
                    | NamedKey::Home
                    | NamedKey::End
                    | NamedKey::Insert
                    | NamedKey::Delete
                    | NamedKey::PageUp
                    | NamedKey::PageDown
                    | NamedKey::F1
                    | NamedKey::F2
                    | NamedKey::F3
                    | NamedKey::F4
                    | NamedKey::F5
                    | NamedKey::F6
                    | NamedKey::F7
                    | NamedKey::F8
                    | NamedKey::F9
                    | NamedKey::F10
                    | NamedKey::F11
                    | NamedKey::F12
            )
        )
    {
        out.insert(0, 27);
    }
    Some(out)
}

pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    // Clipboard escapes must never become arbitrary terminal controls.
    let clean = text.replace("\r\n", "\n").replace('\r', "\n");
    let clean: String = clean
        .chars()
        .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
        .collect();
    if bracketed {
        format!("\x1b[200~{clean}\x1b[201~").into_bytes()
    } else {
        clean.replace('\n', "\r").into_bytes()
    }
}

pub fn keypad(key: winit::keyboard::PhysicalKey, modes: InputModes) -> Option<Vec<u8>> {
    use winit::keyboard::{KeyCode, PhysicalKey};
    if !modes.application_keypad {
        return None;
    }
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    let final_byte = match code {
        KeyCode::Numpad0 => b'p',
        KeyCode::Numpad1 => b'q',
        KeyCode::Numpad2 => b'r',
        KeyCode::Numpad3 => b's',
        KeyCode::Numpad4 => b't',
        KeyCode::Numpad5 => b'u',
        KeyCode::Numpad6 => b'v',
        KeyCode::Numpad7 => b'w',
        KeyCode::Numpad8 => b'x',
        KeyCode::Numpad9 => b'y',
        KeyCode::NumpadAdd => b'k',
        KeyCode::NumpadSubtract => b'm',
        KeyCode::NumpadMultiply => b'j',
        KeyCode::NumpadDivide => b'o',
        KeyCode::NumpadDecimal => b'n',
        KeyCode::NumpadEnter => b'M',
        _ => return None,
    };
    Some(vec![27, b'O', final_byte])
}
pub fn mouse(
    button: u8,
    row: u16,
    col: u16,
    release: bool,
    modifiers: ModifiersState,
    sgr: bool,
) -> Vec<u8> {
    let code = button
        + 4 * u8::from(modifiers.shift_key())
        + 8 * u8::from(modifiers.alt_key())
        + 16 * u8::from(modifiers.control_key());
    if sgr {
        format!(
            "\x1b[<{code};{};{}{}",
            col + 1,
            row + 1,
            if release { 'm' } else { 'M' }
        )
        .into_bytes()
    } else {
        vec![
            27,
            b'[',
            b'M',
            if release { 35 } else { code + 32 },
            (col.min(222) + 33) as u8,
            (row.min(222) + 33) as u8,
        ]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn interrupt_keys_keep_child_control_semantics() {
        use super::*;
        for (key, byte) in [("c", 3), ("z", 26)] {
            assert_eq!(
                encode(
                    &Key::Character(key.into()),
                    None,
                    ModifiersState::CONTROL,
                    InputModes::default()
                ),
                Some(vec![byte])
            );
        }
    }
    #[test]
    fn zoom_shortcuts_respect_altgr_and_keypad() {
        use super::*;
        use winit::keyboard::{KeyCode, PhysicalKey};
        let physical = PhysicalKey::Code(KeyCode::Equal);
        for (key, expected) in [
            ("+", Zoom::Step(1)),
            ("=", Zoom::Step(1)),
            ("-", Zoom::Step(-1)),
            ("0", Zoom::Reset),
        ] {
            let key = Key::Character(key.into());
            assert_eq!(
                zoom(&key, physical, ModifiersState::CONTROL),
                Some(expected)
            );
            assert_eq!(zoom(&key, physical, ModifiersState::empty()), None);
            assert_eq!(
                zoom(
                    &key,
                    physical,
                    ModifiersState::CONTROL | ModifiersState::ALT
                ),
                None
            );
        }
        assert_eq!(
            zoom(
                &Key::Unidentified(winit::keyboard::NativeKey::Unidentified),
                PhysicalKey::Code(KeyCode::NumpadAdd),
                ModifiersState::CONTROL
            ),
            Some(Zoom::Step(1))
        );
    }
    use super::*;
    #[test]
    fn spaces_controls_and_composition() {
        let modes = InputModes::default();
        assert_eq!(
            encode(
                &Key::Named(NamedKey::Space),
                Some(" "),
                ModifiersState::empty(),
                modes
            ),
            Some(vec![32])
        );
        assert_eq!(
            encode(
                &Key::Character("c".into()),
                Some("c"),
                ModifiersState::CONTROL,
                modes
            ),
            Some(vec![3])
        );
        assert_eq!(
            encode(
                &Key::Character("x".into()),
                Some("´x"),
                ModifiersState::empty(),
                modes
            ),
            Some("´x".as_bytes().to_vec())
        );
        assert_eq!(
            encode(
                &Key::Character("q".into()),
                Some("@"),
                ModifiersState::CONTROL | ModifiersState::ALT,
                modes
            ),
            Some(vec![b'@'])
        );
    }
    #[test]
    fn navigation_and_modes() {
        assert_eq!(
            encode(
                &Key::Named(NamedKey::ArrowUp),
                None,
                ModifiersState::empty(),
                InputModes {
                    application_cursor: true,
                    ..Default::default()
                }
            ),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            encode(
                &Key::Named(NamedKey::ArrowLeft),
                None,
                ModifiersState::CONTROL,
                InputModes::default()
            ),
            Some(b"\x1b[1;5D".to_vec())
        );
        assert_eq!(paste("a\r\nb\x1b[31m", true), b"\x1b[200~a\nb[31m\x1b[201~");
    }
    #[test]
    fn keypad_and_mouse_modes_encode_exact_bytes() {
        use winit::keyboard::{KeyCode, PhysicalKey};
        assert_eq!(
            keypad(
                PhysicalKey::Code(KeyCode::NumpadEnter),
                InputModes {
                    application_keypad: true,
                    ..Default::default()
                }
            ),
            Some(b"\x1bOM".to_vec())
        );
        assert_eq!(
            mouse(0, 3, 4, false, ModifiersState::CONTROL, true),
            b"\x1b[<16;5;4M"
        );
        assert_eq!(
            mouse(0, 3, 4, true, ModifiersState::empty(), true),
            b"\x1b[<0;5;4m"
        );
    }
}
