//! Printing and screen-state edge cases that need a computed input (a fixture file would be
//! unreadable).

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{CellFlags, MAX_REPEAT, NullSink, Size, Terminal};

const HISTORY_LINES: usize = 50_000;

fn terminal(cols: u16, rows: u16) -> Terminal<NullSink> {
    let config = ScrollbackConfig::new(HISTORY_LINES, HISTORY_LINES).unwrap_or_default();
    Terminal::new(Size::new(cols, rows), Scrollback::new(config), NullSink)
}

#[test]
fn a_huge_repeat_count_is_bounded() {
    let mut term = terminal(20, 6);
    term.parse(b"x\x1b[65535b");
    let printed = usize::from(MAX_REPEAT) + 1;
    let rows_used = printed.div_ceil(20);
    let scrolled = u64::try_from(rows_used.saturating_sub(6)).unwrap_or(0);
    assert_eq!(term.scrollback().pushed_lines(), scrolled);
}

#[test]
fn a_variation_selector_widens_the_cell_and_moves_the_cursor() {
    let mut term = terminal(20, 3);
    term.parse("\u{2764}".as_bytes());
    assert_eq!(term.cursor().x, 1);
    term.parse("\u{FE0F}".as_bytes());
    let head = term.grid().row(0).and_then(|row| row.get(0)).copied().unwrap_or_default();
    assert!(head.flags().contains(CellFlags::WIDE | CellFlags::GRAPHEME));
    assert_eq!(term.cursor().x, 2);
}

#[test]
fn a_cluster_that_becomes_wide_at_the_last_column_wraps_whole() {
    let mut term = terminal(4, 3);
    term.parse("abc\u{2764}\u{FE0F}".as_bytes());
    let row0 = term.grid().row(0).map(|row| row.cells().to_vec()).unwrap_or_default();
    assert!(row0.last().is_some_and(|cell| cell.flags().contains(CellFlags::LEADING_SPACER)));
    assert!(term.grid().row(0).is_some_and(ferrum_vt::Row::is_wrapped));
    assert_eq!((term.cursor().y, term.cursor().x), (1, 2));
}

#[test]
fn a_width_changing_merge_at_the_end_of_a_wrapped_row_keeps_the_wrap_flag() {
    let mut term = terminal(4, 3);
    term.parse(b"abcdefgh");
    assert!(term.grid().row(0).is_some_and(ferrum_vt::Row::is_wrapped));
    // A wide emoji in the last two columns of the wrapped row, then a text-presentation selector
    // that makes it narrow.
    term.parse("\x1b[1;3H\u{231A}\u{FE0E}".as_bytes());
    assert!(term.grid().row(0).is_some_and(ferrum_vt::Row::is_wrapped));
}

#[test]
fn a_row_of_long_clusters_still_counts_as_one_history_line() {
    let mut term = terminal(1_024, 2);
    let cluster = format!("e{}", "\u{301}".repeat(31));
    term.parse(cluster.repeat(1_024).as_bytes());
    term.parse(b"\r\n\r\nnext");
    assert_eq!(term.scrollback().pushed_lines(), 1);
    assert_eq!(term.scrollback().len(), 1);
}

#[test]
fn each_screen_keeps_its_kitty_keyboard_stack_across_screen_switches() {
    let mut term = terminal(20, 4);
    term.parse(b"\x1b[>1u");
    let primary = term.keyboard_flags();
    term.parse(b"\x1b[?1049h\x1b[>5u");
    let alt = term.keyboard_flags();
    assert_ne!(alt, primary);
    term.parse(b"\x1b[?1049l");
    assert_eq!(term.keyboard_flags(), primary);
    term.parse(b"\x1b[?1049h");
    assert_eq!(term.keyboard_flags(), alt);
}
