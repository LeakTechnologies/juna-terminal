//! The application: window, event loop, and the seam between the engine and the
//! GPU (`ARCHITECTURE.md` 3).
//!
//! Threading follows §3 exactly. The UI thread owns `Window`, `Gpu`, and
//! `TerminalState`. A reader thread owns the session's read half and moves bytes
//! into a `StreamBuffer`. Nothing is shared between them except that buffer, and
//! the buffer is the *only* place bytes cross the thread boundary — which is what
//! keeps §3.0's no-dropping rule enforceable.

use crate::gpu::Gpu;
use crate::session::{self, Dimensions, SessionSpec};
use crate::transport::{Limits, ReadState, StreamBuffer};
use crate::vt::TerminalState;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const READ_CHUNK: usize = 8 * 1022;
const FRAME: Duration = Duration::from_millis(16);
const CELL_W: f32 = 9.0;
const CELL_H: f32 = 18.0;
const PAD: f32 = 8.0;
const BG: wgpu::Color = wgpu::Color {
    r: 0.11,
    g: 0.11,
    b: 0.13,
    a: 1.0,
};

pub struct App {
    window: Option<&'static Window>,
    gpu: Option<Gpu>,
    instance: Option<wgpu::Instance>,
    stream: Arc<StreamBuffer>,
    terminal: TerminalState,
    writer: Option<Box<dyn crate::session::SessionWriter>>,
    control: Option<Box<dyn crate::session::SessionControl>>,
    cols: u16,
    rows: u16,
    frames: u64,
    skipped: u64,
    last_tick: Instant,
}

impl App {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            window: None,
            gpu: None,
            instance: None,
            stream: Arc::new(StreamBuffer::new(Limits::default())),
            terminal: TerminalState::new(cols, rows),
            writer: None,
            control: None,
            cols,
            rows,
            frames: 0,
            skipped: 0,
            last_tick: Instant::now(),
        }
    }

    /// Spawn the shell and start the reader thread. Failure is fatal and
    /// reported with the reason, never swallowed into an empty window.
    fn start_session(&mut self) -> Result<(), String> {
        let spec = SessionSpec {
            program: std::env::var("JUNA_SHELL").unwrap_or_else(|_| "powershell.exe".into()).into(),
            args: std::env::var("JUNA_SHELL_ARGS")
                .unwrap_or_default()
                .split_whitespace()
                .map(std::ffi::OsString::from)
                .collect(),
            cwd: None,
        };
        let dims = Dimensions {
            cols: self.cols,
            rows: self.rows,
        };
        let session = session::spawn(&spec, dims).map_err(|e| format!("spawn: {e}"))?;
        let parts = session.split().map_err(|e| format!("split: {e}"))?;

        let pump = self.stream.clone();
        std::thread::Builder::new()
            .name("juna-reader".into())
            .spawn(move || {
                let mut reader = parts.reader;
                let mut buf = [0u8; READ_CHUNK];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if pump.wait_for_capacity(n) != ReadState::Open {
                                break;
                            }
                            pump.push(&buf[..n]);
                        }
                        Err(_) => break,
                    }
                }
                pump.finish();
            })
            .map_err(|e| format!("reader thread: {e}"))?;

        self.writer = Some(parts.writer);
        self.control = Some(parts.control);
        Ok(())
    }

    /// Move everything the shell has produced into the engine. This is the only
    /// place bytes are handed to `TerminalState`.
    fn drain(&mut self) {
        loop {
            let batch = self.stream.take();
            if batch.is_empty() {
                break;
            }
            self.terminal.advance(&batch);
        }
    }

    fn ensure_gpu(&mut self) -> Result<(), String> {
        if self.gpu.is_some() {
            return Ok(());
        }
        let target: &'static Window = self.window.ok_or("no window")?;
        let size = target.inner_size();

        let gpu = Gpu::new(self.instance.as_ref().expect("instance"), move |instance| {
            instance
                .create_surface(wgpu::SurfaceTarget::Window(Box::new(target)))
                .map_err(|e| e.to_string())
        }, size.width, size.height)
        .map_err(|e| e.to_string())?;

        eprintln!(
            "[gpu] adapter: {} | backend {:?} | type {:?} | driver {}",
            gpu.choice.name, gpu.choice.backend, gpu.choice.device_type, gpu.choice.driver
        );
        if gpu.choice.software_fallback {
            eprintln!("[gpu] WARNING: falling back to a CPU software rasteriser");
        }
        for r in &gpu.rejected {
            eprintln!("[gpu] not selected: {r}");
        }
        eprintln!("[gpu] surface format: {:?}", gpu.format);

        self.gpu = Some(gpu);
        Ok(())
    }

    /// Fit the grid to the window, preferring more rows over a larger cell.
    fn fit(&mut self, width: u32, height: u32) {
        let w = (width as f32 - PAD * 2.0).max(CELL_W);
        let h = (height as f32 - PAD * 2.0).max(CELL_H);
        let cols = ((w / CELL_W) as u16).max(20);
        let rows = ((h / CELL_H) as u16).max(5);
        if cols != self.cols || rows != self.rows {
            self.cols = cols;
            self.rows = rows;
            self.terminal.resize(cols, rows);
            if let Some(c) = self.control.as_mut() {
                let _ = c.resize(Dimensions { cols, rows });
            }
        }
    }

    fn draw(&mut self) {
        self.drain();
        let Some(gpu) = self.gpu.as_mut() else { return };
        let (cols, rows) = (self.cols, self.rows);
        let cursor = self.terminal.grid().cursor;
        match gpu.render(BG, |_pass| {
            // Text is drawn by the renderer slice, not yet implemented. This pass
            // clears only; see DECISIONS.md for why the atlas follows surface
            // verification rather than landing with it.
            let _ = (cols, rows, cursor);
        }) {
            Ok(true) => self.frames += 1,
            Ok(false) => self.skipped += 1,
            Err(skip) => {
                self.skipped += 1;
                if self.skipped < 5 {
                    eprintln!("[gpu] frame skipped: {skip:?}");
                }
            }
        }
    }

    fn report(&mut self) {
        eprintln!(
            "[app] frames={} skipped={} cols={} rows={} scrollback={} title={:?}",
            self.frames,
            self.skipped,
            self.cols,
            self.rows,
            self.terminal.scrollback_len(),
            self.terminal.title()
        );
    }
}

