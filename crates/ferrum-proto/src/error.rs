//! Error type of the crate (`docs/05` §3: one `thiserror` enum per area).

/// Why a value could not be encoded, or a byte sequence is not a valid frame, message or row.
///
/// Decoding never panics: every malformed input becomes one of these (`NFR-REL-01`).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProtoError {
    /// A frame (header included) is longer than [`MAX_FRAME_LEN`](crate::MAX_FRAME_LEN).
    #[error("frame of {len} bytes exceeds the limit of {max} bytes")]
    FrameTooLarge {
        /// Frame length, header included, that was declared or produced.
        len: usize,
        /// The limit.
        max: usize,
    },
    /// A row has more cell columns than the row width it declares.
    #[error("row cells take more than the {cols} columns of the row")]
    TooManyCells {
        /// The declared row width.
        cols: u16,
    },
    /// A cell has no text, or its text is longer than a row can carry (255 bytes).
    #[error("cell text is empty or longer than 255 bytes")]
    BadCellText,
    /// A hyperlink is longer than a link table can carry (65 535 bytes).
    #[error("hyperlink of {0} bytes is longer than 65535 bytes")]
    LinkTooLong(usize),
    /// A link table already holds 65 535 entries.
    #[error("too many distinct hyperlinks in one block")]
    TooManyLinks,
    /// An encoded row or run is longer than its `u32` length field can say.
    #[error("encoded row part of {0} bytes is too long")]
    RowTooLong(usize),
    /// The row does not fit in the block (see
    /// [`MAX_ROW_BLOCK_BYTES`](crate::MAX_ROW_BLOCK_BYTES)); send the block and start a new one.
    #[error("the row does not fit in the block")]
    BlockFull,
    /// A list holds more items than [`MAX_LIST_ITEMS`](crate::MAX_LIST_ITEMS).
    #[error("list of {0} items exceeds the limit")]
    TooManyItems(usize),
    /// A value is outside its documented range; the text says which.
    #[error("invalid value: {0}")]
    InvalidValue(&'static str),
    /// An encoded row is not well formed; the text says which part.
    #[error("malformed row: {0}")]
    MalformedRow(&'static str),
    /// A message could not be encoded or decoded by `postcard`.
    #[error("message encoding failed")]
    Postcard(#[from] postcard::Error),
    /// Bytes remain after the message inside a frame.
    #[error("{0} trailing bytes after the message")]
    TrailingBytes(usize),
    /// The peer speaks an incompatible protocol major version.
    #[error("protocol {theirs} is incompatible with protocol {ours}")]
    VersionMismatch {
        /// This side's version.
        ours: crate::ProtocolVersion,
        /// The peer's version.
        theirs: crate::ProtocolVersion,
    },
}
