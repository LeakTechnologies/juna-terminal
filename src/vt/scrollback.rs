use std::collections::VecDeque;

use crate::vt::cell::Cell;
fn row_bytes(row: &[Cell]) -> usize {
    std::mem::size_of_val(row)
        + row
            .iter()
            .map(|c| match &c.content {
                crate::vt::CellContent::Cluster(s) => s.len(),
                _ => 0,
            })
            .sum::<usize>()
}

pub const DEFAULT_BUDGET: usize = 64 * 1024 * 1024;
const MIN_LINES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrappedLine {
    pub wrapped: bool,
}

/// Scrollback history, bounded by **bytes** rather than by lines.
///
/// A line-count ceiling is a memory ceiling only for a fixed width. With grids up to 512
/// columns wide, a fixed line count would let memory grow with window width rather than with
/// content. The effective line cap is derived from the budget and the current column count.
#[derive(Debug)]
pub struct Scrollback {
    rows: VecDeque<Vec<Cell>>,
    wrapped: VecDeque<WrappedLine>,
    budget: usize,
    cols: u16,
    bytes: usize,
    dropped_lines: u64,
}

impl Scrollback {
    pub fn new(budget: usize, cols: u16) -> Self {
        Self {
            rows: VecDeque::new(),
            wrapped: VecDeque::new(),
            budget,
            cols: cols.max(1),
            bytes: 0,
            dropped_lines: 0,
        }
    }

