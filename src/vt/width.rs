//! Pinned Unicode standards data behind a Juna-owned pure API.
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
pub type Columns = u8;
pub const WIDTH_UNICODE_VERSION: (u8, u8, u8) = unicode_width::UNICODE_VERSION;
pub const GRAPHEME_UNICODE_VERSION: (u64, u64, u64) = unicode_segmentation::UNICODE_VERSION;
pub fn char_width(c: char) -> Columns {
    c.width().unwrap_or(0).min(2) as u8
}
pub fn char_width_u32(cp: u32) -> Columns {
    char::from_u32(cp).map(char_width).unwrap_or(1)
}
pub fn cluster_width(s: &str) -> Columns {
    s.width().min(2) as u8
}
pub fn joins(base: &str, c: char) -> bool {
    let mut text = base.to_owned();
    text.push(c);
    text.graphemes(true).count() == 1
}
pub fn unicode_width_is_exhaustive() -> bool {
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn widths_cover_clusters() {
        assert_eq!(char_width('漢'), 2);
        assert_eq!(cluster_width("e\u{301}"), 1);
        assert_eq!(cluster_width("👨\u{200d}👩"), 2);
        assert_eq!(char_width('\u{200d}'), 0);
        assert!(joins("👨\u{200d}", '👩'));
        assert!(!joins("e", 'x'));
    }
}
