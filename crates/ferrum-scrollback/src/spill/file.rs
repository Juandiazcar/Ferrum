//! The spill file: a FIFO ring of records over positional reads and writes (ADR 0027).
//!
//! Records are placed back to back at monotonically growing *logical* offsets; the physical
//! position is `logical % capacity`, so a record may wrap around the end of the file and the file
//! never exceeds `capacity` bytes however long the session runs. Because scrollback eviction is
//! FIFO, the live bytes are always the contiguous logical range `[oldest, tail)`.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use super::positional::{read_exact_at, write_all_at};
use super::{SpillHandle, SpillStore, check_room};
use crate::error::SpillError;

#[derive(Debug, Clone, Copy)]
struct Record {
    seq: u64,
    offset: u64,
    len: u64,
}

/// A [`SpillStore`] backed by one file, created on the first append (`docs/03` §3 rule 11),
/// owner-only on Unix, removed when dropped.
#[derive(Debug)]
pub struct FileSpill {
    path: PathBuf,
    file: Option<File>,
    capacity: u64,
    records: VecDeque<Record>,
    next_seq: u64,
    tail: u64,
}

impl FileSpill {
    /// A store writing to `path` (not created yet) and holding at most `capacity_bytes`
    /// (`DEFAULT_SPILL_CAP_BYTES` unless configured otherwise). The caller picks a path in the
    /// owner-only run directory (`docs/10`); an existing file at `path` is an error on first append.
    pub fn new(path: impl AsRef<Path>, capacity_bytes: u64) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            file: None,
            capacity: capacity_bytes,
            records: VecDeque::new(),
            next_seq: 0,
            tail: 0,
        }
    }

    /// True once the file exists (some record was appended).
    pub fn is_created(&self) -> bool {
        self.file.is_some()
    }

    fn open(&mut self) -> Result<&File, SpillError> {
        if self.file.is_none() {
            self.file = Some(create_owner_only(&self.path)?);
        }
        self.file.as_ref().ok_or(SpillError::UnknownHandle)
    }

    fn head(&self) -> u64 {
        self.records.front().map_or(self.tail, |record| record.offset)
    }

    fn find(&self, handle: SpillHandle) -> Result<Record, SpillError> {
        let first = self.records.front().ok_or(SpillError::UnknownHandle)?;
        let index = handle.seq.checked_sub(first.seq).ok_or(SpillError::UnknownHandle)?;
        let index = usize::try_from(index).map_err(|_| SpillError::UnknownHandle)?;
        let record = *self.records.get(index).ok_or(SpillError::UnknownHandle)?;
        if record.offset != handle.offset || record.len != handle.len {
            return Err(SpillError::UnknownHandle);
        }
        Ok(record)
    }
}

impl SpillStore for FileSpill {
    fn append(&mut self, record: &[u8]) -> Result<SpillHandle, SpillError> {
        let len = u64::try_from(record.len()).unwrap_or(u64::MAX);
        check_room(self, len)?;
        let (capacity, offset) = (self.capacity, self.tail);
        write_ring(self.open()?, capacity, offset, record)?;
        let seq = self.next_seq;
        self.next_seq += 1;
        self.tail += len;
        self.records.push_back(Record { seq, offset, len });
        Ok(SpillHandle { seq, offset, len })
    }

    fn read(&self, handle: SpillHandle, out: &mut Vec<u8>) -> Result<(), SpillError> {
        let record = self.find(handle)?;
        let file = self.file.as_ref().ok_or(SpillError::UnknownHandle)?;
        let len = usize::try_from(record.len).map_err(|_| SpillError::UnknownHandle)?;
        out.clear();
        out.resize(len, 0);
        read_ring(file, self.capacity, record.offset, out)?;
        Ok(())
    }

    fn discard_oldest(&mut self) -> Result<(), SpillError> {
        self.records.pop_front();
        Ok(())
    }

    fn live_bytes(&self) -> u64 {
        self.tail.saturating_sub(self.head())
    }

    fn capacity_bytes(&self) -> u64 {
        self.capacity
    }
}

impl Drop for FileSpill {
    fn drop(&mut self) {
        if self.file.take().is_some() {
            // Best effort: the run directory is cleaned by the daemon at startup as well.
            std::fs::remove_file(&self.path).ok();
        }
    }
}

fn create_owner_only(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// Splits a ring access at the physical end of the file: `(physical offset, length)` pieces.
fn pieces(capacity: u64, logical: u64, len: usize) -> [(u64, usize); 2] {
    let physical = logical % capacity.max(1);
    let until_end = usize::try_from(capacity - physical).unwrap_or(usize::MAX);
    let first = len.min(until_end);
    [(physical, first), (0, len - first)]
}

fn write_ring(file: &File, capacity: u64, logical: u64, bytes: &[u8]) -> std::io::Result<()> {
    let mut rest = bytes;
    for (physical, len) in pieces(capacity, logical, bytes.len()) {
        let (head, tail) = rest.split_at(len);
        write_all_at(file, head, physical)?;
        rest = tail;
    }
    Ok(())
}

fn read_ring(file: &File, capacity: u64, logical: u64, out: &mut [u8]) -> std::io::Result<()> {
    let mut rest = out;
    for (physical, len) in pieces(capacity, logical, rest.len()) {
        let (head, tail) = std::mem::take(&mut rest).split_at_mut(len);
        read_exact_at(file, head, physical)?;
        rest = tail;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_split_at_the_end_of_the_ring() {
        assert_eq!(pieces(100, 90, 20), [(90, 10), (0, 10)]);
        assert_eq!(pieces(100, 210, 5), [(10, 5), (0, 0)]);
    }
}
