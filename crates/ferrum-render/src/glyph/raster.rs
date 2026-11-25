//! Rasterization with `swash`: grayscale anti-aliased coverage for text, premultiplied color for
//! emoji (`docs/02` §6: grayscale AA is the default).

use swash::FontRef;
use swash::scale::image::{Content, Image};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Angle, Format, Transform, Vector};

use super::cache::{GlyphImage, Pixels};
use crate::blit::ROW_ALIGN;
use crate::font::{Face, Fonts, Glyph};

/// Glyph pen positions are quantized to quarter pixels (the `subpixel_q` of the cache key,
/// `docs/02` §6): the steps `FreeType` and Skia use for subpixel glyph positioning. Cell text
/// starts on whole pixels (step 0); shaped clusters place marks between them.
pub(crate) const SUBPIXEL_STEPS: u8 = 4;

/// Synthetic italic slant: the default angle of CSS `font-style: oblique` (CSS Fonts 4, §2.3).
const OBLIQUE_DEGREES: f32 = 14.0;
/// Synthetic bold strength as a fraction of the size: `FreeType`'s `FT_GlyphSlot_Embolden` uses
/// one 24th of the em.
const EMBOLDEN_FRACTION: f32 = 1.0 / 24.0;

/// Color faces: layered outlines (`COLR`), then color bitmaps (`CBDT`, `sbix`), then outlines.
const COLOR_SOURCES: [Source; 3] =
    [Source::ColorOutline(0), Source::ColorBitmap(StrikeWith::BestFit), Source::Outline];
/// Text faces: outlines, then the face's own grayscale bitmaps.
const TEXT_SOURCES: [Source; 2] = [Source::Outline, Source::Bitmap(StrikeWith::BestFit)];

/// Where a color glyph must fit: at most `width` × `height` pixels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fit {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// The `swash` scaler state and its scratch image, reused for every glyph.
pub(crate) struct Rasterizer {
    context: ScaleContext,
    image: Image,
}

impl std::fmt::Debug for Rasterizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rasterizer").finish_non_exhaustive()
    }
}

impl Rasterizer {
    pub(crate) fn new() -> Self {
        Self { context: ScaleContext::new(), image: Image::new() }
    }

    /// Rasterizes `glyph` at `size` physical pixels per em. A color glyph larger than `fit` is
    /// drawn again at the size that fits. A glyph without ink gives an empty image; `None` when
    /// the face cannot be read.
    pub(crate) fn render(&mut self, fonts: &Fonts, glyph: Glyph, at: At) -> Option<GlyphImage> {
        let face = fonts.face(glyph.face)?;
        let (fit, subpixel) = (at.fit, at.subpixel);
        let mut image = self.render_at(fonts, Request { face, glyph, size: at.size, subpixel })?;
        if face.is_color() && (image.width > fit.width || image.height > fit.height) {
            let scale = ratio(fit.width, image.width).min(ratio(fit.height, image.height));
            let size = at.size * scale;
            image = self.render_at(fonts, Request { face, glyph, size, subpixel })?;
        }
        if face.is_color() {
            // An emoji takes the two cells it is fitted to, whatever its font's own advance.
            image.advance = image.advance.min(crate::font::metrics::f32_from(fit.width));
        }
        Some(image)
    }

    fn render_at(&mut self, fonts: &Fonts, request: Request<'_>) -> Option<GlyphImage> {
        let Request { face, glyph, size, .. } = request;
        face.with_font(fonts.system(), |font| {
            let ok = self.scale_into(font, &request);
            let advance = font.glyph_metrics(&[]).scale(size).advance_width(glyph.id);
            ok.then(|| convert(&self.image, advance))
        })
        .flatten()
        .flatten()
    }

    fn scale_into(&mut self, font: FontRef<'_>, request: &Request<'_>) -> bool {
        let Request { face, glyph, size, subpixel } = *request;
        let is_color = face.is_color();
        let mut scaler = self
            .context
            .builder(font)
            .size(size)
            .hint(!is_color)
            .variations(face.weight_setting(glyph.weight))
            .build();
        let sources: &[Source] = if is_color { &COLOR_SOURCES } else { &TEXT_SOURCES };
        let mut render = Render::new(sources);
        let _format = render.format(Format::Alpha);
        if subpixel > 0 {
            let x = f32::from(subpixel) / f32::from(SUBPIXEL_STEPS);
            let _offset = render.offset(Vector::new(x, 0.0));
        }
        if glyph.synth.embolden {
            let _bold = render.embolden(size * EMBOLDEN_FRACTION);
        }
        if glyph.synth.oblique {
            let slant = Transform::skew(Angle::from_degrees(OBLIQUE_DEGREES), Angle::ZERO);
            let _oblique = render.transform(Some(slant));
        }
        render.render_into(&mut scaler, glyph.id, &mut self.image)
    }
}

