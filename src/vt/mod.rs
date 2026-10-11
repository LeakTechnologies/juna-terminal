pub mod cell;
pub mod grid;
pub mod parser;
pub mod scrollback;
pub mod width;

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

    pub fn interner_len(&self) -> usize {
        self.interner.len()
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
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
            self.apply(a, &mut delta);
        }
        delta.wrap_pending = self.grid().wrap_pending;
        delta.alternate = self.alternate_active();
        delta
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
        let c = char::from_u32(cp).unwrap_or('\u{FFFD}');
        let w = width::char_width(c);
        if w == 0 {
            self.attach_combining(c);
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
            g.wrap_pending = false;
            g.cursor.col = left;
            g.line_feed();
        }
        if g.cursor.col + (w as u16) > right + 1 && g.autowrap && g.cursor.col > left {
            g.cursor.col = right;
            g.line_feed();
            g.cursor.col = left;
        }

        let row = g.cursor.row;
        let col = g.cursor.col;
        let ok = if w == 2 {
            g.write_wide(row, col, cp, cell)
        } else {
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
    }

    fn attach_combining(&mut self, c: char) {
        let row = self.grid().cursor.row;
        let col = if self.grid().cursor.col > 0 {
            self.grid().cursor.col - 1
        } else {
            0
        };
        let base: Option<String> = {
            let g = self.grid_mut();
            let Some(cell) = g.cell_mut(row, col) else {
                return;
            };
            match &cell.content {
                CellContent::Char(cp) => {
                    let mut s = char::from_u32(*cp).unwrap_or('\u{FFFD}').to_string();
                    s.push(c);
                    Some(s)
                }
                CellContent::Cluster(a) => {
                    let mut s = a.to_string();
                    s.push(c);
                    Some(s)
                }
                _ => None,
            }
        };
        let Some(s) = base else { return; };
        let content = match s.chars().next() {
            Some(ch) if s.chars().count() == 1 => CellContent::Char(u32::from(ch)),
            _ => CellContent::Cluster(self.interner.intern(&s)),
        };
        if let Some(cell) = self.grid_mut().cell_mut(row, col) {
            cell.content = content;
        }
    }

    fn execute(&mut self, b: u8, delta: &mut SnapshotDelta) {
        match b {
            0x0A..=0x0C => {
                let row = self.grid().cursor.row;
                self.grid_mut().line_feed();
                delta.note(row);
            }
            0x0D => {
                self.grid_mut().cursor.col = self.grid().scroll_left();
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
            _ => {}
        }
    }

    fn escape(&mut self, intermediates: &[u8], final_byte: u8, delta: &mut SnapshotDelta) {
        match (intermediates.first().copied(), final_byte) {
            (None, b'7') => self.grid_mut().save_cursor(),
            (None, b'8') => self.grid_mut().restore_cursor(),
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
                let g = self.grid_mut();
                *g = Grid::new(c, r);
                self.fg = Color::Default;
                self.bg = Color::Default;
                self.attrs = Attrs::new(0);
                delta.damage = vec![Damage::Full];
            }
            _ => {}
        }
    }

    fn osc(&mut self, payload: &str) {
        if let Some(rest) = payload.strip_prefix('0') {
            self.title = rest.trim_start_matches(';').to_string();
        } else if let Some(rest) = payload.strip_prefix('2') {
            self.title = rest.trim_start_matches(';').to_string();
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
        let p = |i: usize| params.get(i);
        let g_default = |i: usize, d: u16| params.get_or(i, d);
        let row = self.grid().cursor.row;
        match final_byte {
            b'A' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.row = g.cursor.row.saturating_sub(n).max(g.scroll_top());
            }
            b'B' | b'e' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.row = (g.cursor.row + n).min(g.scroll_bottom());
            }
            b'C' | b'a' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.col = (g.cursor.col + n).min(g.scroll_right());
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
                for _ in 0..n {
                    g.line_feed();
                }
                g.cursor.col = g.scroll_left();
                delta.note(row);
            }
            b'F' => {
                let n = g_default(0, 1);
                let g = self.grid_mut();
                g.wrap_pending = false;
                for _ in 0..n {
                    g.reverse_index();
                }
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
                g.cursor.row = (n - 1).min(g.scroll_bottom());
            }
            b'H' | b'f' => {
                let row = g_default(0, 1) - 1;
                let col = g_default(1, 1) - 1;
                let origin = self.grid().origin_mode;
                let (top, bottom, left, right) = self.grid().scroll_region();
                let g = self.grid_mut();
                g.wrap_pending = false;
                g.cursor.row = if origin {
                    (top + row).min(bottom)
                } else {
                    row.min(bottom)
                };
                g.cursor.col = if origin {
                    (left + col).min(right)
                } else {
                    col.min(right)
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
                if r <= bottom {
                    let g = self.grid_mut();
                    for _ in 0..n {
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
                if r <= bottom {
                    let g = self.grid_mut();
                    for _ in 0..n {
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
            b's' => self.grid_mut().save_cursor(),
            b'u' => self.grid_mut().restore_cursor(),
            b'g' => {
                let mode = p(0);
                let cur = self.grid().cursor.col;
                match mode {
                    0 => self.grid_mut().clear_tab(cur),
                    3 => self.grid_mut().reset_tabs(),
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
            attrs: self.attrs,
            ..Default::default()
        }
    }

    fn erase_display(&mut self, mode: u16, delta: &mut SnapshotDelta) {
        let blank = self.default_cell();
        let mut clear_history = false;
        {
            let g = self.grid_mut();
            let (top, bottom, left, right) = g.scroll_region();
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
                2 | 3 => {
                    for r in top..=bottom {
                        g.clear_row_range(r, left, right, blank.clone());
                    }
                    if mode == 3 {
                        clear_history = true;
                    }
                    g.cursor.row = top;
                    g.cursor.col = left;
                    g.wrap_pending = false;
                }
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
                6 => self.grid_mut().origin_mode = on,
                7 => self.grid_mut().autowrap = on,
                1048 => {
                    if on {
                        self.grid_mut().save_cursor()
                    } else {
                        self.grid_mut().restore_cursor()
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
        }
    }

    fn sgr(&mut self, params: &parser::Params) {
        if params.is_empty() {
            self.fg = Color::Default;
            self.bg = Color::Default;
            self.attrs = Attrs::new(0);
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
    }
}

fn insert_lines(g: &mut Grid, at: u16, bottom: u16, blank: &Cell) {
    if at > bottom {
        return;
    }
    for r in (at..bottom).rev() {
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
}

fn erase_chars(g: &mut Grid, cur: grid::Cursor, n: u16, blank: &Cell) {
    let end = (cur.col + n).min(g.cols());
    g.clear_row_range(cur.row, cur.col, end.saturating_sub(1), blank.clone());
}