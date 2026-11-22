//! Errors of the crate (`docs/05` §3). The parser never returns one: hostile bytes are bounded
//! and dropped, not reported. Only operations that touch the scrollback can fail.

use ferrum_scrollback::ScrollbackError;

/// Why a terminal operation failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VtError {
    /// The scrollback refused or failed an operation (corrupt chunk, spill store failure).
    #[error("scrollback operation failed")]
    Scrollback(#[from] ScrollbackError),
}
