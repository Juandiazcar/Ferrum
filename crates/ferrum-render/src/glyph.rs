//! Glyphs: the fonts, the glyph cache, the rasterizer and the blit kernels of one window, and
//! drawing a glyph or a line of text into a [`Target`].
//!
//! One [`Glyphs`] serves every pane of a window, so the glyph cache cap (`docs/02` §6, default
//! 3 MB, a setting) is one budget. Steady-state drawing (every glyph already cached) allocates
//! nothing.

mod ascii;
mod cache;
mod raster;
mod shape;

use ascii::AsciiSlots;
use cache::{GlyphCache, GlyphImage, GlyphKey, Pixels};
use raster::{Fit, Rasterizer};
use shape::Shaper;

use crate::blit::{Kernel, Shape};
use crate::color::Rgb;
use crate::font::metrics::f32_from;
use crate::font::{CellMetrics, Fonts, Glyph, Presentation, TextStyle};
use crate::synth::{self, Decoration, Lines};
use crate::target::{Rect, Target};

/// A glyph's advance may exceed the cell width by up to half a pixel and still take one cell:
/// cell widths are the advance rounded to whole pixels (`crate::font::metrics`).
const ADVANCE_ROUNDING_PX: f32 = 0.5;

/// A line of text on the cell grid: one cell per character (two for a wide glyph, none for a
/// combining mark).
#[derive(Debug, Clone, Copy)]
pub struct TextRun<'a> {
    /// The characters.
    pub text: &'a str,
    /// Left edge of the first cell.
    pub x: u32,
    /// Top edge of the cells.
    pub y: u32,
    /// The cell grid (size, baseline).
    pub metrics: CellMetrics,
    /// Weight and slant.
    pub style: TextStyle,
    /// Text color.
    pub color: Rgb,
    /// Nothing is drawn outside this rectangle.
    pub clip: Rect,
}

/// Where and how to draw one glyph.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placement {
    /// Pen position: left edge of the glyph's first cell.
    pub(crate) x: u32,
    /// Top of the glyph's row of cells; the baseline is `metrics.baseline` below it.
    pub(crate) y: u32,
    pub(crate) metrics: CellMetrics,
    pub(crate) color: Rgb,
    pub(crate) clip: Rect,
    /// Offset of the glyph from the pen, in pixels right and down (shaped clusters).
    pub(crate) shift: (i32, i32),
}

/// The fonts, glyph cache and kernels of one window.
#[derive(Debug)]
pub struct Glyphs {
    fonts: Fonts,
    cache: GlyphCache,
    raster: Rasterizer,
    shaper: Shaper,
    ascii: AsciiSlots,
    kernel: Kernel,
}

impl Glyphs {
    /// `fonts` with a glyph cache of at most `cache_bytes` (the glyph cache setting).
    pub fn new(fonts: Fonts, cache_bytes: usize) -> Self {
        Self {
            fonts,
            cache: GlyphCache::new(cache_bytes),
            raster: Rasterizer::new(),
            shaper: Shaper::new(),
            ascii: AsciiSlots::new(),
            kernel: Kernel::detect(),
        }
    }

    /// The fonts.
    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    /// Bytes the glyph cache holds now (pixels and bookkeeping), never above its cap.
    pub fn cache_bytes(&self) -> usize {
        self.cache.used_bytes()
    }

    /// Number of cached glyphs.
    pub fn cached_glyphs(&self) -> usize {
        self.cache.len()
    }

    /// Draws `run` and returns the number of cells it used.
    pub fn draw_text(&mut self, target: &mut Target<'_>, run: &TextRun<'_>) -> u32 {
        let mut column = 0;
        let mut last_x = run.x;
        let mut chars = run.text.chars().peekable();
        while let Some(ch) = chars.next() {
            if is_ignorable(ch) {
                continue;
            }
            if synth::is_synthesized(ch) {
                let x = run.x.saturating_add(column * run.metrics.width);
                self.draw_synthetic(target, ch, &Placement { x, ..placement(run) });
                (last_x, column) = (x, column + 1);
                continue;
            }
            let presentation = presentation(ch, chars.peek().copied());
            let glyph = self.fonts.glyph(ch, run.style, presentation);
            let x = run.x.saturating_add(column * run.metrics.width);
            let advance = self.advance(glyph, &run.metrics);
            let pen = if advance == 0 { last_x } else { x };
            let place = Placement { x: pen, ..placement(run) };
            self.draw(target, glyph, &place);
            last_x = pen;
            column += advance;
        }
        column
    }

    /// Cells the glyph's advance takes.
    fn advance(&mut self, glyph: Glyph, metrics: &CellMetrics) -> u32 {
        let slot = self.slot(glyph, metrics);
        let advance =
            slot.and_then(|slot| self.cache.image(slot)).map_or(1.0, |image| image.advance);
        cells_for(advance, metrics.width)
    }

