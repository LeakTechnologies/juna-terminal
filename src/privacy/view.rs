//! Pure committed-view, reveal and overlay policy. Protected values never enter engine cells.
use crate::{
    render::overlay::{Overlay, Rectangle, Text},
    theme::Theme,
    vt::{RenderSnapshot, TerminalState},
};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Default)]
pub struct TabPrivacy {
    values: HashMap<String, String>,
    order: VecDeque<String>,
    committed: Option<RenderSnapshot>,
    markers: HashMap<u32, String>,
    revision: u64,
    offset: usize,
    top: usize,
}
impl TabPrivacy {
    pub fn accept(&mut self, fields: Vec<(String, String)>) {
        for (token, value) in fields {
            if token.len() != 32
                || !token.bytes().all(|b| b.is_ascii_hexdigit())
                || value.len() > super::wire::MAX_VALUE
            {
                continue;
            }
            if !self.values.contains_key(&token) {
                self.order.push_back(token.clone());
            }
            self.values.insert(token, value);
            while self.order.len() > 128 {
                if let Some(old) = self.order.pop_front() {
                    self.values.remove(&old);
                }
            }
        }
    }
    pub fn update(&mut self, terminal: &TerminalState, offset: usize) {
        self.update_inner(terminal, offset, false);
    }
    pub fn commit_timeout(&mut self, terminal: &TerminalState, offset: usize) {
        self.update_inner(terminal, offset, true);
    }
    fn update_inner(&mut self, terminal: &TerminalState, offset: usize, allow_sync: bool) {
        let changed = self.committed.as_ref().is_none_or(|view| {
            view.cols != terminal.grid().cols() || view.rows != terminal.grid().rows()
        }) || self.revision != terminal.revision()
            || self.offset != offset;
        if changed && (!terminal.input_modes().synchronized_output || allow_sync) {
            let snapshot = terminal.snapshot(offset);
            self.markers.clear();
            for cell in &snapshot.cells {
                if cell.private_tag != 0 && !self.markers.contains_key(&cell.private_tag) {
                    if let Some(token) = terminal.private_token(cell.private_tag) {
                        self.markers.insert(cell.private_tag, token.to_owned());
                    }
                }
            }
            self.committed = Some(snapshot);
            self.revision = terminal.revision();
            self.offset = offset;
            self.top = if terminal.alternate_active() {
                0
            } else {
                terminal.scrollback_len().saturating_sub(offset)
            };
        }
        if self.committed.is_none() {
            self.committed = Some(
                TerminalState::new(terminal.grid().cols(), terminal.grid().rows()).snapshot(0),
            );
        }
    }
    pub fn snapshot(&self) -> Option<&RenderSnapshot> {
        self.committed.as_ref()
    }
    pub fn top(&self) -> usize {
        self.top
    }
    pub fn field_at(&self, row: u16, col: u16) -> Option<&str> {
        let view = self.committed.as_ref()?;
        if row >= view.rows || col >= view.cols {
            return None;
        }
        let cell = &view.cells[row as usize * view.cols as usize + col as usize];
        if cell.content.first_scalar() != Some('░' as u32) {
            return None;
        }
        let token = self.markers.get(&cell.private_tag)?;
        self.values.contains_key(token).then_some(token.as_str())
    }
    pub fn value(&self, token: &str) -> Option<&str> {
        self.values.get(token).map(String::as_str)
    }
}

pub struct Reveal {
    pub token: Option<String>,
    shift: bool,
    other: bool,
    focused: bool,
    buttons: HashSet<u32>,
}
impl Default for Reveal {
    fn default() -> Self {
        Self {
            token: None,
            shift: false,
            other: false,
            focused: true,
            buttons: HashSet::new(),
        }
    }
}
impl Reveal {
    pub fn hide(&mut self) {
        self.token = None;
    }
    pub fn modifiers(&mut self, shift: bool, other: bool) {
        self.shift = shift;
        self.other = other;
        self.hide();
    }
    pub fn focus(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.buttons.clear();
        }
        self.hide();
    }
    pub fn button(&mut self, button: u32, pressed: bool) {
        if pressed {
            self.buttons.insert(button);
        } else {
            self.buttons.remove(&button);
        }
        self.hide();
    }
    pub fn moved(&mut self, token: Option<&str>, mode: &str) {
        let allowed = self.focused
            && !self.other
            && self.buttons.is_empty()
            && (self.shift || matches!(mode, "hover" | "off"));
        self.token = if allowed {
            token.map(str::to_owned)
        } else {
            None
        };
    }
}

