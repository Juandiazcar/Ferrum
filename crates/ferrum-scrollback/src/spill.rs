//! The spill seam: cold sealed chunks leave RAM through a [`SpillStore`] (`docs/04` §2,
//! `docs/05` §2 dependency inversion). The daemon wires [`FileSpill`]; tests wire
//! [`MemorySpill`]. No mmap (ADR 0027).
//!
//! # Contract (every implementation, checked by the shared suite in `ferrum-testkit`)
//!
//! - `append` stores a record and returns a handle; it fails with [`SpillError::Full`] (and stores
//!   nothing) when `live_bytes() + record.len()` would exceed [`capacity_bytes`](SpillStore::capacity_bytes).
//! - `read` replaces the contents of `out` with the record; a handle that is discarded or was
//!   never issued gives [`SpillError::UnknownHandle`].
//! - Records are discarded strictly oldest first (`discard_oldest`), which is how the scrollback
//!   evicts. Space of discarded records is reusable.
//! - Empty records are allowed.

mod file;
mod memory;
mod positional;

pub use file::FileSpill;
pub use memory::MemorySpill;

use crate::error::SpillError;

/// Names one record of a [`SpillStore`]. Opaque; valid until the record is discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillHandle {
    pub(crate) seq: u64,
    pub(crate) offset: u64,
    pub(crate) len: u64,
}

impl SpillHandle {
    /// Size of the record this handle names.
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// True for a zero-length record.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Where cold chunks go. Implementations are owned by one session and used from one thread at a
/// time (`Send`, not `Sync`).
pub trait SpillStore: Send + std::fmt::Debug {
    /// Stores `record` and returns its handle.
    ///
    /// # Errors
    /// [`SpillError::Full`] when it does not fit, [`SpillError::Io`] on I/O failure.
    fn append(&mut self, record: &[u8]) -> Result<SpillHandle, SpillError>;

    /// Reads the record into `out` (replacing its contents).
    ///
    /// # Errors
    /// [`SpillError::UnknownHandle`] or [`SpillError::Io`].
    fn read(&self, handle: SpillHandle, out: &mut Vec<u8>) -> Result<(), SpillError>;

    /// Discards the oldest live record, freeing its space. A no-op when the store is empty.
    ///
    /// # Errors
    /// [`SpillError::Io`] when the store cannot update its bookkeeping.
    fn discard_oldest(&mut self) -> Result<(), SpillError>;

    /// Bytes of live records.
    fn live_bytes(&self) -> u64;

    /// Most live bytes the store accepts.
    fn capacity_bytes(&self) -> u64;

    /// A read-only handle on the records live now, for a reader on another thread; `None` when
    /// the store has nothing to read or cannot share it.
    fn reader(&self) -> Option<Box<dyn SpillReader>> {
        None
    }
}

/// A read-only handle on a spill store's records, usable from another thread while the store
/// goes on (a history snapshot reads its spilled chunks through it).
pub trait SpillReader: Send + Sync + std::fmt::Debug {
    /// Reads the record into `out`, as [`SpillStore::read`] does. A record that the store has
    /// discarded since the handle was taken may read as other bytes; the chunk decoder rejects
    /// them.
    ///
    /// # Errors
    /// [`SpillError::UnknownHandle`] or [`SpillError::Io`].
    fn read(&self, handle: SpillHandle, out: &mut Vec<u8>) -> Result<(), SpillError>;
}

/// Shared capacity check for implementations.
pub(crate) fn check_room(store: &dyn SpillStore, len: u64) -> Result<(), SpillError> {
    let free = store.capacity_bytes().saturating_sub(store.live_bytes());
    if len > free { Err(SpillError::Full { needed: len, free }) } else { Ok(()) }
}
