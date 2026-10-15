//! Window-owned protected-field transport, committed views and deliberate reveal input.
use super::*;
impl App {
    pub(super) fn privacy_environment(&self, mut spec: SessionSpec, tab: u64) -> SessionSpec {
        spec.env
            .retain(|(key, _)| !key.to_string_lossy().starts_with("JUNA_PRIVACY_"));
        let environment = self
            .privacy_broker
            .as_ref()
            .and_then(|broker| broker.register(tab).ok())
            .unwrap_or_else(|| {
                vec![
                    ("JUNA_PRIVACY_PIPE".into(), "".into()),
                    ("JUNA_PRIVACY_TOKEN".into(), "".into()),
                    ("JUNA_PRIVACY_SERVER".into(), "0".into()),
                ]
            });
        spec.env.extend(environment);
        spec
    }
    pub(super) fn poll_privacy(&mut self) {
        for _ in 0..8 {
            let Some(report) = self
                .privacy_broker
                .as_ref()
                .and_then(|broker| broker.poll())
            else {
                break;
            };
            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == report.tab) {
                if !tab.saved.as_ref().is_some_and(|saved| saved.closing) {
                    tab.privacy.accept(report.fields);
                    self.dirty = true;
                    self.privacy_dirty = true;
                }
            }
        }
    }
    fn protected_at(&self, point: [f32; 2]) -> Option<&str> {
        if self.ui.is_some() || self.active_closing {
            return None;
        }
        let x = point[0] - PAD * self.scale - self.left_inset;
        let y = point[1] - (PAD + ui::TAB_HEIGHT) * self.scale;
        if x < 0. || y < 0. {
            return None;
        }
        self.tabs[self.active_tab].privacy.field_at(
            (y / (CELL_H * self.scale)) as u16,
            (x / (CELL_W * self.scale)) as u16,
        )
    }
    pub(super) fn privacy_event(&mut self, event: &WindowEvent) {
        let had_reveal = self.privacy_reveal.token.is_some();
        let mut changed = true;
        match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.privacy_reveal.modifiers(
                    state.shift_key(),
                    state.control_key() || state.alt_key() || state.super_key(),
                );
            }
            WindowEvent::Focused(focused) => self.privacy_reveal.focus(*focused),
            WindowEvent::CursorLeft { .. } | WindowEvent::MouseWheel { .. } => {
                self.privacy_reveal.hide()
            }
            WindowEvent::MouseInput { button, state, .. } => {
                let button = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Back => 3,
                    MouseButton::Forward => 4,
                    MouseButton::Other(code) => 5 + u32::from(*code),
                };
                self.privacy_reveal
                    .button(button, *state == ElementState::Pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.tabs[self.active_tab]
                    .privacy
                    .update(&self.terminal, self.offset);
                let token = self
                    .protected_at([position.x as f32, position.y as f32])
                    .map(str::to_owned);
                self.privacy_reveal
                    .moved(token.as_deref(), &self.settings.privacy_mode);
            }
            _ => changed = false,
        }
        if changed && (had_reveal || self.privacy_reveal.token.is_some()) {
            self.dirty = true;
            self.privacy_dirty = true;
            if had_reveal && matches!(event, WindowEvent::Focused(false)) && self.gpu.is_some() {
                // Present a masked frame before yielding focus whenever the surface is available.
                self.draw();
            }
        }
    }
    pub(super) fn privacy_overlay(
        &self,
        width: u32,
        height: u32,
    ) -> Option<crate::render::overlay::Overlay> {
        let token = self.privacy_reveal.token.as_deref()?;
        if self.protected_at(self.pointer_px) != Some(token) {
            return None;
        }
        let value = self.tabs[self.active_tab].privacy.value(token)?;
        Some(crate::privacy::view::popup(
            value,
            self.pointer_px,
            width as f32,
            height as f32,
            self.scale,
            &self.active_theme(),
        ))
    }
}
