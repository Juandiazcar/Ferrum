//! Block elements (U+2580–U+259F) on whole pixels: eighths are rounded to the pixel, and every
//! half (`▀ ▄ ▌ ▐`) and quadrant uses the same split of the cell, so complementary blocks tile it
//! exactly once, at odd sizes too.

use super::canvas::Canvas;

/// Coverage of the shades `░▒▓`: a quarter, a half and three quarters of full coverage, the
/// densities the Unicode code chart's reference glyphs show.
const SHADES: [u8; 3] = [64, 128, 191];

/// Eighths of `len`, rounded to whole pixels.
fn eighths(len: u32, n: u32) -> u32 {
    (len * n + 4) / 8
}

/// Draws `ch` (U+2580–U+259F) on a `width × height` cell.
pub(crate) fn draw(ch: char, width: u32, height: u32) -> Option<Canvas> {
    let mut canvas = Canvas::new(width, height);
    let full = u8::MAX;
    // The halves: `▌` is four eighths from the left, `▄` four eighths from the bottom.
    let (mid_x, mid_y) = (eighths(width, 4), height - eighths(height, 4));
    match ch {
        '\u{2580}' => canvas.fill((0, width), (0, mid_y), full),
        '\u{2581}'..='\u{2588}' => {
            let n = u32::from(ch) - 0x2580;
            canvas.fill((0, width), (height - eighths(height, n), height), full);
        }
        '\u{2589}'..='\u{258F}' => {
            let n = 0x2590 - u32::from(ch);
            canvas.fill((0, eighths(width, n)), (0, height), full);
        }
        '\u{2590}' => canvas.fill((mid_x, width), (0, height), full),
        '\u{2591}'..='\u{2593}' => {
            let shade = SHADES.get(usize::try_from(u32::from(ch) - 0x2591).ok()?).copied()?;
            canvas.fill((0, width), (0, height), shade);
        }
        '\u{2594}' => canvas.fill((0, width), (0, eighths(height, 1)), full),
        '\u{2595}' => canvas.fill((width - eighths(width, 1), width), (0, height), full),
        '\u{2596}'..='\u{259F}' => quadrants(&mut canvas, ch, (mid_x, mid_y)),
        _ => return None,
    }
    Some(canvas)
}

/// `▖▗▘▙▚▛▜▝▞▟`: which of upper left, upper right, lower left, lower right are filled.
fn quadrants(canvas: &mut Canvas, ch: char, (mid_x, mid_y): (u32, u32)) {
    let [upper_left, upper_right, lower_left, lower_right] = match ch {
        '\u{2596}' => [false, false, true, false],
        '\u{2597}' => [false, false, false, true],
        '\u{2598}' => [true, false, false, false],
        '\u{2599}' => [true, false, true, true],
        '\u{259A}' => [true, false, false, true],
        '\u{259B}' => [true, true, true, false],
        '\u{259C}' => [true, true, false, true],
        '\u{259D}' => [false, true, false, false],
        '\u{259E}' => [false, true, true, false],
        _ => [false, true, true, true],
    };
    let (width, height) = (canvas.width, canvas.height);
    let parts = [
        (upper_left, (0, mid_x), (0, mid_y)),
        (upper_right, (mid_x, width), (0, mid_y)),
        (lower_left, (0, mid_x), (mid_y, height)),
        (lower_right, (mid_x, width), (mid_y, height)),
    ];
    for (filled, x, y) in parts {
        if filled {
            canvas.fill(x, y, u8::MAX);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage(canvas: &Canvas) -> u32 {
        canvas.pixels.iter().map(|&p| u32::from(p)).sum()
    }

    #[test]
    fn eighth_blocks_are_rounded_to_whole_pixels() {
        let lower_half = draw('\u{2584}', 8, 17).unwrap();
        let upper_half = draw('\u{2580}', 8, 17).unwrap();
        // ▄ and ▀ of the same cell never overlap and together cover it exactly once.
        let sum: Vec<u16> = lower_half
            .pixels
            .iter()
            .zip(&upper_half.pixels)
            .map(|(&a, &b)| u16::from(a) + u16::from(b))
            .collect();
        assert!(sum.iter().all(|&p| p == 255));
        assert_eq!(coverage(&draw('█', 8, 17).unwrap()), 8 * 17 * 255);
        assert_eq!(coverage(&draw('▁', 8, 16).unwrap()), 8 * 2 * 255);
        assert_eq!(coverage(&draw('▏', 8, 16).unwrap()), 16 * 255);
    }

    #[test]
    fn complementary_blocks_tile_odd_cells_exactly_once() {
        let pairs =
            [('▀', '▄'), ('▌', '▐'), ('▘', '▟'), ('▝', '▙'), ('▖', '▜'), ('▗', '▛'), ('▚', '▞')];
        for (width, height) in [(7, 15), (9, 17), (5, 11), (8, 16)] {
            for (a, b) in pairs {
                let (a, b) = (draw(a, width, height).unwrap(), draw(b, width, height).unwrap());
                let sum: Vec<u16> = a
                    .pixels
                    .iter()
                    .zip(&b.pixels)
                    .map(|(&p, &q)| u16::from(p) + u16::from(q))
                    .collect();
                assert!(sum.iter().all(|&p| p == 255), "{width}x{height}: {sum:?}");
            }
        }
    }

    #[test]
    fn shades_and_quadrants() {
        assert_eq!(draw('▒', 2, 2).unwrap().pixels, [128; 4]);
        assert_eq!(draw('▚', 2, 2).unwrap().pixels, [255, 0, 0, 255]);
        assert_eq!(draw('▟', 2, 2).unwrap().pixels, [0, 255, 255, 255]);
        assert!(draw('a', 2, 2).is_none());
    }
}
