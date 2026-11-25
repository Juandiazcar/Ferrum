//! A read-only view of the history at one moment, for a worker thread (`docs/02` §5: "search
//! receives immutable chunk handles"): sealed chunks are shared (`Arc`), the pending chunks are
//! the immutable handles they already are, the hot chunk and the style table are copied (at most
//! one chunk and a few hundred styles), and spilled chunks are read through the store's
//! read-only handle. The session keeps writing its history meanwhile; the snapshot keeps the
//! lines it was taken with.

use std::sync::Arc;

use super::{Place, Scrollback};
use crate::chunk::{HotChunk, decode_chunk};
use crate::decode::decode_line;
use crate::error::{DecodeError, ScrollbackError};
use crate::line::DecodedLine;
use crate::scrollback::LineCursor;
use crate::spill::{SpillHandle, SpillReader};
use crate::style::StyleTable;

/// One chunk as the snapshot holds it.
#[derive(Debug)]
enum Held {
    Sealed { id: u64, lines: usize, blob: Arc<[u8]> },
    Raw { lines: Arc<HotChunk> },
    Spilled { id: u64, lines: usize, handle: SpillHandle },
}

impl Held {
    fn lines(&self) -> usize {
        match self {
            Self::Sealed { lines, .. } | Self::Spilled { lines, .. } => *lines,
            Self::Raw { lines } => lines.len(),
        }
    }
}

/// The history of a [`Scrollback`] at one moment (see the module documentation). `Send`, so a
/// worker can read it while the session goes on.
#[derive(Debug)]
pub struct HistorySnapshot {
    chunks: Vec<Held>,
    skip: usize,
    len: usize,
    styles: StyleTable,
    spill: Option<Box<dyn SpillReader>>,
}

impl Scrollback {
    /// A read-only view of the retained lines as they are now. Costs a copy of the hot chunk and
    /// of the style table; sealed and pending chunks are shared.
    pub fn snapshot(&self) -> HistorySnapshot {
        let mut chunks: Vec<Held> = self
            .chunks
            .iter()
            .map(|chunk| match &chunk.place {
                Place::Ram(blob) => {
                    Held::Sealed { id: chunk.id, lines: chunk.lines, blob: Arc::clone(blob) }
                }
                Place::Pending { lines, .. } => Held::Raw { lines: Arc::clone(lines) },
                Place::Spilled(handle) => {
                    Held::Spilled { id: chunk.id, lines: chunk.lines, handle: *handle }
                }
            })
            .collect();
        chunks.push(Held::Raw { lines: Arc::new(self.hot.clone()) });
        let skip = if self.chunks.is_empty() { self.hot_skip } else { self.head_skip };
        let spill = self.store.as_ref().and_then(|store| store.reader());
        HistorySnapshot { chunks, skip, len: self.retained, styles: self.styles.clone(), spill }
    }
}

impl HistorySnapshot {
    /// Number of lines (0 = the oldest, as in [`Scrollback::read_line`]).
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there is no line.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The style table the lines' runs refer to.
    pub fn styles(&self) -> &StyleTable {
        &self.styles
    }

    /// Reads line `index`, like [`Scrollback::read_line`]. `Ok(None)` when out of range.
    ///
    /// # Errors
    /// [`ScrollbackError`] when stored data fails validation, or a spilled chunk cannot be read
    /// (the store had no read-only handle, or reused the record since the snapshot).
    pub fn read_line<'c>(
        &self,
        index: usize,
        cursor: &'c mut LineCursor,
    ) -> Result<Option<&'c DecodedLine>, ScrollbackError> {
        if index >= self.len {
            return Ok(None);
        }
        let mut offset = index + self.skip;
        for held in &self.chunks {
            if offset < held.lines() {
                self.read_in(held, offset, cursor)?;
                return Ok(Some(&cursor.line));
            }
            offset -= held.lines();
        }
        Ok(None)
    }

    fn read_in(
        &self,
        held: &Held,
        line: usize,
        cursor: &mut LineCursor,
    ) -> Result<(), ScrollbackError> {
        match held {
            Held::Raw { lines } => {
                decode_line(lines.line(line).ok_or(DecodeError::BadIndex)?, &mut cursor.line)?;
            }
            Held::Sealed { id, blob, .. } => {
                if cursor.chunk.id != Some(*id) {
                    decode_chunk(blob, &mut cursor.chunk)?;
                    cursor.chunk.id = Some(*id);
                }
                cursor.chunk.line(line, &mut cursor.line).ok_or(DecodeError::BadIndex)??;
            }
            Held::Spilled { id, handle, .. } => {
                if cursor.chunk.id != Some(*id) {
                    let reader = self.spill.as_ref().ok_or(ScrollbackError::NoSpillStore)?;
                    reader.read(*handle, &mut cursor.spill)?;
                    decode_chunk(&cursor.spill, &mut cursor.chunk)?;
                    cursor.chunk.id = Some(*id);
                }
                cursor.chunk.line(line, &mut cursor.line).ok_or(DecodeError::BadIndex)??;
            }
        }
        match cursor.line.runs.iter().find(|run| self.styles.get(run.style).is_none()) {
            Some(run) => Err(DecodeError::UnknownStyle(run.style.get()).into()),
            None => Ok(()),
        }
    }
}
