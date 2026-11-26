//! Rasterization with `swash`: grayscale anti-aliased coverage for text (the default), coverage
//! per color channel for LCD text (`docs/02` §6: optional, on opaque backgrounds), premultiplied
//! color for emoji.

use swash::FontRef;
use swash::scale::image::{Content, Image};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Angle, Format, Transform, Vector};

use super::Antialias;
use super::cache::{GlyphImage, Pixels};
use crate::blit::ROW_ALIGN;
use crate::font::metrics::f32_from;
use crate::font::{Face, Fonts, Glyph};

/// Glyph pen positions are quantized to quarter pixels (the `subpixel_q` of the cache key,
/// `docs/02` §6), the step `FreeType` and Skia use for subpixel glyph positioning: a rendering
/// quality step. Cell text starts on whole pixels (step 0); shaped clusters place marks between.
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

/// How a glyph is rasterized.
#[derive(Debug, Clone, Copy)]
pub(crate) struct At {
    /// Pixels per em.
    pub(crate) size: f32,
    /// The box a color glyph is fitted to.
    pub(crate) fit: Fit,
    /// Pen position within its pixel, in steps of 1/[`SUBPIXEL_STEPS`] px.
    pub(crate) subpixel: u8,
    pub(crate) antialias: Antialias,
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

    /// Rasterizes `glyph` as `at` says. A color glyph larger than its fit is drawn again at the
    /// size that fits, and takes the two cells of the fit. A glyph without ink gives an empty
    /// image; `None` when the face cannot be read.
    pub(crate) fn render(&mut self, fonts: &Fonts, glyph: Glyph, at: At) -> Option<GlyphImage> {
        let face = fonts.face(glyph.face)?;
        let request = Request { face, glyph, at };
        let mut image = self.render_at(fonts, request)?;
        let fit = at.fit;
        if face.is_color() && (image.width > fit.width || image.height > fit.height) {
            let scale = ratio(fit.width, image.width).min(ratio(fit.height, image.height));
            let at = At { size: at.size * scale, ..at };
            image = self.render_at(fonts, Request { at, ..request })?;
        }
        if face.is_color() {
            image.advance = image.advance.map(|advance| advance.min(f32_from(fit.width)));
        }
        Some(image)
    }

    fn render_at(&mut self, fonts: &Fonts, request: Request<'_>) -> Option<GlyphImage> {
        let Request { face, glyph, at } = request;
        face.with_font(fonts.system(), |font| {
            let ok = self.scale_into(font, &request);
            let advance = font.glyph_metrics(&[]).scale(at.size).advance_width(glyph.id);
            ok.then(|| convert(&self.image, advance))
        })
        .flatten()
        .flatten()
    }

    fn scale_into(&mut self, font: FontRef<'_>, request: &Request<'_>) -> bool {
        let Request { face, glyph, at } = *request;
        let is_color = face.is_color();
        let mut scaler = self
            .context
            .builder(font)
            .size(at.size)
            .hint(!is_color)
            .variations(face.weight_setting(glyph.weight))
            .build();
        let sources: &[Source] = if is_color { &COLOR_SOURCES } else { &TEXT_SOURCES };
        let mut render = Render::new(sources);
        let format = match at.antialias {
            Antialias::Grayscale => Format::Alpha,
            Antialias::Lcd => Format::Subpixel,
        };
        let _format = render.format(format);
        if at.subpixel > 0 {
            let x = f32::from(at.subpixel) / f32::from(SUBPIXEL_STEPS);
            let _offset = render.offset(Vector::new(x, 0.0));
        }
        if glyph.synth.embolden {
            let _bold = render.embolden(at.size * EMBOLDEN_FRACTION);
        }
        if glyph.synth.oblique {
            let slant = Transform::skew(Angle::from_degrees(OBLIQUE_DEGREES), Angle::ZERO);
            let _oblique = render.transform(Some(slant));
        }
        // The rasterizer only writes covered pixels: a reused buffer must start empty, or the
        // previous glyph shows through.
        self.image.clear();
        render.render_into(&mut scaler, glyph.id, &mut self.image)
    }
}

/// One glyph to rasterize.
#[derive(Clone, Copy)]
struct Request<'a> {
    face: &'a Face,
    glyph: Glyph,
    at: At,
}

