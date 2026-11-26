//! A three-pass box blur of an alpha mask, the usual approximation of a Gaussian: each pass
//! averages a window of `2r + 1` pixels along rows, then along columns, with `r` a third of the
//! blur distance, so the three passes together spread an edge over the whole distance.

/// Blurs the `side × side` mask `alpha` over `distance` pixels (nothing for 0).
pub(crate) fn blur(alpha: &mut [u8], side: u32, distance: u32) {
    let radius = usize::try_from(distance.div_ceil(3)).unwrap_or(0);
    let side = usize::try_from(side).unwrap_or(0);
    if radius == 0 || side == 0 {
        return;
    }
    let mut line = vec![0; side];
    for _ in 0..3 {
        for row in alpha.chunks_mut(side) {
            box_pass(row, &mut line, radius);
        }
        for x in 0..side {
            let mut column: Vec<u8> = alpha.iter().skip(x).step_by(side).copied().collect();
            box_pass(&mut column, &mut line, radius);
            for (pixel, value) in alpha.iter_mut().skip(x).step_by(side).zip(column) {
                *pixel = value;
            }
        }
    }
}

/// One box pass over `values` (pixels past the ends count as transparent).
fn box_pass(values: &mut [u8], scratch: &mut [u8], radius: usize) {
    let window = u32::try_from(2 * radius + 1).unwrap_or(u32::MAX);
    let at = |values: &[u8], index: Option<usize>| {
        index.and_then(|i| values.get(i)).map_or(0, |&v| u32::from(v))
    };
    let mut sum: u32 = (0..=radius).map(|i| at(values, Some(i))).sum();
    for (i, out) in scratch.iter_mut().enumerate().take(values.len()) {
        *out = u8::try_from((sum + window / 2) / window).unwrap_or(u8::MAX);
        sum += at(values, Some(i + radius + 1));
        sum -= at(values, i.checked_sub(radius));
    }
    values.copy_from_slice(scratch.get(..values.len()).unwrap_or(&[]));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blur_spreads_an_edge_and_keeps_the_middle_solid() {
        let side = 31;
        let mut alpha = vec![0; 31 * 31];
        for y in 8..23 {
            for x in 8..23 {
                alpha[y * 31 + x] = 255;
            }
        }
        blur(&mut alpha, side, 6);
        assert_eq!(alpha[15 * 31 + 15], 255, "the middle stays solid");
        assert!(alpha[15 * 31 + 5] > 0, "the edge spreads out");
        assert!(alpha[15 * 31 + 7] < 255 && alpha[15 * 31 + 9] > alpha[15 * 31 + 7]);
        assert_eq!(alpha[0], 0, "past the distance nothing");
    }

    #[test]
    fn no_distance_changes_nothing() {
        let mut alpha = vec![7, 0, 255, 3];
        blur(&mut alpha, 2, 0);
        assert_eq!(alpha, [7, 0, 255, 3]);
    }
}
