//! Error type of the crate (`docs/05` §3: one `thiserror` enum per area).

/// Why a value could not be encoded, or a byte sequence is not a valid frame, message or row.
///
/// Decoding never panics: every malformed input becomes one of these (`NFR-REL-01`).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProtoError {
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
    /// An encoded row is not well formed; the text says which part.
    #[error("malformed row: {0}")]
    MalformedRow(&'static str),
}
