//! Cell metrics: the pixel grid every glyph, line and cursor is placed on.
//!
//! Everything is derived from the primary face at its regular weight and the [`FontSpec`]; no
//! size is chosen here. The cell width is the advance of `0` (the CSS `ch` unit) plus the letter
//! spacing; the height is ascent + descent + line gap times the line height, with the extra
//! space split above and below the text (CSS half-leading). Underline and strikeout come from
//! the font's own `post` and `OS/2` values. All values are whole physical pixels, so every cell
//! starts on a pixel boundary.

use swash::FontRef;

use crate::error::RenderError;

/// Largest effective font size in physical pixels. The settings allow at most 72 pt
/// (`docs/14` Q-30) at a 200 % UI scale (`docs/09` §11), 144 logical pixels, which a scale factor
/// of 4 (above the 2× and 3× displays in use) makes 576 px; 1 024 px keeps headroom while a single
/// glyph mask stays under 4 MiB and a hostile caller cannot make the renderer allocate more.
pub const MAX_FONT_PX: f32 = 1024.0;

/// What the caller asks for, in plain values (`ferrum-render` takes no config types, `docs/04`
/// §2). Sizes are logical pixels; the scale factor turns them into physical pixels (`HiDPI`,
/// `FR-UI-013`, fractional factors included). The font size setting in points is passed 1:1 as
/// logical pixels (maintainer decision Q-36, `docs/14` §2: 13 pt = 13 logical px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontSpec {
    /// Font size (pixels per em) in logical pixels.
    pub size_px: f32,
    /// Display scale factor (1.0, 1.25, 2.0, …).
    pub scale_factor: f32,
    /// Line height as a multiple of the font's natural height.
    pub line_height: f32,
    /// Extra space between cells in logical pixels (may be negative).
    pub letter_spacing_px: f32,
}

impl FontSpec {
    /// Pixels per em in physical pixels, if the spec is usable.
    pub(crate) fn physical_size(&self) -> Result<f32, RenderError> {
        let size = self.size_px * self.scale_factor;
        let usable = size.is_finite()
            && size > 0.0
            && size <= MAX_FONT_PX
            && self.line_height.is_finite()
            && self.line_height > 0.0
            && self.letter_spacing_px.is_finite();
        if usable { Ok(size) } else { Err(RenderError::InvalidFontSpec) }
    }
}

/// Geometry of one cell, in physical pixels, measured from the cell's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellMetrics {
    /// Cell width: the font's advance plus the letter spacing.
    pub width: u32,
    /// The font's own advance of `0`, rounded, without the letter spacing: what a glyph's
    /// advance is compared with to count its cells.
    pub advance: u32,
    /// Cell height.
    pub height: u32,
    /// Baseline, from the top of the cell.
    pub baseline: u32,
    /// Top of the underline stroke.
    pub underline_top: u32,
    /// Thickness of underline, strikeout and the thin cursor shapes.
    pub stroke: u32,
    /// Top of the strikeout stroke.
    pub strikeout_top: u32,
    /// The font size these metrics were made for, in 1/64 physical pixel.
    pub(crate) size_q: u32,
}

/// Physical pixels per em in the fixed point of [`CellMetrics::size_q`] (26.6, as `FreeType`).
pub(crate) const SIZE_Q_ONE: f32 = 64.0;

impl CellMetrics {
    /// Computes the cell of `font` at `spec`.
    pub(crate) fn of(font: FontRef<'_>, spec: &FontSpec) -> Result<Self, RenderError> {
        let size = spec.physical_size()?;
        let metrics = font.metrics(&[]).scale(size);
        let zero = font.charmap().map('0');
        let advance = font.glyph_metrics(&[]).scale(size).advance_width(zero);
        let width = to_px(advance + spec.letter_spacing_px * spec.scale_factor).max(1);
        let own_advance = to_px(advance).max(1);
        let natural = metrics.ascent + metrics.descent + metrics.leading;
        let height = to_px(natural * spec.line_height).max(1);
        let half_leading = (f32_from(height) - (metrics.ascent + metrics.descent)) / 2.0;
        let baseline = to_px(half_leading + metrics.ascent).min(height.saturating_sub(1));
        let stroke = to_px(metrics.stroke_size).max(1);
        let last_line = height.saturating_sub(stroke);
        let below = |offset: f32| to_px(f32_from(baseline) - offset).min(last_line);
        Ok(Self {
            width,
            advance: own_advance,
            height,
            baseline,
            underline_top: below(metrics.underline_offset),
            stroke,
            strikeout_top: below(metrics.strikeout_offset),
            size_q: to_px(size * SIZE_Q_ONE),
        })
    }
}

