//! Rows to scrollback lines (`docs/02` §3.2): text plus style runs.
//!
//! The scrollback stores scalars and trims trailing default blanks; wide characters and clusters
//! are re-derived from the text with the shared width table. A row that soft-wraps must keep its
//! trailing blanks (they are part of the logical line), so they are tagged with [`WRAP_PAD`], a
//! style the renderer never sees: it makes them non-default and the scrollback keeps them.

use ferrum_scrollback::{Style, StyledRun};

use crate::cell::{AsciiStyle, Cell, CellFlags};
use crate::graphemes::GraphemeTable;
use crate::row::{DEFAULT_WORDS, Row, StyleWords};

/// Attribute bit that marks trailing blanks of a wrapped row so they survive trimming. Never
/// present in a grid cell: it is removed when a line is decoded.
pub(crate) const WRAP_PAD: u16 = 1 << 15;

/// Builds the runs of a row: consecutive cells with the same style words share a run.
struct RunBuilder<'a> {
    runs: &'a mut Vec<StyledRun>,
    current: (u32, u32, u16),
    len: usize,
}

impl RunBuilder<'_> {
    /// Adds `scalars` scalars in the style `words` (`fg`, `bg`, `attrs`).
    fn add(&mut self, words: (u32, u32, u16), scalars: usize) {
        if self.len > 0 && self.current == words {
            self.len += scalars;
            return;
        }
        self.finish();
        self.current = words;
        self.len = scalars;
    }

    /// Closes the open run.
    fn finish(&mut self) {
        if self.len > 0 {
            let (fg, bg, attrs) = self.current;
            self.runs.push(StyledRun { style: Style { fg, bg, attrs }, len: self.len });
            self.len = 0;
        }
    }
}

/// The encoded form of one row: text plus style runs, reused between rows.
#[derive(Debug, Default)]
pub(crate) struct Encoded {
    /// UTF-8 text, built as bytes so that ASCII runs are appended without per-character work.
    bytes: Vec<u8>,
    pub(crate) runs: Vec<StyledRun>,
}