    /// Draws `glyph` at `place`.
    fn draw(&mut self, target: &mut Target<'_>, glyph: Glyph, place: &Placement) {
        if let Some(slot) = self.slot(glyph, &place.metrics) {
            self.blit_slot(target, slot, place);
        }
    }

    fn blit_slot(&self, target: &mut Target<'_>, slot: u32, place: &Placement) {
        if let Some(image) = self.cache.image(slot) {
            blit(target, self.kernel, image, place);
        }
    }

    /// Draws the single character `ch` of a cell (synthesized or from a font).
    pub(crate) fn draw_char(
        &mut self,
        target: &mut Target<'_>,
        ch: char,
        style: TextStyle,
        place: &Placement,
    ) {
        if AsciiSlots::index(ch, style).is_some() {
            if let Some(slot) = self.ascii_slot(ch, style, &place.metrics) {
                self.blit_slot(target, slot, place);
            }
        } else if synth::is_synthesized(ch) {
            self.draw_synthetic(target, ch, place);
        } else if !is_ignorable(ch) {
            let glyph = self.fonts.glyph(ch, style, presentation(ch, None));
            self.draw(target, glyph, place);
        }
    }

    /// The cache slot of printable ASCII `ch` through the direct table (`glyph::ascii`).
    fn ascii_slot(&mut self, ch: char, style: TextStyle, metrics: &CellMetrics) -> Option<u32> {
        let index = AsciiSlots::index(ch, style)?;
        if let Some(slot) = self.ascii.get(index, metrics, self.cache.generation()) {
            self.cache.touch(slot);
            return Some(slot);
        }
        let glyph = self.fonts.glyph(ch, style, presentation(ch, None));
        let slot = self.slot(glyph, metrics)?;
        self.ascii.set(index, slot, self.cache.generation());
        Some(slot)
    }

