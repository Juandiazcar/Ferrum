//! [`Scrollback`]: the per-session store of encoded history (`docs/02` §3.2).
//!
//! Lines are numbered from 0 = the oldest retained line. Layout, oldest to newest: spilled
//! chunks (on the spill store), sealed chunks (RAM, LZ4) and pending chunks (RAM, raw, waiting
//! for the worker; at most [`MAX_PENDING_CHUNKS`](crate::MAX_PENDING_CHUNKS)), the hot chunk
//! (RAM, raw). History is
//! bounded: past `max_lines` the oldest lines are dropped; past the spill threshold the oldest
//! sealed chunks move to the spill store; if the store is full the oldest spilled chunks are
//! dropped, and if it fails the spill is disabled for the session (`docs/02` §10).

mod access;
mod pending;
mod retention;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub use access::LineCursor;

use crate::chunk::{ChunkSealer, HotChunk};
use crate::config::ScrollbackConfig;
use crate::encode::Encoder;
use crate::error::ScrollbackError;
use crate::limits::MAX_LINE_TEXT_BYTES;
use crate::line::LineRef;
use crate::memory::MemoryReport;
use crate::spill::{SpillHandle, SpillStore};
use crate::style::StyleTable;

/// The origin token of the next scrollback: chunk ids restart at 0 in every scrollback, so a
/// sealed blob also carries the token of the scrollback it came from.
static NEXT_ORIGIN: AtomicU64 = AtomicU64::new(0);

/// Whether cold chunks can leave RAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpillState {
    /// No store was attached: history is capped at the spill threshold.
    Unavailable,
    /// Cold chunks spill to the store.
    Active,
    /// The store failed; history is capped at the spill threshold (`docs/02` §10).
    Disabled,
}

/// Why spilling was disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpillDisableCause {
    /// The store reported an I/O error (for example a full disk).
    Io,
    /// One chunk is larger than the whole store capacity.
    RecordTooLarge,
    /// A chunk could not be compressed before spilling (LZ4 failed).
    Compress,
}

/// Something the owner should tell the user (once).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollbackEvent {
    /// Spilling stopped; the oldest lines are dropped from now on.
    SpillDisabled(SpillDisableCause),
}

#[derive(Debug)]
enum Place {
    Ram(Box<[u8]>),
    Spilled(SpillHandle),
    /// Full and uncompressed, waiting for [`Scrollback::install`]; `handed` once given out.
    Pending {
        lines: Arc<HotChunk>,
        handed: Handed,
    },
}

/// Whether a pending chunk was given out by [`Scrollback::take_pending`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handed {
    No,
    Yes,
}

#[derive(Debug)]
struct SealedChunk {
    id: u64,
    lines: usize,
    place: Place,
}

/// The scrollback of one session.
#[derive(Debug)]
pub struct Scrollback {
    /// Unique per process; matches [`SealedBlob`](crate::SealedBlob)s to their scrollback.
    origin: u64,
    config: ScrollbackConfig,
    styles: StyleTable,
    encoder: Encoder,
    hot: HotChunk,
    sealer: ChunkSealer,
    chunks: VecDeque<SealedChunk>,
    /// Chunk buffers for the next hot chunk (with the pending chunks, at most
    /// `MAX_PENDING_CHUNKS`), so a pending seal allocates nothing in steady state. A buffer may
    /// still be shared with the worker (its chunk was dropped while handed out) until the worker
    /// drops its handle.
    pool: Vec<Arc<HotChunk>>,
    pending_chunks: usize,
    inline_seals: u64,
    /// Lines of the oldest sealed chunk already dropped (the chunk is freed when all are).
    head_skip: usize,
    /// Lines of the hot chunk already dropped (only while there is no sealed chunk).
    hot_skip: usize,
    next_chunk_id: u64,
    retained: usize,
    ram_sealed_lines: usize,
    spilled_chunks: usize,
    store: Option<Box<dyn SpillStore>>,
    spill: SpillState,
    event: Option<ScrollbackEvent>,
    pushed_lines: u64,
    dropped_lines: u64,
}

impl Scrollback {
    /// A scrollback without a spill store: history is capped at the spill threshold.
    pub fn new(config: ScrollbackConfig) -> Self {
        Self::build(config, None)
    }

    /// A scrollback that spills cold chunks to `store` (wired by the composition root).
    pub fn with_spill(config: ScrollbackConfig, store: Box<dyn SpillStore>) -> Self {
        Self::build(config, Some(store))
    }

    fn build(config: ScrollbackConfig, store: Option<Box<dyn SpillStore>>) -> Self {
        let spill = if store.is_some() { SpillState::Active } else { SpillState::Unavailable };
        Self {
            // Only uniqueness matters, no ordering with other memory.
            origin: NEXT_ORIGIN.fetch_add(1, Ordering::Relaxed),
            config,
            styles: StyleTable::new(),
            encoder: Encoder::default(),
            hot: HotChunk::new(),
            sealer: ChunkSealer::default(),
            chunks: VecDeque::new(),
            pool: Vec::new(),
            pending_chunks: 0,
            inline_seals: 0,
            head_skip: 0,
            hot_skip: 0,
            next_chunk_id: 0,
            retained: 0,
            ram_sealed_lines: 0,
            spilled_chunks: 0,
            store,
            spill,
            event: None,
            pushed_lines: 0,
            dropped_lines: 0,
        }
    }

