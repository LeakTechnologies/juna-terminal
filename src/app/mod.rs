//! Single-owner window, input, engine and rendering integration.
mod clipboard;
mod input;
mod runtime;
pub mod ui;
use crate::{
    gpu::{FrameSkip, Gpu},
    session::{Dimensions, SessionSpec},
    vt::{CellContent, TerminalState, MAX_COLS, MAX_ROWS},
};
use runtime::Runtime;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};
const FRAME: Duration = Duration::from_millis(16);
const PAD: f32 = 8.;
const CELL_W: f32 = 10.;
const CELL_H: f32 = 22.;
const FONT_PX: f32 = 16.;
pub struct App {
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    renderer: Option<crate::render::Renderer>,
    terminal: TerminalState,
    runtime: Option<Runtime>,
    modifiers: ModifiersState,
    dirty: bool,
    focused: bool,
    minimized: bool,
    ime: bool,
    scale: f32,
    offset: usize,
    selection: Option<((usize, u16), (usize, u16))>,
    selecting: bool,
    mouse_button: Option<u8>,
    frames: u64,
    pointer: (u16, u16),
    cols: u16,
    rows: u16,
    last_report: Instant,
    failure: Option<String>,
    last_sync: Instant,
    settings: crate::settings::Settings,
    themes: Vec<crate::theme::Theme>,
    store: crate::settings::Store,
    ui: Option<ui::Ui>,
    pointer_px: [f32; 2],
    ui_mouse_latch: Vec<MouseButton>,
}
impl App {
    pub fn new(cols: u16, rows: u16) -> Self {
        let store = crate::settings::Store::new(crate::settings::application_directory());
        let (settings, themes, errors) = store.load();
        for error in errors {
            log::warn!("Settings/theme: {error}");
        }
        Self {
            window: None,
            gpu: None,
            renderer: None,
            terminal: TerminalState::new(cols, rows),
            runtime: None,
            modifiers: ModifiersState::empty(),
            dirty: true,
            focused: true,
            minimized: false,
            ime: false,
            scale: 1.,
            offset: 0,
            selection: None,
            selecting: false,
            mouse_button: None,
            frames: 0,
            pointer: (0, 0),
            cols,
            rows,
            last_report: Instant::now(),
            failure: None,
            last_sync: Instant::now(),
            settings,
            themes,
            store,
            ui: None,
            pointer_px: [0., 0.],
            ui_mouse_latch: Vec::new(),
        }
    }
    fn fail(&mut self, message: String) {
        log::error!("{message}");
        self.failure = Some(message);
        if let Some(r) = self.runtime.as_mut() {
            r.request_close();
        }
    }
    fn send(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(r) = self.runtime.as_mut() {
            if let Err(e) = r.write(bytes) {
                self.fail(format!("Session write failed: {e}"));
            }
        }
    }
    fn drain(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let batch = runtime.stream.take_up_to(64 * 1024);
        if batch.is_empty() {
            return;
        }
        let history = self.terminal.scrollback_len();
        let dropped = self.terminal.scrollback_dropped();
        self.terminal.advance(&batch);
        if self.offset > 0 {
            self.offset = (self.offset
                + self.terminal.scrollback_len().saturating_sub(history)
                + self.terminal.scrollback_dropped().saturating_sub(dropped) as usize)
                .min(self.terminal.scrollback_len());
        }
        if self.terminal.scrollback_dropped() != dropped {
            self.selection = None;
        }
        let replies = self.terminal.take_responses();
        self.send(&replies);
        self.dirty = true;
        if let Some(w) = self.window.as_ref() {
            let title = self.terminal.title();
            if !title.is_empty() {
                w.set_title(title);
            }
        }
    }
    fn fit(&mut self, width: u32, height: u32) {
        self.minimized = width == 0 || height == 0;
        if self.minimized {
            return;
        }
        let cols = (((width as f32 - 2. * PAD * self.scale) / (CELL_W * self.scale)).max(1.)
            as u16)
            .clamp(1, MAX_COLS);
        let rows = (((height as f32 - 2. * PAD * self.scale) / (CELL_H * self.scale)).max(1.)
            as u16)
            .clamp(1, MAX_ROWS);
        if (cols, rows) != (self.cols, self.rows) {
            if let Some(r) = self.runtime.as_mut() {
                if let Err(e) = r.control.resize(Dimensions { cols, rows }) {
                    self.fail(format!("PTY resize failed: {e}"));
                    return;
                }
            }
            self.terminal.resize(cols, rows);
            self.cols = cols;
            self.rows = rows;
            self.selection = None;
            self.offset = self.offset.min(self.terminal.scrollback_len());
        }
        if let Some(g) = self.gpu.as_mut() {
            g.resize(width, height);
        }
        self.dirty = true;
    }
    fn draw(&mut self) {
        if self.minimized {
            return;
        }
        let selection_theme = self.active_theme();
        let selection_top = self.history_start();
        let Some(g) = self.gpu.as_ref() else { return };
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let (w, h) = (g.config.width, g.config.height);
        let mut snapshot = self.terminal.snapshot(self.offset);
        if !self.focused {
            snapshot.cursor_style = 4;
        }
        if let Some((a, b)) = self.selection {
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            let top = selection_top;
            for row in 0..snapshot.rows {
                for col in 0..snapshot.cols {
                    let point = (top + row as usize, col);
                    if point >= a && point <= b {
                        let cell = &mut snapshot.cells
                            [row as usize * snapshot.cols as usize + col as usize];
                        let theme = &selection_theme;
                        let to_color = |value: &str| {
                            let c = crate::theme::rgb(value).unwrap();
                            crate::vt::Color::Rgb(
                                (c[0] * 255.).round() as u8,
                                (c[1] * 255.).round() as u8,
                                (c[2] * 255.).round() as u8,
                            )
                        };
                        cell.fg = to_color(&theme.colors.selection_foreground);
                        cell.bg = to_color(&theme.colors.selection_background);
                        cell.attrs.set(crate::vt::Attrs::INVERSE, false);
                    }
                }
            }
        }
        renderer.build(&snapshot, w, h);
        if let Some(ui) = self.ui.as_mut() {
            let rows = ui.rows(&self.themes, &self.settings);
            let (_, visible, _) = ui.geometry(w as f32, h as f32, self.scale, rows.len());
            ui.ensure_visible(visible, rows.len());
            let scene = ui.scene(&rows, w as f32, h as f32, self.scale);
            renderer.overlay(&scene);
        }
        match g.render(renderer.clear_color(), |device, queue, pass| {
            renderer.draw(device, queue, w, h, pass)
        }) {
            Ok(true) => {
                self.dirty = false;
                self.frames += 1;
            }
            Ok(false) => {}
            Err(FrameSkip::Lost | FrameSkip::Outdated) => {
                self.gpu.as_mut().unwrap().resize(w, h);
            }
            Err(FrameSkip::Validation) => self.fail("GPU presentation validation failed".into()),
            Err(_) => {}
        }
    }
    fn selection_text(&self) -> String {
        let Some((a, b)) = self.selection else {
            return String::new();
        };
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let history = if self.terminal.alternate_active() {
            0
        } else {
            self.terminal.scrollback_len()
        };
        let mut out = String::new();
        for row in a.0..=b.0 {
            let cells = if row < history {
                self.terminal.scrollback_line(row)
            } else {
                self.terminal.grid().row_slice((row - history) as u16)
            };
            let Some(cells) = cells else { continue };
            let start = if row == a.0 { a.1 as usize } else { 0 };
            let end = if row == b.0 {
                b.1 as usize + 1
            } else {
                cells.len()
            };
            let mut line = String::new();
            for cell in cells.iter().take(end).skip(start) {
                match &cell.content {
                    CellContent::Char(cp) | CellContent::WideChar(cp) => {
                        line.push(char::from_u32(*cp).unwrap_or('�'))
                    }
                    CellContent::Cluster(s) => line.push_str(s),
                    CellContent::Empty => line.push(' '),
                    CellContent::WideCharSpacer => {}
                }
            }
            out.push_str(line.trim_end());
            if row < b.0 {
                let wrapped = if row < history {
                    self.terminal.history_wrapped(row)
                } else {
                    self.terminal.grid().row_wrapped((row - history) as u16)
                };
                if !wrapped {
                    out.push('\n');
                }
            }
        }
        out
    }
    fn paste(&mut self) {
        match clipboard::read() {
            Ok(text) => {
                let bytes = input::paste(&text, self.terminal.input_modes().bracketed_paste);
                self.offset = 0;
                self.selection = None;
                self.send(&bytes);
            }
            Err(e) => log::warn!("Paste: {e}"),
        }
    }
    fn active_theme(&self) -> crate::theme::Theme {
        self.themes
            .iter()
            .find(|t| t.id == self.settings.active_theme_id)
            .cloned()
            .unwrap_or_else(crate::theme::default_theme)
    }
    fn history_start(&self) -> usize {
        if self.terminal.alternate_active() {
            0
        } else {
            self.terminal.scrollback_len().saturating_sub(self.offset)
        }
    }
    fn rebuild_renderer(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        self.scale = window.scale_factor() as f32 * self.settings.font_size / FONT_PX;
        if let Some(gpu) = self.gpu.as_ref() {
            self.renderer = crate::render::Renderer::new_with_family(
                &gpu.device,
                &gpu.queue,
                gpu.format,
                2048,
                (CELL_W * self.scale, CELL_H * self.scale),
                PAD * self.scale,
                FONT_PX * self.scale,
                &self.settings.font_family,
            );
        }
        let theme = self.active_theme();
        if let Some(r) = self.renderer.as_mut() {
            let _ = r.apply_theme(&theme, self.settings.minimum_contrast);
        }
        let size = window.inner_size();
        self.fit(size.width, size.height);
        self.dirty = true;
    }
    fn open_ui(&mut self, kind: ui::Kind) {
        log::info!("Options panel: {kind:?}");
        let anchor = if kind == ui::Kind::Context {
            self.pointer_px
        } else {
            [16. * self.scale, 16. * self.scale]
        };
        let mut ui = ui::Ui::new(kind, anchor);
        if kind == ui::Kind::Themes {
            ui.focus = self
                .themes
                .iter()
                .position(|t| t.id == self.settings.active_theme_id)
                .unwrap_or(0)
                + 1;
        }
        let rows = ui.rows(&self.themes, &self.settings);
        ui.focus_row(ui.focus, &rows);
        self.ui = Some(ui);
        self.selecting = false;
        self.dirty = true;
    }
    fn preview_ui(&mut self) {
        let mut preview = None;
        if let Some(ui) = self.ui.as_mut() {
            let rows = ui.rows(&self.themes, &self.settings);
            if ui.kind == ui::Kind::Themes {
                if let Some(ui::Action::Pick(id)) = rows.get(ui.focus).map(|r| &r.action) {
                    preview = self.themes.iter().find(|t| &t.id == id).cloned();
                }
            }
            if ui.kind == ui::Kind::Editor {
                if let Some(row) = rows.get(ui.focus) {
                    let draft = ui.draft.as_mut().unwrap();
                    let result = match row.action {
                        ui::Action::Name => {
                            draft.name = ui.buffer.clone();
                            Ok(())
                        }
                        ui::Action::Color(i) => draft.set_color(i, ui.buffer.clone()),
                        _ => Ok(()),
                    };
                    ui.message = result.err().unwrap_or_default();
                    if draft.validate().is_ok() {
                        preview = Some(draft.clone());
                    }
                }
            }
        }
        if let (Some(theme), Some(renderer)) = (preview, self.renderer.as_mut()) {
            let _ = renderer.apply_theme(&theme, self.settings.minimum_contrast);
        }
        self.dirty = true;
    }
    fn close_ui(&mut self) {
        self.ui = None;
        let theme = self.active_theme();
        if let Some(r) = self.renderer.as_mut() {
            let _ = r.apply_theme(&theme, self.settings.minimum_contrast);
        }
        self.dirty = true;
    }
    fn ui_action(&mut self, action: ui::Action) {
        use ui::{Action, Kind};
        let result = (|| -> Result<(), String> {
            match action {
                Action::Copy => {
                    let text = self.selection_text();
                    if !text.is_empty() {
                        if let Some(window) = self.window.as_ref() {
                            clipboard::write(window, &text)?;
                        }
                    }
                    self.close_ui();
                }
                Action::Paste => {
                    self.close_ui();
                    self.paste();
                }
                Action::SelectAll => {
                    let history = if self.terminal.alternate_active() {
                        0
                    } else {
                        self.terminal.scrollback_len()
                    };
                    self.selection =
                        Some(((0, 0), (history + self.rows as usize - 1, self.cols - 1)));
                    self.close_ui();
                }
                Action::Themes => self.open_ui(Kind::Themes),
                Action::Settings => self.open_ui(Kind::Settings),
                Action::Cancel => self.close_ui(),
                Action::Pick(id) => {
                    let mut settings = self.settings.clone();
                    settings.active_theme_id = id;
                    self.store.save_settings(&settings)?;
                    self.settings = settings;
                    self.close_ui();
                }
                Action::Duplicate | Action::Edit => {
                    let mut theme = self.active_theme();
                    if action == Action::Duplicate || !theme.id.starts_with("custom-") {
                        theme.id = crate::settings::new_id();
                        theme.name = format!("My {}", theme.name);
                        theme.source = None;
                    }
                    let mut ui = ui::Ui::new(Kind::Editor, [16. * self.scale, 16. * self.scale]);
                    ui.draft = Some(theme);
                    let rows = ui.rows(&self.themes, &self.settings);
                    ui.focus_row(0, &rows);
                    self.ui = Some(ui);
                }
                Action::Save => {
                    let ui = self.ui.as_ref().ok_or("No theme editor")?;
                    if !ui.message.is_empty() {
                        return Err(ui.message.clone());
                    }
                    let theme = ui.draft.as_ref().ok_or("No theme draft")?.clone();
                    self.store.save_theme(&theme)?;
                    let mut settings = self.settings.clone();
                    settings.active_theme_id = theme.id.clone();
                    self.store.save_settings(&settings)?;
                    if let Some(existing) = self.themes.iter_mut().find(|t| t.id == theme.id) {
                        *existing = theme;
                    } else {
                        self.themes.push(theme);
                    }
                    self.settings = settings;
                    self.close_ui();
                }
                Action::Setting(i) => {
                    let value = self
                        .ui
                        .as_ref()
                        .ok_or("No settings form")?
                        .buffer
                        .trim()
                        .to_owned();
                    let mut settings = self.settings.clone();
                    match i {
                        0 => settings.font_family = value,
                        1 => {
                            settings.font_size =
                                value.parse().map_err(|_| "Font size must be 8–40")?
                        }
                        2 => {
                            settings.minimum_contrast =
                                value.parse().map_err(|_| "Contrast must be 1–21")?
                        }
                        _ => return Err("Unknown setting".into()),
                    };
                    self.store.save_settings(&settings)?;
                    self.settings = settings;
                    self.rebuild_renderer();
                }
                Action::Search | Action::Color(_) | Action::Name => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            log::warn!("Options: {error}");
            if let Some(ui) = self.ui.as_mut() {
                ui.message = error;
            }
        }
        self.dirty = true;
    }
    fn handle_ui(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::CursorMoved { position, .. } = event {
            self.pointer_px = [position.x as f32, position.y as f32];
        }
        if let WindowEvent::MouseInput {
            state: ElementState::Released,
            button,
            ..
        } = event
        {
            if self.ui_mouse_latch.contains(button) {
                self.ui_mouse_latch.retain(|b| b != button);
                return true;
            }
        }
        let Some(mut ui) = self.ui.take() else {
            return false;
        };
        let rows = ui.rows(&self.themes, &self.settings);
        let size = self
            .window
            .as_ref()
            .map(|w| w.inner_size())
            .unwrap_or(winit::dpi::PhysicalSize::new(1024, 768));
        let width = size.width as f32;
        let height = size.height as f32;
        let mut action = None;
        let consumed = match event {
            WindowEvent::CursorMoved { .. } => {
                if let Some(index) = ui.hit(self.pointer_px, width, height, self.scale, rows.len())
                {
                    if index != ui.focus {
                        ui.focus_row(index, &rows);
                    }
                }
                true
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *state == ElementState::Pressed {
                    if !self.ui_mouse_latch.contains(button) {
                        self.ui_mouse_latch.push(*button);
                    }
                    if *button == MouseButton::Left {
                        if let Some(index) =
                            ui.hit(self.pointer_px, width, height, self.scale, rows.len())
                        {
                            ui.focus_row(index, &rows);
                            if rows[index].value.is_none() {
                                action = Some(rows[index].action.clone());
                            }
                        } else {
                            let (r, _, _) = ui.geometry(width, height, self.scale, rows.len());
                            if self.pointer_px[0] < r[0]
                                || self.pointer_px[0] > r[0] + r[2]
                                || self.pointer_px[1] < r[1]
                                || self.pointer_px[1] > r[1] + r[3]
                            {
                                action = Some(ui::Action::Cancel);
                            }
                        }
                    } else if *button == MouseButton::Right {
                        action = Some(ui::Action::Cancel);
                    }
                }
                true
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if self.modifiers.control_key()
                        && matches!(&event.logical_key,Key::Character(s)if s.eq_ignore_ascii_case("v"))
                    {
                        if ui.kind == ui::Kind::Themes
                            || rows.get(ui.focus).is_some_and(|r| r.value.is_some())
                        {
                            match clipboard::read() {
                                Ok(text) => ui.paste_text(&text),
                                Err(e) => ui.message = e,
                            }
                        }
                    } else {
                        action = ui.key(
                            &event.logical_key,
                            event.text.as_deref(),
                            self.modifiers.control_key(),
                            &rows,
                        );
                    }
                }
                true
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.ime = false;
                ui.key(
                    &Key::Character(text.clone().into()),
                    Some(text),
                    false,
                    &rows,
                );
                true
            }
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                self.ime = !text.is_empty();
                true
            }
            WindowEvent::Ime(Ime::Disabled) => {
                self.ime = false;
                true
            }
            WindowEvent::Ime(_) => true,
            WindowEvent::MouseWheel { delta, .. } => {
                let up = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y > 0.,
                    MouseScrollDelta::PixelDelta(p) => p.y > 0.,
                };
                let focus = if up {
                    ui.focus.saturating_sub(3)
                } else {
                    (ui.focus + 3).min(rows.len().saturating_sub(1))
                };
                ui.focus_row(focus, &rows);
                true
            }
            _ => false,
        };
        self.ui = Some(ui);
        if consumed {
            self.preview_ui();
            if let Some(action) = action {
                self.ui_action(action);
            }
        }
        consumed
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Juna Terminal")
                            .with_inner_size(winit::dpi::LogicalSize::new(
                                self.cols as f32 * CELL_W + PAD * 2.,
                                self.rows as f32 * CELL_H + PAD * 2.,
                            )),
                    )
                    .map_err(|e| e.to_string())?,
            );
            window.set_ime_allowed(true);
            self.scale = window.scale_factor() as f32 * self.settings.font_size / FONT_PX;
            let size = window.inner_size();
            let display = event_loop.owned_display_handle();
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
                Box::new(display),
            ));
            let target = window.clone();
            let gpu = Gpu::new(
                &instance,
                |instance| instance.create_surface(target).map_err(|e| e.to_string()),
                size.width,
                size.height,
            )
            .map_err(|e| e.to_string())?;
            log::info!("GPU: {:?}", gpu.choice);
            self.renderer = Some(
                crate::render::Renderer::new_with_family(
                    &gpu.device,
                    &gpu.queue,
                    gpu.format,
                    2048,
                    (CELL_W * self.scale, CELL_H * self.scale),
                    PAD * self.scale,
                    FONT_PX * self.scale,
                    &self.settings.font_family,
                )
                .ok_or("No usable font")?,
            );
            self.gpu = Some(gpu);
            self.window = Some(window);
            let theme = self.active_theme();
            if let Some(r) = self.renderer.as_mut() {
                r.apply_theme(&theme, self.settings.minimum_contrast)?;
            }
            self.fit(size.width, size.height);
            let spec = SessionSpec::from_environment().map_err(|e| e.to_string())?;
            self.runtime = Some(
                Runtime::spawn(
                    &spec,
                    Dimensions {
                        cols: self.cols,
                        rows: self.rows,
                    },
                )
                .map_err(|e| e.to_string())?,
            );
            #[cfg(debug_assertions)]
            if let Ok(panel) = std::env::var("JUNA_STARTUP_PANEL") {
                match panel.as_str() {
                    "themes" => self.open_ui(ui::Kind::Themes),
                    "settings" => self.open_ui(ui::Kind::Settings),
                    _ => {}
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.failure = Some(e);
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.handle_ui(&event) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(r) = self.runtime.as_mut() {
                    r.request_close();
                } else {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => self.fit(size.width, size.height),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32 * self.settings.font_size / FONT_PX;
                self.rebuild_renderer();
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::Focused(f) => {
                self.focused = f;
                self.dirty = true;
                if self.terminal.input_modes().focus_events {
                    self.send(if f { b"\x1b[I" } else { b"\x1b[O" });
                }
            }
            WindowEvent::Ime(Ime::Enabled) => {}
            WindowEvent::Ime(Ime::Disabled) => self.ime = false,
            WindowEvent::Ime(Ime::Preedit(text, _)) => self.ime = !text.is_empty(),
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.ime = false;
                self.send(text.as_bytes());
                self.offset = 0;
                self.dirty = true;
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let shortcut = self.modifiers.control_key() && self.modifiers.shift_key();
                if shortcut
                    && matches!(&event.logical_key,Key::Character(s)if s.eq_ignore_ascii_case("v"))
                {
                    self.paste();
                } else if shortcut
                    && matches!(&event.logical_key,Key::Character(s)if s.eq_ignore_ascii_case("c"))
                {
                    let text = self.selection_text();
                    if !text.is_empty() {
                        if let Some(w) = self.window.as_ref() {
                            if let Err(e) = clipboard::write(w, &text) {
                                log::warn!("Copy: {e}");
                            }
                        }
                    }
                } else if self.modifiers.shift_key()
                    && matches!(event.logical_key, Key::Named(NamedKey::Insert))
                {
                    self.paste();
                } else if self.modifiers.shift_key()
                    && matches!(
                        event.logical_key,
                        Key::Named(NamedKey::PageUp | NamedKey::PageDown)
                    )
                {
                    if matches!(event.logical_key, Key::Named(NamedKey::PageUp)) {
                        self.offset =
                            (self.offset + self.rows as usize).min(self.terminal.scrollback_len());
                    } else {
                        self.offset = self.offset.saturating_sub(self.rows as usize);
                    }
                    self.dirty = true;
                } else if !self.ime {
                    if let Some(bytes) =
                        input::keypad(event.physical_key, self.terminal.input_modes()).or_else(
                            || {
                                input::encode(
                                    &event.logical_key,
                                    event.text.as_deref(),
                                    self.modifiers,
                                    self.terminal.input_modes(),
                                )
                            },
                        )
                    {
                        self.offset = 0;
                        self.selection = None;
                        self.send(&bytes);
                        self.dirty = true;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let col =
                    ((position.x as f32 - PAD * self.scale) / (CELL_W * self.scale)).max(0.) as u16;
                let row =
                    ((position.y as f32 - PAD * self.scale) / (CELL_H * self.scale)).max(0.) as u16;
                self.pointer = (row.min(self.rows - 1), col.min(self.cols - 1));
                let modes = self.terminal.input_modes();
                if !self.modifiers.shift_key()
                    && (modes.mouse_tracking == 1003
                        || (modes.mouse_tracking == 1002 && self.mouse_button.is_some()))
                {
                    self.send(&input::mouse(
                        32 + self.mouse_button.unwrap_or(3),
                        self.pointer.0,
                        self.pointer.1,
                        false,
                        self.modifiers,
                        modes.mouse_sgr,
                    ));
                }
                if self.selecting {
                    let top = self.history_start();
                    if let Some((_, b)) = self.selection.as_mut() {
                        *b = (top + self.pointer.0 as usize, self.pointer.1);
                    }
                    self.dirty = true;
                }
            }
            WindowEvent::MouseInput { state, button, .. }
                if self.terminal.input_modes().mouse_tracking > 0
                    && !self.modifiers.shift_key() =>
            {
                let code = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                let released = state == ElementState::Released;
                self.mouse_button = if released { None } else { Some(code) };
                self.send(&input::mouse(
                    code,
                    self.pointer.0,
                    self.pointer.1,
                    released,
                    self.modifiers,
                    self.terminal.input_modes().mouse_sgr,
                ));
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.selecting = state == ElementState::Pressed;
                if self.selecting {
                    let p = (
                        self.history_start() + self.pointer.0 as usize,
                        self.pointer.1,
                    );
                    self.selection = Some((p, p));
                }
                self.dirty = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if !self.ui_mouse_latch.contains(&MouseButton::Right) {
                    self.ui_mouse_latch.push(MouseButton::Right);
                }
                self.open_ui(ui::Kind::Context);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 3.,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / (CELL_H * self.scale),
                };
                if self.terminal.input_modes().mouse_tracking > 0 && !self.modifiers.shift_key() {
                    for _ in 0..lines.abs().ceil().min(64.) as u8 {
                        self.send(&input::mouse(
                            if lines > 0. { 64 } else { 65 },
                            self.pointer.0,
                            self.pointer.1,
                            false,
                            self.modifiers,
                            self.terminal.input_modes().mouse_sgr,
                        ));
                    }
                    return;
                }
                if lines > 0. {
                    self.offset =
                        (self.offset + lines.ceil() as usize).min(self.terminal.scrollback_len());
                } else {
                    self.offset = self.offset.saturating_sub((-lines).ceil() as usize);
                }
                self.dirty = true;
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.drain();
        if let Some(runtime) = self.runtime.as_mut() {
            match runtime.tick() {
                Ok(true) => {
                    self.terminal.finish();
                    #[cfg(debug_assertions)]
                    if let Ok(marker) = std::env::var("JUNA_ACCEPT_MARKER") {
                        let content: String = self
                            .terminal
                            .snapshot(0)
                            .cells
                            .iter()
                            .map(|c| match &c.content {
                                CellContent::Char(cp) | CellContent::WideChar(cp) => {
                                    char::from_u32(*cp).unwrap_or('�').to_string()
                                }
                                CellContent::Cluster(s) => s.to_string(),
                                _ => " ".into(),
                            })
                            .collect();
                        log::info!("Acceptance marker present={}", content.contains(&marker));
                    }
                    log::info!(
                        "Shell exited: {:?}; stream: {:?}; frames={}",
                        runtime.exit,
                        runtime.stream.stats(),
                        self.frames
                    );
                    event_loop.exit();
                }
                Ok(false) => {}
                Err(e) => {
                    let timed_out = e.to_string().contains("exceeded");
                    self.fail(e.to_string());
                    if timed_out || self.runtime.as_ref().and_then(|r| r.exit).is_some() {
                        event_loop.exit();
                    }
                }
            }
        }
        let sync = self.terminal.input_modes().synchronized_output;
        if !sync {
            self.last_sync = Instant::now();
        }
        if self.dirty
            && !self.minimized
            && (!sync || self.last_sync.elapsed() > Duration::from_secs(1))
        {
            if let Some(w) = self.window.as_ref() {
                w.request_redraw();
            }
        }
        if self.last_report.elapsed() > Duration::from_secs(10) {
            self.last_report = Instant::now();
            if let Some(r) = self.runtime.as_ref() {
                log::info!(
                    "stream={:?}, history={}",
                    r.stream.stats(),
                    self.terminal.scrollback_len()
                );
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }
}
pub fn run(cols: u16, rows: u16) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Juna");
    std::fs::create_dir_all(&path)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path.join("juna.log"))?;
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder
        .target(env_logger::Target::Pipe(Box::new(log)))
        .try_init()?;
    let event_loop = EventLoop::new()?;
    let mut app = App::new(cols, rows);
    event_loop.run_app(&mut app)?;
    if let Some(failure) = app.failure.take() {
        clipboard::error_dialog(&failure);
        return Err(failure.into());
    }
    Ok(())
}

#[cfg(test)]
mod option_tests {
    use super::*;
    #[test]
    fn previews_cancel_and_custom_saves_preserve_engine_cells() {
        let dir = std::env::temp_dir().join(crate::settings::new_id());
        let mut app = App::new(10, 3);
        app.store = crate::settings::Store::new(dir.clone());
        app.settings = crate::settings::Settings::default();
        app.themes = crate::theme::builtins();
        app.terminal.advance(b"terminal stays here");
        let before = app.terminal.snapshot(0).cells;
        app.ui_action(ui::Action::Themes);
        app.ui_action(ui::Action::Pick("nord".into()));
        assert_eq!(app.settings.active_theme_id, "nord");
        app.ui_action(ui::Action::Duplicate);
        assert!(app.handle_ui(&WindowEvent::Ime(Ime::Commit("My custom".into()))));
        assert_eq!(
            app.ui.as_ref().unwrap().draft.as_ref().unwrap().name,
            "My custom"
        );
        app.ui_action(ui::Action::Cancel);
        assert_eq!(app.settings.active_theme_id, "nord");
        assert_eq!(app.themes.len(), 20);
        app.ui_action(ui::Action::Duplicate);
        app.handle_ui(&WindowEvent::Ime(Ime::Commit("Stored custom".into())));
        app.ui_action(ui::Action::Save);
        assert_eq!(app.themes.len(), 21);
        let (settings, themes, errors) = app.store.load();
        assert!(errors.is_empty());
        assert_eq!(settings.active_theme_id, app.settings.active_theme_id);
        assert!(themes.iter().any(|t| t.name == "Stored custom"));
        assert_eq!(app.terminal.snapshot(0).cells, before);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
