//! Rows to scrollback lines (`docs/02` §3.2): text plus style runs.
//!
//! The scrollback stores scalars and trims trailing default blanks; wide characters and clusters
//! are re-derived from the text with the shared width table. A row that soft-wraps must keep its
//! trailing blanks (they are part of the logical line), so they are tagged with [`WRAP_PAD`], a
//! style the renderer never sees: it makes them non-default and the scrollback keeps them.

use ferrum_scrollback::{Style, StyledRun};

use crate::cell::{Cell, CellFlags};
use crate::graphemes::GraphemeTable;
use crate::row::Row;

/// Attribute bit that marks trailing blanks of a wrapped row so they survive trimming. Never
/// present in a grid cell: it is removed when a line is decoded.
pub(crate) const WRAP_PAD: u16 = 1 << 15;

/// The scrollback style of a cell.
pub(crate) fn style_of(cell: &Cell) -> Style {
    Style { fg: cell.fg_word(), bg: cell.bg_word(), attrs: cell.attr_word() }
}

fn push_run(runs: &mut Vec<StyledRun>, style: Style, len: usize) {
    match runs.last_mut() {
        Some(last) if last.style == style => last.len += len,
        _ => runs.push(StyledRun { style, len }),
    }
}

/// The encoded form of one row: text plus style runs, reused between rows.
#[derive(Debug, Default)]
pub(crate) struct Encoded {
    pub(crate) text: String,
    pub(crate) runs: Vec<StyledRun>,
}

/// What happens to the default blanks at the end of an encoded row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tail {
    /// The scrollback trims them (the row ends a logical line).
    Trim,
    /// They are content: the row soft-wraps into the next one (see the module docs).
    Keep,
}

impl Tail {
    /// The tail rule of `row`: a wrapped row keeps its blanks.
    pub(crate) fn of(row: &Row) -> Self {
        if row.is_wrapped() { Self::Keep } else { Self::Trim }
    }
}

/// Encodes `row` into `out` (cleared first).
pub(crate) fn encode_row(row: &Row, graphemes: &GraphemeTable, out: &mut Encoded) {
    encode_cells(row.cells(), Tail::of(row), graphemes, out);
}

/// Encodes `cells` as one row into `out` (cleared first).
pub(crate) fn encode_cells(
    cells: &[Cell],
    tail: Tail,
    graphemes: &GraphemeTable,
    out: &mut Encoded,
) {
    let Encoded { text, runs } = out;
    text.clear();
    runs.clear();
    for cell in cells {
        if cell.flags().intersects(CellFlags::SPACER | CellFlags::LEADING_SPACER) {
            continue;
        }
        let scalars = if cell.flags().contains(CellFlags::GRAPHEME) {
            let cluster = graphemes.get(cell.raw_char()).unwrap_or(" ");
            text.push_str(cluster);
            cluster.chars().count()
        } else {
            text.push(cell.scalar());
            1
        };
        push_run(runs, style_of(cell), scalars);
    }
    if tail == Tail::Keep {
        pad_trailing_blanks(text, runs);
    }
}

/// Retags the default-styled trailing spaces of a wrapped row.
pub(crate) fn pad_trailing_blanks(text: &str, runs: &mut Vec<StyledRun>) {
    let mut remaining = text.bytes().rev().take_while(|&byte| byte == b' ').count();
    let mut padded = 0;
    while remaining > 0 {
        let Some(last) = runs.last_mut().filter(|run| run.style == Style::default()) else {
            break;
        };
        let take = last.len.min(remaining);
        last.len -= take;
        remaining -= take;
        padded += take;
        if last.len == 0 {
            runs.pop();
        }
    }
    if padded > 0 {
        runs.push(StyledRun { style: Style { attrs: WRAP_PAD, ..Style::default() }, len: padded });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::BLANK_CHAR;

    fn row_of(text: &str, wrapped: bool) -> Row {
        let mut row = Row::new(text.chars().count(), Cell::default());
        for (x, c) in text.chars().enumerate() {
            if let Some(cell) = row.get_mut(x) {
                cell.set_scalar(c);
            }
        }
        if wrapped {
            row.insert_flag(ferrum_scrollback::LineFlags::WRAPPED);
        }
        row
    }

    #[test]
    fn plain_rows_encode_as_one_default_run() {
        let mut out = Encoded::default();
        encode_row(&row_of("abc ", false), &GraphemeTable::new(), &mut out);
        assert_eq!(out.text, "abc ");
        assert_eq!(out.runs, vec![StyledRun { style: Style::default(), len: 4 }]);
    }

    #[test]
    fn wrapped_rows_keep_their_trailing_blanks_as_padding() {
        let mut out = Encoded::default();
        encode_row(&row_of("ab  ", true), &GraphemeTable::new(), &mut out);
        assert_eq!(out.runs.len(), 2);
        assert_eq!(out.runs.last().map(|run| (run.len, run.style.attrs)), Some((2, WRAP_PAD)));
        assert_eq!(out.text.chars().filter(|&c| c == BLANK_CHAR).count(), 2);
    }

    #[test]
    fn spacers_are_not_text_and_clusters_count_all_their_scalars() {
        let mut graphemes = GraphemeTable::new();
        let index = graphemes.intern("e\u{0301}").unwrap_or(0);
        let mut row = Row::new(4, Cell::default());
        if let Some(cell) = row.get_mut(0) {
            cell.set_scalar('漢');
            cell.insert_flags(CellFlags::WIDE);
        }
        if let Some(cell) = row.get_mut(1) {
            cell.insert_flags(CellFlags::SPACER);
        }
        if let Some(cell) = row.get_mut(2) {
            cell.set_raw_char(index);
            cell.insert_flags(CellFlags::GRAPHEME);
        }
        let mut out = Encoded::default();
        encode_row(&row, &graphemes, &mut out);
        assert_eq!(out.text, "漢e\u{0301} ");
        assert_eq!(out.runs.iter().map(|run| run.len).sum::<usize>(), out.text.chars().count());
    }
}
