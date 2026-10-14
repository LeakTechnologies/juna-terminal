pub mod cell;
mod cwd;
pub mod grid;
pub mod parser;
pub mod scrollback;
mod snapshot;
pub mod width;
pub use snapshot::{InputModes, RenderSnapshot};

#[cfg(test)]
mod regression;
#[cfg(test)]
mod tests;

use std::marker::PhantomData;

pub use cell::{Attrs, Cell, CellContent, Color, Interner};
pub use grid::{Grid, MAX_COLS, MAX_ROWS};
pub use parser::{Action, Parser, State};
pub use scrollback::Scrollback;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Damage {
    Full,
    Rows(std::ops::Range<u16>),
}

#[derive(Clone, Debug, Default)]
pub struct SnapshotDelta {
    pub damage: Vec<Damage>,
    pub resized: bool,
    pub wrap_pending: bool,
    pub alternate: bool,
    pub dirty_row: Option<u16>,
}

impl SnapshotDelta {
    pub fn is_empty(&self) -> bool {
        self.damage.is_empty() && !self.resized
    }

    fn note(&mut self, row: u16) {
        match self.dirty_row {
            None => {
                self.dirty_row = Some(row);
                self.damage = vec![Damage::Rows(row..row + 1)];
            }
            Some(prev) if row != prev => {
                let lo = prev.min(row);
                let hi = prev.max(row) + 1;
                self.dirty_row = Some(row);
                self.damage = vec![Damage::Rows(lo..hi)];
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Primary,
    Alternate,
}

struct Stack {
    primary: Box<Grid>,
    alternate: Box<Grid>,
    active: Screen,
}

pub struct TerminalState {
    parser: Parser,
    stack: Stack,
    scrollback: Scrollback,
    interner: Interner,
    fg: Color,
    bg: Color,
    attrs: Attrs,
    title: String,
    icon_title: String,
    cwd: Option<String>,
    cwd_revision: u64,
    modes: InputModes,
    cursor_style: u16,
    responses: Vec<u8>,
    saved_rendition: Option<(Color, Color, Attrs)>,
    last_print: Option<(u16, u16)>,
    last_scalar: Option<u32>,
    dec_graphics: bool,
    g0_graphics: bool,
    g1_graphics: bool,
    /// Makes the engine `!Sync` while remaining `Send`, so the single-owner rule in
    /// ARCHITECTURE.md 3 is a compiler guarantee rather than a convention.
    not_sync: PhantomData<std::cell::Cell<()>>,
}

impl TerminalState {
    pub fn new(cols: u16, rows: u16) -> Self {
        let primary = Grid::new(cols, rows);
        let alternate = Grid::new(cols, rows);
        Self {
            parser: Parser::new(),
            stack: Stack {
                primary: Box::new(primary),
                alternate: Box::new(alternate),
                active: Screen::Primary,
            },
            scrollback: Scrollback::new(scrollback::DEFAULT_BUDGET, cols.max(1)),
            interner: Interner::new(),
            fg: Color::Default,
            bg: Color::Default,
            attrs: Attrs::new(0),
            title: String::new(),
            icon_title: String::new(),
            cwd: None,
            cwd_revision: 0,
            modes: InputModes::default(),
            cursor_style: 0,
            responses: Vec::new(),
            saved_rendition: None,
            last_print: None,
            last_scalar: None,
            dec_graphics: false,
            g0_graphics: false,
            g1_graphics: false,
            not_sync: PhantomData,
        }
    }

    pub fn grid(&self) -> &Grid {
        match self.stack.active {
            Screen::Primary => &self.stack.primary,
            Screen::Alternate => &self.stack.alternate,
        }
    }

    fn grid_mut(&mut self) -> &mut Grid {
        match self.stack.active {
            Screen::Primary => &mut self.stack.primary,
            Screen::Alternate => &mut self.stack.alternate,
        }
    }

    pub fn alternate_active(&self) -> bool {
        self.stack.active == Screen::Alternate
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn working_directory(&self) -> (u64, Option<&str>) {
        (self.cwd_revision, self.cwd.as_deref())
    }
    pub fn input_modes(&self) -> InputModes {
        self.modes
    }
    pub fn history_wrapped(&self, index: usize) -> bool {
        self.scrollback.is_wrapped(index)
    }
    pub fn take_responses(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.responses)
    }
    pub fn finish(&mut self) {
        for action in self.parser.flush() {
            self.apply(action, &mut SnapshotDelta::default());
            self.collect_history();
        }
    }
    pub fn snapshot(&self, offset: usize) -> RenderSnapshot {
        let g = self.grid();
        let history = if self.alternate_active() {
            0
        } else {
            self.scrollback_len()
        };
        let offset = offset.min(history);
        let start = history - offset;
        let mut cells = Vec::with_capacity(g.cols() as usize * g.rows() as usize);
        let mut wrapped = Vec::new();
        for i in start..start + g.rows() as usize {
            let (row, wrap) = if i < history {
                (
                    self.scrollback_line(i).unwrap(),
                    self.scrollback.is_wrapped(i),
                )
            } else {
                (
                    g.row_slice((i - history) as u16).unwrap(),
                    g.row_wrapped((i - history) as u16),
                )
            };
            cells.extend(row.iter().take(g.cols() as usize).cloned());
            cells.resize((wrapped.len() + 1) * g.cols() as usize, Cell::default());
            wrapped.push(wrap);
        }
        let mut cursor = g.cursor;
        cursor.visible &= offset == 0;
        RenderSnapshot {
            cols: g.cols(),
            rows: g.rows(),
            cells,
            wrapped,
            cursor,
            cursor_style: self.cursor_style,
        }
    }

    pub fn interner_len(&self) -> usize {
        self.interner.len()
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// Borrow one scrollback line, oldest first. `None` when `index` is out of range.
    ///
    /// Added because scrollback was otherwise write-only from outside the module:
    /// nothing outside `vt` could read what it had captured, so a scrollback bug
    /// would be undetectable until a viewport existed to show it.
    pub fn scrollback_line(&self, index: usize) -> Option<&[Cell]> {
        self.scrollback.get(index)
    }

    /// Lines evicted because the byte budget (D10.3) was exceeded. Non-zero is not
    /// an error, but it means history is incomplete and a test must say so.
    pub fn scrollback_dropped(&self) -> u64 {
        self.scrollback.dropped_lines()
    }

    pub fn scrollback_bytes(&self) -> usize {
        self.scrollback.bytes()
    }

    pub fn parser_state(&self) -> State {
        self.parser.state()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> SnapshotDelta {
        let cols = cols.clamp(1, MAX_COLS);
        let rows = rows.clamp(1, MAX_ROWS);
        self.scrollback.set_cols(cols);
        self.stack.primary.resize(cols, rows);
        self.stack.alternate.resize(cols, rows);
        self.collect_history();
        SnapshotDelta {
            damage: vec![Damage::Full],
            resized: true,
            ..Default::default()
        }
    }

    pub fn advance(&mut self, bytes: &[u8]) -> SnapshotDelta {
        let actions = self.parser.advance(bytes);
        let mut delta = SnapshotDelta::default();
        for a in actions {
            if !matches!(a, Action::Print(_)) {
                self.last_print = None;
            }
            self.apply(a, &mut delta);
            self.collect_history();
        }
        delta.wrap_pending = self.grid().wrap_pending;
        delta.alternate = self.alternate_active();
        if !bytes.is_empty() {
            delta.damage = vec![Damage::Full];
        }
        delta
    }

    /// Move rows the grid scrolled off into scrollback, when they belong there.
    ///
    /// `ARCHITECTURE.md` 2.3: the primary screen pushes scrolled-off lines into
    /// scrollback; the alternate screen does not. Lines leaving a *restricted*
    /// scroll region are excluded too, because they are still on screen and a
    /// terminal that filed them into history would duplicate them once the region
    /// was reset.
    ///
    /// **This was absent until the headless harness caught it.** The `Scrollback`
    /// type worked and passed its own tests, but nothing ever called `push`, so
    /// scrollback was permanently empty no matter what the program printed.
    fn collect_history(&mut self) {
        let region_is_full = self.grid().scrolls_whole_screen();
        let primary = !self.alternate_active();
        if !region_is_full || !primary {
            // Still drain: rows leaving a region are discarded, not buffered
            // forever. Buffering them would leak a line per scroll.
            self.grid_mut().take_pending_history();
            return;
        }
        for (row, wrapped) in self.grid_mut().take_history() {
            self.scrollback.push(row, wrapped);
        }
    }

    fn apply(&mut self, action: Action, delta: &mut SnapshotDelta) {
        match action {
            Action::Print(cp) => self.print(cp, delta),
            Action::Execute(b) => self.execute(b, delta),
            Action::CsiDispatch {
                private,
                params,
                intermediates,
                final_byte,
            } => self.csi(private, &params, &intermediates, final_byte, delta),
            Action::EscapeDispatch {
                intermediates,
                final_byte,
            } => self.escape(&intermediates, final_byte, delta),
            Action::OscDispatch { payload, .. } => self.osc(&payload),
            Action::DcsDispatch { .. } => {}
            Action::Unsupported => {}
        }
    }

    fn print(&mut self, cp: u32, delta: &mut SnapshotDelta) {
        let cp = if self.dec_graphics {
            dec_line_drawing(cp)
        } else {
            cp
        };
        let c = char::from_u32(cp).unwrap_or('\u{FFFD}');
        self.last_scalar = Some(cp);
        if self.extend_cluster(c, delta) {
            return;
        }
        let w = width::char_width(c);
        if w == 2 && self.grid().cols() == 1 {
            self.print(0xfffd, delta);
            return;
        }
        if w == 0 {
            self.print(0xfffd, delta);
            return;
        }

        let cell = Cell {
            fg: self.fg,
            bg: self.bg,
            attrs: self.attrs,
            ..Default::default()
        };

        let g = self.grid_mut();
        let left = g.scroll_left();
        let right = g.scroll_right();

        if g.wrap_pending {
            g.mark_wrapped();
            g.wrap_pending = false;
            g.cursor.col = left;
            g.line_feed();
        }
        if g.cursor.col + (w as u16) > right + 1 && g.autowrap && g.cursor.col > left {
            g.mark_wrapped();
            g.cursor.col = right;
            g.line_feed();
            g.cursor.col = left;
        }

        let row = g.cursor.row;
        let col = g.cursor.col;
        let ok = if w == 2 {
            g.write_wide(row, col, cp, cell)
        } else {
            g.clear_wide_at(row, col);
            match g.cell_mut(row, col) {
                Some(target) => {
                    *target = cell;
                    target.content = CellContent::Char(cp);
                    true
                }
                None => false,
            }
        };
        if ok {
            delta.note(row);
        }
        g.advance_cursor(w);
        self.last_print = Some((row, col));
    }

    fn extend_cluster(&mut self, c: char, delta: &mut SnapshotDelta) -> bool {
        let Some((row, col)) = self.last_print else {
            return false;
        };
        let Some(cell) = self.grid().cell(row, col).cloned() else {
            return false;
        };
        let mut text = match &cell.content {
            CellContent::Char(cp) | CellContent::WideChar(cp) => {
                char::from_u32(*cp).unwrap_or('\u{FFFD}').to_string()
            }
            CellContent::Cluster(s) => s.to_string(),
            _ => return false,
        };
        if !width::joins(&text, c) {
            return false;
        }
        // Bound a malicious endlessly extended grapheme; preserve visible replacement.
        if text.len() >= 256 {
            return false;
        }
        text.push(c);
        let old = cell.content.width();
        let new = width::cluster_width(&text).max(1);
        if new == 2 && col + 1 >= self.grid().cols() {
            return false;
        }
        let content = CellContent::Cluster(self.interner.intern(&text));
        let g = self.grid_mut();
        g.cell_mut(row, col).unwrap().content = content;
        if new == 2 {
            let mut spacer = cell;
            spacer.content = CellContent::WideCharSpacer;
            *g.cell_mut(row, col + 1).unwrap() = spacer;
        }
        if new > old {
            g.advance_cursor(new - old);
        }
        if new < old {
            g.cell_mut(row, col + 1).unwrap().content = CellContent::Empty;
            g.cursor.col = g.cursor.col.saturating_sub((old - new) as u16);
        }
        delta.note(row);
        true
    }

    fn execute(&mut self, b: u8, delta: &mut SnapshotDelta) {
        match b {
            0x0A..=0x0C => {
                let row = self.grid().cursor.row;
                self.grid_mut().line_feed();
                delta.note(row);
            }
            0x0D => {
                let g = self.grid_mut();
                g.cursor.col = g.scroll_left();
                g.wrap_pending = false;
            }
            0x08 => {
                let g = self.grid_mut();
                if g.cursor.col > g.scroll_left() {
                    g.cursor.col -= 1;
                    g.wrap_pending = false;
                }
            }
            0x09 => {
                let next = self.grid().next_tab(self.grid().cursor.col);
                self.grid_mut().cursor.col = next.min(self.grid().scroll_right());
                self.grid_mut().wrap_pending = false;
            }
            0x07 | 0x00 => {}
            0x0e => self.dec_graphics = self.g1_graphics,
            0x0f => self.dec_graphics = self.g0_graphics,
            0x84 => {
                self.grid_mut().line_feed();
                delta.damage = vec![Damage::Full];
            }
            0x85 => {
                let g = self.grid_mut();
                g.line_feed();
                g.cursor.col = 0;
                delta.damage = vec![Damage::Full];
            }
            0x88 => {
                let g = self.grid_mut();
                g.set_tab(g.cursor.col);
            }
            0x8d => {
                self.grid_mut().reverse_index();
                delta.damage = vec![Damage::Full];
            }
            _ => {}
        }
    }

    fn escape(&mut self, intermediates: &[u8], final_byte: u8, delta: &mut SnapshotDelta) {
        match (intermediates.first().copied(), final_byte) {
            (None, b'7') => self.save_cursor(),
            (None, b'8') => self.restore_cursor(),
            (Some(b'('), b'0') => {
                self.g0_graphics = true;
                self.dec_graphics = true;
            }
            (Some(b'('), b'B') => {
                self.g0_graphics = false;
                self.dec_graphics = false;
            }
            (Some(b')'), b'0') => self.g1_graphics = true,
            (Some(b')'), b'B') => self.g1_graphics = false,
            (None, b'=') => self.modes.application_keypad = true,
            (None, b'>') => self.modes.application_keypad = false,
            (None, b'D') => {
                let row = self.grid().cursor.row;
                self.grid_mut().line_feed();
                delta.note(row);
            }
            (None, b'E') => {
                let row = self.grid().cursor.row;
                self.grid_mut().line_feed();
                self.grid_mut().cursor.col = 0;
                delta.note(row);
            }
            (None, b'M') => {
                let row = self.grid().cursor.row;
                self.grid_mut().reverse_index();
                delta.note(row);
            }
            (None, b'H') => {
                let col = self.grid().cursor.col;
                self.grid_mut().set_tab(col);
            }
            (None, b'c') => {
                let (c, r) = (self.grid().cols(), self.grid().rows());
                *self = Self::new(c, r);
                delta.damage = vec![Damage::Full];
            }
            _ => {}
        }
    }

    fn osc(&mut self, payload: &str) {
        let Some((command, text)) = payload.split_once(';') else {
            return;
        };
        match command {
            "0" => {
                self.title = text.to_owned();
                self.icon_title = text.to_owned();
            }
            "1" => self.icon_title = text.to_owned(),
            "2" => self.title = text.to_owned(),
            "7" => {
                self.cwd = cwd::local_path(text);
                self.cwd_revision = self.cwd_revision.wrapping_add(1);
            }
            _ => {}
        }
    }

    fn csi(
        &mut self,
        private: Option<u8>,
        params: &parser::Params,
        intermediates: &[u8],
        final_byte: u8,
        delta: &mut SnapshotDelta,
    ) {
        if params.overflow {
            return;
        }
        if intermediates == b" " && final_byte == b'q' && private.is_none() {
            self.cursor_style = params.get(0).min(6);
            return;
        }
        if intermediates == b"$" && final_byte == b'p' && private == Some(b'?') {
            let mode = params.get(0);
            let status = match mode {
                1 => Some(self.modes.application_cursor),
                6 => Some(self.grid().origin_mode),
                7 => Some(self.grid().autowrap),
                25 => Some(self.grid().cursor.visible),
                1004 => Some(self.modes.focus_events),
                1006 => Some(self.modes.mouse_sgr),
                2004 => Some(self.modes.bracketed_paste),
                2026 => Some(self.modes.synchronized_output),
                _ => None,
            };
            self.responses.extend_from_slice(
                format!(
                    "\x1b[?{mode};{}$y",
                    status.map(|s| if s { 1 } else { 2 }).unwrap_or(0)
                )
                .as_bytes(),
            );
            return;
        }
        if !intermediates.is_empty()
            || (private.is_some() && !matches!(final_byte, b'h' | b'l' | b'n' | b'c'))
        {
            return;
        }
        let p = |i: usize| params.get(i);
        let g_default = |i: usize, d: u16| params.get_or(i, d);
        let row = self.grid().cursor.row;
        match final_byte {
            b'n' => match p(0) {
                5 if private.is_none() => self.responses.extend_from_slice(b"\x1b[0n"),
                6 => {
                    let cur = self.grid().cursor;
                    self.responses.extend_from_slice(
                        format!(
                            "\x1b[{}{};{}R",
                            if private == Some(b'?') { "?" } else { "" },
                            cur.row + 1,
                            cur.col + 1
                        )
                        .as_bytes(),
                    );
                }
                _ => {}
            },
            b'c' if private.is_none() => self.responses.extend_from_slice(b"\x1b[?1;2c"),
            b'b' => {
                if let Some(cp) = self.last_scalar {
                    for _ in 0..g_default(0, 1).min(MAX_COLS) {
                        self.print(cp, delta);
                    }
                }
            }
            b'A' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                let top = if g.origin_mode
                    || (g.cursor.row >= g.scroll_top() && g.cursor.row <= g.scroll_bottom())
                {
                    g.scroll_top()
                } else {
                    0
                };
                g.cursor.row = g.cursor.row.saturating_sub(n).max(top);
            }
            b'B' | b'e' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                let bottom = if g.origin_mode
                    || (g.cursor.row >= g.scroll_top() && g.cursor.row <= g.scroll_bottom())
                {
                    g.scroll_bottom()
                } else {
                    g.rows() - 1
                };
                g.cursor.row = g.cursor.row.saturating_add(n).min(bottom);
            }
            b'C' | b'a' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.col = g.cursor.col.saturating_add(n).min(g.scroll_right());
            }
            b'D' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.col = g.cursor.col.saturating_sub(n).max(g.scroll_left());
            }
            b'E' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.row = g.cursor.row.saturating_add(n).min(g.rows() - 1);
                g.cursor.col = g.scroll_left();
                delta.note(row);
            }
            b'F' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.row = g.cursor.row.saturating_sub(n);
                g.cursor.col = g.scroll_left();
                delta.note(row);
            }
            b'G' | b'`' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.col = (n - 1).min(g.scroll_right());
            }
            b'd' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.row = if g.origin_mode {
                    g.scroll_top().saturating_add(n - 1).min(g.scroll_bottom())
                } else {
                    (n - 1).min(g.rows() - 1)
                };
            }
            b'H' | b'f' => {
                let row = g_default(0, 1) - 1;
                let col = g_default(1, 1) - 1;
                let origin = self.grid().origin_mode;
                let (top, bottom, left, right) = self.grid().scroll_region();
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.row = if origin {
                    top.saturating_add(row).min(bottom)
                } else {
                    row.min(g.rows() - 1)
                };
                g.cursor.col = if origin {
                    left.saturating_add(col).min(right)
                } else {
                    col.min(g.cols() - 1)
                };
            }
            b'J' => {
                self.erase_display(p(0), delta);
            }
            b'K' => {
                self.erase_line(p(0), delta);
            }
            b'L' => {
                let n = g_default(0, 1);
                let blank = self.default_cell();
                let r = self.grid().cursor.row;
                let bottom = self.grid().scroll_bottom();
                if r >= self.grid().scroll_top() && r <= bottom {
                    let g = self.grid_mut();
                    for _ in 0..n.min(bottom - r + 1) {
                        insert_lines(g, r, bottom, &blank);
                    }
                }
                delta.damage = vec![Damage::Full];
            }
            b'M' => {
                let n = g_default(0, 1);
                let blank = self.default_cell();
                let r = self.grid().cursor.row;
                let bottom = self.grid().scroll_bottom();
                if r >= self.grid().scroll_top() && r <= bottom {
                    let g = self.grid_mut();
                    for _ in 0..n.min(bottom - r + 1) {
                        delete_lines(g, r, bottom, &blank);
                    }
                }
                delta.damage = vec![Damage::Full];
            }
            b'P' => {
                let n = g_default(0, 1);
                let blank = self.default_cell();
                let cur = self.grid().cursor;
                delete_chars(self.grid_mut(), cur, n, &blank);
                delta.damage = vec![Damage::Full];
            }
            b'@' => {
                let n = g_default(0, 1);
                let blank = self.default_cell();
                let cur = self.grid().cursor;
                insert_chars(self.grid_mut(), cur, n, &blank);
                delta.damage = vec![Damage::Full];
            }
            b'X' => {
                let n = g_default(0, 1);
                let blank = self.default_cell();
                let cur = self.grid().cursor;
                erase_chars(self.grid_mut(), cur, n, &blank);
                delta.damage = vec![Damage::Full];
            }
            b'S' => {
                self.grid_mut().scroll_up(g_default(0, 1));
                delta.damage = vec![Damage::Full];
            }
            b'T' => {
                self.grid_mut().scroll_down(g_default(0, 1));
                delta.damage = vec![Damage::Full];
            }
            b'r' => {
                let top = g_default(0, 1) - 1;
                let bottom = g_default(1, self.grid().rows()) - 1;
                self.grid_mut().set_scroll_region(top, bottom);
                let g = self.grid_mut();
                g.cursor.row = 0;
                g.cursor.col = 0;
            }
            b'm' => self.sgr(params),
            b'h' => self.set_modes(private, params, true),
            b'l' => self.set_modes(private, params, false),
            b's' => self.save_cursor(),
            b'u' => self.restore_cursor(),
            b'g' => {
                let mode = p(0);
                let cur = self.grid().cursor.col;
                match mode {
                    0 => self.grid_mut().clear_tab(cur),
                    3 => {
                        let cols = self.grid().cols();
                        for c in 0..cols {
                            self.grid_mut().clear_tab(c);
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                if final_byte == b'q' && intermediates.first() == Some(&b' ') {
                    // DECSCUSR consumed; cursor style is a render concern.
                }
            }
        }
    }

    fn default_cell(&self) -> Cell {
        Cell {
            bg: self.bg,
            ..Default::default()
        }
    }

    fn erase_display(&mut self, mode: u16, delta: &mut SnapshotDelta) {
        let blank = self.default_cell();
        let mut clear_history = false;
        {
            let g = self.grid_mut();
            let (top, bottom, left, right) = (0, g.rows() - 1, 0, g.cols() - 1);
            match mode {
                0 => {
                    g.clear_row_range(g.cursor.row, g.cursor.col, right, blank.clone());
                    for r in (g.cursor.row + 1)..=bottom {
                        g.clear_row_range(r, left, right, blank.clone());
                    }
                }
                1 => {
                    for r in top..g.cursor.row {
                        g.clear_row_range(r, left, right, blank.clone());
                    }
                    g.clear_row_range(g.cursor.row, left, g.cursor.col, blank.clone());
                }
                2 => {
                    for r in top..=bottom {
                        g.clear_row_range(r, left, right, blank.clone());
                    }
                    g.wrap_pending = false;
                }
                3 => clear_history = true,
                _ => {}
            }
        }
        if clear_history {
            self.scrollback.clear();
        }
        delta.damage = vec![Damage::Full];
    }

    fn erase_line(&mut self, mode: u16, delta: &mut SnapshotDelta) {
        let blank = self.default_cell();
        let g = self.grid_mut();
        let (left, right) = (g.scroll_left(), g.scroll_right());
        match mode {
            0 => g.clear_row_range(g.cursor.row, g.cursor.col, right, blank),
            1 => g.clear_row_range(g.cursor.row, left, g.cursor.col, blank),
            2 => g.clear_row_range(g.cursor.row, left, right, blank),
            _ => {}
        }
        delta.note(g.cursor.row);
    }

    fn set_modes(&mut self, private: Option<u8>, params: &parser::Params, on: bool) {
        if private != Some(b'?') {
            return;
        }
        for i in 0..params.len().max(1) {
            match params.get(i) {
                1 => self.modes.application_cursor = on,
                25 => self.grid_mut().cursor.visible = on,
                1004 => self.modes.focus_events = on,
                2004 => self.modes.bracketed_paste = on,
                1000 | 1002 | 1003 => {
                    self.modes.mouse_tracking = if on { params.get(i) } else { 0 }
                }
                1006 => self.modes.mouse_sgr = on,
                2026 => self.modes.synchronized_output = on,
                6 => {
                    let g = self.grid_mut();
                    g.origin_mode = on;
                    g.cursor.row = if on { g.scroll_top() } else { 0 };
                    g.cursor.col = 0;
                    g.wrap_pending = false;
                }
                7 => {
                    let g = self.grid_mut();
                    g.autowrap = on;
                    if !on {
                        g.wrap_pending = false;
                    }
                }
                1048 => {
                    if on {
                        self.save_cursor()
                    } else {
                        self.restore_cursor()
                    }
                }
                1049 | 1047 => self.switch_screen(on, params.get(i) == 1049),
                _ => {}
            }
        }
    }

    fn switch_screen(&mut self, on: bool, clear: bool) {
        if on {
            if self.stack.active == Screen::Alternate {
                return;
            }
            if clear {
                self.save_cursor();
            }
            self.stack.active = Screen::Alternate;
            let (c, r) = (self.grid().cols(), self.grid().rows());
            if clear {
                *self.stack.alternate = Grid::new(c, r);
            }
        } else {
            if self.stack.active == Screen::Primary {
                return;
            }
            self.stack.active = Screen::Primary;
            if clear {
                self.restore_cursor();
            }
        }
    }
    fn save_cursor(&mut self) {
        self.grid_mut().save_cursor();
        self.saved_rendition = Some((self.fg, self.bg, self.attrs));
    }
    fn restore_cursor(&mut self) {
        self.grid_mut().restore_cursor();
        if let Some((fg, bg, attrs)) = self.saved_rendition {
            self.fg = fg;
            self.bg = bg;
            self.attrs = attrs;
            let g = self.grid_mut();
            g.default_fg = fg;
            g.default_bg = bg;
        }
    }

    fn sgr(&mut self, params: &parser::Params) {
        if params.is_empty() {
            self.fg = Color::Default;
            self.bg = Color::Default;
            self.attrs = Attrs::new(0);
            let g = self.grid_mut();
            g.default_fg = Color::Default;
            g.default_bg = Color::Default;
            return;
        }
        let mut i = 0usize;
        while i < params.len() {
            let v = params.get(i);
            match v {
                0 => {
                    self.fg = Color::Default;
                    self.bg = Color::Default;
                    self.attrs = Attrs::new(0);
                }
                1 => self.attrs.set(Attrs::BOLD, true),
                2 => self.attrs.set(Attrs::DIM, true),
                3 => self.attrs.set(Attrs::ITALIC, true),
                4 => self.attrs.set_underline_style(1),
                5 | 6 => self.attrs.set(Attrs::BLINK, true),
                7 => self.attrs.set(Attrs::INVERSE, true),
                8 => self.attrs.set(Attrs::INVISIBLE, true),
                9 => self.attrs.set(Attrs::STRIKETHROUGH, true),
                21 | 22 => {
                    self.attrs.set(Attrs::BOLD, false);
                    self.attrs.set(Attrs::DIM, false);
                }
                23 => self.attrs.set(Attrs::ITALIC, false),
                24 => self.attrs.set_underline_style(0),
                25 => self.attrs.set(Attrs::BLINK, false),
                27 => self.attrs.set(Attrs::INVERSE, false),
                28 => self.attrs.set(Attrs::INVISIBLE, false),
                29 => self.attrs.set(Attrs::STRIKETHROUGH, false),
                30..=37 => self.fg = Color::Indexed((v - 30) as u8),
                39 => self.fg = Color::Default,
                40..=47 => self.bg = Color::Indexed((v - 40) as u8),
                49 => self.bg = Color::Default,
                53 => self.attrs.set(Attrs::OVERLINE, true),
                55 => self.attrs.set(Attrs::OVERLINE, false),
                90..=97 => self.fg = Color::Indexed((v - 90 + 8) as u8),
                100..=107 => self.bg = Color::Indexed((v - 100 + 8) as u8),
                38 | 48 => {
                    let target_fg = v == 38;
                    match params.get(i + 1) {
                        5 => {
                            let idx = params.get(i + 2) as u8;
                            let c = Color::Indexed(idx);
                            if target_fg {
                                self.fg = c
                            } else {
                                self.bg = c
                            }
                            i += 2;
                        }
                        2 => {
                            let c = Color::Rgb(
                                params.get(i + 2) as u8,
                                params.get(i + 3) as u8,
                                params.get(i + 4) as u8,
                            );
                            if target_fg {
                                self.fg = c
                            } else {
                                self.bg = c
                            }
                            i += 4;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let fg = self.fg;
        let bg = self.bg;
        let g = self.grid_mut();
        g.default_fg = fg;
        g.default_bg = bg;
    }
}

fn dec_line_drawing(cp: u32) -> u32 {
    match cp {
        0x6a => 0x2518,
        0x6b => 0x2510,
        0x6c => 0x250c,
        0x6d => 0x2514,
        0x6e => 0x253c,
        0x71 => 0x2500,
        0x74 => 0x251c,
        0x75 => 0x2524,
        0x76 => 0x2534,
        0x77 => 0x252c,
        0x78 => 0x2502,
        _ => cp,
    }
}

fn insert_lines(g: &mut Grid, at: u16, bottom: u16, blank: &Cell) {
    if at > bottom {
        return;
    }
    for r in (at + 1..=bottom).rev() {
        g.copy_row(r - 1, r);
    }
    g.fill_row(at, blank.clone());
}

fn delete_lines(g: &mut Grid, at: u16, bottom: u16, blank: &Cell) {
    if at > bottom {
        return;
    }
    for r in at..bottom {
        g.copy_row(r + 1, r);
    }
    g.fill_row(bottom, blank.clone());
}

fn delete_chars(g: &mut Grid, cur: grid::Cursor, n: u16, blank: &Cell) {
    let width = g.cols() as usize;
    let start = cur.col as usize;
    let n = (n as usize).min(width.saturating_sub(start));
    for i in start..width {
        let src = i + n;
        if src < width {
            g.copy_cell((cur.row, src as u16), (cur.row, i as u16));
        } else if let Some(c) = g.cell_mut(cur.row, i as u16) {
            *c = blank.clone();
        }
    }
    g.repair_wide(cur.row);
}

fn insert_chars(g: &mut Grid, cur: grid::Cursor, n: u16, blank: &Cell) {
    let width = g.cols() as usize;
    let start = cur.col as usize;
    let n = (n as usize).min(width.saturating_sub(start));
    for i in (start..width).rev() {
        if i >= start + n {
            g.copy_cell((cur.row, (i - n) as u16), (cur.row, i as u16));
        } else if let Some(c) = g.cell_mut(cur.row, i as u16) {
            *c = blank.clone();
        }
    }
    g.repair_wide(cur.row);
}

fn erase_chars(g: &mut Grid, cur: grid::Cursor, n: u16, blank: &Cell) {
    let end = cur.col.saturating_add(n).min(g.cols());
    g.clear_row_range(cur.row, cur.col, end.saturating_sub(1), blank.clone());
}
