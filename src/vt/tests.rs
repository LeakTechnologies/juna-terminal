use super::*;

fn state(cols: u16, rows: u16) -> TerminalState {
    TerminalState::new(cols, rows)
}

fn screen_text(s: &TerminalState, row: u16) -> String {
    let g = s.grid();
    let mut out = String::new();
    for col in 0..g.cols() {
        match g.cell(row, col).map(|c| c.content.clone()) {
            Some(CellContent::Char(cp)) => out.push(char::from_u32(cp).unwrap_or('?')),
            Some(CellContent::Cluster(a)) => out.push_str(&a),
            Some(CellContent::WideChar(cp)) => {
                out.push(char::from_u32(cp).unwrap_or('?'));
                out.push(' ');
            }
            _ => out.push(' '),
        }
    }
    out.trim_end().to_string()
}

#[test]
fn advance_returns_actions_applied_to_the_grid() {
    let mut s = state(20, 5);
    s.advance(b"hello");
    assert_eq!(screen_text(&s, 0), "hello");
    assert_eq!(s.grid().cursor.col, 5);
}

#[test]
fn bel_increments_the_bell_count_without_touching_the_grid() {
    let mut s = state(20, 5);
    assert_eq!(s.bell_count(), 0);
    s.advance(b"a\x07b");
    assert_eq!(s.bell_count(), 1);
    assert_eq!(screen_text(&s, 0), "ab");
    assert_eq!(s.grid().cursor.col, 2);
    s.advance(b"\x07\x07");
    assert_eq!(s.bell_count(), 3);
}

#[test]
fn protected_metadata_follows_cells_without_increasing_cell_storage() {
    assert!(std::mem::size_of::<Cell>() <= 48);
    let token = "0123456789abcdef0123456789abcdef";
    let mut s = state(4, 2);
    let bytes = format!("\x1b]777;JunaPrivate;{token}\x1b\\░░░░░\x1b]777;JunaPrivate;0\x1b\\");
    for byte in bytes.as_bytes() {
        s.advance(&[*byte]);
    }
    let tag = s.grid().cell(0, 0).unwrap().private_tag;
    assert_ne!(tag, 0);
    assert_eq!(s.private_token(tag), Some(token));
    assert_eq!(s.grid().cell(1, 0).unwrap().private_tag, tag);
    s.advance(b"\r\n\r\n");
    assert_eq!(s.scrollback_line(0).unwrap()[0].private_tag, tag);
    s.resize(8, 2);
    assert_eq!(s.scrollback_line(0).unwrap()[0].private_tag, tag);
    s.advance(b"\x1b[H\x1b[2J");
    assert!(s.snapshot(0).cells.iter().all(|cell| cell.private_tag == 0));
    let responses = s.take_responses();
    s.advance(b"\x1b]777;JunaPrivate;not-a-valid-token\x1b\\X");
    assert_eq!(s.grid().cell(0, 0).unwrap().private_tag, 0);
    assert_eq!(s.take_responses(), responses);
}

#[test]
fn crlf_moves_to_the_next_row_column_zero() {
    let mut s = state(20, 5);
    s.advance(b"ab\r\ncd");
    assert_eq!(screen_text(&s, 0), "ab");
    assert_eq!(screen_text(&s, 1), "cd");
    assert_eq!(s.grid().cursor.row, 1);
    assert_eq!(s.grid().cursor.col, 2);
}

#[test]
fn cup_positions_the_cursor_one_based() {
    let mut s = state(20, 5);
    s.advance(b"\x1b[3;2HX");
    assert_eq!(
        s.grid().cell(2, 1).unwrap().content,
        CellContent::Char(u32::from('X'))
    );
    assert_eq!(
        s.grid().cursor.col,
        2,
        "cursor advances past the printed glyph"
    );
}

#[test]
fn cup_zero_is_treated_as_one() {
    let mut s = state(20, 5);
    s.advance(b"\x1b[0;0HX");
    assert_eq!(
        s.grid().cell(0, 0).unwrap().content,
        CellContent::Char(u32::from('X')),
        "a zero parameter must be read as one"
    );
}