/// One glyph to rasterize.
#[derive(Clone, Copy)]
struct Request<'a> {
    face: &'a Face,
    glyph: Glyph,
    size: f32,
    subpixel: u8,
}

/// How a glyph is rasterized: its size, the box a color glyph must fit, and its pen position
/// within the pixel.
#[derive(Debug, Clone, Copy)]
pub(crate) struct At {
    /// Pixels per em.
    pub(crate) size: f32,
    pub(crate) fit: Fit,
    /// Pen position within its pixel, in steps of 1/[`SUBPIXEL_STEPS`] px.
    pub(crate) subpixel: u8,
}

/// The cacheable form of `image`, rows padded to the blit step.
fn convert(image: &Image, advance: f32) -> Option<GlyphImage> {
    let placement = image.placement;
    let width = usize::try_from(placement.width).ok()?;
    let height = usize::try_from(placement.height).ok()?;
    if width == 0 || height == 0 {
        return Some(GlyphImage::empty(advance));
    }
    let stride = aligned(placement.width)?;
    let padded = usize::try_from(stride).ok()?;
    let area = width.checked_mul(height)?;
    let pixels = match image.content {
        Content::Mask => {
            let rows = image.data.get(..area)?.chunks_exact(width);
            Pixels::Mask(pad_rows(rows, padded, |&coverage| coverage))
        }
        Content::Color | Content::SubpixelMask => {
            let straight = matches!(image.source, Source::ColorBitmap(_) | Source::Bitmap(_));
            let rgba = image.data.get(..area.checked_mul(4)?)?;
            let rows = rgba.as_chunks::<4>().0.chunks_exact(width);
            Pixels::Color(pad_rows(rows, padded, |&pixel| premultiply(pixel, straight)))
        }
    };
    Some(GlyphImage {
        left: placement.left,
        top: placement.top,
        width: placement.width,
        height: placement.height,
        stride,
        advance,
        pixels,
    })
}

/// `width` rounded up to `crate::blit::ROW_ALIGN`.
pub(crate) fn aligned(width: u32) -> Option<u32> {
    Some(width.checked_add(ROW_ALIGN - 1)? / ROW_ALIGN * ROW_ALIGN)
}

/// The rows converted by `pixel`, each padded with the default (transparent) value to `stride`.
pub(crate) fn pad_rows<'a, S: 'a, T: Default + Clone>(
    rows: impl Iterator<Item = &'a [S]>,
    stride: usize,
    pixel: impl Fn(&S) -> T,
) -> Vec<T> {
    let mut out = Vec::new();
    for row in rows {
        let start = out.len();
        out.extend(row.iter().map(&pixel));
        out.resize(start + stride, T::default());
    }
    out
}

/// One RGBA pixel as premultiplied `0xAARRGGBB`. `swash` composites color outlines already
/// premultiplied; decoded bitmaps (PNG strikes) are straight alpha.
fn premultiply(rgba: [u8; 4], straight: bool) -> u32 {
    let [red, green, blue, alpha] = rgba;
    let scale = |channel: u8| {
        if straight {
            let t = u32::from(channel) * u32::from(alpha) + 128;
            (t + (t >> 8)) >> 8
        } else {
            u32::from(channel.min(alpha))
        }
    };
    u32::from(alpha) << 24 | scale(red) << 16 | scale(green) << 8 | scale(blue)
}

#[allow(clippy::cast_precision_loss, reason = "glyph sides are far below 2^24")]
fn ratio(limit: u32, actual: u32) -> f32 {
    if actual == 0 { 1.0 } else { limit as f32 / actual as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_alpha_is_premultiplied_and_premultiplied_is_kept() {
        assert_eq!(premultiply([255, 128, 0, 128], true), 0x8080_4000);
        assert_eq!(premultiply([200, 100, 0, 100], false), 0x6464_6400);
        assert_eq!(premultiply([10, 20, 30, 255], true), 0xFF0A_141E);
    }
}
