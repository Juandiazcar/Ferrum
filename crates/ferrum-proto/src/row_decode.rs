//! Decoding of the row format of [`encode_row`](crate::encode_row). Every byte is untrusted:
//! lengths are checked against the input, text against UTF-8, styles against their defined bits,
//! columns against the row width. Nothing panics and nothing allocates.

use crate::error::ProtoError;
use crate::links::LinkTable;
use crate::row::{CLUSTER_MARK, CellWidth};
use crate::style::{Attrs, Color, Style};

/// Longest UTF-8 encoding of one scalar.
const MAX_UTF8_LEN: usize = 4;

/// One decoded cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellView<'a> {
    /// First column of the cell.
    pub col: u16,
    /// The cell text.
    pub text: &'a str,
    /// Columns taken.
    pub width: CellWidth,
    /// The style.
    pub style: Style<'a>,
}

/// The cells of one encoded row, left to right; see [`decode_row`].
///
/// Yields `Err` once at the first malformed byte and then stops. A row is valid only when the
/// iteration ends without an error.
#[derive(Debug, Clone)]
pub struct RowCells<'a> {
    cols: u16,
    col: u16,
    runs_left: u16,
    rest: Reader<'a>,
    run: Reader<'a>,
    style: Style<'a>,
    links: &'a LinkTable,
    is_done: bool,
}

/// Reads the header of an encoded row and returns an iterator over its cells; hyperlink indices
/// are resolved in `links`, the table the row was encoded with.
///
/// # Errors
///
/// [`ProtoError::MalformedRow`] when the input is shorter than the row header.
pub fn decode_row<'a>(bytes: &'a [u8], links: &'a LinkTable) -> Result<RowCells<'a>, ProtoError> {
    let mut rest = Reader(bytes);
    let cols = rest.u16()?;
    let runs_left = rest.u16()?;
    Ok(RowCells {
        cols,
        col: 0,
        runs_left,
        rest,
        run: Reader(&[]),
        style: Style::default(),
        links,
        is_done: false,
    })
}

impl<'a> RowCells<'a> {
    /// The width of the row in columns.
    pub const fn cols(&self) -> u16 {
        self.cols
    }

    fn next_cell(&mut self) -> Result<Option<CellView<'a>>, ProtoError> {
        if self.run.0.is_empty() {
            if self.runs_left == 0 {
                return match self.rest.0.len() {
                    0 => Ok(None),
                    _ => Err(ProtoError::MalformedRow("bytes after the last run")),
                };
            }
            self.open_run()?;
        }
        let (text, width) = self.run.cell()?;
        let col = self.col;
        self.col = col
            .checked_add(width as u16)
            .filter(|&end| end <= self.cols)
            .ok_or(ProtoError::MalformedRow("cells past the row width"))?;
        Ok(Some(CellView { col, text, width, style: self.style }))
    }

    fn open_run(&mut self) -> Result<(), ProtoError> {
        self.runs_left -= 1; // checked by the caller: `runs_left > 0`
        self.style = self.rest.style(self.links)?;
        let len = usize::try_from(self.rest.u32()?).unwrap_or(usize::MAX);
        self.run = Reader(self.rest.take(len)?);
        if self.run.0.is_empty() { Err(ProtoError::MalformedRow("empty run")) } else { Ok(()) }
    }
}

impl<'a> Iterator for RowCells<'a> {
    type Item = Result<CellView<'a>, ProtoError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done {
            return None;
        }
        let item = self.next_cell().transpose();
        self.is_done = !matches!(item, Some(Ok(_)));
        item
    }
}

/// A cursor over untrusted bytes.
#[derive(Debug, Clone, Copy)]
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], ProtoError> {
        let (head, tail) =
            self.0.split_at_checked(len).ok_or(ProtoError::MalformedRow("truncated"))?;
        self.0 = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProtoError> {
        let bytes = self.take(N)?;
        bytes.try_into().map_err(|_| ProtoError::MalformedRow("truncated"))
    }

    fn u16(&mut self) -> Result<u16, ProtoError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ProtoError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn color(&mut self) -> Result<Color, ProtoError> {
        Color::from_bits(self.u32()?).ok_or(ProtoError::MalformedRow("undefined color"))
    }

    fn style(&mut self, links: &'a LinkTable) -> Result<Style<'a>, ProtoError> {
        let attrs = Attrs::from_bits(self.u16()?)
            .ok_or(ProtoError::MalformedRow("undefined attributes"))?;
        let fg = self.color()?;
        let bg = self.color()?;
        let underline_color = self.color()?;
        let link = match self.u16()? {
            0 => None,
            index => Some(links.get(index).ok_or(ProtoError::MalformedRow("unknown link index"))?),
        };
        Ok(Style { attrs, fg, bg, underline_color, link })
    }

    fn cell(&mut self) -> Result<(&'a str, CellWidth), ProtoError> {
        if self.0.first() == Some(&CLUSTER_MARK) {
            let [_, width, len] = self.array()?;
            let width = match width {
                1 => CellWidth::Single,
                2 => CellWidth::Double,
                _ => return Err(ProtoError::MalformedRow("undefined cell width")),
            };
            let text = utf8(self.take(usize::from(len))?)?;
            return if text.is_empty() {
                Err(ProtoError::MalformedRow("empty cell"))
            } else {
                Ok((text, width))
            };
        }
        let head = self.0.get(..MAX_UTF8_LEN.min(self.0.len())).unwrap_or_default();
        let valid = match std::str::from_utf8(head) {
            Ok(text) => text,
            Err(error) => utf8(head.get(..error.valid_up_to()).unwrap_or_default())?,
        };
        let scalar = valid.chars().next().ok_or(ProtoError::MalformedRow("invalid UTF-8"))?;
        let text = utf8(self.take(scalar.len_utf8())?)?;
        Ok((text, CellWidth::Single))
    }
}

fn utf8(bytes: &[u8]) -> Result<&str, ProtoError> {
    std::str::from_utf8(bytes).map_err(|_| ProtoError::MalformedRow("invalid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_row;
    use crate::row::CellRef;

    static NO_LINKS: LinkTable = LinkTable::new();

    fn decode_all(bytes: &[u8]) -> Result<Vec<CellView<'_>>, ProtoError> {
        decode_row(bytes, &NO_LINKS)?.collect()
    }

    #[test]
    fn truncated_and_trailing_input_is_refused() {
        let mut out = Vec::new();
        let cell = CellRef { text: "x", width: CellWidth::Single, style: Style::default() };
        encode_row(3, [cell], &mut out, &mut LinkTable::new()).unwrap();
        for len in 0..out.len() {
            assert!(decode_all(&out[..len]).is_err(), "prefix of {len} bytes");
        }
        out.push(0);
        assert!(decode_all(&out).is_err());
    }

    #[test]
    fn a_cell_past_the_width_is_refused() {
        let mut out = Vec::new();
        let cell = CellRef { text: "x", width: CellWidth::Double, style: Style::default() };
        encode_row(2, [cell], &mut out, &mut LinkTable::new()).unwrap();
        out[0] = 1;
        assert!(matches!(decode_all(&out), Err(ProtoError::MalformedRow(_))));
    }

    #[test]
    fn the_iterator_stops_after_an_error() {
        let mut cells = decode_row(&[1, 0, 1, 0], &NO_LINKS).unwrap();
        assert!(matches!(cells.next(), Some(Err(_))));
        assert!(cells.next().is_none());
    }
}