    /// Draws a grapheme cluster of several characters, shaped (`glyph::shape`).
    pub(crate) fn draw_cluster(
        &mut self,
        target: &mut Target<'_>,
        text: &str,
        style: TextStyle,
        place: &Placement,
    ) {
        let mut glyphs = [None; shape::MAX_CLUSTER_GLYPHS];
        let shaped = self.shaper.shape(&mut self.fonts, text, style, place.metrics.size_q);
        for (slot, positioned) in glyphs.iter_mut().zip(shaped) {
            *slot = Some(*positioned);
        }
        for positioned in glyphs.into_iter().flatten() {
            let shifted = Placement { shift: (positioned.x, positioned.y), ..*place };
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
        if let Some(slot) = self.image(GlyphKey::Decoration { decoration, lines }, make) {
            self.blit_slot(target, slot, place);
        }
    }

    /// Draws the synthesized `ch` (box drawing, blocks, powerline) over the cell at `place`.
    pub(crate) fn draw_synthetic(&mut self, target: &mut Target<'_>, ch: char, place: &Placement) {
        let metrics = place.metrics;
        let key = GlyphKey::Synthetic {
            ch,
            width: metrics.width,
            height: metrics.height,
            light: metrics.stroke,
        };
        let make = |_: &Fonts, _: &mut Rasterizer| {
            mask_image(&synth::draw(ch, metrics.width, metrics.height, metrics.stroke)?, &metrics)
        };
        if let Some(slot) = self.image(key, make) {
            self.blit_slot(target, slot, place);
        }
    }

    /// The cache slot of `glyph` on the grid of `metrics`, rasterizing it on a miss.
    fn slot(&mut self, glyph: Glyph, metrics: &CellMetrics) -> Option<u32> {
        // Color glyphs (emoji) are wide: they are fitted to two cells.
        let fit = Fit { width: metrics.width.saturating_mul(2), height: metrics.height };
        let key = GlyphKey::Font {
            face: glyph.face,
            id: glyph.id,
            size_q: metrics.size_q,
            weight: glyph.weight,
            synth: glyph.synth,
            fit: (fit.width, fit.height),
        };
        let size = f32_from(metrics.size_q) / crate::font::metrics::SIZE_Q_ONE;
        self.image(key, |fonts, raster| raster.render(fonts, glyph, size, fit))
    }

    /// The cache slot of `key`, made by `make` and cached on a miss. A failed `make` is cached
    /// as an empty image (not retried every frame); an image larger than the whole cache is not
    /// kept (nor drawn: it could not fit the screen).
    fn image(
        &mut self,
        key: GlyphKey,
        make: impl FnOnce(&Fonts, &mut Rasterizer) -> Option<GlyphImage>,
    ) -> Option<u32> {
        if let Some(slot) = self.cache.lookup(&key) {
            return Some(slot);
        }
        let image = make(&self.fonts, &mut self.raster).unwrap_or_else(|| GlyphImage::empty(0.0));
        self.cache.insert(key, image).ok()
    }
}

/// A cell-sized mask as an image whose top-left corner is the cell's.
fn mask_image(canvas: &synth::Canvas, metrics: &CellMetrics) -> Option<GlyphImage> {
    let stride = raster::aligned(canvas.width)?;
    let width = usize::try_from(canvas.width).ok()?.max(1);
    let rows = canvas.pixels.chunks_exact(width);
    Some(GlyphImage {
        left: 0,
        top: i32::try_from(metrics.baseline).ok()?,
        width: canvas.width,
        height: canvas.height,
        stride,
        advance: f32_from(metrics.width),
        pixels: Pixels::Mask(raster::pad_rows(rows, usize::try_from(stride).ok()?, |&coverage| {
            coverage
        })),
    })
}

fn placement(run: &TextRun<'_>) -> Placement {
    Placement {
        x: run.x,
        y: run.y,
        metrics: run.metrics,
        color: run.color,
        clip: run.clip,
        shift: (0, 0),
    }
}

/// Blends `image` into `target` at `place`, clipped to the clip rectangle, the target and the
/// glyph's row of cells. Rows are blended with their transparent padding when it is visible, so
/// narrow glyphs take whole SIMD steps.
fn blit(target: &mut Target<'_>, kernel: Kernel, image: &GlyphImage, place: &Placement) {
    let band = Rect::new(0, place.y, target.width(), place.metrics.height);
    let clip = place.clip.intersect(&band).intersect(&target.bounds());
    let left = i64::from(place.x) + i64::from(image.left) + i64::from(place.shift.0);
    let top = i64::from(place.y) + i64::from(place.metrics.baseline) - i64::from(image.top)
        + i64::from(place.shift.1);
    let columns = clip_span(left, image.stride, (clip.x, clip.right()));
    let lines = clip_span(top, image.height, (clip.y, clip.bottom()));
    let (Some((x, skip, width)), Some((y, first, rows))) = (columns, lines) else { return };
    let (Ok(stride), Ok(width), Ok(rows), Ok(target_stride)) = (
        usize::try_from(image.stride),
        usize::try_from(width),
        usize::try_from(rows),
        usize::try_from(target.width()),
    ) else {
        return;
    };
    let shape = Shape { width, rows, dst_stride: target_stride, src_stride: stride };
    let from = first * stride + skip;
    let Some(dst) = target.tail_mut(x, y) else { return };
    match &image.pixels {
        Pixels::Mask(mask) => {
            if let Some(mask) = mask.get(from..) {
                kernel.blend_mask_rect(dst, mask, place.color.pixel(), shape);
            }
        }
        Pixels::Color(pixels) => {
            if let Some(pixels) = pixels.get(from..) {
                kernel.blend_color_rect(dst, pixels, shape);
            }
        }
    }
}

/// The visible part of `len` pixels starting at `start` inside `lo..hi`: where it starts on the
/// target, how many pixels are skipped at its start, how many remain.
fn clip_span(start: i64, len: u32, (lo, hi): (u32, u32)) -> Option<(u32, usize, u32)> {
    let from = start.max(i64::from(lo));
    let to = (start + i64::from(len)).min(i64::from(hi));
    if to <= from {
        return None;
    }
    Some((
        u32::try_from(from).ok()?,
        usize::try_from(from - start).ok()?,
        u32::try_from(to - from).ok()?,
    ))
}

/// Cells an advance of `advance` pixels takes on a grid of `cell_width`: none for a combining
/// mark, at least one otherwise.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "small, rounded up, clamped at 0"
)]
fn cells_for(advance: f32, cell_width: u32) -> u32 {
    if advance <= ADVANCE_ROUNDING_PX || cell_width == 0 {
        return 0;
    }
    ((advance - ADVANCE_ROUNDING_PX) / f32_from(cell_width)).ceil().max(1.0) as u32
}

/// Whether `ch` asks for emoji presentation: followed by VARIATION SELECTOR-16, or in the
/// planes of pictographs (U+1F000 and above, the emoji blocks).
pub(crate) fn presentation(ch: char, next: Option<char>) -> Presentation {
    const EMOJI_VARIATION: char = '\u{FE0F}';
    const PICTOGRAPHS: char = '\u{1F000}';
    if next == Some(EMOJI_VARIATION) || (PICTOGRAPHS..='\u{1FFFF}').contains(&ch) {
        Presentation::Emoji
    } else {
        Presentation::Text
    }
}

/// Default-ignorable characters that have no glyph of their own: joiners, variation selectors,
/// zero-width spaces and marks, word joiner and invisible operators, tag characters (Unicode
/// `DerivedCoreProperties.txt`, `Default_Ignorable_Code_Point`, the ranges terminals meet).
pub(crate) fn is_ignorable(ch: char) -> bool {
    matches!(ch,
        '\u{00AD}' | '\u{034F}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}'
        | '\u{E0000}'..='\u{E007F}' | '\u{E0100}'..='\u{E01EF}')
}

#[cfg(test)]
mod tests;
