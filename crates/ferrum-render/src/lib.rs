//! `ferrum-render`: font loading, glyph cache and damage-limited CPU painting into a pixel buffer.
//!
//! The crate writes into a caller-owned `&mut [u32]` (the `softbuffer` buffer, `0x00RRGGBB`,
//! `docs/02` §6) and takes plain values: colors, sizes, metrics. It never opens a window and
//! depends on no other Ferrum crate (`docs/04` §2).
//!
//! - [`font`]: the embedded and system fonts, the fallback chain and the cell metrics.

#![deny(unsafe_code)] // docs/05 §4: unsafe only in the blit kernels, each block with `// SAFETY:`

mod error;
pub mod font;

pub use error::RenderError;

#[cfg(test)]
mod test_fonts;
