//! Row encoding for the screen rows the daemon sends (`docs/02` §4): one row as style runs over a
//! cell stream. All integers are little-endian. Hyperlinks are indices into the [`LinkTable`] of
//! the block the row belongs to.
//!
//! ```text
//! row   := cols:u16 | run_count:u16 | run{run_count}
//! run   := style | cells_len:u32 | cell+            (cells_len bytes of cells, never empty)
//! style := attrs:u16 | fg:u32 | bg:u32 | underline_color:u32 | link:u16   (0 or a link index)
//! cell  := scalar                                  one UTF-8 scalar, one column
//!        | 0xFF width:u8 len:u8 text               any other cell: 1 or 2 columns, len UTF-8 bytes
//! ```
//!
//! `0xFF` never starts a UTF-8 sequence, so a plain cell is its own text. Columns after the last
//! cell are blank with the default style. A wide cell covers its column and the next one (the
//! spacer is not encoded). The widths are the daemon's: the UI needs no width tables.

use crate::error::ProtoError;
use crate::links::LinkTable;
use crate::style::Style;

/// Marks a cell that is not a single one-column scalar.
pub(crate) const CLUSTER_MARK: u8 = 0xFF;
/// Width of the run length field, written before its content is known.
const U32_LEN: usize = 4;

/// How many columns a cell takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CellWidth {
    /// One column.
    Single = 1,
    /// Two columns (wide characters, emoji).
    Double = 2,
}

/// One cell handed to [`encode_row`]: its text (a scalar or a grapheme cluster), its width and
/// its style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRef<'a> {
    /// The cell text, 1 to 255 bytes.
    pub text: &'a str,
    /// Columns taken.
    pub width: CellWidth,
    /// The style.
    pub style: Style<'a>,
}

/// Appends the encoding of one row of `cols` columns made of `cells` (left to right, spacers of
/// wide cells left out, trailing default blanks may be left out) to `out`. Hyperlinks are stored
/// in `links` (once per distinct URI) and the row refers to them by index; decode the row with
/// the same table.
///
/// Allocation-free once `out` and `links` have the capacity (hot path, `docs/05` §6;
/// `tests/alloc_row.rs`).
///
/// # Errors
///
/// [`ProtoError::TooManyCells`] when the cells take more than `cols` columns,
/// [`ProtoError::BadCellText`] for an empty cell or one over 255 bytes,
/// [`ProtoError::LinkTooLong`], [`ProtoError::TooManyLinks`] and [`ProtoError::RowTooLong`]. On
/// error `out` and `links` are left as they were.
///
/// ```
/// use ferrum_proto::{CellRef, CellWidth, LinkTable, Style, decode_row, encode_row};
/// let style = Style { link: Some("https://example.test"), ..Style::default() };
/// let cell = CellRef { text: "a", width: CellWidth::Single, style };
/// let (mut out, mut links) = (Vec::new(), LinkTable::new());
/// encode_row(80, [cell], &mut out, &mut links)?;
/// let mut row = decode_row(&out, &links)?;
/// assert_eq!(row.cols(), 80);
/// assert_eq!(row.next().transpose()?.map(|c| (c.text, c.style.link)), Some(("a", style.link)));
/// # Ok::<(), ferrum_proto::ProtoError>(())
/// ```
pub fn encode_row<'a, I>(
    cols: u16,
    cells: I,
    out: &mut Vec<u8>,
    links: &mut LinkTable,
) -> Result<(), ProtoError>
where
    I: IntoIterator<Item = CellRef<'a>>,
{
    let start = out.len();
    let mark = links.mark();
    out.extend_from_slice(&cols.to_le_bytes());
    let mut encoder = Encoder { cols, used: 0, runs: 0, runs_at: out.len(), run: None, links };
    out.extend_from_slice(&0_u16.to_le_bytes());
    let result = cells
        .into_iter()
        .try_for_each(|cell| encoder.push(cell, out))
        .and_then(|()| encoder.close_run(out));
    if result.is_err() {
        out.truncate(start);
        encoder.links.rollback(mark);
    }
    result
}

/// The open run: its style and where its `cells_len` field is.
struct OpenRun<'a> {
    style: Style<'a>,
    len_at: usize,
}

struct Encoder<'a, 't> {
    cols: u16,
    used: u16,
    runs: u16,
    runs_at: usize,
    run: Option<OpenRun<'a>>,
    links: &'t mut LinkTable,
}

