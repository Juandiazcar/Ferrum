//! The hot chunk: the newest lines, uncompressed, with bounded buffer growth.

use crate::encode::{Encoder, Planned};
use crate::limits::{HOT_CHUNK_LINES, HOT_GROWTH_STEP_BYTES, POOLED_BUFFER_KEEP_BYTES};

/// Empties `buffer`, and frees it when its capacity is above [`POOLED_BUFFER_KEEP_BYTES`].
pub(crate) fn free_if_above_keep(buffer: &mut Vec<u8>) {
    if buffer.capacity() > POOLED_BUFFER_KEEP_BYTES {
        *buffer = Vec::new();
    } else {
        buffer.clear();
    }
}

/// Up to [`HOT_CHUNK_LINES`] encoded lines back to back plus the start offset of each.
#[derive(Debug)]
pub(crate) struct HotChunk {
    data: Vec<u8>,
    starts: Vec<u32>,
}

impl HotChunk {
    pub(crate) fn new() -> Self {
        // The offset index is allocated once at its final size: 1 024 x 4 B = 4 KiB (ADR 0024).
        Self { data: Vec::new(), starts: Vec::with_capacity(HOT_CHUNK_LINES) }
    }

    pub(crate) fn len(&self) -> usize {
        self.starts.len()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.starts.len() >= HOT_CHUNK_LINES
    }

    pub(crate) fn data(&self) -> &[u8] {
        &self.data
    }

    /// Start offset of every line, in order.
    pub(crate) fn starts(&self) -> &[u32] {
        &self.starts
    }

    /// Appends a planned line. The buffer grows by exact steps, never by doubling (ADR 0024).
    pub(crate) fn append(&mut self, encoder: &Encoder, planned: &Planned<'_>) {
        let needed = encoder.max_len(planned);
        let free = self.data.capacity() - self.data.len();
        if free < needed {
            self.data.reserve_exact(HOT_GROWTH_STEP_BYTES.max(needed));
        }
        // INVARIANT: data holds at most 1 024 lines of at most 64 KiB, below u32::MAX.
        self.starts.push(u32::try_from(self.data.len()).unwrap_or(u32::MAX));
        encoder.write(planned, &mut self.data);
    }

    /// The encoded bytes of line `index`.
    pub(crate) fn line(&self, index: usize) -> Option<&[u8]> {
        let start = usize::try_from(*self.starts.get(index)?).ok()?;
        let end = match self.starts.get(index + 1) {
            Some(&next) => usize::try_from(next).ok()?,
            None => self.data.len(),
        };
        self.data.get(start..end)
    }

    /// Forgets all lines but keeps the buffers (steady state allocates nothing), except a data
    /// buffer above [`POOLED_BUFFER_KEEP_BYTES`], which is freed (`docs/03` §3 rule 7).
    pub(crate) fn clear(&mut self) {
        free_if_above_keep(&mut self.data);
        self.starts.clear();
    }

    pub(crate) fn data_capacity(&self) -> usize {
        self.data.capacity()
    }

    pub(crate) fn index_capacity_bytes(&self) -> usize {
        self.starts.capacity() * size_of::<u32>()
    }

    /// Heap bytes of both buffers.
    pub(crate) fn capacity_bytes(&self) -> usize {
        self.data_capacity() + self.index_capacity_bytes()
    }
}
