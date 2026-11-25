//! Text decorations as cell-sized masks: underline (single, double, curly, dotted, dashed),
//! strikeout and overline. Positions and the stroke come from the font (`crate::font::metrics`);
//! shapes are derived from the cell: the curly wave is one period per cell, so it continues
//! across cells, and the dashed line is one dash per cell, centered.

use super::canvas::{Canvas, stroke};
use crate::font::metrics::f32_from;

/// How a cell is underlined (SGR 4 and its `4:n` forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Underline {
    /// Not underlined.
    #[default]
    None,
    /// One line.
    Single,
    /// Two lines.
    Double,
    /// A wave.
    Curly,
    /// Dots.
    Dotted,
    /// Dashes.
    Dashed,
}

/// One decoration of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Decoration {
    Underline(Underline),
    Strikeout,
    Overline,
}

/// Where the decorations of a cell go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Lines {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) underline_top: u32,
    pub(crate) strikeout_top: u32,
    pub(crate) stroke: u32,
}

/// The mask of `decoration` on a cell, or `None` for no underline.
pub(crate) fn draw(decoration: Decoration, lines: &Lines) -> Option<Canvas> {
    let mut canvas = Canvas::new(lines.width, lines.height);
    let full = (0, lines.width);
    let band = |top: u32| (top, top.saturating_add(lines.stroke));
    match decoration {
        Decoration::Strikeout => canvas.fill(full, band(lines.strikeout_top), u8::MAX),
        Decoration::Overline => canvas.fill(full, band(0), u8::MAX),
        Decoration::Underline(Underline::None) => return None,
        Decoration::Underline(Underline::Single) => {
            canvas.fill(full, band(lines.underline_top), u8::MAX);
        }
        Decoration::Underline(Underline::Double) => {
            // Two strokes one stroke apart; moved up if the cell is too short below.
            let span = lines.stroke.saturating_mul(3);
            let top = lines.underline_top.min(lines.height.saturating_sub(span));
            canvas.fill(full, band(top), u8::MAX);
            canvas.fill(full, band(top + 2 * lines.stroke), u8::MAX);
        }
        Decoration::Underline(Underline::Dotted) => {
            let dot = lines.stroke.max(1);
            for x in (0..lines.width).step_by(usize::try_from(dot * 2).unwrap_or(2)) {
                canvas.fill((x, x + dot), band(lines.underline_top), u8::MAX);
            }
        }
        Decoration::Underline(Underline::Dashed) => {
            canvas.fill(
                (lines.width / 4, lines.width - lines.width / 4),
                band(lines.underline_top),
                u8::MAX,
            );
        }
        Decoration::Underline(Underline::Curly) => curly(&mut canvas, lines),
    }
    Some(canvas)
}

/// A sine wave of one period per cell between the underline position and the cell bottom.
fn curly(canvas: &mut Canvas, lines: &Lines) {
    let thickness = f32_from(lines.stroke.max(1));
    let room = f32_from(lines.height.saturating_sub(lines.underline_top));
    let amplitude = ((room - thickness) / 2.0).max(thickness / 2.0);
    let middle = f32_from(lines.underline_top) + thickness / 2.0 + amplitude;
    let period = f32_from(lines.width.max(1));
    let tau = std::f32::consts::TAU;
    canvas.cover(|x, y| {
        let phase = tau * x / period;
        let wave = middle - amplitude * phase.sin();
        let slope = amplitude * tau / period * phase.cos();
        // Vertical distance scaled to the perpendicular one, so the stroke keeps its thickness.
        stroke((y - wave) / (1.0 + slope * slope).sqrt(), thickness)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINES: Lines =
        Lines { width: 8, height: 17, underline_top: 14, strikeout_top: 8, stroke: 1 };

    fn rows_lit(canvas: &Canvas) -> Vec<u32> {
        (0..canvas.height)
            .filter(|&y| {
                (0..canvas.width).any(|x| canvas.pixels[(y * canvas.width + x) as usize] > 0)
            })
            .collect()
    }

    #[test]
    fn straight_decorations_sit_on_the_font_positions() {
        let single = draw(Decoration::Underline(Underline::Single), &LINES).unwrap();
        assert_eq!(rows_lit(&single), [14]);
        assert_eq!(rows_lit(&draw(Decoration::Strikeout, &LINES).unwrap()), [8]);
        assert_eq!(rows_lit(&draw(Decoration::Overline, &LINES).unwrap()), [0]);
        assert_eq!(
            rows_lit(&draw(Decoration::Underline(Underline::Double), &LINES).unwrap()),
            [14, 16]
        );
        assert!(draw(Decoration::Underline(Underline::None), &LINES).is_none());
    }

    #[test]
    fn a_double_underline_moves_up_to_fit() {
        let low = Lines { underline_top: 16, ..LINES };
        assert_eq!(
            rows_lit(&draw(Decoration::Underline(Underline::Double), &low).unwrap()),
            [14, 16]
        );
    }

    #[test]
    fn the_curly_wave_joins_the_next_cell() {
        let curly = draw(Decoration::Underline(Underline::Curly), &LINES).unwrap();
        let column =
            |x: u32| (0..17).map(|y| curly.pixels[(y * 8 + x) as usize]).collect::<Vec<_>>();
        let rows = rows_lit(&curly);
        assert!(rows.iter().all(|&y| y >= 14), "below the underline position: {rows:?}");
        // One period per cell: the first column continues where the last one ends.
        let (first, last) = (column(0), column(7));
        let first_peak = first.iter().position(|&p| p == *first.iter().max().unwrap()).unwrap();
        let last_peak = last.iter().position(|&p| p == *last.iter().max().unwrap()).unwrap();
        assert!(first_peak.abs_diff(last_peak) <= 1, "{first:?} {last:?}");
    }

    #[test]
    fn dots_and_dashes_leave_gaps() {
        let dotted = draw(Decoration::Underline(Underline::Dotted), &LINES).unwrap();
        assert_eq!(&dotted.pixels[14 * 8..15 * 8], [255, 0, 255, 0, 255, 0, 255, 0]);
        let dashed = draw(Decoration::Underline(Underline::Dashed), &LINES).unwrap();
        assert_eq!(&dashed.pixels[14 * 8..15 * 8], [0, 0, 255, 255, 255, 255, 0, 0]);
    }
}