/// Map a key to the bytes a terminal should receive. Control characters are the
/// same on every shell, so this is terminal semantics, not shell-specific.
fn key_to_bytes(key: &Key) -> Option<Vec<u8>> {
    let b = match key {
        Key::Character(s) => {
            // `smol_str::SmolStr` derefs to `str`, so this is the plain UTF-8 of
            // the key. A lone control character (Ctrl-C, Ctrl-L) passes through as
            // its control code, which is what a shell expects.
            s.as_str().as_bytes().to_vec()
        }
        Key::Named(NamedKey::Enter) => vec![b'\r'],
        Key::Named(NamedKey::Backspace) => vec![0x7f],
        Key::Named(NamedKey::Tab) => vec![b'\t'],
        Key::Named(NamedKey::Escape) => vec![0x1b],
        Key::Named(NamedKey::ArrowUp) => b"\x1b[A".to_vec(),
        Key::Named(NamedKey::ArrowDown) => b"\x1b[B".to_vec(),
        Key::Named(NamedKey::ArrowRight) => b"\x1b[C".to_vec(),
        Key::Named(NamedKey::ArrowLeft) => b"\x1b[D".to_vec(),
        Key::Named(NamedKey::Home) => b"\x1b[H".to_vec(),
        Key::Named(NamedKey::End) => b"\x1b[F".to_vec(),
        Key::Named(NamedKey::PageUp) => b"\x1b[5~".to_vec(),
        Key::Named(NamedKey::PageDown) => b"\x1b[6~".to_vec(),
        _ => return None,
    };
    Some(b)
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Juna Terminal")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.cols as f32 * CELL_W + PAD * 2.0,
                self.rows as f32 * CELL_H + PAD * 2.0,
            ));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[app] window creation failed: {e}");
                event_loop.exit();
                return;
            }
        };

        // winit 0.30 hands out a *borrowed* window handle (`WindowHandle<'_>`),
        // and `Window` is not `Clone`, so a `Surface<'static>` needs a window that
        // outlives it. The window already lives for the entire process -- we exit
        // the process to quit -- so one leaked allocation is the sound, simple
        // option. The alternative is parameterising `Gpu` by a lifetime, which
        // would make `App` self-referential: owning both a window and a borrow of
        // it. That is a worse design smell than a single startup leak.
        let window: &'static Window = Box::leak(Box::new(window));
        self.window = Some(window);

        // wgpu 29 requires the platform display handle in the instance
        // descriptor. Without it surface creation fails with "No `DisplayHandle`
        // is available" -- which is exactly what the first run of this binary did.
        // `OwnedDisplayHandle` is the documented source; it must come from the
        // EventLoop, and it is `'static`, so the instance can outlive this scope.
let display = event_loop.owned_display_handle();
        let mut desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(display));
        desc.backends = wgpu::Backends::all();
        self.instance = Some(wgpu::Instance::new(desc));

        if let Err(e) = self.ensure_gpu() {
            eprintln!("[app] gpu init failed: {e}");
            event_loop.exit();
            return;
        }
        if let Err(e) = self.start_session() {
            eprintln!("[app] session failed: {e}");
            event_loop.exit();
            return;
        }
        eprintln!("[app] running. Ctrl+C or close the window to quit.");
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.report();
                self.stream.cancel();
                if let Some(c) = self.control.as_mut() {
                    let _ = c.kill();
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.fit(size.width, size.height);
                if let Some(g) = self.gpu.as_mut() {
                    g.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let Some(bytes) = key_to_bytes(&event.logical_key) {
                        if let Some(w) = self.writer.as_mut() {
                            if let Err(e) = w.write(&bytes) {
                                eprintln!("[app] write failed: {e}");
                            }
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.draw();
                self.window.as_ref().expect("window").request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.drain();
        if self.last_tick.elapsed() >= Duration::from_secs(10) {
            self.last_tick = Instant::now();
            self.report();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }
}

pub fn run(cols: u16, rows: u16) -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(cols, rows);
    event_loop.run_app(&mut app)?;
    Ok(())
}