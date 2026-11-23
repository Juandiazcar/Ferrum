//! Named limits of the terminal state (`docs/05` §1 rule 9: no magic numbers). Every one of them
//! bounds memory or work against hostile output (`docs/10` §3 items 9 and 11).

/// Bytes of the longest UTF-8 encoded scalar.
pub(crate) const MAX_UTF8_BYTES: usize = 4;
/// First printable ASCII byte (space).
pub(crate) const FIRST_PRINTABLE: u8 = 0x20;
/// Last printable ASCII byte (`~`).
pub(crate) const LAST_PRINTABLE: u8 = 0x7E;
/// Smallest grid dimension: a terminal of one cell is legal (`docs/05` §13 item 3).
pub const MIN_DIMENSION: u16 = 1;
/// Most columns a grid may have. A request above it is clamped: 1 024 x 512 cells of 16 bytes is
/// 8 MiB, the worst case a hostile resize can cost.
pub const MAX_COLS: u16 = 1_024;
/// Most rows a grid may have (see [`MAX_COLS`]).
pub const MAX_ROWS: u16 = 512;
/// Default grid width when a caller has no better value.
pub const DEFAULT_COLS: u16 = 80;
/// Default grid height when a caller has no better value.
pub const DEFAULT_ROWS: u16 = 24;

/// Largest OSC payload in bytes (`docs/10` §3 item 9: 1 MiB). Longer payloads are discarded.
pub const MAX_OSC_BYTES: usize = 1_024 * 1_024;
/// An OSC buffer larger than this is released after the sequence, so one big OSC 52 does not keep
/// a megabyte for the life of the session.
pub const OSC_BUFFER_KEEP_BYTES: usize = 4 * 1_024;
/// Largest OSC 52 clipboard payload in bytes before decoding: 1 MB, decimal, exactly as
/// `docs/10` §3 item 4 says. It is below [`MAX_OSC_BYTES`], so the check is reachable.
pub const MAX_CLIPBOARD_BYTES: usize = 1_000_000;
/// Characters one base64 group takes.
pub const BASE64_GROUP_CHARS: usize = 4;
/// Bytes one base64 group encodes.
pub const BASE64_GROUP_BYTES: usize = 3;
/// Bytes of clipboard text one OSC 52 reply may carry: the largest text whose base64 form still
/// fits in [`MAX_CLIPBOARD_BYTES`] (base64 turns 3 bytes into 4).
pub const MAX_CLIPBOARD_REPLY_BYTES: usize =
    MAX_CLIPBOARD_BYTES / BASE64_GROUP_CHARS * BASE64_GROUP_BYTES;
/// Most color queries (`?`) of one OSC 4 that are answered; the rest are ignored. The palette has
/// 256 entries, so a legitimate "query everything" still fits (`docs/10` §3 items 3 and 11).
pub const MAX_COLOR_QUERIES: usize = 256;
/// Largest title in bytes after sanitizing (`docs/10` §3 item 1).
pub const MAX_TITLE_BYTES: usize = 1_024;
/// Largest OSC 7 working-directory URI in bytes.
pub const MAX_CWD_BYTES: usize = 4_096;
/// Largest OSC 8 hyperlink URI in bytes; a longer one is ignored.
pub const MAX_URI_BYTES: usize = 4_096;
/// Largest OSC 8 `id=` value in bytes.
pub const MAX_LINK_ID_BYTES: usize = 256;
/// Most bytes of URIs and ids the hyperlink table holds in total (`docs/10` §3 item 11).
pub const MAX_LINK_TABLE_BYTES: usize = 256 * 1_024;
/// Bytes of a DCS string kept for the DECRQSS request; the rest is discarded unread.
pub const MAX_DCS_REQUEST_BYTES: usize = 8;
/// Parameters of one sequence (`docs/10` §3 item 9). Enforced by `vte`.
pub const MAX_PARAMS: usize = 32;

/// Most extra styles (hyperlink and underline color combinations) per session: they are carried
/// in 12 spare bits of the cell colors (`color.rs`).
pub const MAX_EXTRAS: usize = 4_095;
/// Most interned multi-scalar grapheme clusters per grid.
pub const MAX_GRAPHEMES: usize = 4_096;
/// Longest grapheme cluster kept, in bytes; a longer run of combining marks is cut.
pub const MAX_CLUSTER_BYTES: usize = 64;
/// Depth of the kitty keyboard protocol mode stack.
pub const KEYBOARD_STACK_DEPTH: usize = 16;
/// Tab stop distance after a reset.
pub const DEFAULT_TAB_WIDTH: u16 = 8;
/// Largest `REP`, `ICH`-style repeat count that is honored (a hostile `CSI 65535 b` is bounded by
/// the grid anyway; this bounds the work of `REP`).
pub const MAX_REPEAT: u16 = 8_192;
/// Cells scanned that one refused entry pays for after a compaction of the grapheme table: a
/// compaction that scanned `w` cells and freed `f` entries is not repeated for
/// `w / WORK_PER_REFUSAL - f` refusals. 64 cells cost about as much as parsing one short escape
/// sequence, so a hostile stream that forces a full table to be compacted again and again is
/// slowed by a constant factor, independent of the grid size, while legitimate reuse is rarely
/// refused.
pub const WORK_PER_REFUSAL: usize = 64;
