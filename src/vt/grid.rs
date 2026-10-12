use crate::vt::cell::{Cell, CellContent, Color};
use crate::vt::width::Columns;

pub const MAX_COLS: u16 = 512;
pub const MAX_ROWS: u16 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

impl Cursor {
    pub fn new(row: u16, col: u16) -> Self {
        Self {
            row,
            col,
            visible: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedCursor {
    pub cursor: Cursor,
    pub origin_mode: bool,
    pub wrap_pending: bool,
}

#[derive(Clone, Debug)]
pub struct Grid {
    cols: u16,
    rows: u16,
    cells: Vec<Cell>,
    pub cursor: Cursor,
    scroll_top: u16,
    scroll_bottom: u16,
    scroll_left: u16,
    scroll_right: u16,
    tabs: Vec<bool>,
    pub wrap_pending: bool,
    pub origin_mode: bool,
    pub saved_cursor: Option<SavedCursor>,
    pub default_bg: Color,
    pub default_fg: Color,
    pub autowrap: bool,
    /// Rows pushed off the top of the scroll region since the last drain.
    ///
    /// The grid cannot decide whether these belong in scrollback: that depends on
    /// whether the scroll region is the whole screen and on whether the alternate
    /// screen is active, and both facts live in `TerminalState`. So the grid
    /// records them and the engine decides.
    pending_history: Vec<Vec<Cell>>,
    wrapped: Vec<bool>,
    pending_wrapped: Vec<bool>,
}

impl Grid {
    pub fn new(cols: u16, rows: u16) -> Self {
        let cols = cols.clamp(1, MAX_COLS);
        let rows = rows.clamp(1, MAX_ROWS);
        let mut g = Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols as usize * rows as usize],
            cursor: Cursor::new(0, 0),
            scroll_top: 0,
            scroll_bottom: rows - 1,
            scroll_left: 0,
            scroll_right: cols - 1,
            tabs: vec![true; cols as usize],
            wrap_pending: false,
            origin_mode: false,
            saved_cursor: None,
            default_bg: Color::Default,
            default_fg: Color::Default,
            autowrap: true,
            pending_history: Vec::new(),
            wrapped: vec![false; rows as usize],
            pending_wrapped: Vec::new(),
        };
        g.reset_tabs();
        g
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn cell(&self, row: u16, col: u16) -> Option<&Cell> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        self.cells
            .get(row as usize * self.cols as usize + col as usize)
    }

    pub fn cell_mut(&mut self, row: u16, col: u16) -> Option<&mut Cell> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        self.cells
            .get_mut(row as usize * self.cols as usize + col as usize)
    }

    pub fn row_slice(&self, row: u16) -> Option<&[Cell]> {
        if row >= self.rows {
            return None;
        }
        let start = row as usize * self.cols as usize;
        self.cells.get(start..start + self.cols as usize)
    }

    pub fn fill_row(&mut self, row: u16, cell: Cell) {
        if row >= self.rows {
            return;
        }
        self.wrapped[row as usize] = false;
        let start = row as usize * self.cols as usize;
        for i in 0..self.cols as usize {
            self.cells[start + i] = cell.clone();
        }
    }

    /// Copies one whole row onto another, validated at the boundary.
    pub fn copy_row(&mut self, from: u16, to: u16) {
        if from >= self.rows || to >= self.rows {
            return;
        }
        let width = self.cols as usize;
        let src = from as usize * width;
        let dst = to as usize * width;
        for i in 0..width {
            self.cells[dst + i] = self.cells[src + i].clone();
        }
        self.wrapped[to as usize] = self.wrapped[from as usize];
    }

    /// Copies one cell onto another, validated at the boundary.
    pub fn copy_cell(&mut self, from: (u16, u16), to: (u16, u16)) {
        let Some(src) = self.cell(from.0, from.1).cloned() else {
            return;
        };
        if let Some(dst) = self.cell_mut(to.0, to.1) {
            *dst = src;
        }
    }

    pub fn scroll_region(&self) -> (u16, u16, u16, u16) {
        (
            self.scroll_top,
            self.scroll_bottom,
            self.scroll_left,
            self.scroll_right,
        )
    }

