//! Glyphs: the fonts, the glyph cache, the rasterizer and the blit kernels, and drawing a glyph
//! or a line of text into a [`Target`].
//!
//! One [`Glyphs`] serves the whole process, every pane of every window: `docs/03` §2 budgets one
//! glyph cache (3 MB by default, the glyph cache setting), so the caller creates one and passes
//! it to every painter. Steady-state drawing (every glyph already cached) allocates nothing.

mod cache;
mod draw;
mod raster;
mod text;

use cache::{GlyphCache, GlyphImage, GlyphKey};
use raster::{At, Fit, Rasterizer};
pub(crate) use text::{cells_for, is_ignorable, presentation};

use crate::blit::Kernel;
use crate::color::Rgb;
use crate::font::metrics::{SIZE_Q_ONE, f32_from};
use crate::font::{CellMetrics, Fonts, Glyph, TextStyle};
use crate::target::{Rect, Target};

/// How text is antialiased (`docs/02` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Antialias {
    /// Grayscale coverage, the default.
    #[default]
    Grayscale,
    /// Coverage per color channel (LCD subpixels in RGB order), for text on opaque backgrounds
    /// only (the LCD text setting).
    Lcd,
}

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
    /// Pen position within its pixel, in steps of 1/`SUBPIXEL_STEPS` px.
    pub(crate) subpixel: u8,
}

/// Where a looked-up glyph image is.
#[derive(Debug, Clone, Copy)]
enum Stored {
    /// In the cache.
    Slot(u32),
    /// Larger than the whole cache: in [`Glyphs::uncached`] until the next lookup.
    Uncached,
}

/// The fonts, the glyph cache and the kernels: one per process.
#[derive(Debug)]
pub struct Glyphs {
    fonts: Fonts,
    cache: GlyphCache,
    raster: Rasterizer,
    kernel: Kernel,
    antialias: Antialias,
    /// The last image too large for the cache, drawn without being kept.
    uncached: Option<GlyphImage>,
}

impl Glyphs {
    /// `fonts` with a glyph cache of at most `cache_bytes` (the glyph cache setting). Create one
    /// for the process and share it between windows and panes.
    pub fn new(fonts: Fonts, cache_bytes: usize) -> Self {
        Self {
            fonts,
            cache: GlyphCache::new(cache_bytes),
            raster: Rasterizer::new(),
            kernel: Kernel::detect(),
            antialias: Antialias::Grayscale,
            uncached: None,
        }
    }

    /// The fonts.
    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    /// Sets how text is antialiased from now on (glyphs are cached per mode).
    pub fn set_antialias(&mut self, antialias: Antialias) {
        self.antialias = antialias;
    }

    /// Bytes the glyph cache holds now (pixels and containers), never above its cap.
    pub fn cache_bytes(&self) -> usize {
        self.cache.used_bytes()
    }

    /// Number of cached glyphs.
    pub fn cached_glyphs(&self) -> usize {
        self.cache.len()
    }

    /// Draws `run` and returns the number of cells it used.
    pub fn draw_text(&mut self, target: &mut Target<'_>, run: &TextRun<'_>) -> u32 {
        let mut column: u32 = 0;
        let mut last_x = run.x;
        let mut chars = run.text.chars().peekable();
        while let Some(ch) = chars.next() {
            if is_ignorable(ch) {
                continue;
            }
            let presentation = presentation(ch, chars.peek().copied());
            let glyph = self.fonts.glyph(ch, run.style, presentation);
            let x = run.x.saturating_add(column.saturating_mul(run.metrics.width));
            let advance = self.advance(glyph, &run.metrics);
            let pen = if advance == 0 { last_x } else { x };
            let place = Placement { x: pen, ..placement(run) };
            self.draw(target, glyph, &place);
            last_x = pen;
            column = column.saturating_add(advance);
        }
        column
    }

    /// Cells the glyph's advance takes.
    fn advance(&mut self, glyph: Glyph, metrics: &CellMetrics) -> u32 {
        let stored = self.slot(glyph, metrics, 0);
        let advance = self.image(stored).and_then(|image| image.advance);
        cells_for(advance, metrics.advance)
    }

    /// Draws `glyph` at `place`.
    pub(crate) fn draw(&mut self, target: &mut Target<'_>, glyph: Glyph, place: &Placement) {
        let stored = self.slot(glyph, &place.metrics, place.subpixel);
        self.blit(target, stored, place);
    }

    fn blit(&self, target: &mut Target<'_>, stored: Stored, place: &Placement) {
        if let Some(image) = self.image(stored) {
            draw::blit(target, self.kernel, image, place);
        }
    }

    fn image(&self, stored: Stored) -> Option<&GlyphImage> {
        match stored {
            Stored::Slot(slot) => self.cache.image(slot),
            Stored::Uncached => self.uncached.as_ref(),
        }
    }

    /// Where the image of `glyph` on the grid of `metrics` is, rasterizing it on a miss. A
    /// glyph that cannot be rasterized is cached as an empty image (not retried every frame).
    fn slot(&mut self, glyph: Glyph, metrics: &CellMetrics, subpixel: u8) -> Stored {
        self.forget_dropped_faces();
        // Color glyphs (emoji) are wide: they are fitted to two cells.
        let fit = Fit { width: metrics.width.saturating_mul(2), height: metrics.height };
        let antialias = self.antialias;
        let key = GlyphKey {
            face: glyph.face,
            id: glyph.id,
            size_q: metrics.size_q,
            subpixel_q: subpixel,
            weight: glyph.weight,
            synth: glyph.synth,
            antialias,
            fit: (fit.width, fit.height),
        };
        if let Some(slot) = self.cache.lookup(&key) {
            return Stored::Slot(slot);
        }
        let size = f32_from(metrics.size_q) / SIZE_Q_ONE;
        let at = At { size, fit, subpixel, antialias };
        let image =
            self.raster.render(&self.fonts, glyph, at).unwrap_or_else(|| GlyphImage::empty(None));
        match self.cache.insert(key, image) {
            Ok(slot) => Stored::Slot(slot),
            Err(image) => {
                self.uncached = Some(image);
                Stored::Uncached
            }
        }
    }

    /// Removes the cached glyphs of fallback faces the fonts dropped.
    fn forget_dropped_faces(&mut self) {
        let cache = &mut self.cache;
        self.fonts.drain_dropped(|face| cache.evict_where(|key| key.face == face));
    }
}

fn placement(run: &TextRun<'_>) -> Placement {
    Placement {
        x: run.x,
        y: run.y,
        metrics: run.metrics,
        color: run.color,
        clip: run.clip,
        subpixel: 0,
    }
}

#[cfg(test)]
mod tests;
