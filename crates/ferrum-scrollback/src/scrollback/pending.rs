//! Pending chunks: a full hot chunk waits, uncompressed, until the worker seals it (`docs/02`
//! §5). The crate spawns no thread: the owner hands chunks out with [`Scrollback::take_pending`]
//! and installs the result with [`Scrollback::install`].

use std::sync::Arc;

use super::{Handed, Place, Scrollback, SealedChunk};
use crate::chunk::{HotChunk, PendingChunk, SealedBlob};
use crate::error::ScrollbackError;
use crate::limits::MAX_PENDING_CHUNKS;

impl Scrollback {
    /// The oldest full chunk that waits for compression and was not handed out yet, as an
    /// immutable handle for [`ChunkSealer::seal`](crate::ChunkSealer::seal) on another thread.
    /// Each chunk is handed out once. The chunk stays readable here until it is installed.
    pub fn take_pending(&mut self) -> Option<PendingChunk> {
        self.tidy_pool();
        self.chunks.iter_mut().skip(self.spilled_chunks).find_map(|chunk| match &mut chunk.place {
            Place::Pending { lines, handed: handed @ Handed::No } => {
                *handed = Handed::Yes;
                Some(PendingChunk { origin: self.origin, id: chunk.id, lines: Arc::clone(lines) })
            }
            _ => None,
        })
    }

    /// Puts a blob sealed by [`ChunkSealer::seal`](crate::ChunkSealer::seal) in place of its
    /// pending chunk, which frees the raw lines (their buffer goes back to the pool). A blob whose
    /// chunk is gone (evicted, cleared, or already sealed inline for spilling) is dropped.
    ///
    /// # Errors
    /// [`ScrollbackError::ForeignBlob`] when the blob came from another scrollback (a wiring bug
    /// of the owner); it is dropped and this scrollback is unchanged.
    pub fn install(&mut self, sealed: SealedBlob) -> Result<(), ScrollbackError> {
        self.tidy_pool();
        if sealed.origin != self.origin {
            tracing::warn!(chunk = sealed.id, "sealed chunk from another scrollback dropped");
            return Err(ScrollbackError::ForeignBlob);
        }
        let entry = self.chunks.iter_mut().skip(self.spilled_chunks).find(|c| c.id == sealed.id);
        let Some(entry) = entry.filter(|c| matches!(c.place, Place::Pending { .. })) else {
            return Ok(());
        };
        let old = std::mem::replace(&mut entry.place, Place::Ram(sealed.blob));
        self.release(old);
        self.enforce_ram_limit();
        Ok(())
    }

    /// Turns the full hot chunk into a pending chunk and starts a new one from the pool. `None`
    /// when `MAX_PENDING_CHUNKS` are already waiting, or when every chunk buffer is still out
    /// with the worker (the caller seals inline).
    pub(super) fn defer_hot(&mut self) -> Option<Place> {
        if self.pending_chunks >= MAX_PENDING_CHUNKS {
            return None;
        }
        let mut shell = self.take_spare()?;
        let next = Arc::get_mut(&mut shell)?;
        std::mem::swap(next, &mut self.hot);
        self.pending_chunks += 1;
        Some(Place::Pending { lines: shell, handed: Handed::No })
    }

    /// An emptied, unshared chunk buffer: a pooled one the worker no longer holds, else a new one
    /// while pending and pooled buffers are fewer than `MAX_PENDING_CHUNKS` (warm-up). `None`
    /// when every buffer is still out with the worker.
    fn take_spare(&mut self) -> Option<Arc<HotChunk>> {
        let free = self.pool.iter_mut().position(|lines| Arc::get_mut(lines).is_some());
        if let Some(mut lines) = free.map(|index| self.pool.swap_remove(index)) {
            if let Some(buffer) = Arc::get_mut(&mut lines) {
                buffer.clear();
            }
            return Some(lines);
        }
        (self.pool.len() + self.pending_chunks < MAX_PENDING_CHUNKS)
            .then(|| Arc::new(HotChunk::new()))
    }

    /// The full hot chunk as a raw pending chunk beyond the cap, with a new buffer as the next hot
    /// chunk: only when a seal failed, so that no line is lost (allocates; never in steady state).
    pub(super) fn keep_hot_raw(&mut self) -> Place {
        let full = std::mem::replace(&mut self.hot, HotChunk::new());
        self.pending_chunks += 1;
        Place::Pending { lines: Arc::new(full), handed: Handed::No }
    }

    /// Empties, and frees when it grew past `POOLED_BUFFER_KEEP_BYTES`, every pooled buffer the
    /// worker has let go of since it was released while handed out, so a huge one is not kept
    /// until the next hot chunk fills (`docs/03` §3 rule 7). Clearing an empty buffer is free.
    fn tidy_pool(&mut self) {
        for lines in &mut self.pool {
            if let Some(buffer) = Arc::get_mut(lines) {
                buffer.clear();
            }
        }
    }

    /// Releases what a chunk held. Its buffer goes back to the pool for the next hot chunk, even
    /// when the worker still holds it (evicted or cleared while handed out): it is reused once
    /// the worker drops its handle, so no new buffer is allocated meanwhile.
    pub(super) fn release(&mut self, place: Place) {
        let Place::Pending { mut lines, .. } = place else { return };
        self.pending_chunks = self.pending_chunks.saturating_sub(1);
        if let Some(buffer) = Arc::get_mut(&mut lines) {
            buffer.clear();
        }
        if self.pool.len() < MAX_PENDING_CHUNKS {
            self.pool.push(lines);
        }
    }

    /// Seals chunk `index` on this thread if it is pending (the spill frontier reached it before
    /// the worker did). Counted as an inline seal.
    pub(super) fn seal_in_place(&mut self, index: usize) -> Result<(), ScrollbackError> {
        let Some(SealedChunk { place: Place::Pending { lines, .. }, .. }) = self.chunks.get(index)
        else {
            return Ok(());
        };
        let blob = self.sealer.seal_hot(lines)?;
        self.inline_seals += 1;
        if let Some(entry) = self.chunks.get_mut(index) {
            let old = std::mem::replace(&mut entry.place, Place::Ram(blob));
            self.release(old);
        }
        Ok(())
    }
}
