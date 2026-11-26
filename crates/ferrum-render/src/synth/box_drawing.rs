//! Box drawing (U+2500–U+257F), drawn on whole pixels so lines meet across cells.
//!
//! Geometry comes from the cell: a light line is the font's underline thickness, a heavy line
//! twice that, a double line two light lines one light line apart. Every line is centered in the
//! cell, so the same line has the same pixels in every cell and joins its neighbors. Arms meet at
//! the center; where a double line meets another line the strokes stop at, or reach to, the
//! other line as the joint requires (outer and inner corners of `╔`, `╦`, `╬`, …). Dashes split
//! the cell into equal segments with a gap of the line's own thickness (at most half a segment);
//! arcs (`╭╮╯╰`) and diagonals (`╱╲╳`) are anti-aliased.

use super::canvas::{Canvas, segment_distance, stroke};
use crate::font::metrics::f32_from;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Weight {
    None,
    Light,
    Heavy,
    Double,
}

/// Arms of U+2500–U+257F as `up right down left`: `.` none, `l` light, `h` heavy, `d` double.
/// Dashes, arcs and diagonals are `....` here and drawn by their own functions.
const ARMS: &[u8; 512] = b"\
.l.l.h.hl.l.h.h..................................ll..hl..lh..hh.\
..ll..lh..hl..hhll..lh..hl..hh..l..ll..hh..lh..hlll.lhl.hll.llh.\
hlh.hhl.lhh.hhh.l.lll.lhh.lll.hlh.hlh.lhl.hhh.hh.lll.llh.hll.hlh\
.lhl.lhh.hhl.hhhll.lll.hlh.llh.hhl.lhl.hhh.lhh.hlllllllhlhlllhlh\
hlllllhlhlhlhllhhhllllhhlhhlhhlhlhhhhlhhhhhlhhhh................\
.d.dd.d..dl..ld..dd...ld..dl..ddld..dl..dd..l..dd..ld..dldl.dld.\
ddd.l.ldd.dld.dd.dld.ldl.dddld.ddl.ldd.dldlddldldddd............\
...................ll....l....l....hh....h....h..h.ll.h..l.hh.l.";

fn weight(byte: u8) -> Weight {
    match byte {
        b'l' => Weight::Light,
        b'h' => Weight::Heavy,
        b'd' => Weight::Double,
        _ => Weight::None,
    }
}

/// Which end of an axis an arm goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    /// Toward 0: left or up.
    Near,
    /// Toward the far edge: right or down.
    Far,
}

/// Lines along one axis of the cell: horizontal arms (along x) or vertical ones (along y).
#[derive(Debug, Clone, Copy)]
struct Axis {
    /// Length of the cell along the arms.
    len: u32,
    /// Length of the cell across them.
    cross: u32,
    /// Arm toward 0 (left or up) and toward `len` (right or down).
    arms: [Weight; 2],
    /// The perpendicular arms on the 0 side and on the far side of `cross`.
    sides: [Weight; 2],
    light: u32,
}

impl Axis {
    fn thickness(&self, weight: Weight) -> u32 {
        match weight {
            Weight::None => 0,
            Weight::Light => self.light,
            Weight::Heavy => self.light.saturating_mul(2),
            Weight::Double => self.light.saturating_mul(3),
        }
    }

    /// The pixels of a centered stroke of `weight` across `len`.
    fn span(&self, len: u32, weight: Weight) -> (u32, u32) {
        let thickness = self.thickness(weight).min(len);
        let start = (len - thickness) / 2;
        (start, start + thickness)
    }

    /// The two strokes of a double line across `len`.
    fn doubles(&self, len: u32) -> [(u32, u32); 2] {
        let (start, end) = self.span(len, Weight::Double);
        [(start, start + self.light.min(end - start)), (end.saturating_sub(self.light), end)]
    }

    /// Where the perpendicular single (light or heavy) arms are, along this axis.
    fn single_sides(&self) -> Option<(u32, u32)> {
        let spans =
            self.sides.iter().filter(|&&side| matches!(side, Weight::Light | Weight::Heavy));
        spans.map(|&side| self.span(self.len, side)).reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
    }

    /// Where every perpendicular arm is, along this axis.
    fn all_sides(&self) -> Option<(u32, u32)> {
        let spans = self.sides.iter().filter(|&&side| side != Weight::None);
        spans.map(|&side| self.span(self.len, side)).reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
    }

