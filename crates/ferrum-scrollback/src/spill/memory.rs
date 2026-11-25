//! In-memory [`SpillStore`] for tests (`docs/08` §2 contract suites), with failure injection.

use std::collections::VecDeque;
use std::io;
use std::sync::Arc;

use super::{SpillHandle, SpillReader, SpillStore, check_room};
use crate::error::SpillError;

/// Keeps records in RAM. Not for production: it exists so the scrollback spill logic is tested
/// without touching the disk, including disk-full behavior ([`fail_appends_after`](Self::fail_appends_after)).
#[derive(Debug)]
pub struct MemorySpill {
    records: VecDeque<(u64, Arc<[u8]>)>,
    next_seq: u64,
    live: u64,
    capacity: u64,
    appends_before_failure: Option<usize>,
}

impl MemorySpill {
    /// A store accepting at most `capacity_bytes` of live records.
    pub fn new(capacity_bytes: u64) -> Self {
        Self {
            records: VecDeque::new(),
            next_seq: 0,
            live: 0,
            capacity: capacity_bytes,
            appends_before_failure: None,
        }
    }

    /// After `successful` more appends every append fails with an I/O error (a full disk).
    pub fn fail_appends_after(&mut self, successful: usize) {
        self.appends_before_failure = Some(successful);
    }

    /// Number of live records.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }
}

impl SpillStore for MemorySpill {
    fn append(&mut self, record: &[u8]) -> Result<SpillHandle, SpillError> {
        match self.appends_before_failure {
            Some(0) => return Err(io::Error::other("injected: no space left on device").into()),
            Some(left) => self.appends_before_failure = Some(left - 1),
            None => {}
        }
        let len = u64::try_from(record.len()).unwrap_or(u64::MAX);
        check_room(self, len)?;
        let seq = self.next_seq;
        self.next_seq += 1;
        self.live += len;
        self.records.push_back((seq, Arc::from(record)));
        Ok(SpillHandle { seq, offset: 0, len })
    }

    fn read(&self, handle: SpillHandle, out: &mut Vec<u8>) -> Result<(), SpillError> {
        let first = self.records.front().map(|(seq, _)| *seq).ok_or(SpillError::UnknownHandle)?;
        let index = handle.seq.checked_sub(first).ok_or(SpillError::UnknownHandle)?;
        let index = usize::try_from(index).map_err(|_| SpillError::UnknownHandle)?;
        let (_, bytes) = self.records.get(index).ok_or(SpillError::UnknownHandle)?;
        out.clear();
        out.extend_from_slice(bytes);
        Ok(())
    }

    fn discard_oldest(&mut self) -> Result<(), SpillError> {
        if let Some((_, bytes)) = self.records.pop_front() {
            self.live -= u64::try_from(bytes.len()).unwrap_or(0);
        }
        Ok(())
    }

    fn live_bytes(&self) -> u64 {
        self.live
    }

    fn capacity_bytes(&self) -> u64 {
        self.capacity
    }

    fn reader(&self) -> Option<Box<dyn SpillReader>> {
        Some(Box::new(MemoryReader { records: self.records.iter().cloned().collect() }))
    }
}

/// The records of a [`MemorySpill`] at one moment, shared.
#[derive(Debug)]
struct MemoryReader {
    records: Vec<(u64, Arc<[u8]>)>,
}

impl SpillReader for MemoryReader {
    fn read(&self, handle: SpillHandle, out: &mut Vec<u8>) -> Result<(), SpillError> {
        let (_, bytes) = self
            .records
            .iter()
            .find(|(seq, _)| *seq == handle.seq)
            .ok_or(SpillError::UnknownHandle)?;
        out.clear();
        out.extend_from_slice(bytes);
        Ok(())
    }
}
