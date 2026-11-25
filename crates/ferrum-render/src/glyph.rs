//! Glyphs: the fonts, the glyph cache, the rasterizer and the blit kernels of one window, and
//! drawing a glyph or a line of text into a [`Target`].
//!
//! One [`Glyphs`] serves every pane of a window, so the glyph cache cap (`docs/02` §6, default
//! 3 MB, a setting) is one budget. Steady-state drawing (every glyph already cached) allocates
//! nothing.

mod cache;
mod raster;

use cache::{GlyphCache, GlyphImage, GlyphKey, Pixels};
use raster::{Fit, Rasterizer};

use crate::blit::Kernel;
use crate::color::Rgb;
use crate::font::metrics::f32_from;
use crate::font::{CellMetrics, Fonts, Glyph, Presentation, TextStyle};
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
}

/// The fonts, glyph cache and kernels of one window.
#[derive(Debug)]
pub struct Glyphs {
    fonts: Fonts,
    cache: GlyphCache,
    raster: Rasterizer,
    kernel: Kernel,
}

impl Glyphs {
    /// `fonts` with a glyph cache of at most `cache_bytes` (the glyph cache setting).
    pub fn new(fonts: Fonts, cache_bytes: usize) -> Self {
        Self {
            fonts,
            cache: GlyphCache::new(cache_bytes),
            raster: Rasterizer::new(),
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
        let advance = self.cached(glyph, metrics).map_or(1.0, |image| image.advance);
        cells_for(advance, metrics.width)
    }

    /// Draws `glyph` at `place`.
    pub(crate) fn draw(&mut self, target: &mut Target<'_>, glyph: Glyph, place: &Placement) {
        let kernel = self.kernel;
        if let Some(image) = self.cached(glyph, &place.metrics) {
            blit(target, kernel, image, place);
        }
    }

    /// The cached image of `glyph` on the grid of `metrics`, rasterizing it on a miss. A glyph
    /// larger than the whole cache is not kept (and so not drawn: it cannot be on screen).
    fn cached(&mut self, glyph: Glyph, metrics: &CellMetrics) -> Option<&GlyphImage> {
        // Color glyphs (emoji) are wide: they are fitted to two cells.
        let fit = Fit { width: metrics.width.saturating_mul(2), height: metrics.height };
        let key = GlyphKey {
            face: glyph.face,
            id: glyph.id,
            size_q: metrics.size_q,
            weight: glyph.weight,
            synth: glyph.synth,
            fit: (fit.width, fit.height),
        };
        if let Some(slot) = self.cache.lookup(&key) {
            return self.cache.image(slot);
        }
        let size = f32_from(metrics.size_q) / crate::font::metrics::SIZE_Q_ONE;
        let image = self.raster.render(&self.fonts, glyph, size, fit)?;
        let slot = self.cache.insert(key, image).ok()?;
        self.cache.image(slot)
    }
}

fn placement(run: &TextRun<'_>) -> Placement {
    Placement { x: run.x, y: run.y, metrics: run.metrics, color: run.color, clip: run.clip }
}

/// Blends `image` into `target` at `place`, clipped to the clip rectangle, the target and the
/// glyph's row of cells.
fn blit(target: &mut Target<'_>, kernel: Kernel, image: &GlyphImage, place: &Placement) {
    let band = Rect::new(0, place.y, target.width(), place.metrics.height);
    let clip = place.clip.intersect(&band).intersect(&target.bounds());
    let left = i64::from(place.x) + i64::from(image.left);
    let top = i64::from(place.y) + i64::from(place.metrics.baseline) - i64::from(image.top);
    let width = usize::try_from(image.width).unwrap_or(0);
    for row in 0..image.height {
        let y = top + i64::from(row);
        let (Ok(y), Some(start)) =
            (u32::try_from(y), usize::try_from(row).ok().and_then(|r| r.checked_mul(width)))
        else {
            continue;
        };
        if y < clip.y || y >= clip.bottom() {
            continue;
        }
        let Some((x, skip, len)) = clip_span(left, image.width, clip) else { return };
        let Some(dst) = target.span_mut(y, x, len) else { continue };
        let from = start + skip;
        match &image.pixels {
            Pixels::Mask(mask) => {
                if let Some(mask) = mask.get(from..from + dst.len()) {
                    kernel.blend_mask(dst, mask, place.color.pixel());
                }
            }
            Pixels::Color(color) => {
                if let Some(color) = color.get(from..from + dst.len()) {
                    kernel.blend_color(dst, color);
                }
            }
        }
    }
}

/// The visible part of a glyph row starting at `left` and `width` pixels wide: target x, pixels
/// skipped at the left, length.
fn clip_span(left: i64, width: u32, clip: Rect) -> Option<(u32, usize, u32)> {
    let start = left.max(i64::from(clip.x));
    let end = (left + i64::from(width)).min(i64::from(clip.right()));
    if end <= start {
        return None;
    }
    let x = u32::try_from(start).ok()?;
    let skip = usize::try_from(start - left).ok()?;
    Some((x, skip, u32::try_from(end - start).ok()?))
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