    /// Calls `fill(along, across)` for every stroke of the two arms.
    fn draw(&self, fill: &mut impl FnMut((u32, u32), (u32, u32))) {
        let center = self.len / 2;
        for (end, &arm) in [End::Near, End::Far].into_iter().zip(&self.arms) {
            match arm {
                Weight::None => {}
                Weight::Double => {
                    for (line, across) in self.doubles(self.cross).into_iter().enumerate() {
                        fill(self.double_reach(end, line, center), across);
                    }
                }
                Weight::Light | Weight::Heavy => {
                    let sides = self.all_sides().unwrap_or((center, center));
                    let along = match end {
                        End::Near => (0, center.max(sides.1)),
                        End::Far => (center.min(sides.0), self.len),
                    };
                    fill(along, self.span(self.cross, arm));
                }
            }
        }
    }

    /// The extent of stroke `line` (0 nearer the 0 side) of a double arm.
    fn double_reach(&self, end: End, line: usize, center: u32) -> (u32, u32) {
        let same = self.sides.get(line).copied().unwrap_or(Weight::None);
        let other = self.sides.get(1 - line).copied().unwrap_or(Weight::None);
        let [near, far] = self.doubles(self.len);
        let single = self.single_sides();
        if end == End::Near {
            let end = if same == Weight::Double {
                near.1
            } else if other == Weight::Double {
                far.1
            } else {
                single.map_or(center, |span| span.1)
            };
            (0, end)
        } else {
            let start = if same == Weight::Double {
                far.0
            } else if other == Weight::Double {
                near.0
            } else {
                single.map_or(center, |span| span.0)
            };
            (start, self.len)
        }
    }
}

/// Draws `ch` (U+2500–U+257F) on a `width × height` cell with light lines of `light` pixels.
pub(crate) fn draw(ch: char, width: u32, height: u32, light: u32) -> Option<Canvas> {
    let offset = usize::try_from(u32::from(ch).checked_sub(0x2500)?).ok()?;
    let light = light.max(1);
    let mut canvas = Canvas::new(width, height);
    match ch {
        '\u{2504}'..='\u{250B}' | '\u{254C}'..='\u{254F}' => dashes(&mut canvas, ch, light),
        '\u{256D}'..='\u{2570}' => return Some(arc(canvas, ch, light)),
        '\u{2571}'..='\u{2573}' => diagonals(&mut canvas, ch, light),
        _ => {
            let arms = ARMS.get(offset * 4..offset * 4 + 4)?;
            let [up, right, down, left] =
                [0, 1, 2, 3].map(|i| weight(arms.get(i).copied().unwrap_or(b'.')));
            let horizontal =
                Axis { len: width, cross: height, arms: [left, right], sides: [up, down], light };
            let vertical =
                Axis { len: height, cross: width, arms: [up, down], sides: [left, right], light };
            horizontal.draw(&mut |x, y| canvas.fill(x, y, u8::MAX));
            vertical.draw(&mut |y, x| canvas.fill(x, y, u8::MAX));
        }
    }
    Some(canvas)
}

/// `┄┅┆┇┈┉┊┋╌╍╎╏`: 3, 4 or 2 dashes, light or heavy, horizontal or vertical.
fn dashes(canvas: &mut Canvas, ch: char, light: u32) {
    let (count, heavy, vertical) = match ch {
        '\u{2504}'..='\u{2507}' => (3, ch == '\u{2505}' || ch == '\u{2507}', ch >= '\u{2506}'),
        '\u{2508}'..='\u{250B}' => (4, ch == '\u{2509}' || ch == '\u{250B}', ch >= '\u{250A}'),
        _ => (2, ch == '\u{254D}' || ch == '\u{254F}', ch >= '\u{254E}'),
    };
    let weight = if heavy { Weight::Heavy } else { Weight::Light };
    let (len, cross) =
        if vertical { (canvas.height, canvas.width) } else { (canvas.width, canvas.height) };
    let axis = Axis { len, cross, arms: [weight; 2], sides: [Weight::None; 2], light };
    // The gap is the line's own thickness, at most half a segment so a dash is never empty.
    let gap = axis.thickness(weight).min(len / count / 2);
    let across = axis.span(cross, weight);
    for i in 0..count {
        let (start, end) = (len * i / count, len * (i + 1) / count);
        let dash = (start + gap / 2, end.saturating_sub(gap - gap / 2).max(start + gap / 2));
        if vertical {
            canvas.fill(across, dash, u8::MAX);
        } else {
            canvas.fill(dash, across, u8::MAX);
        }
    }
}