impl Encoded {
    /// The encoded text.
    pub(crate) fn text(&self) -> &str {
        // INVARIANT: only whole ASCII bytes and UTF-8 encoded scalars or clusters are appended,
        // so the bytes are always valid UTF-8 and the check (one fast pass) cannot fail.
        std::str::from_utf8(&self.bytes).unwrap_or_default()
    }
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
#[cfg(test)]
pub(crate) fn encode_row(row: &Row, graphemes: &GraphemeTable, out: &mut Encoded) {
    encode_cells(row.cells(), Tail::of(row), graphemes, out);
}

/// Encodes the first `width` cells of `row` into `out` (cleared first).
pub(crate) fn encode_row_prefix(
    row: &Row,
    width: usize,
    graphemes: &GraphemeTable,
    out: &mut Encoded,
) {
    let cells = row.cells().get(..width).unwrap_or_default();
    match row.ascii_style() {
        // perf: a row of printed text in one style needs no per-cell checks (bench
        // `vt_parse/ascii_8mib`). Cells past the written part are default blanks.
        Some((words, used)) if width <= used || words == DEFAULT_WORDS => {
            encode_uniform(cells, words, Tail::of(row), out);
        }
        _ => encode_cells(cells, Tail::of(row), graphemes, out),
    }
}

/// Encodes plain ASCII `cells` that all have the style `words`.
fn encode_uniform(cells: &[Cell], words: StyleWords, tail: Tail, out: &mut Encoded) {
    let Encoded { bytes, runs } = out;
    bytes.clear();
    runs.clear();
    bytes.extend(cells.iter().map(Cell::ascii_byte));
    if !cells.is_empty() {
        let (fg, bg, attrs) = words;
        runs.push(StyledRun { style: Style { fg, bg, attrs }, len: cells.len() });
    }
    if tail == Tail::Keep {
        pad_blank_bytes(bytes, runs);
    }
}

/// Encodes `cells` as one row into `out` (cleared first).
pub(crate) fn encode_cells(
    cells: &[Cell],
    tail: Tail,
    graphemes: &GraphemeTable,
    out: &mut Encoded,
) {
    let Encoded { bytes, runs } = out;
    bytes.clear();
    runs.clear();
    let mut builder = RunBuilder { runs, current: (0, 0, 0), len: 0 };
    let mut rest = cells;
    while let Some((cell, tail_cells)) = rest.split_first() {
        let words = (cell.fg_word(), cell.bg_word(), cell.attr_word());
        if cell.is_plain_ascii() {
            // perf: a run of plain ASCII cells in one style is copied in one pass
            // (bench vt_parse/ascii_8mib).
            let run = push_ascii_run(bytes, rest, &cell.ascii_style());
            builder.add(words, run);
            rest = rest.get(run..).unwrap_or_default();
            continue;
        }
        rest = tail_cells;
        if cell.flags().intersects(CellFlags::SPACER | CellFlags::LEADING_SPACER) {
            continue;
        }
        let scalars = if cell.flags().contains(CellFlags::GRAPHEME) {
            let cluster = graphemes.get(cell.raw_char()).unwrap_or(" ");
            bytes.extend_from_slice(cluster.as_bytes());
            cluster.chars().count()
        } else {
            bytes.extend_from_slice(cell.scalar().encode_utf8(&mut [0; 4]).as_bytes());
            1
        };
        builder.add(words, scalars);
    }
    builder.finish();
    if tail == Tail::Keep {
        pad_blank_bytes(&out.bytes, &mut out.runs);
    }
}

/// Cells per step of [`push_ascii_run`].
const BLOCK: usize = 8;

/// Appends the characters of the leading run of plain ASCII cells that have the style `key` to
/// `bytes` and returns the run length.
///
/// perf: the style check and the character gather share one pass over 8-cell blocks, and each
/// block is appended with one copy (bench `vt_parse/ascii_8mib`).
fn push_ascii_run(bytes: &mut Vec<u8>, cells: &[Cell], key: &AsciiStyle) -> usize {
    // Short runs (colored text changes style every few cells) are cheaper cell by cell.
    let mut run = 0;
    for cell in cells.iter().take(BLOCK).take_while(|cell| cell.differs_from(key) == 0) {
        bytes.push(cell.ascii_byte());
        run += 1;
    }
    if run < BLOCK {
        return run;
    }
    let (blocks, _) = cells.get(run..).unwrap_or_default().as_chunks::<BLOCK>();
    for block in blocks {
        let mut gathered = [0u8; BLOCK];
        let mut diff = 0;
        for (byte, cell) in gathered.iter_mut().zip(block) {
            diff |= cell.differs_from(key);
            *byte = cell.ascii_byte();
        }
        if diff != 0 {
            break;
        }
        bytes.extend_from_slice(&gathered);
        run += BLOCK;
    }
    for cell in cells.iter().skip(run).take_while(|cell| cell.differs_from(key) == 0) {
        bytes.push(cell.ascii_byte());
        run += 1;
    }
    run
}

/// Retags the default-styled trailing spaces of a wrapped row.
pub(crate) fn pad_trailing_blanks(text: &str, runs: &mut Vec<StyledRun>) {
    pad_blank_bytes(text.as_bytes(), runs);
}

fn pad_blank_bytes(text: &[u8], runs: &mut Vec<StyledRun>) {
    let mut remaining = text.iter().rev().copied().take_while(|&byte| byte == b' ').count();
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

    type Write<'a> = (usize, &'a [u8], StyleWords);

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
        assert_eq!(out.text(), "abc ");
        assert_eq!(out.runs, vec![StyledRun { style: Style::default(), len: 4 }]);
    }

    #[test]
    fn wrapped_rows_keep_their_trailing_blanks_as_padding() {
        let mut out = Encoded::default();
        encode_row(&row_of("ab  ", true), &GraphemeTable::new(), &mut out);
        assert_eq!(out.runs.len(), 2);
        assert_eq!(out.runs.last().map(|run| (run.len, run.style.attrs)), Some((2, WRAP_PAD)));
        assert_eq!(out.text().chars().filter(|&c| c == BLANK_CHAR).count(), 2);
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
        assert_eq!(out.text(), "漢e\u{0301} ");
        assert_eq!(out.runs.iter().map(|run| run.len).sum::<usize>(), out.text().chars().count());
    }

    #[test]
    fn the_uniform_shortcut_encodes_exactly_like_the_cell_by_cell_path() {
        let red = (1, 0, 0);
        let writes: [&[Write<'_>]; 6] = [
            &[],
            &[(0, b"hello  ", DEFAULT_WORDS)],
            &[(0, b"hello  ", red)],
            &[(0, b"ab", red), (2, b"pq", red), (1, b"X", red)],
            &[(0, b"ab", red), (2, b"pq", DEFAULT_WORDS)],
            &[(3, b"gap", DEFAULT_WORDS), (1, b"y", red)],
        ];
        for writes in writes {
            for wrapped in [false, true] {
                let mut row = Row::new(10, Cell::default());
                for &(x, bytes, words) in writes {
                    row.write_ascii(x, bytes, words);
                }
                if wrapped {
                    row.insert_flag(ferrum_scrollback::LineFlags::WRAPPED);
                }
                let width = if wrapped { row.len() } else { row.used_len() };
                let (mut fast, mut slow) = (Encoded::default(), Encoded::default());
                encode_row_prefix(&row, width, &GraphemeTable::new(), &mut fast);
                let cells = row.cells().get(..width).unwrap_or_default();
                encode_cells(cells, Tail::of(&row), &GraphemeTable::new(), &mut slow);
                assert_eq!((fast.text(), &fast.runs), (slow.text(), &slow.runs), "{writes:?}");
            }
        }
    }
}
