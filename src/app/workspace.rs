//! Per-window state. One thread owns every workspace; nothing here performs I/O.
//!
//! A workspace is everything that belongs to one native window: its surface, the
//! active engine and session, its tab records and the pointer and view state that
//! describe what is on screen. Process-wide services stay in `App`, so detaching a
//! tab into another window is a move of this value rather than a restart.
use super::{runtime::Runtime, tabs};
use crate::{gpu::Gpu, vt::TerminalState};
use std::{sync::Arc, time::Instant};
use winit::{event::MouseButton, window::Window};

/// In-progress tab drag. Index and window origin are captured at press so the
/// permutation is resolved against a stable order; only the geometry scrolls.
#[derive(Clone, Copy)]
pub struct TabDrag {
    pub from: usize,
    pub press_x: f32,
    pub press_y: f32,
    /// Strip window origin, advanced when the pointer reaches a strip edge.
    pub start: usize,
    pub moved: bool,
}

pub struct Workspace {
    pub window: Option<Arc<Window>>,
    pub gpu: Option<Gpu>,
    pub renderer: Option<crate::render::Renderer>,
    pub terminal: TerminalState,
    pub runtime: Option<Runtime>,
    pub dirty: bool,
    /// Focus belongs to the window, not the process: a detached window can be
    /// unfocused while another one receives input.
    pub focused: bool,
    pub minimized: bool,
    pub ime: bool,
    pub scale: f32,
    pub offset: usize,
    pub selection: tabs::Selection,
    pub selecting: bool,
    pub mouse_button: Option<u8>,
    pub frames: u64,
    pub pointer: (u16, u16),
    pub cols: u16,
    pub rows: u16,
    pub last_report: Instant,
    pub failure: Option<String>,
    pub last_sync: Instant,
    pub left_inset: f32,
    pub pointer_px: [f32; 2],
    pub ui_mouse_latch: Vec<MouseButton>,
    pub zoom_wheel: f32,
    pub tabs: Vec<tabs::Tab>,
    pub active_tab: usize,
    pub tab_drag: Option<TabDrag>,
    pub active_closing: bool,
    pub closing_all: bool,
}

impl Workspace {
    /// Window-free constructor so tests and detached workspaces can exist before
    /// a surface does. The caller owns tab creation, because tab identifiers are
    /// allocated process-wide.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            window: None,
            gpu: None,
            renderer: None,
            terminal: TerminalState::new(cols, rows),
            runtime: None,
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
            left_inset: 0.,
            pointer_px: [0., 0.],
            ui_mouse_latch: Vec::new(),
            zoom_wheel: 0.,
            tabs: Vec::new(),
            active_tab: 0,
            tab_drag: None,
            active_closing: false,
            closing_all: false,
        }
    }

    /// First tab record. The window's active fields host this tab, so `saved` is
    /// `None`; every other record holds a completed session.
    pub fn seed_tab(&mut self, tab: tabs::Tab) {
        self.tabs.push(tab);
        self.active_tab = 0;
    }
}
