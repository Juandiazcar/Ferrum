//! Drawing what a terminal cell holds: one character, a shaped grapheme cluster, or a text
//! decoration (underline, strikeout, overline).

use super::ascii::AsciiSlots;
use super::shape::{MAX_CLUSTER_GLYPHS, Positioned};
use super::text::{is_ignorable, presentation};
use super::{GlyphKey, Glyphs, Placement, Stored, mask_image};
use crate::font::{CellMetrics, Fonts, TextStyle};
use crate::glyph::raster::Rasterizer;
use crate::synth::{self, Decoration, Lines};
use crate::target::Target;

impl Glyphs {
    /// Draws the single character `ch` of a cell (synthesized or from a font).
    pub(crate) fn draw_char(
        &mut self,
        target: &mut Target<'_>,
        ch: char,
        style: TextStyle,
        place: &Placement,
    ) {
        if AsciiSlots::index(ch, style).is_some() {
            let stored = self.ascii_slot(ch, style, &place.metrics);
            self.blit(target, stored, place);
        } else if synth::is_synthesized(ch) {
            self.draw_synthetic(target, ch, place);
        } else if !is_ignorable(ch) {
            let glyph = self.fonts.glyph(ch, style, presentation(ch, None));
            self.draw(target, glyph, place);
        }
    }

    /// Where printable ASCII `ch` is, through the direct table (`glyph::ascii`).
    fn ascii_slot(&mut self, ch: char, style: TextStyle, metrics: &CellMetrics) -> Stored {
        let generation = self.cache.generation();
        if let Some(index) = AsciiSlots::index(ch, style)
            && let Some(slot) = self.ascii.get(index, (metrics, self.generation), generation)
        {
            self.cache.touch(slot);
            return Stored::Slot(slot);
        }
        let glyph = self.fonts.glyph(ch, style, presentation(ch, None));
        let stored = self.slot(glyph, metrics, 0);
        if let (Some(index), Stored::Slot(slot)) = (AsciiSlots::index(ch, style), stored) {
            self.ascii.set(index, slot, self.cache.generation());
        }
        stored
    }

    /// Draws a grapheme cluster of several characters, shaped (`glyph::shape`).
    pub(crate) fn draw_cluster(
        &mut self,
        target: &mut Target<'_>,
        text: &str,
        style: TextStyle,
        place: &Placement,
    ) {
        // Copied out of the memo: drawing needs `self` mutably.
        let mut glyphs: [Option<Positioned>; MAX_CLUSTER_GLYPHS] = [None; MAX_CLUSTER_GLYPHS];
        let shaped = self.shaper.shape(&mut self.fonts, text, style, place.metrics.size_q);
        for (slot, positioned) in glyphs.iter_mut().zip(shaped) {
            *slot = Some(*positioned);
        }
        for positioned in glyphs.into_iter().flatten() {
            let shifted = Placement {
                shift: (positioned.x, positioned.y),
                subpixel: positioned.subpixel,
                ..*place
            };
            self.draw(target, positioned.glyph, &shifted);
        }
    }

    /// Draws a decoration (underline, strikeout, overline) over the cell at `place`.
    pub(crate) fn draw_decoration(
        &mut self,
        target: &mut Target<'_>,
        decoration: Decoration,
        place: &Placement,
    ) {
        let metrics = place.metrics;
        let lines = Lines {
            width: metrics.width,
            height: metrics.height,
            underline_top: metrics.underline_top,
            strikeout_top: metrics.strikeout_top,
            stroke: metrics.stroke,
        };
        let make = |_: &Fonts, _: &mut Rasterizer| {
            mask_image(&synth::decoration(decoration, &lines)?, &metrics)
        };
        let stored = self.image(GlyphKey::Decoration { decoration, lines }, make);
        self.blit(target, stored, place);
    }
}
