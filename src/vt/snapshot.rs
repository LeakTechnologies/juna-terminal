//! Owned values crossing the engine/renderer boundary. No mutable engine access.
use super::{grid::Cursor, Cell};

#[derive(Clone, Debug)]
pub struct RenderSnapshot {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<Cell>,
    pub wrapped: Vec<bool>,
    pub cursor: Cursor,
    pub cursor_style: u16,
}

impl RenderSnapshot {
    pub fn row(&self, row: u16) -> &[Cell] {
        let start = row as usize * self.cols as usize;
        &self.cells[start..start + self.cols as usize]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputModes {
    pub application_cursor: bool,
    pub application_keypad: bool,
    pub bracketed_paste: bool,
    pub focus_events: bool,
    pub mouse_tracking: u16,
    pub mouse_sgr: bool,
    pub synchronized_output: bool,
}