impl<'a> Encoder<'a, '_> {
    fn push(&mut self, cell: CellRef<'a>, out: &mut Vec<u8>) -> Result<(), ProtoError> {
        let used = self.used.checked_add(cell.width as u16);
        self.used = used
            .filter(|&used| used <= self.cols)
            .ok_or(ProtoError::TooManyCells { cols: self.cols })?;
        if self.run.as_ref().is_none_or(|run| run.style != cell.style) {
            self.close_run(out)?;
            let link = cell.style.link.map_or(Ok(0), |uri| self.links.intern(uri))?;
            write_style(&cell.style, link, out);
            self.runs += 1; // at most one run per column, so it fits like `used`
            self.run = Some(OpenRun { style: cell.style, len_at: out.len() });
            out.extend_from_slice(&0_u32.to_le_bytes());
        }
        write_cell(cell, out)
    }

    /// Writes the length of the open run and the run count.
    fn close_run(&mut self, out: &mut [u8]) -> Result<(), ProtoError> {
        if let Some(run) = self.run.take() {
            let cells_len = out.len() - run.len_at - U32_LEN;
            let cells_len =
                u32::try_from(cells_len).map_err(|_| ProtoError::RowTooLong(cells_len))?;
            patch(out, run.len_at, &cells_len.to_le_bytes());
        }
        patch(out, self.runs_at, &self.runs.to_le_bytes());
        Ok(())
    }
}

/// Writes a style whose hyperlink is entry `link` of the table (0: none).
fn write_style(style: &Style<'_>, link: u16, out: &mut Vec<u8>) {
    out.extend_from_slice(&style.attrs.bits().to_le_bytes());
    out.extend_from_slice(&style.fg.bits().to_le_bytes());
    out.extend_from_slice(&style.bg.bits().to_le_bytes());
    out.extend_from_slice(&style.underline_color.bits().to_le_bytes());
    out.extend_from_slice(&link.to_le_bytes());
}

fn write_cell(cell: CellRef<'_>, out: &mut Vec<u8>) -> Result<(), ProtoError> {
    let mut chars = cell.text.chars();
    let is_plain = chars.next().is_some() && chars.next().is_none();
    if is_plain && cell.width == CellWidth::Single {
        out.extend_from_slice(cell.text.as_bytes());
        return Ok(());
    }
    let len =
        u8::try_from(cell.text.len()).ok().filter(|&len| len > 0).ok_or(ProtoError::BadCellText)?;
    out.extend_from_slice(&[CLUSTER_MARK, cell.width as u8, len]);
    out.extend_from_slice(cell.text.as_bytes());
    Ok(())
}

/// Overwrites the placeholder at `at` with `bytes`.
fn patch(out: &mut [u8], at: usize, bytes: &[u8]) {
    // INVARIANT: every placeholder was pushed with this exact size before, so the range exists.
    if let Some(slot) = out.get_mut(at..at + bytes.len()) {
        slot.copy_from_slice(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Attrs, Color};

    /// Bytes of the `cols` and `run_count` fields.
    const ROW_HEADER_LEN: usize = 4;

    fn cell(text: &str, width: CellWidth) -> CellRef<'_> {
        CellRef { text, width, style: Style::default() }
    }

    #[test]
    fn an_empty_row_is_its_header() {
        let mut out = Vec::new();
        encode_row(80, [], &mut out, &mut LinkTable::new()).unwrap_or_default();
        assert_eq!(out, [80, 0, 0, 0]);
        assert_eq!(out.len(), ROW_HEADER_LEN);
    }

    #[test]
    fn plain_ascii_is_one_byte_per_cell() {
        let mut out = Vec::new();
        let cells = "ab".split_inclusive(|_| true).map(|t| cell(t, CellWidth::Single));
        assert!(encode_row(2, cells, &mut out, &mut LinkTable::new()).is_ok());
        let style_len = 2 + 3 * 4 + 2;
        assert_eq!(out.len(), ROW_HEADER_LEN + style_len + U32_LEN + 2);
        assert_eq!(&out[out.len() - 2..], b"ab");
    }

    #[test]
    fn a_style_change_opens_a_run() {
        let mut out = Vec::new();
        let bold = Style { attrs: Attrs::BOLD, fg: Color::indexed(1), ..Style::default() };
        let cells =
            [cell("a", CellWidth::Single), CellRef { style: bold, ..cell("b", CellWidth::Single) }];
        assert!(encode_row(4, cells, &mut out, &mut LinkTable::new()).is_ok());
        assert_eq!(&out[2..4], [2, 0]);
    }

    #[test]
    fn errors_leave_the_buffer_unchanged() {
        let mut out = vec![9];
        let wide = [cell("界", CellWidth::Double)];
        assert!(matches!(
            encode_row(1, wide, &mut out, &mut LinkTable::new()),
            Err(ProtoError::TooManyCells { cols: 1 })
        ));
        assert!(matches!(
            encode_row(1, [cell("", CellWidth::Single)], &mut out, &mut LinkTable::new()),
            Err(ProtoError::BadCellText)
        ));
        let long = "e\u{301}".repeat(100);
        let cells = [cell(&long, CellWidth::Single)];
        assert!(matches!(
            encode_row(1, cells, &mut out, &mut LinkTable::new()),
            Err(ProtoError::BadCellText)
        ));
        assert_eq!(out, [9]);
    }
}