    pub fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        if top < bottom && bottom < self.rows {
            self.scroll_top = top;
            self.scroll_bottom = bottom;
        }
    }

    pub fn set_lr_margins(&mut self, left: u16, right: u16) {
        if left < right && right < self.cols {
            self.scroll_left = left;
            self.scroll_right = right;
        }
    }

    pub fn scroll_top(&self) -> u16 {
        self.scroll_top
    }

    pub fn scroll_bottom(&self) -> u16 {
        self.scroll_bottom
    }

    pub fn scroll_left(&self) -> u16 {
        self.scroll_left
    }

    pub fn scroll_right(&self) -> u16 {
        self.scroll_right
    }

    pub fn tab_at(&self, col: u16) -> bool {
        self.tabs.get(col as usize).copied().unwrap_or(false)
    }

    pub fn set_tab(&mut self, col: u16) {
        if (col as usize) < self.tabs.len() {
            self.tabs[col as usize] = true;
        }
    }

    pub fn clear_tab(&mut self, col: u16) {
        if (col as usize) < self.tabs.len() {
            self.tabs[col as usize] = false;
        }
    }

    pub fn reset_tabs(&mut self) {
        for i in 0..self.tabs.len() {
            self.tabs[i] = i % 8 == 0;
        }
    }

    pub fn next_tab(&self, from: u16) -> u16 {
        let mut c = from.saturating_add(1);
        while c < self.cols && !self.tab_at(c) {
            c += 1;
        }
        c.min(self.cols - 1)
    }

    pub fn prev_tab(&self, from: u16) -> u16 {
        let mut c = from;
        while c > 0 {
            c -= 1;
            if self.tab_at(c) {
                return c;
            }
        }
        0
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Vec<u16> {
        let cols = cols.clamp(1, MAX_COLS);
        let rows = rows.clamp(1, MAX_ROWS);
        let mut pushed = Vec::new();
        let full_vertical = self.scroll_top == 0 && self.scroll_bottom == self.rows - 1;
        let full_horizontal = self.scroll_left == 0 && self.scroll_right == self.cols - 1;
        if cols != self.cols {
            self.set_cols(cols);
        }
        if rows != self.rows {
            pushed = self.set_rows(rows);
        }
        self.cursor.row = self.cursor.row.min(self.rows - 1);
        self.cursor.col = self.cursor.col.min(self.cols - 1);
        self.wrap_pending = false;
        self.scroll_bottom = if full_vertical {
            self.rows - 1
        } else {
            self.scroll_bottom.min(self.rows - 1)
        };
        self.scroll_top = self.scroll_top.min(self.scroll_bottom);
        self.scroll_right = if full_horizontal {
            self.cols - 1
        } else {
            self.scroll_right.min(self.cols - 1)
        };
        self.scroll_left = self.scroll_left.min(self.scroll_right);
        if let Some(saved) = self.saved_cursor.as_mut() {
            saved.cursor.row = saved.cursor.row.min(self.rows - 1);
            saved.cursor.col = saved.cursor.col.min(self.cols - 1);
            saved.wrap_pending = false;
        }
        pushed
    }

    fn set_cols(&mut self, cols: u16) {
        let mut next = vec![Cell::default(); cols as usize * self.rows as usize];
        let common = self.cols.min(cols) as usize;
        for r in 0..self.rows as usize {
            let src = r * self.cols as usize;
            let dst = r * cols as usize;
            for (dst_cell, src_cell) in next[dst..dst + common]
                .iter_mut()
                .zip(&self.cells[src..src + common])
            {
                *dst_cell = src_cell.clone();
            }
        }
        self.cells = next;
        self.cols = cols;
        let old_tabs = self.tabs.len();
        self.tabs.resize(cols as usize, false);
        for c in old_tabs..cols as usize {
            self.tabs[c] = c % 8 == 0;
        }
        for row in 0..self.rows {
            self.repair_wide(row);
        }
    }

    fn set_rows(&mut self, rows: u16) -> Vec<u16> {
        let old = self.rows;
        let mut pushed = Vec::new();
        if rows < old {
            let dropped = (old - rows) as usize;
            // Keep the live cursor visible, moving displaced top rows to history.
            let shift = dropped.min(self.cursor.row.saturating_sub(rows - 1) as usize);
            for r in 0..shift {
                self.pending_history
                    .push(self.row_slice(r as u16).unwrap().to_vec());
                self.pending_wrapped.push(self.wrapped[r]);
                pushed.push(r as u16);
            }
            self.cells.drain(..shift * self.cols as usize);
            self.wrapped.drain(..shift);
            self.cursor.row = self.cursor.row.saturating_sub(shift as u16);
            self.cells.truncate(rows as usize * self.cols as usize);
            self.wrapped.truncate(rows as usize);
        } else {
            let extra = (rows - old) as usize;
            let blank = Cell::default();
            for _ in 0..extra {
                for _ in 0..self.cols {
                    self.cells.push(blank.clone());
                }
            }
        }
        self.wrapped.resize(rows as usize, false);
        self.rows = rows;
        pushed
    }

    pub fn scroll_up(&mut self, n: u16) {
        let top = self.scroll_top;
        let bottom = self.scroll_bottom;
        self.scroll_region_impl(top, bottom, n, true)
    }

    pub fn scroll_down(&mut self, n: u16) {
        let top = self.scroll_top;
        let bottom = self.scroll_bottom;
        self.scroll_region_impl(top, bottom, n, false)
    }

    fn scroll_region_impl(&mut self, top: u16, bottom: u16, n: u16, up: bool) {
        let height = bottom.saturating_sub(top) + 1;
        if n == 0 {
            return;
        }
        let n = n.min(height);
        let width = self.cols as usize;
        let mut removed = Vec::with_capacity(n as usize);
        for offset in 0..n {
            let idx = if up {
                (top + offset) as usize
            } else {
                bottom as usize
            };
            let start = idx * width;
            removed.push(self.cells[start..start + width].to_vec());
            if up {
                self.pending_wrapped.push(self.wrapped[idx]);
            }
        }
        let blank = Cell {
            bg: self.default_bg,
            ..Cell::default()
        };
        let region = &mut self.cells[top as usize * width..(bottom as usize + 1) * width];
        let shift = n as usize * width;
        let len = region.len();
        if up {
            for i in 0..len - shift {
                region[i] = region[i + shift].clone();
            }
            for c in region[len - shift..].iter_mut() {
                *c = blank.clone();
            }
            // `drain(..)` moves each row rather than copying it, so recording
            // history costs no extra allocation beyond the row the grid already
            // built for the scroll itself.
            self.pending_history.append(&mut removed);
            for r in top..=bottom {
                self.wrapped[r as usize] = if r + n <= bottom {
                    self.wrapped[(r + n) as usize]
                } else {
                    false
                };
            }
        } else {
            for i in (0..len - shift).rev() {
                region[i + shift] = region[i].clone();
            }
            for r in (top..=bottom).rev() {
                self.wrapped[r as usize] = if r >= top + n {
                    self.wrapped[(r - n) as usize]
                } else {
                    false
                };
            }
            for c in region[..shift].iter_mut() {
                *c = blank.clone();
            }
        }
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some(SavedCursor {
            cursor: self.cursor,
            origin_mode: self.origin_mode,
            wrap_pending: self.wrap_pending,
        });
    }

    pub fn restore_cursor(&mut self) {
        if let Some(s) = self.saved_cursor {
            self.cursor = s.cursor;
            self.origin_mode = s.origin_mode;
            self.wrap_pending = s.wrap_pending;
        }
    }

    /// Writes a wide character and its mandatory spacer via a validated path only.
    pub fn write_wide(&mut self, row: u16, col: u16, cp: u32, cell: Cell) -> bool {
        if col + 1 >= self.cols || row >= self.rows {
            return false;
        }
        self.clear_wide_at(row, col);
        self.clear_wide_at(row, col + 1);
        if let Some(c) = self.cell_mut(row, col) {
            *c = cell.clone();
            c.content = CellContent::WideChar(cp);
        }
        if let Some(c) = self.cell_mut(row, col + 1) {
            *c = cell;
            c.content = CellContent::WideCharSpacer;
        }
        true
    }

    pub fn clear_row_range(&mut self, row: u16, from: u16, to: u16, cell: Cell) {
        self.clear_wide_at(row, from);
        self.clear_wide_at(row, to);
        let from = from.min(to);
        let to = to.min(self.cols - 1);
        for col in from..=to {
            if let Some(c) = self.cell_mut(row, col) {
                *c = cell.clone();
            }
        }
    }

    pub fn advance_cursor(&mut self, width: Columns) {
        if self.wrap_pending && width > 0 {
            self.cursor.col = self.scroll_left;
            self.line_feed();
            self.wrap_pending = false;
        }
        if width == 0 {
            return;
        }
        let target = self.cursor.col as u32 + width as u32;
        let right = self.scroll_right as u32;
        if target > right {
            if self.autowrap {
                self.wrap_pending = true;
            } else {
                self.cursor.col = self.scroll_right;
            }
        } else {
            self.cursor.col = (target as u16).min(self.cols - 1);
        }
    }

    pub fn line_feed(&mut self) {
        self.wrap_pending = false;
        if self.cursor.row == self.scroll_bottom {
            self.scroll_up(1);
        } else if self.cursor.row + 1 < self.rows {
            self.cursor.row += 1;
        }
    }

    pub fn reverse_index(&mut self) {
        self.wrap_pending = false;
        if self.cursor.row == self.scroll_top {
            self.scroll_down(1);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
        }
    }

    /// Take every row scrolled off the top since the last call, oldest first.
    ///
    /// Scrolling *down* never records history: it pulls blank lines in at the top,
    /// so nothing left the screen.
    pub fn take_pending_history(&mut self) -> Vec<Vec<Cell>> {
        self.pending_wrapped.clear();
        std::mem::take(&mut self.pending_history)
    }

    pub fn take_history(&mut self) -> Vec<(Vec<Cell>, bool)> {
        std::mem::take(&mut self.pending_history)
            .into_iter()
            .zip(std::mem::take(&mut self.pending_wrapped))
            .collect()
    }

    pub fn mark_wrapped(&mut self) {
        self.wrapped[self.cursor.row as usize] = true;
    }
    pub fn row_wrapped(&self, row: u16) -> bool {
        self.wrapped.get(row as usize).copied().unwrap_or(false)
    }
    pub fn clear_wide_at(&mut self, row: u16, col: u16) {
        let Some(cell) = self.cell(row, col) else {
            return;
        };
        let start = if cell.content == CellContent::WideCharSpacer {
            col.saturating_sub(1)
        } else {
            col
        };
        let wide = self
            .cell(row, start)
            .is_some_and(|c| c.content.width() == 2);
        if wide {
            for c in start..=start.saturating_add(1) {
                if let Some(v) = self.cell_mut(row, c) {
                    v.content = CellContent::Empty;
                }
            }
        }
    }
    pub fn repair_wide(&mut self, row: u16) {
        for c in 0..self.cols {
            let Some(cell) = self.cell(row, c) else {
                continue;
            };
            let orphan = match cell.content {
                CellContent::WideCharSpacer => {
                    c == 0
                        || !self
                            .cell(row, c - 1)
                            .is_some_and(|v| v.content.width() == 2)
                }
                _ if cell.content.width() == 2 => {
                    c + 1 >= self.cols
                        || !self
                            .cell(row, c + 1)
                            .is_some_and(|v| v.content == CellContent::WideCharSpacer)
                }
                _ => false,
            };
            if orphan {
                self.cell_mut(row, c).unwrap().content = CellContent::Empty;
            }
        }
    }

    /// True when the scroll region covers the whole screen. Lines leaving a
    /// *restricted* region are still on screen conceptually and must not become
    /// scrollback (`ARCHITECTURE.md` 2.3).
    pub fn scrolls_whole_screen(&self) -> bool {
        self.scroll_top == 0 && self.scroll_bottom + 1 >= self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_are_clamped_to_the_specified_maximum() {
        let g = Grid::new(9999, 9999);
        assert_eq!(g.cols(), MAX_COLS);
        assert_eq!(g.rows(), MAX_ROWS);
        let g = Grid::new(0, 0);
        assert_eq!((g.cols(), g.rows()), (1, 1));
    }

    #[test]
    fn default_scroll_region_covers_the_grid() {
        let g = Grid::new(80, 24);
        assert_eq!((g.scroll_top(), g.scroll_bottom()), (0, 23));
        assert_eq!((g.scroll_left(), g.scroll_right()), (0, 79));
    }

    #[test]
    fn invalid_scroll_regions_are_rejected() {
        let mut g = Grid::new(80, 24);
        g.set_scroll_region(10, 5);
        assert_eq!((g.scroll_top(), g.scroll_bottom()), (0, 23));
        g.set_scroll_region(5, 500);
        assert_eq!((g.scroll_top(), g.scroll_bottom()), (0, 23));
        g.set_scroll_region(5, 20);
        assert_eq!((g.scroll_top(), g.scroll_bottom()), (5, 20));
    }

    #[test]
    fn deferred_wrap_latches_at_the_right_margin() {
        let mut g = Grid::new(5, 3);
        g.autowrap = true;
        g.cursor.col = 4;
        assert!(!g.wrap_pending);
        g.advance_cursor(1);
        assert!(g.wrap_pending, "writing in last column must latch");
        assert_eq!(g.cursor.col, 4, "column must not advance while latched");
        g.advance_cursor(1);
        assert!(!g.wrap_pending);
        assert_eq!(g.cursor.row, 1, "wrapped to the next row");
        assert_eq!(g.cursor.col, 1, "cursor sits after the printed glyph");
    }

    #[test]
    fn autowrap_disabled_clamps_to_the_margin() {
        let mut g = Grid::new(5, 3);
        g.autowrap = false;
        g.cursor.col = 4;
        g.advance_cursor(1);
        assert!(!g.wrap_pending);
        assert_eq!(g.cursor.col, 4);
    }

    #[test]
    fn zero_width_content_never_moves_the_cursor() {
        let mut g = Grid::new(5, 3);
        g.cursor.col = 2;
        g.advance_cursor(0);
        assert_eq!(g.cursor.col, 2);
        assert!(!g.wrap_pending);
    }

    #[test]
    fn scroll_up_removes_from_the_top_and_blanks_the_bottom() {
        let mut g = Grid::new(3, 3);
        for r in 0..3u16 {
            g.cell_mut(r, 0)
                .unwrap()
                .attrs
                .set(crate::vt::cell::Attrs::BOLD, r == 1);
        }
        g.scroll_up(1);
        let removed = g.take_pending_history();
        assert_eq!(removed.len(), 1);
        assert!(!removed[0][0].attrs.is(crate::vt::cell::Attrs::BOLD));
        assert!(
            g.cell(0, 0).unwrap().attrs.is(crate::vt::cell::Attrs::BOLD),
            "row 1 must move up into row 0"
        );
        assert!(!g.cell(2, 0).unwrap().attrs.is(crate::vt::cell::Attrs::BOLD));
    }

    #[test]
    fn scroll_down_blanks_the_top() {
        let mut g = Grid::new(3, 3);
        for r in 0..3u16 {
            for c in 0..3u16 {
                g.cell_mut(r, c)
                    .unwrap()
                    .attrs
                    .set(crate::vt::cell::Attrs::BOLD, r == 0);
            }
        }
        g.scroll_down(1);
        assert!(!g.cell(0, 0).unwrap().attrs.is(crate::vt::cell::Attrs::BOLD));
        assert!(g.cell(1, 0).unwrap().attrs.is(crate::vt::cell::Attrs::BOLD));
    }

    #[test]
    fn scrolling_respects_a_restricted_region() {
        let mut g = Grid::new(3, 4);
        g.set_scroll_region(1, 2);
        for r in 0..4u16 {
            g.cell_mut(r, 0)
                .unwrap()
                .attrs
                .set(crate::vt::cell::Attrs::BOLD, r == 0);
        }
        g.scroll_up(1);
        assert!(
            g.cell(0, 0).unwrap().attrs.is(crate::vt::cell::Attrs::BOLD),
            "rows outside the scroll region must not move"
        );
        assert!(!g.cell(3, 0).unwrap().attrs.is(crate::vt::cell::Attrs::BOLD));
    }

    #[test]
    fn line_feed_at_the_bottom_scrolls_the_region() {
        let mut g = Grid::new(3, 3);
        g.cursor.row = 2;
        g.line_feed();
        assert_eq!(g.cursor.row, 2, "cursor stays pinned at region bottom");
    }

    #[test]
    fn save_restore_preserves_the_wrap_latch() {
        let mut g = Grid::new(5, 3);
        g.cursor.row = 1;
        g.cursor.col = 3;
        g.wrap_pending = true;
        g.save_cursor();
        g.cursor.row = 0;
        g.cursor.col = 0;
        g.wrap_pending = false;
        g.restore_cursor();
        assert_eq!(g.cursor.row, 1);
        assert_eq!(g.cursor.col, 3);
        assert!(g.wrap_pending, "wrap_pending must survive save/restore");
    }

    #[test]
    fn tabs_follow_the_eight_column_cadence() {
        let g = Grid::new(80, 24);
        assert!(g.tab_at(0));
        assert!(g.tab_at(8));
        assert!(!g.tab_at(3));
        assert_eq!(g.next_tab(3), 8);
        assert_eq!(g.next_tab(8), 16);
        assert_eq!(g.prev_tab(20), 16);
    }

    #[test]
    fn wide_write_places_the_spacer_and_refuses_the_edge() {
        let mut g = Grid::new(4, 2);
        assert!(g.write_wide(0, 0, u32::from('漢'), Cell::default()));
        assert!(matches!(
            g.cell(0, 0).unwrap().content,
            CellContent::WideChar(_)
        ));
        assert!(matches!(
            g.cell(0, 1).unwrap().content,
            CellContent::WideCharSpacer
        ));
        assert!(
            !g.write_wide(0, 3, u32::from('漢'), Cell::default()),
            "edge must be refused"
        );
        assert!(!g.write_wide(9, 0, u32::from('漢'), Cell::default()));
    }

    #[test]
    fn resize_preserves_the_top_left_corner() {
        let mut g = Grid::new(4, 4);
        for r in 0..4u16 {
            for c in 0..4u16 {
                g.cell_mut(r, c)
                    .unwrap()
                    .attrs
                    .set(crate::vt::cell::Attrs::BOLD, r == 1 && c == 2);
            }
        }
        g.resize(6, 6);
        assert_eq!((g.cols(), g.rows()), (6, 6));
        assert!(
            g.cell(1, 2).unwrap().attrs.is(crate::vt::cell::Attrs::BOLD),
            "growth must not disturb existing content"
        );
        assert_eq!(g.cell(5, 5).unwrap().content, CellContent::Empty);
    }

    #[test]
    fn narrowing_discards_columns_beyond_the_new_edge() {
        let mut g = Grid::new(4, 4);
        for c in 0..4u16 {
            g.cell_mut(1, c)
                .unwrap()
                .attrs
                .set(crate::vt::cell::Attrs::BOLD, c == 2);
        }
        g.resize(2, 2);
        assert_eq!((g.cols(), g.rows()), (2, 2));
        assert_eq!(g.cell(1, 1).unwrap().content, CellContent::Empty);
    }

    #[test]
    fn resize_clamps_the_cursor() {
        let mut g = Grid::new(10, 10);
        g.cursor.row = 9;
        g.cursor.col = 9;
        g.resize(3, 3);
        assert_eq!((g.cursor.row, g.cursor.col), (2, 2));
    }

    #[test]
    fn out_of_range_access_returns_none() {
        let g = Grid::new(4, 4);
        assert!(g.cell(9, 0).is_none());
        assert!(g.cell(0, 9).is_none());
        assert!(g.row_slice(9).is_none());
    }
}
