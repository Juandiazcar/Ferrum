//! Errors of `ferrum-render`.

/// What can go wrong when setting up fonts or tiles.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// The font size, scale factor, line height or letter spacing is not a usable number.
    #[error("the font size, scale factor, line height or letter spacing is out of range")]
    InvalidFontSpec,
    /// The requested system family is not installed (or no system fonts were scanned).
    #[error("font family not found")]
    FamilyNotFound,
    /// A font file could not be read as a font.
    #[error("the font file could not be read")]
    UnreadableFont,
    /// The pixel buffer is not `width × height` pixels long.
    #[error("the pixel buffer does not match its width and height")]
    BufferSize,
}
