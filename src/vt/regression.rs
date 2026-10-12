use super::*;
fn line(s: &TerminalState, r: u16) -> String {
    s.grid()
        .row_slice(r)
        .unwrap()
        .iter()
        .map(|c| match &c.content {
            CellContent::Char(cp) | CellContent::WideChar(cp) => {
                char::from_u32(*cp).unwrap_or('�').to_string()
            }
            CellContent::Cluster(v) => v.to_string(),
            CellContent::Empty => " ".into(),
            CellContent::WideCharSpacer => String::new(),
        })
        .collect::<String>()
        .trim_end()
        .to_owned()
}
#[test]
fn scrolling_moves_every_row_once() {
    let mut s = TerminalState::new(8, 3);
    s.advance(b"A\r\nB\r\nC\x1b[T");
    assert_eq!(
        (line(&s, 0), line(&s, 1), line(&s, 2)),
        ("".into(), "A".into(), "B".into())
    );
    let mut s = TerminalState::new(8, 3);
    s.advance(b"A\r\nB\r\nC\x1b[2S");
    assert_eq!(
        s.scrollback_line(0).unwrap()[0].content,
        CellContent::Char(65)
    );
    assert_eq!(
        s.scrollback_line(1).unwrap()[0].content,
        CellContent::Char(66)
    );
    let mut s = TerminalState::new(8, 1);
    s.advance(b"A\r\nB");
    assert_eq!(s.scrollback_len(), 1);
}
#[test]
fn line_insertion_preserves_bottom_and_never_underflows() {
    let mut s = TerminalState::new(8, 3);
    s.advance(b"A\r\nB\r\nC\x1b[H\x1b[L");
    assert_eq!(
        (line(&s, 0), line(&s, 1), line(&s, 2)),
        ("".into(), "A".into(), "B".into())
    );
}
#[test]
fn huge_cursor_parameters_clamp_and_cnl_never_scrolls() {
    let mut s = TerminalState::new(8, 3);
    s.advance(b"A\x1b[2;2H\x1b[65535B\x1b[65535C\x1b[65535X");
    assert_eq!((s.grid().cursor.row, s.grid().cursor.col), (2, 7));
    s.advance(b"\x1b[65535E");
    assert_eq!(s.scrollback_len(), 0);
    assert_eq!(line(&s, 0), "A");
}
#[test]
fn carriage_return_cancels_wrap_and_leading_defaults_stay_in_place() {
    let mut s = TerminalState::new(8, 3);
    s.advance(b"12345678\rX");
    assert_eq!(line(&s, 0), "X2345678");
    assert_eq!(line(&s, 1), "");
    s.advance(b"\x1b[;3HY");
    assert_eq!(s.grid().cell(0, 2).unwrap().content, CellContent::Char(89));
}
#[test]
fn erasing_display_preserves_cursor_and_ed3_only_clears_history() {
    let mut s = TerminalState::new(8, 2);
    s.advance(b"A\r\nB\r\nC\x1b[1;3H");
    s.advance(b"\x1b[3J");
    assert_eq!(line(&s, 1), "C");
    assert_eq!(s.scrollback_len(), 0);
    s.advance(b"\x1b[2J");
    assert_eq!((s.grid().cursor.row, s.grid().cursor.col), (0, 2));
}
#[test]
fn resize_updates_full_margins_and_keeps_live_bottom_rows() {
    let mut s = TerminalState::new(8, 3);
    s.resize(12, 5);
    assert_eq!(s.grid().scroll_region(), (0, 4, 0, 11));
    let mut s = TerminalState::new(8, 3);
    s.advance(b"A\r\nB\r\nC");
    s.resize(8, 2);
    assert_eq!((line(&s, 0), line(&s, 1)), ("B".into(), "C".into()));
    assert_eq!(s.scrollback_len(), 1);
}
#[test]
fn wide_cells_preserve_rendition_and_repair_partial_overwrites() {
    let mut s = TerminalState::new(8, 3);
    s.advance("\x1b[31;44m漢".as_bytes());
    let c = s.grid().cell(0, 0).unwrap();
    assert_eq!((c.fg, c.bg), (Color::Indexed(1), Color::Indexed(4)));
    s.advance(b"\rX");
    assert_eq!(s.grid().cell(0, 1).unwrap().content, CellContent::Empty);
    s.advance("\x1b[1;7H漢".as_bytes());
    s.resize(7, 3);
    assert_eq!(s.grid().cell(0, 6).unwrap().content, CellContent::Empty);
}
#[test]
fn repeated_restore_and_alt_screen_restore_rendition() {
    let mut s = TerminalState::new(8, 3);
    s.advance(b"\x1b[31m\x1b[2;2H\x1b7\x1b[H\x1b8\x1b[H\x1b8");
    assert_eq!((s.grid().cursor.row, s.grid().cursor.col), (1, 1));
    s.advance(b"\x1b[?1049h\x1b[32mX\x1b[?1049lY");
    assert_eq!(s.grid().cell(1, 1).unwrap().fg, Color::Indexed(1));
}
#[test]
fn clusters_survive_every_byte_boundary_and_reflow_history() {
    let bytes = "e\u{301} 👨\u{200d}👩 \x1b[31m漢\x1b[0m\r\nabcdefghi\r\nlast".as_bytes();
    let mut reference = TerminalState::new(8, 2);
    reference.advance(bytes);
    let expected = reference.snapshot(0);
    for split in 0..=bytes.len() {
        let mut s = TerminalState::new(8, 2);
        s.advance(&bytes[..split]);
        s.advance(&bytes[split..]);
        assert_eq!(s.snapshot(0).cells, expected.cells, "split {split}");
    }
    reference.resize(4, 2);
    assert!(reference.history_wrapped(0));
}
#[test]
fn queries_modes_and_unsupported_sequences() {
    let mut s = TerminalState::new(8, 3);
    s.advance(b"\x1b[2;3H\x1b[6n\x1b[5n\x1b[?2004h\x1b[?2004$p");
    assert_eq!(s.take_responses(), b"\x1b[2;3R\x1b[0n\x1b[?2004;1$y");
    s.advance(b"\x1b[?25l\x1b[>5A");
    assert!(!s.snapshot(0).cursor.visible);
    assert_eq!(s.grid().cursor.row, 1);
    s.advance(b"x\x1b[3b");
    assert_eq!(&line(&s, 1)[2..], "xxxx");
}
#[test]
fn structured_adversarial_sequences_are_total() {
    for final_byte in b"ABCDEFLMPSTXHf" {
        let mut s = TerminalState::new(4, 2);
        let sequence = format!("\x1b[2;2H\x1b[65535{}", *final_byte as char);
        s.advance(sequence.as_bytes());
        assert!(s.grid().cursor.row < 2 && s.grid().cursor.col < 4);
    }
    let mut p = Parser::new();
    let mut bytes = b"\x1b[".to_vec();
    bytes.extend(std::iter::repeat_n(b' ', 10000));
    bytes.push(b'm');
    p.advance(&bytes);
    assert_eq!(p.state(), State::Ground);
}

#[test]
fn utf8_string_terminator_continuations_and_history_snapshots() {
    let mut s = TerminalState::new(8, 2);
    s.advance("\x1b]2;Ü title\x07A\r\nB\r\nC".as_bytes());
    assert_eq!(s.title(), "Ü title");
    let snapshot = s.snapshot(1);
    assert_eq!(snapshot.row(0)[0].content, CellContent::Char(65));
    assert!(!snapshot.cursor.visible);
}
