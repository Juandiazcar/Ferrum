//! Pending chunks: a full hot chunk waiting, uncompressed and immutable, for the thread that
//! compresses it (`docs/02` §5: "compression of sealed chunks" runs on the daemon worker, which
//! receives immutable chunk handles).

use std::sync::Arc;

use super::HotChunk;

/// A full hot chunk handed out for compression by
/// [`Scrollback::take_pending`](crate::Scrollback::take_pending).
///
/// An immutable handle (`Send`, no copy of the lines): the scrollback keeps reading the same
/// lines while the chunk is being compressed. Give it to [`ChunkSealer::seal`](crate::ChunkSealer::seal)
/// on any thread and the result back to [`Scrollback::install`](crate::Scrollback::install) of
/// the scrollback it came from.
#[derive(Debug)]
pub struct PendingChunk {
    /// The scrollback it came from (unique per process).
    pub(crate) origin: u64,
    pub(crate) id: u64,
    pub(crate) lines: Arc<HotChunk>,
}

impl PendingChunk {
    /// Lines in the chunk.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Bytes of the encoded lines (the input of the compression).
    pub fn raw_bytes(&self) -> usize {
        self.lines.data().len()
    }
}

/// The compressed form of a [`PendingChunk`], to install in the scrollback it came from.
#[derive(Debug)]
pub struct SealedBlob {
    /// The scrollback it came from (unique per process).
    pub(crate) origin: u64,
    pub(crate) id: u64,
    pub(crate) blob: Box<[u8]>,
}

impl SealedBlob {
    /// Bytes of the sealed blob (what the chunk costs in RAM once installed).
    pub fn len(&self) -> usize {
        self.blob.len()
    }

    /// Always false: a sealed blob holds at least its header.
    pub fn is_empty(&self) -> bool {
        self.blob.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkSealer;

    fn assert_send<T: Send>() {}

    #[test]
    fn the_handles_and_the_sealer_can_move_to_another_thread() {
        assert_send::<PendingChunk>();
        assert_send::<SealedBlob>();
        assert_send::<ChunkSealer>();
    }
}
