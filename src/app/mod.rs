//! Single-owner window, input, engine and rendering integration.
mod clipboard;
mod input;
mod runtime;
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
}
impl App {
    pub fn new(cols: u16, rows: u16) -> Self {
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
            let top = self.terminal.scrollback_len().saturating_sub(self.offset);
            for row in 0..snapshot.rows {
                for col in 0..snapshot.cols {
                    let point = (top + row as usize, col);
                    if point >= a && point <= b {
                        snapshot.cells[row as usize * snapshot.cols as usize + col as usize]
                            .attrs
                            .set(crate::vt::Attrs::INVERSE, true);
                    }
                }
            }
        }
        renderer.build(&snapshot, w, h);
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
            self.scale = window.scale_factor() as f32;
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
                crate::render::Renderer::new(
                    &gpu.device,
                    &gpu.queue,
                    gpu.format,
                    2048,
                    (CELL_W * self.scale, CELL_H * self.scale),
                    PAD * self.scale,
                    FONT_PX * self.scale,
                )
                .ok_or("No usable font")?,
            );
            self.gpu = Some(gpu);
            self.window = Some(window);
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
            Ok(())
        })();
        if let Err(e) = result {
            self.failure = Some(e);
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
                self.scale = scale_factor as f32;
                if let Some(g) = self.gpu.as_ref() {
                    self.renderer = crate::render::Renderer::new(
                        &g.device,
                        &g.queue,
                        g.format,
                        2048,
                        (CELL_W * self.scale, CELL_H * self.scale),
                        PAD * self.scale,
                        FONT_PX * self.scale,
                    );
                }
                if let Some(w) = self.window.as_ref() {
                    let s = w.inner_size();
                    self.fit(s.width, s.height);
                }
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
                    let top = self.terminal.scrollback_len().saturating_sub(self.offset);
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
                        self.terminal.scrollback_len().saturating_sub(self.offset)
                            + self.pointer.0 as usize,
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
            } => self.paste(),
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