    pub fn max_lines(&self) -> usize {
        let cell = std::mem::size_of::<Cell>().max(1);
        (self.budget / (self.cols as usize * cell)).max(MIN_LINES)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn dropped_lines(&self) -> u64 {
        self.dropped_lines
    }

    pub fn set_cols(&mut self, cols: u16) {
        let cols = cols.max(1);
        if cols == self.cols {
            return;
        }
        let old_rows = std::mem::take(&mut self.rows);
        let old_wrapped = std::mem::take(&mut self.wrapped);
        self.cols = cols;
        self.bytes = 0;
        let mut line = Vec::new();
        for (mut row, wrap) in old_rows.into_iter().zip(old_wrapped) {
            if !wrap.wrapped {
                while row.last().is_some_and(|c| c.content.is_empty()) {
                    row.pop();
                }
            }
            line.extend(row);
            if !wrap.wrapped {
                self.reflow_line(std::mem::take(&mut line));
            }
        }
        if !line.is_empty() {
            self.reflow_line(line);
        }
        self.enforce();
    }

    fn reflow_line(&mut self, line: Vec<Cell>) {
        use crate::vt::CellContent;
        let mut row = Vec::new();
        for mut cell in line {
            if cell.content == CellContent::WideCharSpacer {
                continue;
            }
            let mut width = cell.content.width().max(1) as usize;
            if width > self.cols as usize {
                cell.content = CellContent::Char(0xfffd);
                width = 1;
            }
            if row.len() + width > self.cols as usize {
                row.resize(self.cols as usize, Cell::default());
                self.push(std::mem::take(&mut row), true);
            }
            let mut spacer = cell.clone();
            spacer.content = CellContent::WideCharSpacer;
            row.push(cell);
            if width == 2 {
                row.push(spacer);
            }
        }
        row.resize(self.cols as usize, Cell::default());
        self.push(row, false);
    }

    pub fn set_budget(&mut self, budget: usize) {
        self.budget = budget;
        self.enforce();
    }

    pub fn push(&mut self, row: Vec<Cell>, wrapped: bool) {
        if self.budget == 0 {
            self.dropped_lines += 1;
            return;
        }
        self.bytes += row_bytes(&row);
        self.rows.push_back(row);
        self.wrapped.push_back(WrappedLine { wrapped });
        self.enforce();
    }

    fn enforce(&mut self) {
        let max = self.max_lines();
        while self.rows.len() > max {
            self.bytes = self
                .bytes
                .saturating_sub(self.rows.pop_front().map(|r| row_bytes(&r)).unwrap_or(0));
            self.wrapped.pop_front();
            self.dropped_lines += 1;
        }
        while self.bytes > self.budget && !self.rows.is_empty() {
            self.bytes = self
                .bytes
                .saturating_sub(self.rows.pop_front().map(|r| row_bytes(&r)).unwrap_or(0));
            self.wrapped.pop_front();
            self.dropped_lines += 1;
        }
    }

    pub fn get(&self, index: usize) -> Option<&[Cell]> {
        self.rows.get(index).map(|v| v.as_slice())
    }

    pub fn is_wrapped(&self, index: usize) -> bool {
        self.wrapped.get(index).map(|w| w.wrapped).unwrap_or(false)
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.wrapped.clear();
        self.bytes = 0;
        self.dropped_lines = 0;
    }

    pub fn iter_rev(&self) -> impl Iterator<Item = (&[Cell], bool)> {
        (0..self.rows.len()).rev().map(move |i| {
            (
                self.rows[i].as_slice(),
                self.wrapped.get(i).map(|w| w.wrapped).unwrap_or(false),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(n: usize, marker: u32) -> Vec<Cell> {
        (0..n)
            .map(|i| Cell {
                content: crate::vt::cell::CellContent::Char(marker + i as u32),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn budget_derives_line_count_from_width() {
        let narrow = Scrollback::new(1024 * 1024, 80);
        let wide = Scrollback::new(1024 * 1024, 512);
        assert!(
            wide.max_lines() < narrow.max_lines(),
            "wider grids must admit fewer lines under a fixed byte budget"
        );
    }

    #[test]
    fn zero_budget_discards_immediately() {
        let mut s = Scrollback::new(0, 80);
        s.push(row(80, 1), false);
        assert!(s.is_empty());
        assert_eq!(s.dropped_lines(), 1);
    }

    #[test]
    fn oldest_lines_are_dropped_first() {
        let mut s = Scrollback::new(1024 * 1024, 80);
        let cap = s.max_lines();
        for i in 0..(cap as u32 + 50) {
            s.push(row(80, i), false);
        }
        assert!(s.len() <= cap);
        assert_eq!(
            s.get(0).unwrap()[0].content,
            crate::vt::cell::CellContent::Char(50)
        );
        assert!(s.dropped_lines() >= 50);
    }

    #[test]
    fn byte_budget_is_enforced_independently_of_line_count() {
        let mut s = Scrollback::new(1024 * 1024, 80);
        for _ in 0..10_000 {
            s.push(row(80, 1), false);
        }
        assert!(s.bytes() <= 1024 * 1024 + 80 * std::mem::size_of::<Cell>());
    }

    #[test]
    fn widening_the_grid_shrinks_the_admitted_line_count() {
        let mut s = Scrollback::new(1024 * 1024, 80);
        for _ in 0..2000 {
            s.push(row(80, 1), false);
        }
        let before = s.len();
        s.set_cols(512);
        assert!(
            s.len() < before,
            "history must be trimmed, not kept, after widening"
        );
    }

    #[test]
    fn wrapped_flags_survive_push_and_drop() {
        let mut s = Scrollback::new(1024 * 1024, 80);
        s.push(row(80, 1), true);
        s.push(row(80, 2), false);
        assert!(s.is_wrapped(0));
        assert!(!s.is_wrapped(1));
    }

    #[test]
    fn reverse_iteration_walks_newest_first() {
        let mut s = Scrollback::new(1024 * 1024, 80);
        s.push(row(80, 1), false);
        s.push(row(80, 2), false);
        let seen: Vec<u32> = s
            .iter_rev()
            .map(|(r, _)| r[0].content.first_scalar().unwrap_or(0))
            .collect();
        assert_eq!(seen, vec![2, 1]);
    }

    #[test]
    fn clear_resets_accounting() {
        let mut s = Scrollback::new(1024 * 1024, 80);
        s.push(row(80, 1), false);
        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.bytes(), 0);
        assert_eq!(s.dropped_lines(), 0);
    }
}
