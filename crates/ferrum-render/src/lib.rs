//! `ferrum-render`: font loading, glyph cache and damage-limited CPU painting into a pixel buffer.
//!
//! The crate writes into a caller-owned `&mut [u32]` (the `softbuffer` buffer, `0x00RRGGBB`,
//! `docs/02` §6) and takes plain values: colors, sizes, metrics. It never opens a window and
//! depends on no other Ferrum crate (`docs/04` §2).
//!
//! - [`font`]: the embedded and system fonts, the fallback chain and the cell metrics.
//! - [`Glyphs`]: fonts, glyph cache (bounded LRU) and blit kernels, one per process; draws glyphs and
//!   lines of text into a [`Target`]. Box drawing, block elements and the powerline separators
//!   are drawn pixel-perfect by the renderer itself, bypassing the font.
//! - [`chrome`]: rounded rectangles and shadows as cached tiles, drawn by nine-slicing.
//! - [`Painter`]: paints a pane's grid of [`Cell`]s with cursor and selection, repaints only the
//!   rows whose content hash changed since the frame the buffer holds, and returns the damage.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in the blit kernels, each block with `// SAFETY:`

#[allow(unsafe_code, reason = "docs/05 §4: the SIMD blit kernels, every block with `// SAFETY:`")]
mod blit;
pub mod chrome;
mod color;
mod error;
pub mod font;
mod glyph;
mod paint;
mod synth;
mod target;

pub use color::Rgb;
pub use error::RenderError;
pub use glyph::{Antialias, Glyphs, TextRun};
pub use paint::{
    Cell, CellText, CellWidth, Cursor, CursorShape, Frame, Grid, Overlay, Painter, Row, Rows,
    Selection, SelectionKind, Strokes,
};
pub use synth::Underline;
pub use target::{Rect, Target};

#[cfg(test)]
mod test_fonts;
