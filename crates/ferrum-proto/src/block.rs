//! A block of encoded rows: the row payload of `Snapshot`, `Rows` and `Damage` messages.
//!
//! ```text
//! block := links:table | rows: { y:u16 | row_len:u32 | row }*     little-endian
//! ```
//!
//! `table` is the block's [`LinkTable`]; `row` is as in `encode_row`. A block never grows past
//! [`MAX_ROW_BLOCK_BYTES`], so the message that carries it always fits one frame: when a row
//! does not fit, [`RowBlock::push_row`] says so and the daemon sends the block and starts the
//! next one. Decoding a block allocates its two buffers; the daemon refills the same buffers
//! every frame.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ProtoError;
use crate::frame::MAX_FRAME_LEN;
use crate::links::LinkTable;
use crate::row::{CellRef, encode_row};
use crate::row_decode::{RowCells, decode_row};

/// Room left in a frame for everything of a `Snapshot`, `Rows` or `Damage` message except the
/// block bytes.
///
/// Reason: those fields (ids, viewport, cursor, modes, region, flags, the varint lengths of the
/// two block buffers, the frame header) take under 64 bytes, plus the title of a `Snapshot`,
/// which `ferrum-vt` caps at 1 024 bytes (`MAX_TITLE_BYTES`); 4 KiB covers both with margin for
/// fields a later minor version adds. A daemon that sent a longer title would get
/// `FrameTooLarge` from `encode_frame`, never an oversized frame.
pub const ROW_BLOCK_HEADROOM: usize = 4 * 1_024;

/// Most bytes of a block (link table plus rows): a frame minus [`ROW_BLOCK_HEADROOM`].
pub const MAX_ROW_BLOCK_BYTES: usize = MAX_FRAME_LEN - ROW_BLOCK_HEADROOM;

/// Bytes of the `y` and `row_len` fields of an entry.
const Y_LEN: usize = 2;
const ROW_LEN_LEN: usize = 4;

/// Encoded rows, each with the row it belongs to, and their link table.
///
/// `Debug` prints the sizes only: rows are terminal content (`docs/05` §8).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct RowBlock {
    links: LinkTable,
    rows: Vec<u8>,
}

impl RowBlock {
    /// An empty block.
    pub const fn new() -> Self {
        Self { links: LinkTable::new(), rows: Vec::new() }
    }

    /// Removes every row and link and keeps the capacity.
    pub fn clear(&mut self) {
        self.links.clear();
        self.rows.clear();
    }

    /// Whether the block holds no row.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Bytes the block takes in a message (link table plus rows).
    pub fn len_bytes(&self) -> usize {
        self.links.as_bytes().len() + self.rows.len()
    }

    /// Appends row `y` of `cols` columns made of `cells` (see [`encode_row`]).
    ///
    /// Allocation-free once the block has the capacity.
    ///
    /// # Errors
    ///
    /// [`ProtoError::BlockFull`] when the block would grow past [`MAX_ROW_BLOCK_BYTES`]: send it
    /// and push the row into an empty block. Otherwise as [`encode_row`]. On error the block is
    /// left as it was.
    pub fn push_row<'a, I>(&mut self, y: u16, cols: u16, cells: I) -> Result<(), ProtoError>
    where
        I: IntoIterator<Item = CellRef<'a>>,
    {
        let start = self.rows.len();
        let mark = self.links.mark();
        let result = self.encode_entry(y, cols, cells);
        if result.is_err() || self.len_bytes() > MAX_ROW_BLOCK_BYTES {
            self.rows.truncate(start);
            self.links.rollback(mark);
            return result.and(Err(ProtoError::BlockFull));
        }
        Ok(())
    }

