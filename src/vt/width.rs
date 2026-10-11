pub type Columns = u8;

/// Width classes derived from East Asian Width `W`/`F` (wide) and combining marks (zero).
/// Sorted, non-overlapping, searched by binary search so lookup is deterministic.
///
/// **Provisional.** Decision D3 (pinned Unicode version and table source) is still open. This
/// table is hand-maintained and covers the ranges that occur in terminal output. It is isolated
/// behind this pure function precisely so it can be replaced by a version-pinned generated table
/// without touching call sites. `unicode_width_is_exhaustive()` reports the gap honestly.
const WIDE: &[(u32, u32)] = &[
    (0x1100, 0x115F),
    (0x2329, 0x232A),
    (0x2E80, 0x303E),
    (0x3041, 0x33FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xA000, 0xA4CF),
    (0xA960, 0xA97F),
    (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF),
    (0xFE10, 0xFE19),
    (0xFE30, 0xFE6F),
    (0xFF00, 0xFF60),
    (0xFFE0, 0xFFE6),
    (0x1F300, 0x1F64F),
    (0x1F680, 0x1F6FF),
    (0x1F900, 0x1F9FF),
    (0x20000, 0x2FFFD),
    (0x30000, 0x3FFFD),
];

const ZERO: &[(u32, u32)] = &[
    (0x0300, 0x036F),
    (0x0483, 0x0489),
    (0x0591, 0x05BD),
    (0x0610, 0x061A),
    (0x064B, 0x065F),
    (0x0670, 0x0670),
    (0x06D6, 0x06DC),
    (0x0E31, 0x0E31),
    (0x0E34, 0x0E3A),
    (0x0E47, 0x0E4E),
    (0x1AB0, 0x1AFF),
    (0x1DC0, 0x1DFF),
    (0x200B, 0x200F),
    (0x2028, 0x202E),
    (0x2060, 0x2064),
    (0x20D0, 0x20F0),
    (0xFE00, 0xFE0F),
    (0xFE20, 0xFE2F),
    (0xE0100, 0xE01EF),
];

fn in_ranges(table: &[(u32, u32)], c: u32) -> bool {
    table
        .binary_search_by(|&(lo, hi)| {
            if c < lo {
                std::cmp::Ordering::Greater
            } else if c > hi {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Display columns occupied by a scalar value.
///
/// Returns 0 for control characters and zero-width marks, 2 for East Asian Wide and Fullwidth
/// scalars, and 1 otherwise.
pub fn char_width(c: char) -> Columns {
    char_width_u32(u32::from(c))
}

pub fn char_width_u32(cp: u32) -> Columns {
    if cp < 0x20 || (0x7F..0xA0).contains(&cp) {
        return 0;
    }
    if in_ranges(ZERO, cp) {
        return 0;
    }
    if in_ranges(WIDE, cp) {
        return 2;
    }
    1
}

/// Total columns for a grapheme cluster, which is the width of its base scalar.
pub fn cluster_width(text: &str) -> Columns {
    text.chars().next().map(char_width).unwrap_or(0)
}

/// True while `WIDE`/`ZERO` remain hand-maintained rather than generated from a pinned Unicode
/// version. Callers may assert this is still `true` so the gap is noticed rather than forgotten.
pub fn unicode_width_is_exhaustive() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_one_column() {
        for c in ' '..='~' {
            assert_eq!(char_width(c), 1, "{c:?} should be 1 column");
        }
    }

    #[test]
    fn controls_are_zero_columns() {
        assert_eq!(char_width('\n'), 0);
        assert_eq!(char_width('\x1b'), 0);
        assert_eq!(char_width('\x7f'), 0);
        assert_eq!(char_width_u32(0x9F), 0, "C1 controls");
    }

    #[test]
    fn combining_marks_are_zero_columns() {
        assert_eq!(char_width('\u{301}'), 0);
        assert_eq!(char_width('\u{200b}'), 0);
        assert_eq!(char_width('\u{fe0f}'), 0, "variation selector");
        assert_eq!(char_width('\u{e0100}'), 0, "variation selector supplement");
    }

    #[test]
    fn wide_scripts_are_two_columns() {
        assert_eq!(char_width('\u{6f22}'), 2);
        assert_eq!(char_width('\u{3042}'), 2);
        assert_eq!(char_width('\u{ac00}'), 2);
        assert_eq!(char_width('\u{1f600}'), 2, "emoji");
    }

    #[test]
    fn narrow_scripts_are_one_column() {
        assert_eq!(char_width('\u{00e9}'), 1);
        assert_eq!(char_width('\u{03b1}'), 1, "greek alpha");
        assert_eq!(char_width('\u{0915}'), 1, "devanagari ka is narrow");
    }

    #[test]
    fn cluster_width_follows_the_base_scalar() {
        assert_eq!(cluster_width("e\u{301}"), 1);
        assert_eq!(cluster_width("\u{1f468}\u{200d}\u{1f469}"), 2);
        assert_eq!(cluster_width(""), 0);
    }

    #[test]
    fn range_tables_are_sorted_and_disjoint() {
        for t in [WIDE, ZERO] {
            for w in t.windows(2) {
                assert!(w[0].0 <= w[0].1, "inverted range {w:?}");
                assert!(w[0].1 < w[1].0, "overlapping or unsorted {w:?}");
            }
        }
    }

    #[test]
    fn the_gap_is_reported_honestly() {
        assert!(
            !unicode_width_is_exhaustive(),
            "D3 is open; this must not silently claim completeness"
        );
    }
}