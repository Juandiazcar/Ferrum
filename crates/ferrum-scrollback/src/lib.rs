//! `ferrum-scrollback`: compact line encoding, style interning, chunked `LZ4` compression and disk
//! spill behind a trait (`docs/02` §3.2).
//!
//! # Model
//!
//! A row that scrolls off the grid is pushed with [`Scrollback::push_line`] as
//! `(flags, text, style runs)`. It is encoded as
//! `flags:u8 | text_len | utf8 text | run_count | {style_id | run_len}*` (varints; trailing
//! default blanks trimmed; styles interned per session in a [`StyleTable`]) and appended to the
//! hot chunk of at most [`HOT_CHUNK_LINES`] lines. A full hot chunk becomes a [`PendingChunk`]
//! (no copy; a pooled buffer becomes the next hot chunk) that the owner hands to a worker with
//! [`Scrollback::take_pending`]; the worker's [`ChunkSealer`] compresses it into an LZ4 blob with
//! a delta-encoded line index, and [`Scrollback::install`] puts the blob in its place. At most
//! [`MAX_PENDING_CHUNKS`] wait; beyond that the push path seals inline (backpressure, counted in
//! [`MemoryReport::inline_seals`]). The crate spawns no thread. Beyond the spill threshold the oldest sealed chunks go to a
//! [`SpillStore`] (no mmap, ADR 0027). Lines are read back by index
//! ([`Scrollback::read_line`]) or iterated by [`Scrollback::search`].
//!
//! Wide characters and combining sequences are stored as scalars; the consumer derives column
//! widths with its width table. Reflow is `ferrum-vt` (WP-02) and is not part of this crate.
//!
//! Allowed internal dependencies: none (foundation).
//! Must never depend on: any other ferrum crate, OS-specific code (the spill file only uses
//! positional I/O from `std`, ADR 0027).

#![forbid(unsafe_code)]

mod chunk;
mod config;
mod decode;
mod encode;
mod error;
mod limits;
mod line;
mod memory;
mod scrollback;
mod search;
mod spill;
mod style;
mod varint;

pub use chunk::{ChunkBuf, ChunkSealer, PendingChunk, SealedBlob, decode_chunk};
pub use config::ScrollbackConfig;
pub use error::{DecodeError, ScrollbackError, SpillError};
pub use limits::{
    DEFAULT_MAX_LINES, DEFAULT_SPILL_CAP_BYTES, DEFAULT_SPILL_THRESHOLD_LINES, HOT_CHUNK_LINES,
    HOT_GROWTH_STEP_BYTES, MAX_ENCODED_LINE_BYTES, MAX_LINE_TEXT_BYTES, MAX_LINES,
    MAX_PENDING_CHUNKS, MAX_STYLES, POOLED_BUFFER_KEEP_BYTES,
};
pub use line::{DecodedLine, LineFlags, LineRef, Run, StyledRun};
pub use memory::MemoryReport;
pub use scrollback::{
    HistorySnapshot, LineCursor, Scrollback, ScrollbackEvent, SpillDisableCause, SpillState,
};
pub use search::{CaseMode, Direction, LineMatch, LineMatcher, LiteralMatcher, Search};
pub use spill::{FileSpill, MemorySpill, SpillHandle, SpillReader, SpillStore};
pub use style::{Style, StyleId, StyleTable};
