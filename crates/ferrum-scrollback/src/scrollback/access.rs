//! Random access by line index.

use super::{Place, Scrollback};
use crate::chunk::{ChunkBuf, decode_chunk};
use crate::decode::decode_line;
use crate::error::{DecodeError, ScrollbackError};
use crate::line::DecodedLine;

/// Caller-owned buffers for reading: the decoded line and the last decompressed chunk, so that
/// sequential reads (scrolling, search) decompress each chunk once.
#[derive(Debug, Default)]
pub struct LineCursor {
    chunk: ChunkBuf,
    spill: Vec<u8>,
    line: DecodedLine,
}

impl LineCursor {
    /// A cursor with empty buffers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Heap bytes held by the buffers (one decompressed chunk, about 80 KB at most in practice).
    pub fn capacity_bytes(&self) -> usize {
        self.chunk.capacity_bytes() + self.spill.capacity() + self.line.capacity_bytes()
    }
}

/// Where a line lives: sealed chunk `index` line `line`, or hot line `line`.
enum Location {
    Sealed { chunk: usize, line: usize },
    Hot(usize),
}

impl Scrollback {
    /// Reads retained line `index` (0 = oldest). `Ok(None)` when out of range.
    ///
    /// Reading a sealed chunk decompresses it into the cursor; a spilled chunk is first read from
    /// the spill store. The returned line borrows the cursor.
    ///
    /// # Errors
    /// [`ScrollbackError`] when stored data fails validation or the spill store fails.
    pub fn read_line<'c>(
        &self,
        index: usize,
        cursor: &'c mut LineCursor,
    ) -> Result<Option<&'c DecodedLine>, ScrollbackError> {
        let Some(location) = self.locate(index) else { return Ok(None) };
        match location {
            Location::Hot(line) => {
                let bytes = self.hot.line(line).ok_or(DecodeError::BadIndex)?;
                decode_line(bytes, &mut cursor.line)?;
            }
            Location::Sealed { chunk, line } => {
                self.load_chunk(chunk, cursor)?;
                cursor.chunk.line(line, &mut cursor.line).ok_or(DecodeError::BadIndex)??;
            }
        }
        self.check_styles(&cursor.line)?;
        Ok(Some(&cursor.line))
    }

    /// Removes the oldest retained line and returns it, decoded into `cursor`. `Ok(None)` when
    /// the store is empty. The removal counts in [`dropped_lines`](Self::dropped_lines), so a
    /// caller that rewrites the history (reflow in `ferrum-vt`) by popping the front and pushing
    /// at the back can tell by the absolute index `dropped_lines()` which lines are still the
    /// old ones. Chunks are freed as they are consumed, so rewriting needs no second copy.
    ///
    /// # Errors
    /// [`ScrollbackError`] when stored data fails validation or the spill store fails; the line
    /// is then not removed.
    pub fn pop_front<'c>(
        &mut self,
        cursor: &'c mut LineCursor,
    ) -> Result<Option<&'c DecodedLine>, ScrollbackError> {
        if self.read_line(0, cursor)?.is_none() {
            return Ok(None);
        }
        self.drop_front_line();
        Ok(Some(&cursor.line))
    }

    fn locate(&self, index: usize) -> Option<Location> {
        if index >= self.retained {
            return None;
        }
        let skipped = if self.chunks.is_empty() { self.hot_skip } else { self.head_skip };
        let mut offset = index + skipped;
        for (chunk, sealed) in self.chunks.iter().enumerate() {
            if offset < sealed.lines {
                return Some(Location::Sealed { chunk, line: offset });
            }
            offset -= sealed.lines;
        }
        (offset < self.hot.len()).then_some(Location::Hot(offset))
    }

    fn load_chunk(&self, index: usize, cursor: &mut LineCursor) -> Result<(), ScrollbackError> {
        let sealed = self.chunks.get(index).ok_or(DecodeError::BadIndex)?;
        if cursor.chunk.id == Some(sealed.id) {
            return Ok(());
        }
        match &sealed.place {
            Place::Ram(blob) => decode_chunk(blob, &mut cursor.chunk)?,
            Place::Spilled(handle) => {
                let store = self.store.as_ref().ok_or(ScrollbackError::NoSpillStore)?;
                store.read(*handle, &mut cursor.spill)?;
                decode_chunk(&cursor.spill, &mut cursor.chunk)?;
            }
        }
        cursor.chunk.id = Some(sealed.id);
        Ok(())
    }

    fn check_styles(&self, line: &DecodedLine) -> Result<(), DecodeError> {
        match line.runs.iter().find(|run| self.styles.get(run.style).is_none()) {
            Some(run) => Err(DecodeError::UnknownStyle(run.style.get())),
            None => Ok(()),
        }
    }
}
