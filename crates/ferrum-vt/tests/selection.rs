//! Selection logic on a live terminal (FR-TERM-014).

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{NullSink, Point, Selection, SelectionKind, Size, Terminal};

fn terminal(cols: u16, rows: u16) -> Terminal<NullSink> {
    let config = ScrollbackConfig::default();
    Terminal::new(Size::new(cols, rows), Scrollback::new(config), NullSink)
}

fn point(term: &Terminal<NullSink>, y: usize, col: usize) -> Point {
    Point { line: term.screen_line(y), col }
}

#[test]
fn a_double_click_selects_the_word_or_the_separators_under_it() {
    let mut term = terminal(30, 4);
    term.parse(b"git commit -m \"fix: it\"");
    let word = term.select_word(point(&term, 0, 5));
    assert_eq!(term.selection_text(&word), "commit");
    let gap = term.select_word(point(&term, 0, 10));
    assert_eq!(term.selection_text(&gap), "");
    let path = term.select_word(point(&term, 0, 20));
    assert_eq!(term.selection_text(&path), "it");
}

#[test]
fn wide_characters_are_selected_from_either_of_their_columns() {
    let mut term = terminal(20, 2);
    term.parse("ab 漢字 cd".as_bytes());
    for col in [3, 4, 5, 6] {
        let word = term.select_word(point(&term, 0, col));
        assert_eq!(term.selection_text(&word), "漢字", "column {col}");
    }
}

#[test]
fn a_triple_click_selects_the_whole_wrapped_line_and_copy_joins_it_without_a_newline() {
    let mut term = terminal(10, 5);
    term.parse(b"abcdefghijklmnopqrstuvwxy\r\nsecond");
    let line = term.select_logical_line(point(&term, 1, 3));
    assert_eq!(line.kind(), SelectionKind::Line);
    assert_eq!(term.selection_text(&line), "abcdefghijklmnopqrstuvwxy");
}

#[test]
fn copy_spans_the_history_and_the_screen_and_survives_scrolling() {
    let mut term = terminal(10, 3);
    term.parse(b"one\r\ntwo\r\nthree");
    let start = point(&term, 0, 0);
    let end = point(&term, 2, 4);
    let selection = Selection::between(SelectionKind::Simple, start, end);
    assert_eq!(term.selection_text(&selection), "one\ntwo\nthree");
    term.parse(b"\r\nfour\r\nfive");
    assert!(term.scrollback().len() >= 2);
    assert_eq!(term.selection_text(&selection), "one\ntwo\nthree");
}

#[test]
fn a_block_selection_copies_a_rectangle_row_by_row() {
    let mut term = terminal(10, 3);
    term.parse(b"abcdef\r\nghijkl\r\nmnopqr");
    let selection =
        Selection::between(SelectionKind::Block, point(&term, 0, 1), point(&term, 2, 3));
    assert_eq!(term.selection_text(&selection), "bcd\nhij\nnop");
}

#[test]
fn positions_outside_the_text_select_nothing_and_never_panic() {
    let mut term = terminal(10, 3);
    term.parse(b"abc");
    let far = Point { line: u64::MAX, col: usize::MAX };
    assert_eq!(term.selection_text(&term.select_word(far)), "");
    let gone = Selection::between(
        SelectionKind::Simple,
        Point { line: 0, col: 0 },
        Point { line: 3, col: 99 },
    );
    assert_eq!(
        term.selection_text(&gone),
        "abc

"
    );
    assert_eq!(term.selection_text(&term.select_logical_line(far)).len(), 0);
}
