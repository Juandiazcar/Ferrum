//! `ferrum-render`: font loading, glyph cache and damage-limited CPU painting into a pixel buffer.
//!
//! The crate writes into a caller-owned `&mut [u32]` (the `softbuffer` buffer, `0x00RRGGBB`,
//! `docs/02` §6) and takes plain values: colors, sizes, metrics. It never opens a window and
//! depends on no other Ferrum crate (`docs/04` §2).
//!
//! - [`font`]: the embedded and system fonts, the fallback chain and the cell metrics.
//! - [`Glyphs`]: fonts, glyph cache (bounded LRU) and blit kernels of a window; draws glyphs and
//!   lines of text into a [`Target`]. Box drawing, block elements and the powerline separators
//!   are drawn pixel-perfect by the renderer itself, bypassing the font.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in the blit kernels, each block with `// SAFETY:`

#[allow(unsafe_code, reason = "docs/05 §4: the SIMD blit kernels, every block with `// SAFETY:`")]
mod blit;
mod color;
mod error;
pub mod font;
mod glyph;
mod synth;
mod target;

pub use color::Rgb;
pub use error::RenderError;
pub use glyph::{Glyphs, TextRun};
pub use target::{Rect, Target};