/// The cacheable form of `image`.
fn convert(image: &Image, advance: f32) -> Option<GlyphImage> {
    let placement = image.placement;
    let width = usize::try_from(placement.width).ok()?;
    let height = usize::try_from(placement.height).ok()?;
    if width == 0 || height == 0 {
        return Some(GlyphImage::empty(Some(advance)));
    }
    let stride = aligned(placement.width)?;
    let padded = usize::try_from(stride).ok()?;
    let area = width.checked_mul(height)?;
    let pixels = match image.content {
        Content::Mask => {
            let rows = image.data.get(..area)?.chunks_exact(width);
            Pixels::Mask(pad_rows(rows, padded, |&coverage| coverage))
        }
        Content::SubpixelMask => {
            let rgba = image.data.get(..area.checked_mul(4)?)?;
            let rows = rgba.as_chunks::<4>().0.chunks_exact(width);
            Pixels::Subpixel(pad_rows(rows, padded, |&[r, g, b, _]| {
                u32::from_be_bytes([0, r, g, b])
            }))
        }
        Content::Color => {
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
        advance: Some(advance),
        pixels,
    })
}

/// `width` rounded up to `crate::blit::ROW_ALIGN`.
pub(crate) fn aligned(width: u32) -> Option<u32> {
    Some(width.checked_add(ROW_ALIGN - 1)? / ROW_ALIGN * ROW_ALIGN)
}

/// The rows converted by `pixel`, each padded with the default (transparent) value to `stride`.
pub(crate) fn pad_rows<'a, S: 'a, T: Default + Clone>(
    rows: impl ExactSizeIterator<Item = &'a [S]>,
    stride: usize,
    pixel: impl Fn(&S) -> T,
) -> Vec<T> {
    // Exactly the image's size: the glyph cache counts capacity.
    let mut out = Vec::with_capacity(rows.len().saturating_mul(stride));
    for row in rows {
        let start = out.len();
        out.extend(row.iter().map(&pixel));
        out.resize(start.saturating_add(stride), T::default());
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
    use crate::font::{Family, Presentation, TextStyle};

    fn at(antialias: Antialias) -> At {
        At { size: 26.0, fit: Fit { width: 99, height: 99 }, subpixel: 0, antialias }
    }

    /// The glyph as `swash` draws it into a fresh image, with no state from earlier glyphs.
    fn direct(fonts: &Fonts, glyph: Glyph) -> GlyphImage {
        Rasterizer::new().render(fonts, glyph, at(Antialias::Grayscale)).unwrap()
    }

    #[test]
    fn a_glyph_does_not_keep_pixels_of_the_one_before() {
        let mut fonts = Fonts::new(Family::Embedded, None).unwrap();
        let [r, a] = ['r', 'A'].map(|ch| fonts.glyph(ch, TextStyle::REGULAR, Presentation::Text));
        let mut reused = Rasterizer::new();
        let _r = reused.render(&fonts, r, at(Antialias::Grayscale)).unwrap();
        assert_eq!(reused.render(&fonts, a, at(Antialias::Grayscale)).unwrap(), direct(&fonts, a));
        let Pixels::Mask(mask) = direct(&fonts, a).pixels else { panic!("a coverage mask") };
        assert!(
            mask.contains(&255)
                && mask.iter().copied().filter(|c| *c == 0).count() > mask.len() / 3
        );
    }

    #[test]
    fn lcd_text_has_coverage_per_channel() {
        let mut fonts = Fonts::new(Family::Embedded, None).unwrap();
        let glyph = fonts.glyph('W', TextStyle::REGULAR, Presentation::Text);
        let image = Rasterizer::new().render(&fonts, glyph, at(Antialias::Lcd)).unwrap();
        let Pixels::Subpixel(pixels) = image.pixels else { panic!("subpixel coverage") };
        let fringed = pixels.iter().any(|&p| {
            let [_, r, g, b] = p.to_be_bytes();
            r != g || g != b
        });
        assert!(fringed, "the channels differ at the edges");
    }

    #[test]
    fn straight_alpha_is_premultiplied_and_premultiplied_is_kept() {
        assert_eq!(premultiply([255, 128, 0, 128], true), 0x8080_4000);
        assert_eq!(premultiply([200, 100, 0, 100], false), 0x6464_6400);
        assert_eq!(premultiply([10, 20, 30, 255], true), 0xFF0A_141E);
    }
}