#[test]
fn cursor_movement_clamps_at_the_regions() {
    let mut s = state(10, 5);
    s.advance(b"\x1b[10;10H");
    assert_eq!(s.grid().cursor.row, 4);
    assert_eq!(s.grid().cursor.col, 9);
    s.advance(b"\x1b[100A");
    assert_eq!(s.grid().cursor.row, 0);
    s.advance(b"\x1b[100D");
    assert_eq!(s.grid().cursor.col, 0);
}

#[test]
fn autowrap_defers_until_the_next_print() {
    let mut s = state(5, 3);
    s.advance(b"abcde");
    assert_eq!(screen_text(&s, 0), "abcde");
    assert_eq!(s.grid().cursor.row, 0, "must not wrap yet");
    assert!(s.grid().wrap_pending);
    s.advance(b"f");
    assert_eq!(s.grid().cursor.row, 1);
    assert_eq!(screen_text(&s, 1), "f");
}

#[test]
fn autowrap_disabled_overwrites_the_last_column() {
    let mut s = state(5, 3);
    s.advance(b"\x1b[?7l");
    s.advance(b"abcdef");
    assert_eq!(screen_text(&s, 0), "abcdf");
    assert_eq!(s.grid().cursor.row, 0);
}

#[test]
fn line_feed_at_the_bottom_scrolls() {
    let mut s = state(10, 3);
    s.advance(b"one\r\ntwo\r\nthree\r\nfour");
    assert_eq!(screen_text(&s, 2), "four");
    assert_eq!(screen_text(&s, 0), "two");
}

#[test]
fn decstbm_limits_scrolling_to_the_region() {
    let mut s = state(10, 5);
    s.advance(b"\x1b[2;4r");
    s.advance(b"\x1b[5;1H");
    s.advance(b"\x1b[100S");
    assert_eq!(screen_text(&s, 0), "", "outside region must be untouched");
    assert_eq!(screen_text(&s, 4), "", "below region must be untouched");
}

#[test]
fn el_erases_to_the_end_of_the_line() {
    let mut s = state(10, 2);
    s.advance(b"abcdef\r\nx");
    s.advance(b"\x1b[1;1H\x1b[K");
    assert_eq!(screen_text(&s, 0), "");
}

#[test]
fn ed_two_clears_the_screen_and_preserves_the_cursor() {
    let mut s = state(10, 3);
    s.advance(b"a\r\nb\r\nc");
    s.advance(b"\x1b[2J");
    assert_eq!((s.grid().cursor.row, s.grid().cursor.col), (2, 1));
    assert!(screen_text(&s, 0).is_empty());
    assert!(screen_text(&s, 2).is_empty());
}

#[test]
fn ich_and_dch_shift_the_row() {
    let mut s = state(10, 2);
    s.advance(b"abcdef");
    s.advance(b"\x1b[1;3H\x1b[2@");
    assert_eq!(screen_text(&s, 0), "ab  cdef");
    s.advance(b"\x1b[1;3H\x1b[2P");
    assert_eq!(screen_text(&s, 0), "abcdef");
}

#[test]
fn ech_erases_in_place_without_shifting() {
    let mut s = state(10, 2);
    s.advance(b"abcdef");
    s.advance(b"\x1b[1;3H\x1b[2X");
    assert_eq!(screen_text(&s, 0), "ab  ef");
}

#[test]
fn il_and_dl_shift_the_region_down_and_up() {
    let mut s = state(10, 4);
    s.advance(b"a\r\nb\r\nc\r\nd");
    s.advance(b"\x1b[2;1H\x1b[L");
    assert_eq!(screen_text(&s, 1), "");
    assert_eq!(screen_text(&s, 2), "b");
    s.advance(b"\x1b[2;1H\x1b[M");
    assert_eq!(screen_text(&s, 1), "b");
}