/// `╭╮╯╰`: a quarter circle from the middle of one edge to the middle of the next, then straight
/// to the edges. The straight parts use the same pixels as `│` and `─`, so arcs join them.
fn arc(mut canvas: Canvas, ch: char, light: u32) -> Canvas {
    let (width, height) = (canvas.width, canvas.height);
    // Which way the arc opens: toward the right (`╭╰`) or left, toward the bottom (`╭╮`) or top.
    let (horizontal, vertical) = match ch {
        '\u{256D}' => (End::Far, End::Far),
        '\u{256E}' => (End::Near, End::Far),
        '\u{256F}' => (End::Near, End::Near),
        _ => (End::Far, End::Near),
    };
    let axis = Axis {
        len: width,
        cross: height,
        arms: [Weight::None; 2],
        sides: [Weight::None; 2],
        light,
    };
    let (x0, x1) = axis.span(width, Weight::Light);
    let (y0, y1) = axis.span(height, Weight::Light);
    let (mid_x, mid_y) = (f32_from(x0 + x1) / 2.0, f32_from(y0 + y1) / 2.0);
    // The arc stops half a pixel short of the nearest edge, rounded down, so the last pixels at
    // every edge are straight and match the neighbor cell exactly.
    let room = mid_x.min(f32_from(width) - mid_x).min(mid_y).min(f32_from(height) - mid_y);
    let radius = (room - 0.5).floor().max(0.0);
    let center = (mid_x + sign(horizontal) * radius, mid_y + sign(vertical) * radius);
    let quarter =
        Quarter { center, radius, opens: (horizontal, vertical), thickness: f32_from(light) };
    quarter.draw(&mut canvas);
    let down = match vertical {
        End::Far => (to_u32(center.1.floor()), height),
        End::Near => (0, to_u32(center.1.ceil())),
    };
    let across = match horizontal {
        End::Far => (to_u32(center.0.floor()), width),
        End::Near => (0, to_u32(center.0.ceil())),
    };
    canvas.fill((x0, x1), down, u8::MAX);
    canvas.fill(across, (y0, y1), u8::MAX);
    canvas
}

/// `-1.0` toward 0, `1.0` toward the far edge.
fn sign(end: End) -> f32 {
    match end {
        End::Near => -1.0,
        End::Far => 1.0,
    }
}

/// A quarter of the circle at `center`, the one that opens toward `opens` (horizontal,
/// vertical), stroked `thickness` wide.
struct Quarter {
    center: (f32, f32),
    radius: f32,
    opens: (End, End),
    thickness: f32,
}

impl Quarter {
    fn draw(&self, canvas: &mut Canvas) {
        let (center, radius, thickness) = (self.center, self.radius, self.thickness);
        let (horizontal, vertical) = (sign(self.opens.0), sign(self.opens.1));
        canvas.cover(|x, y| {
            if (x - center.0) * horizontal > 0.0 || (y - center.1) * vertical > 0.0 {
                return 0.0;
            }
            let distance = ((x - center.0).powi(2) + (y - center.1).powi(2)).sqrt() - radius;
            stroke(distance, thickness)
        });
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped at 0; cell-sized"
)]
fn to_u32(value: f32) -> u32 {
    value.max(0.0) as u32
}

/// `╱╲╳`: corner-to-corner lines.
fn diagonals(canvas: &mut Canvas, ch: char, light: u32) {
    let (width, height) = (f32_from(canvas.width), f32_from(canvas.height));
    let thickness = f32_from(light);
    let rising = ch != '\u{2572}';
    let falling = ch != '\u{2571}';
    canvas.cover(|x, y| {
        let up = if rising {
            stroke(segment_distance((x, y), (width, 0.0), (0.0, height)), thickness)
        } else {
            0.0
        };
        let down = if falling {
            stroke(segment_distance((x, y), (0.0, 0.0), (width, height)), thickness)
        } else {
            0.0
        };
        up.max(down)
    });
}

#[cfg(test)]
mod tests;