pub fn popup(
    value: &str,
    pointer: [f32; 2],
    width: f32,
    height: f32,
    scale: f32,
    theme: &Theme,
) -> Overlay {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;
    let mut scene = Overlay::default();
    let pad = 12. * scale;
    let popup_width = ((value.width().clamp(16, 52) as f32) * 10. * scale + 2. * pad)
        .min(width - 2. * pad)
        .max(1.);
    let columns = ((popup_width - 2. * pad) / (10. * scale)).max(1.) as usize;
    let mut lines = vec![String::new()];
    let mut columns_used = 0;
    for grapheme in value.graphemes(true) {
        let count = grapheme.width();
        if columns_used + count > columns {
            if lines.len() >= 8 {
                let last = lines.last_mut().unwrap();
                if let Some((index, _)) = last.grapheme_indices(true).next_back() {
                    last.truncate(index);
                }
                last.push('…');
                break;
            }
            lines.push(String::new());
            columns_used = 0;
        }
        lines.last_mut().unwrap().push_str(grapheme);
        columns_used += count;
    }
    let popup_height = ((lines.len() as f32 + 1.) * 24. * scale + 2. * pad).min(height);
    let x = (pointer[0] + pad).min(width - popup_width).max(0.);
    let y = if pointer[1] + pad + popup_height <= height {
        pointer[1] + pad
    } else {
        (pointer[1] - popup_height - pad).max(0.)
    };
    let bg = crate::theme::rgb(&theme.colors.background).unwrap();
    let fg = crate::theme::rgb(&theme.colors.foreground).unwrap();
    let fg = crate::render::contrast::foreground(fg, bg, 4.5);
    let border = crate::theme::rgb(&theme.colors.cursor).unwrap();
    scene.rectangles.push(Rectangle {
        rect: [x, y, popup_width, popup_height],
        color: [border[0], border[1], border[2], 1.],
    });
    scene.rectangles.push(Rectangle {
        rect: [
            x + scale,
            y + scale,
            (popup_width - 2. * scale).max(0.),
            (popup_height - 2. * scale).max(0.),
        ],
        color: [bg[0], bg[1], bg[2], 1.],
    });
    for (index, text) in std::iter::once("Protected value".to_owned())
        .chain(lines)
        .enumerate()
    {
        scene.text.push(Text {
            pos: [x + pad, y + pad + index as f32 * 24. * scale],
            width: (popup_width - 2. * pad).max(0.),
            value: text,
            color: [fg[0], fg[1], fg[2], 1.],
        });
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    const SECRET: &str = "192.0.2.55";
    fn tagged() -> Vec<u8> {
        format!("\x1b]777;JunaPrivate;{TOKEN}\x1b\\░░░░░\x1b]777;JunaPrivate;0\x1b\\").into_bytes()
    }
    #[test]
    fn committed_views_keep_secrets_out_of_cells_and_sync_redraws() {
        let mut terminal = TerminalState::new(20, 3);
        for byte in tagged() {
            terminal.advance(&[byte]);
        }
        let mut view = TabPrivacy::default();
        view.accept(vec![(TOKEN.into(), SECRET.into())]);
        view.update(&terminal, 0);
        let cells = view.snapshot().unwrap().cells.clone();
        assert_eq!(view.field_at(0, 0), Some(TOKEN));
        assert_eq!(view.field_at(0, 4), Some(TOKEN));
        assert_eq!(view.field_at(0, 5), None);
        assert!(!format!("{:?}", cells).contains(SECRET));
        terminal.advance(b"\x1b[?2026h\rchanged");
        view.update(&terminal, 0);
        assert_eq!(view.snapshot().unwrap().cells, cells);
        assert_eq!(view.value(TOKEN), Some(SECRET));
        terminal.advance(b"\x1b[?2026l");
        view.update(&terminal, 0);
        assert_ne!(view.snapshot().unwrap().cells, cells);
        assert_eq!(view.field_at(0, 0), None);
        // RIS must still invalidate a previously committed snapshot.
        terminal.advance(b"\x1bcnew");
        view.update(&terminal, 0);
        assert_eq!(
            view.snapshot().unwrap().cells[0].content.first_scalar(),
            Some('n' as u32)
        );
    }
    #[test]
    fn reveal_requires_deliberate_movement_and_hides_for_every_exit_condition() {
        let mut reveal = Reveal::default();
        reveal.moved(Some(TOKEN), "shift-hover");
        assert!(reveal.token.is_none());
        reveal.modifiers(true, false);
        assert!(reveal.token.is_none());
        reveal.moved(Some(TOKEN), "shift-hover");
        assert_eq!(reveal.token.as_deref(), Some(TOKEN));
        reveal.modifiers(false, false);
        assert!(reveal.token.is_none());
        for mode in ["shift-hover", "always-hidden"] {
            reveal.modifiers(true, true);
            reveal.moved(Some(TOKEN), mode);
            assert!(reveal.token.is_none());
            reveal.modifiers(true, false);
            reveal.button(2, true);
            reveal.moved(Some(TOKEN), mode);
            assert!(reveal.token.is_none());
            reveal.button(2, false);
            assert!(reveal.token.is_none());
            reveal.moved(Some(TOKEN), mode);
            assert!(reveal.token.is_some());
            reveal.focus(false);
            reveal.moved(Some(TOKEN), mode);
            assert!(reveal.token.is_none());
            reveal.focus(true);
            reveal.moved(Some(TOKEN), mode);
            assert!(reveal.token.is_some());
            reveal.moved(None, mode);
            assert!(reveal.token.is_none());
            reveal.moved(Some(TOKEN), mode);
            reveal.hide();
            assert!(reveal.token.is_none());
        }
        reveal.modifiers(false, false);
        reveal.moved(Some(TOKEN), "hover");
        assert!(reveal.token.is_some());
    }
    #[test]
    fn expired_and_other_tab_fields_never_reveal_a_different_value() {
        let mut terminal = TerminalState::new(20, 3);
        terminal.advance(&tagged());
        let mut view = TabPrivacy::default();
        view.update(&terminal, 0);
        assert_eq!(view.field_at(0, 0), None);
        view.accept(vec![(TOKEN.into(), SECRET.into())]);
        assert_eq!(view.field_at(0, 0), Some(TOKEN));
        for index in 0..128 {
            view.accept(vec![(format!("{index:032x}"), "198.51.100.1".into())]);
        }
        assert_eq!(view.field_at(0, 0), None);
        assert_eq!(view.value(TOKEN), None);
        let mut other = TabPrivacy::default();
        other.update(&terminal, 0);
        assert_eq!(other.field_at(0, 0), None);
    }
}