#[test]
fn sgr_sets_colors_and_attributes() {
    let mut s = state(10, 2);
    s.advance(b"\x1b[1;31mA");
    let cell = s.grid().cell(0, 0).unwrap();
    assert!(cell.attrs.is(Attrs::BOLD));
    assert_eq!(cell.fg, Color::Indexed(1));
    s.advance(b"\x1b[0mB");
    let cell = s.grid().cell(0, 1).unwrap();
    assert!(!cell.attrs.is(Attrs::BOLD));
    assert_eq!(cell.fg, Color::Default);
}

#[test]
fn sgr_truecolor_and_256_color() {
    let mut s = state(10, 2);
    s.advance(b"\x1b[38;2;10;20;30mA");
    assert_eq!(s.grid().cell(0, 0).unwrap().fg, Color::Rgb(10, 20, 30));
    s.advance(b"\x1b[48;5;200mB");
    assert_eq!(s.grid().cell(0, 1).unwrap().bg, Color::Indexed(200));
}

/// Text after an extended-colour SGR must still print. The colour assertions above
/// pass even if the engine swallows the following characters as parameters, so the
/// only way to catch that is to assert the text actually reached the grid.
#[test]
fn text_after_extended_colour_sgr_is_printed_not_consumed() {
    for prefix in [
        &b"\x1b[38;2;255;100;50m"[..],
        &b"\x1b[48;2;255;100;50m"[..],
        &b"\x1b[38;5;208m"[..],
        &b"\x1b[38;2;1;2;3;4;5;6m"[..],
        &b"\x1b[1;4;7;38;2;9;9;9m"[..],
    ] {
        let mut s = state(20, 2);
        let mut stream = prefix.to_vec();
        stream.extend_from_slice(b"XY");
        s.advance(&stream);
        let row: String = (0..2)
            .map(|c| match s.grid().cell(0, c).unwrap().content {
                CellContent::Char(cp) => char::from_u32(cp).unwrap_or('?').to_string(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(
            row,
            "XY",
            "text after {:?} was consumed instead of printed",
            String::from_utf8_lossy(prefix)
        );
    }
}

#[test]
fn bce_makes_erased_cells_take_the_current_background() {
    let mut s = state(10, 2);
    s.advance(b"abcdef\r\n\x1b[44m\x1b[1;1H\x1b[K");
    assert_eq!(s.grid().cell(0, 5).unwrap().bg, Color::Indexed(4));
}

#[test]
fn tabs_advance_to_the_next_stop() {
    let mut s = state(40, 2);
    s.advance(b"a\tb");
    assert_eq!(
        s.grid().cell(0, 8).unwrap().content,
        CellContent::Char(u32::from('b'))
    );
    assert_eq!(s.grid().cursor.col, 9);
}

#[test]
fn wide_characters_occupy_two_columns() {
    let mut s = state(10, 2);
    s.advance("漢".as_bytes());
    assert!(matches!(
        s.grid().cell(0, 0).unwrap().content,
        CellContent::WideChar(_)
    ));
    assert!(matches!(
        s.grid().cell(0, 1).unwrap().content,
        CellContent::WideCharSpacer
    ));
    assert_eq!(s.grid().cursor.col, 2);
}

#[test]
fn combining_marks_attach_to_the_previous_cell() {
    let mut s = state(10, 2);
    s.advance("e\u{301}".as_bytes());
    match &s.grid().cell(0, 0).unwrap().content {
        CellContent::Cluster(a) => assert_eq!(a.as_ref(), "e\u{301}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(s.grid().cursor.col, 1, "a mark must not add a column");
}

#[test]
fn clusters_are_interned_not_reallocated() {
    let mut s = state(20, 2);
    for _ in 0..10 {
        s.advance("e\u{301}x".as_bytes());
    }
    assert!(s.interner_len() <= 1, "repeated clusters must intern once");
}

#[test]
fn save_and_restore_cursor_round_trip() {
    let mut s = state(20, 5);
    s.advance(b"\x1b[3;4H\x1b7");
    s.advance(b"\x1b[1;1H");
    s.advance(b"\x1b8");
    assert_eq!((s.grid().cursor.row, s.grid().cursor.col), (2, 3));
}

#[test]
fn alternate_screen_is_independent_and_restores_primary() {
    let mut s = state(10, 3);
    s.advance(b"primary");
    s.advance(b"\x1b[?1049h");
    assert!(s.alternate_active());
    s.advance(b"alt");
    assert_eq!(screen_text(&s, 0), "alt");
    s.advance(b"\x1b[?1049l");
    assert!(!s.alternate_active());
    assert_eq!(screen_text(&s, 0), "primary");
}

#[test]
fn alternate_screen_does_not_collect_scrollback() {
    let mut s = state(10, 3);
    for _ in 0..10 {
        s.advance(b"\x1b[?1049hline\r\n\x1b[?1049l");
    }
    assert_eq!(
        s.scrollback_len(),
        0,
        "alternate screen must not grow history"
    );
}

#[test]
fn osc_title_is_captured() {
    let mut s = state(10, 2);
    s.advance(b"\x1b]0;Juna\x07");
    assert_eq!(s.title(), "Juna");
    s.advance(b"\x1b]2;other\x1b\\");
    assert_eq!(s.title(), "other");
}

#[test]
fn ris_resets_the_grid() {
    let mut s = state(10, 3);
    s.advance(b"\x1b[31mjunk\x1bc");
    assert!(screen_text(&s, 0).is_empty());
    assert_eq!((s.grid().cursor.row, s.grid().cursor.col), (0, 0));
}

#[test]
fn damage_is_emitted_only_when_something_changed() {
    let mut s = state(10, 2);
    assert!(s.advance(b"").is_empty());
    assert!(!s.advance(b"a").is_empty());
}

#[test]
fn resize_reports_full_damage() {
    let mut s = state(10, 2);
    let d = s.resize(20, 4);
    assert!(d.resized);
    assert_eq!(d.damage, vec![Damage::Full]);
    assert_eq!((s.grid().cols(), s.grid().rows()), (20, 4));
}

#[test]
fn malformed_input_never_panics_and_never_sticks() {
    let mut s = state(20, 5);
    let mut seed = 0xDEAD_BEEF_1234_5678u64;
    for _ in 0..50_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let b = (seed >> 24) as u8;
        let _ = s.advance(&[b]);
    }
    let _ = s.advance(&[0x9C]);
    assert_eq!(
        s.parser_state(),
        State::Ground,
        "a string terminator must always resynchronise the parser"
    );
}

#[test]
fn engine_is_send_but_not_sync() {
    fn assert_send<T: Send>() {}
    assert_send::<TerminalState>();

    trait Probe<A> {
        fn probe() {}
    }
    impl<T: ?Sized> Probe<()> for T {}
    impl<T: ?Sized + Sync> Probe<u8> for T {}
    let _ = <TerminalState as Probe<_>>::probe;
}

#[test]
fn realistic_shell_prompt_sequence() {
    let mut s = state(80, 24);
    s.advance(b"\x1b]0;Windows PowerShell\x07");
    s.advance(b"\x1b[1;32mPS C:\\Users\\dev\x1b[0m>");
    s.advance(b" ");
    assert_eq!(s.grid().cursor.row, 0);
    assert!(s.title().contains("PowerShell"));
    assert_eq!(
        screen_text(&s, 0).trim_end(),
        "PS C:\\Users\\dev>".trim_end()
    );
}
#[test]
fn osc_working_directory_is_metadata_and_invalid_updates_clear_it() {
    let mut terminal = super::TerminalState::new(10, 2);
    let before = terminal.snapshot(0).cells;
    terminal.advance(b"\x1b]7;file://localhost/C:/one%20two\x1b\\");
    assert!(terminal.working_directory().1.unwrap().contains("one two"));
    assert_eq!(terminal.snapshot(0).cells, before);
    terminal.advance(b"\x1b]7;file://remote/share\x07");
    assert_eq!(terminal.working_directory(), (2, None));
}