/// Rounds a non-negative length to whole pixels (negative and NaN give 0, `MAX_FONT_PX` bounds
/// every length a font can produce).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rounded, clamped to 0..=u32::MAX first; `as` saturates"
)]
pub(crate) fn to_px(length: f32) -> u32 {
    length.round().max(0.0) as u32
}

/// A pixel count as a float, exact up to 2^24 (far above any cell or buffer side).
#[allow(clippy::cast_precision_loss, reason = "pixel counts are far below 2^24")]
pub(crate) fn f32_from(pixels: u32) -> f32 {
    pixels as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::face::EMBEDDED;

    fn spec(size_px: f32, scale_factor: f32) -> FontSpec {
        FontSpec { size_px, scale_factor, line_height: 1.0, letter_spacing_px: 0.0 }
    }

    fn cell(spec: &FontSpec) -> Result<CellMetrics, RenderError> {
        let font = FontRef::from_index(EMBEDDED, 0).ok_or(RenderError::InvalidFontSpec)?;
        CellMetrics::of(font, spec)
    }

    #[test]
    fn jetbrains_mono_at_12_px_is_a_7_by_16_cell() {
        // Spike S3 measured the same font at 12 px: 7×16 cells (ADR 0025).
        let metrics = cell(&spec(12.0, 1.0));
        assert!(matches!(metrics, Ok(CellMetrics { width: 7, height: 16, .. })), "{metrics:?}");
    }

    #[test]
    fn strokes_and_baseline_stay_inside_the_cell() {
        for size in [6.0, 9.5, 13.0, 18.0, 72.0] {
            for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
                let Ok(m) = cell(&spec(size, scale)) else { panic!("{size} {scale}") };
                assert!(m.baseline < m.height);
                assert!(m.underline_top + m.stroke <= m.height, "{m:?}");
                assert!(m.strikeout_top + m.stroke <= m.height, "{m:?}");
                assert!(m.strikeout_top < m.baseline && m.underline_top >= m.baseline, "{m:?}");
            }
        }
    }

    #[test]
    fn hidpi_scales_the_cell() {
        let (Ok(one), Ok(two)) = (cell(&spec(13.0, 1.0)), cell(&spec(13.0, 2.0))) else {
            panic!("metrics")
        };
        assert!(two.width >= 2 * one.width - 1 && two.width <= 2 * one.width + 1);
        assert!(two.height >= 2 * one.height - 1 && two.height <= 2 * one.height + 1);
        assert_eq!(two.size_q, 26 * 64);
    }

    #[test]
    fn line_height_and_letter_spacing_grow_the_cell() {
        let Ok(base) = cell(&spec(13.0, 1.0)) else { panic!("metrics") };
        let tall = cell(&FontSpec { line_height: 1.5, letter_spacing_px: 2.0, ..spec(13.0, 1.0) });
        let Ok(tall) = tall else { panic!("metrics") };
        assert_eq!(tall.width, base.width + 2);
        assert!(tall.height > base.height);
        assert!(tall.baseline > base.baseline, "half-leading moves the text down");
    }

    #[test]
    fn unusable_specs_are_refused() {
        for bad in [spec(0.0, 1.0), spec(f32::NAN, 1.0), spec(13.0, 0.0), spec(600.0, 2.0)] {
            assert_eq!(cell(&bad), Err(RenderError::InvalidFontSpec));
        }
        let bad = FontSpec { line_height: f32::INFINITY, ..spec(13.0, 1.0) };
        assert_eq!(cell(&bad), Err(RenderError::InvalidFontSpec));
    }
}