    /// Appends a row that scrolled off the grid. Hot path: allocates only when a new style is
    /// seen and while the hot and pooled buffers grow to their working size. A full hot chunk
    /// becomes a pending chunk (see [`take_pending`](Self::take_pending)); only when
    /// [`MAX_PENDING_CHUNKS`](crate::MAX_PENDING_CHUNKS) are already waiting is it sealed here,
    /// which allocates the sealed blob (the retained data itself).
    ///
    /// # Errors
    /// A line over the size limits, or with runs that do not cover its text, is still stored and
    /// counted, so that line numbers stay stable: as its text cut to
    /// [`MAX_LINE_TEXT_BYTES`](crate::MAX_LINE_TEXT_BYTES) at a character boundary, without
    /// styles; the error says what was lost. A compression failure is returned too, but the line
    /// is stored and counted all the same: the chunk that could not be sealed stays readable,
    /// uncompressed.
    pub fn push_line(&mut self, line: &LineRef<'_>) -> Result<(), ScrollbackError> {
        if self.config.max_lines() == 0 {
            self.pushed_lines += 1;
            return Ok(());
        }
        let before = if self.hot.is_full() { self.seal_hot() } else { Ok(()) };
        let stored = self.append(line);
        self.pushed_lines += 1;
        self.retained += 1;
        let after = if self.hot.is_full() { self.seal_hot() } else { Ok(()) };
        self.evict();
        before.and(stored).and(after)
    }

    /// Appends `line` to the hot chunk, or its clipped plain text when it cannot be stored as is.
    fn append(&mut self, line: &LineRef<'_>) -> Result<(), ScrollbackError> {
        let error = match self.encoder.plan(&mut self.styles, line) {
            Ok(planned) => {
                self.hot.append(&self.encoder, &planned);
                return Ok(());
            }
            Err(error) => error,
        };
        let text = line.text.get(..line.text.floor_char_boundary(MAX_LINE_TEXT_BYTES));
        let plain = LineRef { flags: line.flags, text: text.unwrap_or_default(), runs: &[] };
        // A text within the limit and no runs always plans.
        let planned = self.encoder.plan(&mut self.styles, &plain)?;
        self.hot.append(&self.encoder, &planned);
        Err(error)
    }

    /// Number of retained lines.
    pub fn len(&self) -> usize {
        self.retained
    }

    /// True when no line is retained.
    pub fn is_empty(&self) -> bool {
        self.retained == 0
    }

    /// Lines ever pushed.
    pub fn pushed_lines(&self) -> u64 {
        self.pushed_lines
    }

    /// Lines dropped because of the limits (not counting `max_lines == 0`).
    pub fn dropped_lines(&self) -> u64 {
        self.dropped_lines
    }

    /// The session style table.
    pub fn styles(&self) -> &StyleTable {
        &self.styles
    }

    /// The configuration.
    pub fn config(&self) -> &ScrollbackConfig {
        &self.config
    }

    /// Whether cold chunks can leave RAM.
    pub fn spill_state(&self) -> SpillState {
        self.spill
    }

    /// Chunks currently on the spill store.
    pub fn spilled_chunks(&self) -> usize {
        self.spilled_chunks
    }

    /// Takes the pending notification, if any (each event is reported once).
    pub fn take_event(&mut self) -> Option<ScrollbackEvent> {
        self.event.take()
    }

    /// Heap bytes by category (capacities).
    pub fn memory_report(&self) -> MemoryReport {
        let (mut blobs, mut pending) = (0, 0);
        for chunk in &self.chunks {
            match &chunk.place {
                Place::Ram(blob) => blobs += blob.len(),
                Place::Pending { lines, .. } => pending += pooled_bytes(lines),
                Place::Spilled(_) => {}
            }
        }
        let pooled: usize = self.pool.iter().map(|lines| pooled_bytes(lines)).sum();
        MemoryReport {
            hot_chunk_bytes: self.hot.capacity_bytes(),
            sealed_bytes: blobs + self.chunks.capacity() * size_of::<SealedChunk>(),
            pending_bytes: pending + pooled + self.pool.capacity() * size_of::<Arc<HotChunk>>(),
            inline_seals: self.inline_seals,
            misc_bytes: self.styles.capacity_bytes() + self.encoder.capacity_bytes(),
            shared_scratch_bytes: self.sealer.capacity_bytes(),
            spill_disk_bytes: self.store.as_ref().map_or(0, |store| store.live_bytes()),
        }
    }
}

/// Heap bytes of a shared chunk buffer: its two buffers and the shared allocation that holds the
/// `HotChunk` (the reference counts are within the reconciliation slack of `memory_budget.rs`).
fn pooled_bytes(lines: &HotChunk) -> usize {
    lines.capacity_bytes() + size_of::<HotChunk>()
}
