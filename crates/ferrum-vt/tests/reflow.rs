//! Reflow (FR-TERM-010, AC-TERM-02): resizing never changes the logical text.

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{CellFlags, NullSink, Size, Terminal};
use proptest::prelude::*;

/// Large enough that nothing is evicted in these tests.
const HISTORY_LINES: usize = 20_000;
const WIDTHS: [u16; 3] = [132, 40, 80];
const ROWS: u16 = 24;

fn terminal(cols: u16, rows: u16) -> Terminal<NullSink> {
    let config = ScrollbackConfig::new(HISTORY_LINES, HISTORY_LINES).unwrap_or_default();
    Terminal::new(Size::new(cols, rows), Scrollback::new(config), NullSink)
}

/// One user-visible character: ASCII, a wide character, a base with a combining mark, an emoji.
fn glyph() -> impl Strategy<Value = String> {
    prop_oneof![
        12 => prop::char::range('!', '~').prop_map(String::from),
        3 => Just(" ".to_owned()),
        2 => prop::sample::select(vec!["漢", "字", "ｱ", "あ", "한"]).prop_map(str::to_owned),
        1 => prop::sample::select(vec!["e\u{301}", "n\u{303}", "a\u{308}"]).prop_map(str::to_owned),
        1 => prop::sample::select(vec!["😀", "👨‍👩‍👧", "🇺🇸", "\u{2764}\u{fe0f}"]).prop_map(str::to_owned),
    ]
}

fn line() -> impl Strategy<Value = String> {
    prop::collection::vec(glyph(), 0..300).prop_map(|glyphs| glyphs.concat())
}

fn lines() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(line(), 1..25)
}

fn feed(term: &mut Terminal<NullSink>, lines: &[String]) {
    for line in lines {
        term.parse(line.as_bytes());
        term.parse(b"\r\n");
    }
}

fn expected(lines: &[String]) -> String {
    let mut kept: Vec<&str> = lines.iter().map(|line| line.trim_end_matches(' ')).collect();
    while kept.last().is_some_and(|line| line.is_empty()) {
        kept.pop();
    }
    kept.join("\n")
}

proptest! {

    #[test]
    fn resizing_80_to_132_to_40_to_80_keeps_the_logical_text(lines in lines()) {
        let mut term = terminal(80, ROWS);
        feed(&mut term, &lines);
        prop_assert_eq!(term.logical_text(), expected(&lines), "the text as typed");
        let before = term.logical_text();
        for width in WIDTHS {
            prop_assert!(term.resize(Size::new(width, ROWS)).is_ok());
            prop_assert_eq!(term.logical_text(), before.clone(), "after resizing to {} columns", width);
        }
    }

    #[test]
    fn every_row_fits_the_new_width_and_the_cursor_stays_inside(lines in lines(), width in 2u16..160) {
        let mut term = terminal(80, ROWS);
        feed(&mut term, &lines);
        prop_assert!(term.resize(Size::new(width, ROWS)).is_ok());
        prop_assert!(term.cursor().x < usize::from(width));
        prop_assert!(term.cursor().y < usize::from(ROWS));
        let mut cursor = ferrum_scrollback::LineCursor::new();
        for index in 0..term.scrollback().len() {
            let line = term.scrollback().read_line(index, &mut cursor).ok().flatten();
            let columns: usize = line.map_or(0, |l| ferrum_vt::clusters(&l.text).map(ferrum_vt::cluster_width).sum());
            prop_assert!(columns <= usize::from(width), "history row {} is {} columns", index, columns);
        }
        let grid = term.grid();
        for y in 0..grid.lines() {
            let used: usize = grid.row(y).map_or(0, |row| row.cells().iter().map(ferrum_vt::Cell::columns).sum());
            prop_assert!(used <= usize::from(width));
        }
    }

    #[test]
    fn changing_only_the_height_keeps_the_logical_text(lines in lines(), rows in 1u16..60) {
        let mut term = terminal(80, ROWS);
        feed(&mut term, &lines);
        let before = term.logical_text();
        prop_assert!(term.resize(Size::new(80, rows)).is_ok());
        prop_assert_eq!(term.logical_text(), before);
    }
}

#[test]
fn wrapped_lines_join_when_the_terminal_gets_wider() {
    let mut term = terminal(10, 4);
    term.parse(b"abcdefghijklmnopqrstuvwxy\r\nnext");
    assert!(term.resize(Size::new(40, 4)).is_ok());
    let dump = term.dump();
    assert!(dump.contains("|abcdefghijklmnopqrstuvwxy"), "{dump}");
    assert_eq!((term.cursor().y, term.cursor().x), (1, 4));
}

#[test]
fn a_narrower_terminal_pushes_wrapped_rows_into_the_history_and_keeps_the_cursor_row() {
    let mut term = terminal(20, 3);
    term.parse(b"first line that is long\r\nsecond\r\nprompt$ ");
    assert!(term.resize(Size::new(8, 3)).is_ok());
    assert_eq!(term.logical_text(), "first line that is long\nsecond\nprompt$");
    assert_eq!(term.cursor().y, 2);
    assert!(term.scrollback().len() >= 2);
}

#[test]
fn the_cursor_keeps_its_row_when_there_is_history_above() {
    let mut term = terminal(20, 6);
    let text = "history row\r\n".repeat(12);
    term.parse(text.as_bytes());
    term.parse(b"\x1b[2;1Hnear the top");
    assert_eq!(term.cursor().y, 1);
    assert!(term.resize(Size::new(30, 6)).is_ok());
    assert_eq!(term.cursor().y, 1, "the cursor row does not jump to the bottom");
}

#[test]
fn a_wide_character_that_no_longer_fits_wraps_with_a_leading_spacer() {
    let mut term = terminal(6, 8);
    term.parse("\r\n\r\n\r\nab漢字".as_bytes());
    assert!(term.resize(Size::new(3, 8)).is_ok());
    let row = term.grid().row(1).map(|row| row.cells().to_vec()).unwrap_or_default();
    assert!(
        row.get(2).is_some_and(|cell| cell.flags().contains(CellFlags::LEADING_SPACER)),
        "{}",
        term.dump()
    );
    assert!(term.grid().row(1).is_some_and(ferrum_vt::Row::is_wrapped));
    assert_eq!(term.logical_text(), "\n\n\nab漢字");
}

#[test]
fn the_alternate_screen_is_cut_not_reflowed() {
    let mut term = terminal(10, 4);
    term.parse(b"\x1b[?1049habcdefghij\r\nsixth");
    assert!(term.resize(Size::new(5, 2)).is_ok());
    let dump = term.dump();
    assert!(dump.contains("00 |abcde|"), "{dump}");
    assert!(dump.contains("01 |sixth|"), "{dump}");
    assert_eq!(term.scrollback().len(), 0);
}

#[test]
fn extreme_sizes_are_clamped_and_never_panic() {
    let mut term = terminal(80, 24);
    term.parse(b"some text\r\nmore text");
    for size in [
        Size::new(1, 1),
        Size::new(u16::MAX, u16::MAX),
        Size::new(1, 500),
        Size::new(500, 1),
        Size::new(80, 24),
    ] {
        assert!(term.resize(size).is_ok());
        assert!(term.cursor().x < term.size().cols() && term.cursor().y < term.size().rows());
    }
}
