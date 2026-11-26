//! Grapheme clusters of several characters: emoji sequences (ZWJ families, flags, keycaps, skin
//! tones) and combining marks are shaped by `swash` with the face of their first character, so a
//! sequence becomes the font's single glyph and marks get their positions. Results are memoized;
//! a memo hit allocates nothing.
//!
//! One [`Shaper`] serves the process with its [`crate::Glyphs`]; the memo forgets the clusters
//! of fallback faces the fonts drop.

use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHasher};
use swash::shape::ShapeContext;

use super::raster::SUBPIXEL_STEPS;
use super::text::{cluster_presentation, is_ignorable, presentation};
use crate::font::metrics::{SIZE_Q_ONE, f32_from};
use crate::font::{FaceId, Fonts, Glyph, TextStyle};

/// Most glyphs kept for one cluster: a sequence shapes to one glyph, a base with combining marks
/// to one glyph per mark; eight holds a base and seven stacked marks, beyond anything a terminal
/// cell shows legibly (longer clusters are drawn with their first eight glyphs).
pub(crate) const MAX_CLUSTER_GLYPHS: usize = 8;
/// Most remembered clusters. Clusters are rare next to single characters; 4 096 is a third of a
/// 200×60 screen made only of distinct clusters. The memo is emptied when full.
pub(crate) const MAX_SHAPED_CLUSTERS: usize = 4096;
/// Longest cluster kept in the memo, in bytes: `ferrum-vt` stores clusters of at most 64 bytes
/// (`ferrum_vt::limits::MAX_CLUSTER_BYTES`), so this only guards the memo against a caller that
/// passes more. A longer cluster is shaped every time it is drawn and not remembered.
pub(crate) const MAX_MEMO_CLUSTER_BYTES: usize = 64;

/// A glyph of a cluster and its offset from the cell's pen position: whole pixels plus a
/// horizontal rest in steps of 1/`SUBPIXEL_STEPS` px.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Positioned {
    pub(crate) glyph: Glyph,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) subpixel: u8,
}

#[derive(Debug)]
struct Shaped {
    text: Box<str>,
    style: TextStyle,
    size_q: u32,
    glyphs: Vec<Positioned>,
}

/// The shaping context and the memo.
pub(crate) struct Shaper {
    context: ShapeContext,
    memo: FxHashMap<u64, Shaped>,
    /// The glyphs of the last cluster too long for the memo.
    scratch: Vec<Positioned>,
}

impl std::fmt::Debug for Shaper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shaper").field("memo", &self.memo.len()).finish_non_exhaustive()
    }
}

impl Shaper {
    pub(crate) fn new() -> Self {
        Self { context: ShapeContext::new(), memo: FxHashMap::default(), scratch: Vec::new() }
    }

    /// The glyphs of `text` in `style` at `size_q` (1/64 px).
    pub(crate) fn shape(
        &mut self,
        fonts: &mut Fonts,
        text: &str,
        style: TextStyle,
        size_q: u32,
    ) -> &[Positioned] {
        if text.len() > MAX_MEMO_CLUSTER_BYTES {
            self.scratch = self.shape_uncached(fonts, text, style, size_q);
            return &self.scratch;
        }
        let mut hasher = FxHasher::default();
        (text, style, size_q).hash(&mut hasher);
        let key = hasher.finish();
        let hit = self.memo.get(&key).is_some_and(|shaped| {
            *shaped.text == *text && shaped.style == style && shaped.size_q == size_q
        });
        if !hit {
            let glyphs = self.shape_uncached(fonts, text, style, size_q);
            if self.memo.len() >= MAX_SHAPED_CLUSTERS {
                self.memo.clear();
            }
            let shaped = Shaped { text: text.into(), style, size_q, glyphs };
            let _previous = self.memo.insert(key, shaped);
        }
        self.memo.get(&key).map_or(&[], |shaped| shaped.glyphs.as_slice())
    }

    /// Forgets the clusters shaped with `face` (a fallback face the fonts dropped).
    pub(crate) fn forget(&mut self, face: FaceId) {
        self.memo.retain(|_, shaped| {
            shaped.glyphs.iter().all(|positioned| positioned.glyph.face != face)
        });
    }

    fn shape_uncached(
        &mut self,
        fonts: &mut Fonts,
        text: &str,
        style: TextStyle,
        size_q: u32,
    ) -> Vec<Positioned> {
        let mut chars = text.chars().filter(|&ch| !is_ignorable(ch));
        let Some(first) = chars.next() else { return Vec::new() };
        let base = fonts.glyph(first, style, cluster_presentation(text));
        let shaped = self.run(fonts, base, text, size_q);
        if !shaped.is_empty() && shaped.iter().all(|positioned| positioned.glyph.id != 0) {
            return shaped;
        }
        // The base face lacks part of the cluster: each character from its own face, stacked.
        std::iter::once(base)
            .chain(chars.map(|ch| fonts.glyph(ch, style, presentation(ch, None))))
            .take(MAX_CLUSTER_GLYPHS)
            .map(|glyph| Positioned { glyph, x: 0, y: 0, subpixel: 0 })
            .collect()
    }

    /// Shapes `text` with the face of `base`.
    fn run(&mut self, fonts: &Fonts, base: Glyph, text: &str, size_q: u32) -> Vec<Positioned> {
        let Some(face) = fonts.face(base.face) else { return Vec::new() };
        let size = f32_from(size_q) / SIZE_Q_ONE;
        let context = &mut self.context;
        let shaped = face.with_font(fonts.system(), |font| {
            let mut glyphs = Vec::new();
            let mut shaper = context
                .builder(font)
                .size(size)
                .variations(face.weight_setting(base.weight))
                .build();
            shaper.add_str(text);
            let mut pen = 0.0;
            shaper.shape_with(|cluster| {
                for glyph in cluster.glyphs {
                    if glyphs.len() < MAX_CLUSTER_GLYPHS {
                        glyphs.push(positioned(base, glyph.id, pen + glyph.x, glyph.y));
                    }
                    pen += glyph.advance;
                }
            });
            glyphs
        });
        shaped.unwrap_or_default()
    }
}

/// `id` of `base`'s face at an offset (`y` up, as fonts measure): `x` to the nearest subpixel
/// step, `y` to the nearest pixel.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "offsets of a glyph inside one cell; the step is in 0..SUBPIXEL_STEPS"
)]
fn positioned(base: Glyph, id: u16, x: f32, y: f32) -> Positioned {
    let glyph = Glyph { face: base.face, id, ..base };
    let steps = (x * f32::from(SUBPIXEL_STEPS)).round();
    let whole = (steps / f32::from(SUBPIXEL_STEPS)).floor();
    let subpixel = (steps - whole * f32::from(SUBPIXEL_STEPS)) as u8;
    Positioned { glyph, x: whole as i32, y: -(y.round() as i32), subpixel }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{FaceId, Synthesis};

    #[test]
    fn offsets_split_into_pixels_and_quarter_steps() {
        let base =
            Glyph { face: FaceId::EMBEDDED, id: 1, weight: 400, synth: Synthesis::default() };
        let at = |x: f32| {
            let p = positioned(base, 2, x, 0.0);
            (p.x, p.subpixel)
        };
        assert_eq!(at(0.0), (0, 0));
        assert_eq!(at(2.3), (2, 1));
        assert_eq!(at(2.6), (2, 2));
        assert_eq!(at(2.9), (3, 0), "rounds up into the next pixel");
        assert_eq!(at(-0.3), (-1, 3));
        assert_eq!(positioned(base, 2, 0.0, 3.4).y, -3, "y up in fonts, down on screen");
    }
}