    fn encode_entry<'a, I>(&mut self, y: u16, cols: u16, cells: I) -> Result<(), ProtoError>
    where
        I: IntoIterator<Item = CellRef<'a>>,
    {
        let len_at = self.rows.len() + Y_LEN;
        self.rows.extend_from_slice(&y.to_le_bytes());
        self.rows.extend_from_slice(&[0; ROW_LEN_LEN]);
        encode_row(cols, cells, &mut self.rows, &mut self.links)?;
        let row_len = self.rows.len() - len_at - ROW_LEN_LEN;
        let row_len = u32::try_from(row_len).map_err(|_| ProtoError::RowTooLong(row_len))?;
        // INVARIANT: the placeholder was pushed above, so the range exists.
        if let Some(slot) = self.rows.get_mut(len_at..len_at + ROW_LEN_LEN) {
            slot.copy_from_slice(&row_len.to_le_bytes());
        }
        Ok(())
    }

    /// The rows in the order they were pushed, as `(y, cells)`. Yields `Err` once at the first
    /// malformed entry and stops; each row's cells are checked as they are read.
    pub fn rows(&self) -> impl Iterator<Item = Result<(u16, RowCells<'_>), ProtoError>> + '_ {
        let mut rest = self.rows.as_slice();
        std::iter::from_fn(move || {
            if rest.is_empty() {
                return None;
            }
            let entry = next_entry(rest);
            rest = entry.as_ref().map_or(&[], |entry| entry.rest);
            Some(entry.and_then(|entry| Ok((entry.y, decode_row(entry.row, &self.links)?))))
        })
    }
}

impl std::fmt::Debug for RowBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RowBlock({} row bytes, {:?})", self.rows.len(), self.links)
    }
}

impl Serialize for RowBlock {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.links.as_bytes(), self.rows.as_slice()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RowBlock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (links, rows) = <(Vec<u8>, Vec<u8>)>::deserialize(deserializer)?;
        let links = LinkTable::from_bytes(links).map_err(D::Error::custom)?;
        Ok(Self { links, rows })
    }
}

/// One entry of a block and the bytes after it.
struct Entry<'a> {
    y: u16,
    row: &'a [u8],
    rest: &'a [u8],
}

/// Splits the first entry off `bytes`.
fn next_entry(bytes: &[u8]) -> Result<Entry<'_>, ProtoError> {
    let truncated = ProtoError::MalformedRow("truncated row block");
    let Some((y, rest)) = bytes.split_first_chunk::<Y_LEN>() else { return Err(truncated) };
    let Some((len, rest)) = rest.split_first_chunk::<ROW_LEN_LEN>() else { return Err(truncated) };
    let len = usize::try_from(u32::from_le_bytes(*len)).unwrap_or(usize::MAX);
    let (row, rest) = rest.split_at_checked(len).ok_or(truncated)?;
    Ok(Entry { y: u16::from_le_bytes(*y), row, rest })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::row::CellWidth;
    use crate::style::Style;

    #[test]
    fn rows_come_back_with_their_y_and_links() {
        let mut block = RowBlock::new();
        let style = Style { link: Some("https://a.test"), ..Style::default() };
        let cell = CellRef { text: "z", width: CellWidth::Single, style };
        block.push_row(3, 10, [cell, cell]).unwrap();
        block.push_row(7, 10, [cell]).unwrap();
        assert!(block.push_row(8, 0, [cell]).is_err());
        let rows: Vec<_> = block
            .rows()
            .map(|row| row.map(|(y, mut cells)| (y, cells.next().unwrap().unwrap().style.link)))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, [(3, style.link), (7, style.link)]);
        assert_eq!(block.links.len(), 1);
    }

    #[test]
    fn a_truncated_block_yields_one_error() {
        let mut block = RowBlock::new();
        block.push_row(1, 4, []).unwrap();
        block.rows.pop();
        let items: Vec<_> = block.rows().collect();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_err());
    }

    #[test]
    fn a_full_block_refuses_the_row_and_stays_as_it_was() {
        let mut block = RowBlock::new();
        block.rows.resize(MAX_ROW_BLOCK_BYTES - 8, 0);
        let style = Style { link: Some("https://new.test"), ..Style::default() };
        let cell = CellRef { text: "z", width: CellWidth::Single, style };
        assert!(matches!(block.push_row(0, 1, [cell]), Err(ProtoError::BlockFull)));
        assert_eq!(block.len_bytes(), MAX_ROW_BLOCK_BYTES - 8);
        assert!(block.links.is_empty());
        assert!(!format!("{block:?}").contains("new.test"));
    }
}
