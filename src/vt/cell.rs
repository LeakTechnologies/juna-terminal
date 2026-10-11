use std::collections::HashMap;
use std::sync::Arc;

use crate::vt::width::{char_width, char_width_u32};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Default)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Attrs(u32);

impl Attrs {
    pub const BOLD: u32 = 1 << 4;
    pub const DIM: u32 = 1 << 5;
    pub const ITALIC: u32 = 1 << 6;
    pub const UNDERLINE: u32 = 1 << 7;
    pub const BLINK: u32 = 1 << 8;
    pub const INVERSE: u32 = 1 << 9;
    pub const INVISIBLE: u32 = 1 << 10;
    pub const STRIKETHROUGH: u32 = 1 << 11;
    pub const OVERLINE: u32 = 1 << 12;

    pub const UNDERLINE_MASK: u32 = 0xF;

    pub const fn new(bits: u32) -> Self {
        Self(bits & 0x000F_FFFF)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn is(self, flag: u32) -> bool {
        self.0 & flag != 0
    }

    pub fn set(&mut self, flag: u32, on: bool) {
        if on {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }

    pub fn underline_style(self) -> u8 {
        (self.0 & Self::UNDERLINE_MASK) as u8
    }

    pub fn set_underline_style(&mut self, style: u8) {
        self.0 = (self.0 & !Self::UNDERLINE_MASK) | (style as u32 & Self::UNDERLINE_MASK);
    }

    pub fn reset(&mut self) {
        self.0 = 0;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum CellContent {
    #[default]
    Empty,
    Char(u32),
    Cluster(Arc<str>),
    WideChar(u32),
    WideCharSpacer,
}

impl CellContent {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn width(&self) -> u8 {
        match self {
            Self::Empty | Self::WideCharSpacer => 0,
            Self::Char(c) => char_width_u32(*c),
            Self::WideChar(c) => {
                let _ = c;
                2
            }
            Self::Cluster(s) => s.chars().next().map(char_width).unwrap_or(1).max(1),
        }
    }

    pub fn first_scalar(&self) -> Option<u32> {
        match self {
            Self::Char(c) | Self::WideChar(c) => Some(*c),
            Self::Cluster(s) => s.chars().next().map(u32::from),
            Self::Empty | Self::WideCharSpacer => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Cell {
    pub content: CellContent,
    pub fg: Color,
    pub bg: Color,
    pub attrs: Attrs,
    pub underline_color: Option<Color>,
}

impl Cell {
    pub fn blank_with(bg: Color, attrs: Attrs) -> Self {
        Self {
            content: CellContent::Empty,
            fg: Color::Default,
            bg,
            attrs,
            underline_color: None,
        }
    }
}

#[derive(Default)]
pub struct Interner {
    map: HashMap<Box<str>, Arc<str>>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, text: &str) -> Arc<str> {
        if let Some(found) = self.map.get(text) {
            return found.clone();
        }
        let shared: Arc<str> = Arc::from(text);
        self.map
            .insert(Box::from(text), shared.clone());
        shared
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_scalar_cells_allocate_nothing() {
        let c = CellContent::Char(u32::from('A'));
        assert_eq!(c.width(), 1);
        assert!(matches!(c, CellContent::Char(_)));
    }

    #[test]
    fn wide_char_reports_two_columns() {
        assert_eq!(CellContent::WideChar(u32::from('漢')).width(), 2);
        assert_eq!(CellContent::WideCharSpacer.width(), 0);
        assert_eq!(CellContent::Empty.width(), 0);
    }

    #[test]
    fn cluster_width_follows_its_base_scalar() {
        let mut i = Interner::new();
        let c = CellContent::Cluster(i.intern("e\u{301}"));
        assert_eq!(c.width(), 1, "combining mark must not add a column");
    }

    #[test]
    fn interning_deduplicates_and_borrows() {
        let mut i = Interner::new();
        let a = i.intern("e\u{301}");
        let b = i.intern("e\u{301}");
        assert_eq!(i.len(), 1, "identical clusters must intern to one entry");
        assert!(Arc::ptr_eq(&a, &b));
        i.intern("\u{1f468}\u{200d}\u{1f469}");
        assert_eq!(i.len(), 2);
    }

    #[test]
    fn attrs_bit_layout_matches_the_specification() {
        let mut a = Attrs::new(0);
        assert_eq!(a.underline_style(), 0);
        a.set_underline_style(3);
        assert_eq!(a.underline_style(), 3);
        assert_eq!(a.bits() & Attrs::UNDERLINE_MASK, 3);

        a.set(Attrs::BOLD, true);
        assert!(a.is(Attrs::BOLD));
        a.set(Attrs::BOLD, false);
        assert!(!a.is(Attrs::BOLD));

        a.set_underline_style(3);
        assert_eq!(a.underline_style(), 3, "style bits must survive flag edits");
    }

    #[test]
    fn underline_style_out_of_range_is_masked() {
        let mut a = Attrs::new(0);
        a.set_underline_style(0xFF);
        assert_eq!(a.underline_style(), 0xF);
    }

    #[test]
    fn attrs_masks_undefined_high_bits() {
        assert_eq!(Attrs::new(u32::MAX).bits() & !0x000F_FFFF, 0);
    }
}