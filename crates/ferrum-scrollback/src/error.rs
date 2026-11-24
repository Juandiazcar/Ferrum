//! Error types of the crate (`docs/05` §3: one `thiserror` enum per area).

use std::io;

/// Why a byte sequence is not a valid encoded line or chunk (`decode_chunk`, line decoding).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The input ended in the middle of a field.
    #[error("input ends before the field is complete")]
    Truncated,
    /// A varint is longer than 10 bytes or does not fit in 64 bits.
    #[error("malformed varint")]
    BadVarint,
    /// The flags byte sets bits that are not defined.
    #[error("undefined line flag bits {0:#04x}")]
    BadFlags(u8),
    /// The stored text is not valid UTF-8.
    #[error("line text is not valid UTF-8")]
    BadUtf8,
    /// A style run has length zero, or the runs do not cover exactly the text.
    #[error("style runs do not cover the line text")]
    BadRuns,
    /// A declared size is above the format limits or implausible for the input.
    #[error("declared size {0} exceeds the format limits")]
    TooLarge(u64),
    /// The LZ4 block is corrupt or does not decompress to the declared size.
    #[error("corrupt LZ4 block")]
    Lz4,
    /// The line index does not describe the decompressed bytes.
    #[error("line index does not match the chunk data")]
    BadIndex,
    /// Bytes remain after the last field of a line.
    #[error("trailing bytes after the line")]
    TrailingBytes,
    /// A run refers to a style id that the session style table does not hold.
    #[error("style id {0} is not in the style table")]
    UnknownStyle(u32),
}

/// Failure of a [`SpillStore`](crate::SpillStore) implementation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SpillError {
    /// Storing the record would exceed the capacity of the store.
    #[error("spill store is full ({needed} bytes needed, {free} free)")]
    Full {
        /// Size of the record that did not fit.
        needed: u64,
        /// Free bytes at that moment.
        free: u64,
    },
    /// The handle does not name a live record (already discarded, or never issued).
    #[error("spill handle does not name a live record")]
    UnknownHandle,
    /// The operating system refused an operation on the spill file.
    #[error("spill file I/O failed")]
    Io(#[from] io::Error),
}

/// Failure of a [`Scrollback`](crate::Scrollback) operation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ScrollbackError {
    /// The encoded line is above `MAX_ENCODED_LINE_BYTES`.
    #[error("encoded line is {len} bytes, the limit is {max}")]
    LineTooLong {
        /// Encoded size the line would have.
        len: usize,
        /// The limit.
        max: usize,
    },
    /// The style runs given with a line do not cover exactly its characters.
    #[error("style runs cover {runs} characters but the text has {chars}")]
    RunsMismatch {
        /// Characters covered by the runs.
        runs: usize,
        /// Characters in the text.
        chars: usize,
    },
    /// The configuration is outside the documented range.
    #[error("invalid scrollback configuration: {0}")]
    Config(&'static str),
    /// LZ4 could not compress a chunk (output scratch too small, which must not happen).
    #[error("LZ4 compression failed")]
    Compress,
    /// A stored chunk or line failed validation.
    #[error("stored scrollback data is invalid")]
    Decode(#[from] DecodeError),
    /// The spill store failed while reading a cold chunk.
    #[error("spill store failed")]
    Spill(#[from] SpillError),
    /// A sealed blob given to [`Scrollback::install`](crate::Scrollback::install) came from
    /// another scrollback; it was dropped.
    #[error("sealed chunk belongs to another scrollback")]
    ForeignBlob,
    /// A spilled chunk is needed but the session has no spill store.
    #[error("chunk is spilled but no spill store is attached")]
    NoSpillStore,
}
