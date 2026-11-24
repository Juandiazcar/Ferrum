//! Named limits of the format and of the store (`docs/05` §1 rule 9: no magic numbers).

/// Lines in the hot chunk before it is sealed (`docs/02` §3.2: "the newest <= 1 024 lines").
pub const HOT_CHUNK_LINES: usize = 1_024;

/// Most full chunks that may wait, uncompressed, for the worker to seal them (`docs/02` §5).
///
/// Two (maintainer decision Q-26, amended 2026-11-23): the pending chunks and the pooled buffers
/// together are at most two full chunk buffers, 0.172 MB measured and 0.18 MB by allowance (the
/// "per session pending chunks" line of `docs/03` §2, whose extra 0.09 MB comes from the sealed
/// chunks line), so a session stays within its 1.0 MB allowance. When both are taken, the push
/// path seals inline (backpressure, `MemoryReport::inline_seals`). With one, half of the chunks
/// of `vt_parse/ascii_8mib` were sealed inline on an Apple M2 (blobs come back only between
/// reads); the measurements with two are in ADR 0028.
pub const MAX_PENDING_CHUNKS: usize = 2;

/// The hot chunk buffer grows by exactly this many bytes at a time (`reserve_exact`), never by
/// `Vec` doubling, so its capacity stays within the 0.10 MB allowance (ADR 0024, `docs/03` §2).
pub const HOT_GROWTH_STEP_BYTES: usize = 16 * 1_024;

/// A pooled chunk buffer (hot chunk data, pooled pending buffer, sealer output scratch) whose
/// capacity is above this after use is freed instead of kept (`docs/03` §3 rule 7), so a burst of
/// huge rows (up to `MAX_CHUNK_RAW_BYTES` per chunk) does not pin its memory for the rest of the
/// session. It must stay above the normal working size, or steady state would allocate again:
/// a full reference chunk is 0.086 MB measured (0.10 MB allowance, `docs/03` §2), and a chunk of
/// full-width rows of the reference 200-column grid with a style run every word is about
/// 0.3 MB; 512 KiB keeps both.
pub const POOLED_BUFFER_KEEP_BYTES: usize = 512 * 1_024;

/// Largest encoded line in bytes (flags, text, runs). Rows are bounded by the terminal width, so
/// this is far above any real row and only stops a hostile or corrupt producer.
pub const MAX_ENCODED_LINE_BYTES: usize = 64 * 1_024;

/// Largest text of one line in bytes (a quarter of the encoded limit leaves room for the runs).
pub const MAX_LINE_TEXT_BYTES: usize = 16 * 1_024;

/// Largest uncompressed chunk: a full chunk of maximal lines.
pub(crate) const MAX_CHUNK_RAW_BYTES: usize = HOT_CHUNK_LINES * MAX_ENCODED_LINE_BYTES;

/// An LZ4 block cannot expand more than this factor; a declared raw size above
/// `compressed * factor` is a decompression bomb and is rejected before any allocation.
pub(crate) const LZ4_MAX_EXPANSION: usize = 255;

/// Most lines a scrollback may hold (`FR-TERM-011`: max 1 000 000).
pub const MAX_LINES: usize = 1_000_000;

/// Default scrollback length (`FR-TERM-011`).
pub const DEFAULT_MAX_LINES: usize = 10_000;

/// Default number of lines kept in RAM before the oldest sealed chunks spill (`FR-TERM-011`).
pub const DEFAULT_SPILL_THRESHOLD_LINES: usize = 50_000;

/// Default per-session disk cap of the spill file: 256 MiB (`docs/10` §3 item 11).
pub const DEFAULT_SPILL_CAP_BYTES: u64 = 256 * 1_024 * 1_024;

/// Most distinct styles a session interns. Typical sessions use fewer than 200 (`docs/02` §3.2);
/// a program emitting a truecolor gradient could otherwise grow the table without bound, so past
/// this cap new styles are stored as the default style and counted.
pub const MAX_STYLES: usize = 8_192;
