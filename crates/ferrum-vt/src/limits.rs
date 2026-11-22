//! Named limits of the terminal state (`docs/05` §1 rule 9: no magic numbers). Every one of them
//! bounds memory or work against hostile output (`docs/10` §3 items 9 and 11).

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
/// Largest OSC 52 clipboard payload in bytes before decoding (`docs/10` §3 item 4: 1 MB).
pub const MAX_CLIPBOARD_BYTES: usize = 1_024 * 1_024;
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
