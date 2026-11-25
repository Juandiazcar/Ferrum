//! Blit kernels: blend a row of glyph coverage, or of premultiplied color pixels, into a row of
//! the target. The only module of the crate where `unsafe` is allowed (`docs/05` §4), for the
//! SIMD loads and stores and for calling a kernel whose CPU feature was detected at run time.
//!
//! Every kernel computes exactly what the scalar one does: per channel
//! `round((src × a + dst × (255 − a)) / 255)` for coverage `a`, and
//! `min(255, src + round(dst × (255 − α) / 255))` for a premultiplied source of alpha `α`, with the
//! division done as `(t + 128 + ((t + 128) >> 8)) >> 8`, which is exact for every `t ≤ 255²`. The
//! top byte of every written pixel is zero; target pixels are expected to have a zero top byte
//! already (everything `ferrum-render` writes has), and zero coverage leaves a pixel as it is.
//! Tests compare every available kernel with the scalar one on random rows.

#[cfg(target_arch = "aarch64")]
mod neon;
#[cfg(target_arch = "x86_64")]
mod x86;

/// Mask of the three color channels of a target pixel (`0x00RRGGBB`).
const RGB_MASK: u32 = 0x00FF_FFFF;

/// Glyph rows are stored padded to a multiple of this many pixels (zero coverage), the step of
/// the widest kernels (AVX2, NEON), so narrow glyphs are blended in whole SIMD steps.
pub(crate) const ROW_ALIGN: u32 = 8;

/// A rectangle to blend: `rows` rows of `width` pixels; consecutive rows are `dst_stride`
/// pixels apart in the target and `src_stride` apart in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    pub(crate) width: usize,
    pub(crate) rows: usize,
    pub(crate) dst_stride: usize,
    pub(crate) src_stride: usize,
}

/// Calls `blend` on every row of `shape` (rows that do not fit either buffer are skipped).
// perf: `render_repaint/*` — inlined into each kernel so the row loop runs with its CPU features
// and the row function is inlined into it.
#[allow(clippy::inline_always, reason = "see the perf comment: inlining is what this is for")]
#[inline(always)]
fn for_each_row<T>(
    dst: &mut [u32],
    src: &[T],
    shape: Shape,
    mut blend: impl FnMut(&mut [u32], &[T]),
) {
    if shape.dst_stride == 0 || shape.src_stride == 0 {
        return;
    }
    let rows = dst.chunks_mut(shape.dst_stride).zip(src.chunks(shape.src_stride)).take(shape.rows);
    for (dst, src) in rows {
        if let (Some(dst), Some(src)) = (dst.get_mut(..shape.width), src.get(..shape.width)) {
            blend(dst, src);
        }
    }
}

/// A set of blit kernels for this CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kernel {
    /// Portable code.
    #[cfg_attr(
        any(target_arch = "x86_64", target_arch = "aarch64"),
        allow(dead_code, reason = "built by the equivalence tests; these CPUs always get SIMD")
    )]
    Scalar,
    /// SSE2, 4 pixels per step (always present on `x86_64`).
    #[cfg(target_arch = "x86_64")]
    Sse2,
    /// AVX2, 8 pixels per step. Only built by [`Kernel::detect`] after a run-time check.
    #[cfg(target_arch = "x86_64")]
    Avx2,
    /// NEON, 8 pixels per step (always present on `aarch64`).
    #[cfg(target_arch = "aarch64")]
    Neon,
}

impl Kernel {
    /// The fastest kernel this CPU runs.
    pub(crate) fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if std::arch::is_x86_feature_detected!("avx2") { Self::Avx2 } else { Self::Sse2 }
        }
        #[cfg(target_arch = "aarch64")]
        {
            Self::Neon
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            Self::Scalar
        }
    }

    /// Every kernel this CPU runs, scalar first.
    #[cfg(test)]
    pub(crate) fn available() -> Vec<Self> {
        let mut kernels = vec![Self::Scalar];
        #[cfg(target_arch = "x86_64")]
        kernels.push(Self::Sse2);
        if Self::detect() != Self::Scalar && !kernels.contains(&Self::detect()) {
            kernels.push(Self::detect());
        }
        kernels
    }

    /// Blends `color` into `dst` with the coverage of `mask` (as many pixels as the shorter of
    /// the two).
    #[cfg(test)]
    pub(crate) fn blend_mask(self, dst: &mut [u32], mask: &[u8], color: u32) {
        let color = color & RGB_MASK;
        match self {
            Self::Scalar => blend_mask_scalar(dst, mask, color),
            #[cfg(target_arch = "x86_64")]
            // SAFETY: SSE2 is part of the `x86_64` baseline, so every such CPU runs it.
            Self::Sse2 => unsafe { x86::blend_mask_sse2(dst, mask, color) },
            #[cfg(target_arch = "x86_64")]
            // SAFETY: `Avx2` is only constructed by `detect` after `is_x86_feature_detected!`
            // reported AVX2, so the CPU executes the function's instructions.
            Self::Avx2 => unsafe { x86::blend_mask_avx2(dst, mask, color) },
            #[cfg(target_arch = "aarch64")]
            // SAFETY: NEON is part of the `aarch64` baseline, so every such CPU runs it.
            Self::Neon => unsafe { neon::blend_mask(dst, mask, color) },
        }
    }

    /// [`Kernel::blend_mask`] over the rows of `shape`; `dst` starts at the rectangle's
    /// top-left pixel.
    pub(crate) fn blend_mask_rect(self, dst: &mut [u32], mask: &[u8], color: u32, shape: Shape) {
        let color = color & RGB_MASK;
        match self {
            Self::Scalar => for_each_row(dst, mask, shape, |d, m| blend_mask_scalar(d, m, color)),
            #[cfg(target_arch = "x86_64")]
            // SAFETY: SSE2 is part of the `x86_64` baseline.
            Self::Sse2 => unsafe { x86::blend_mask_rect_sse2(dst, mask, color, shape) },
            #[cfg(target_arch = "x86_64")]
            // SAFETY: `Avx2` exists only when the CPU has AVX2 (see `blend_mask`).
            Self::Avx2 => unsafe { x86::blend_mask_rect_avx2(dst, mask, color, shape) },
            #[cfg(target_arch = "aarch64")]
            // SAFETY: NEON is part of the `aarch64` baseline.
            Self::Neon => unsafe { neon::blend_mask_rect(dst, mask, color, shape) },
        }
    }

    /// [`Kernel::blend_color`] over the rows of `shape`; `dst` starts at the rectangle's
    /// top-left pixel.
    pub(crate) fn blend_color_rect(self, dst: &mut [u32], src: &[u32], shape: Shape) {
        match self {
            Self::Scalar => for_each_row(dst, src, shape, blend_color_scalar),
            #[cfg(target_arch = "x86_64")]
            // SAFETY: SSE2 is part of the `x86_64` baseline.
            Self::Sse2 => unsafe { x86::blend_color_rect_sse2(dst, src, shape) },
            #[cfg(target_arch = "x86_64")]
            // SAFETY: `Avx2` exists only when the CPU has AVX2 (see `blend_mask`).
            Self::Avx2 => unsafe { x86::blend_color_rect_avx2(dst, src, shape) },
            #[cfg(target_arch = "aarch64")]
            // SAFETY: NEON is part of the `aarch64` baseline.
            Self::Neon => unsafe { neon::blend_color_rect(dst, src, shape) },
        }
    }

    /// Composites premultiplied `0xAARRGGBB` pixels of `src` over `dst` (as many as the shorter
    /// of the two).
    #[cfg(test)]
    pub(crate) fn blend_color(self, dst: &mut [u32], src: &[u32]) {
        match self {
            Self::Scalar => blend_color_scalar(dst, src),
            #[cfg(target_arch = "x86_64")]
            // SAFETY: SSE2 is part of the `x86_64` baseline.
            Self::Sse2 => unsafe { x86::blend_color_sse2(dst, src) },
            #[cfg(target_arch = "x86_64")]
            // SAFETY: as in `blend_mask`, `Avx2` exists only when the CPU has AVX2.
            Self::Avx2 => unsafe { x86::blend_color_avx2(dst, src) },
            #[cfg(target_arch = "aarch64")]
            // SAFETY: NEON is part of the `aarch64` baseline.
            Self::Neon => unsafe { neon::blend_color(dst, src) },
        }
    }
}

/// `round(t / 255)` for `t ≤ 255 × 255`.
const fn div255(t: u32) -> u32 {
    let t = t + 128;
    (t + (t >> 8)) >> 8
}

/// One pixel of [`Kernel::blend_mask`].
pub(crate) fn blend_mask_pixel(dst: u32, color: u32, coverage: u8) -> u32 {
    let a = u32::from(coverage);
    let mut out = 0;
    for shift in [0, 8, 16] {
        let s = (color >> shift) & 0xFF;
        let d = (dst >> shift) & 0xFF;
        out |= div255(s * a + d * (255 - a)) << shift;
    }
    out
}

/// One pixel of [`Kernel::blend_color`].
pub(crate) fn blend_color_pixel(dst: u32, src: u32) -> u32 {
    let inverse = 255 - (src >> 24);
    let mut out = 0;
    for shift in [0, 8, 16] {
        let s = (src >> shift) & 0xFF;
        let d = (dst >> shift) & 0xFF;
        out |= (s + div255(d * inverse)).min(255) << shift;
    }
    out
}

pub(crate) fn blend_mask_scalar(dst: &mut [u32], mask: &[u8], color: u32) {
    for (pixel, &coverage) in dst.iter_mut().zip(mask) {
        match coverage {
            0 => {}
            255 => *pixel = color,
            _ => *pixel = blend_mask_pixel(*pixel, color, coverage),
        }
    }
}

pub(crate) fn blend_color_scalar(dst: &mut [u32], src: &[u32]) {
    for (pixel, &source) in dst.iter_mut().zip(src) {
        *pixel = blend_color_pixel(*pixel, source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// xorshift64: deterministic test input without a dependency.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn pixel(&mut self) -> u32 {
            let [a, b, c, d, ..] = self.next().to_le_bytes();
            u32::from_le_bytes([a, b, c, d])
        }

        fn byte(&mut self) -> u8 {
            self.next().to_le_bytes()[0]
        }

        /// A coverage byte biased to the 0 and 255 fast paths half of the time.
        fn coverage(&mut self) -> u8 {
            match self.next() % 4 {
                0 => 0,
                1 => 255,
                _ => self.byte(),
            }
        }

        fn premultiplied(&mut self) -> u32 {
            let alpha = u32::from(self.coverage());
            let channel = |rng: &mut Self| div255(u32::from(rng.byte()) * alpha);
            alpha << 24 | channel(self) << 16 | channel(self) << 8 | channel(self)
        }
    }

    #[test]
    fn division_by_255_is_exact() {
        for t in 0..=255 * 255 {
            assert_eq!(div255(t), (t * 2 + 255) / 510, "{t}");
        }
    }

    #[test]
    fn full_and_zero_coverage_and_alpha_are_exact() {
        assert_eq!(blend_mask_pixel(0x0012_3456, 0x00AB_CDEF, 255), 0x00AB_CDEF);
        assert_eq!(blend_mask_pixel(0x0012_3456, 0x00AB_CDEF, 0), 0x0012_3456);
        assert_eq!(blend_color_pixel(0x0012_3456, 0xFFAB_CDEF), 0x00AB_CDEF);
        assert_eq!(blend_color_pixel(0x0012_3456, 0), 0x0012_3456);
    }

    #[test]
    fn every_kernel_equals_the_scalar_one() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let kernels = Kernel::available();
        assert!(kernels.contains(&Kernel::detect()));
        for len in (0..70).chain([255, 1024]) {
            let dst: Vec<u32> = (0..len).map(|_| rng.pixel() & RGB_MASK).collect();
            let mask: Vec<u8> = (0..len).map(|_| rng.coverage()).collect();
            let src: Vec<u32> = (0..len).map(|_| rng.premultiplied()).collect();
            let color = rng.pixel();
            let mut expected_mask = dst.clone();
            blend_mask_scalar(&mut expected_mask, &mask, color & RGB_MASK);
            let mut expected_color = dst.clone();
            blend_color_scalar(&mut expected_color, &src);
            for &kernel in &kernels {
                let mut out = dst.clone();
                kernel.blend_mask(&mut out, &mask, color);
                assert_eq!(out, expected_mask, "{kernel:?} mask, {len} pixels");
                let mut out = dst.clone();
                kernel.blend_color(&mut out, &src);
                assert_eq!(out, expected_color, "{kernel:?} color, {len} pixels");
            }
        }
    }

    #[test]
    fn rectangles_blend_every_row_with_its_strides() {
        let shape = Shape { width: 9, rows: 3, dst_stride: 20, src_stride: 16 };
        let mask: Vec<u8> = (0..48).map(|i| if i % 16 < 9 { 200 } else { 7 }).collect();
        let src: Vec<u32> =
            (0..48).map(|i| if i % 16 < 9 { 0xC080_4020 } else { 0xFF00_0000 }).collect();
        for kernel in Kernel::available() {
            let mut dst = vec![0x0010_2030; 60];
            kernel.blend_mask_rect(&mut dst, &mask, 0x00FF_FFFF, shape);
            for (i, &pixel) in dst.iter().enumerate() {
                let inside = i % 20 < 9;
                assert_eq!(
                    pixel,
                    if inside {
                        blend_mask_pixel(0x0010_2030, 0x00FF_FFFF, 200)
                    } else {
                        0x0010_2030
                    },
                    "{kernel:?} {i}"
                );
            }
            let mut dst = vec![0x0010_2030; 60];
            kernel.blend_color_rect(&mut dst, &src, shape);
            for (i, &pixel) in dst.iter().enumerate() {
                let inside = i % 20 < 9;
                assert_eq!(
                    pixel,
                    if inside { blend_color_pixel(0x0010_2030, 0xC080_4020) } else { 0x0010_2030 },
                    "{kernel:?} {i}"
                );
            }
        }
    }

    #[test]
    fn kernels_stop_at_the_shorter_row() {
        for kernel in Kernel::available() {
            let mut dst = vec![0; 9];
            kernel.blend_mask(&mut dst, &[255; 20], 0x00FF_FFFF);
            assert_eq!(dst, [0x00FF_FFFF; 9]);
            let mut dst = vec![0; 20];
            kernel.blend_color(&mut dst, &[0xFF00_00FF; 9]);
            assert_eq!(dst[..9], [0xFF; 9]);
            assert_eq!(dst[9..], [0; 11]);
        }
    }

    #[test]
    fn written_pixels_have_a_zero_top_byte() {
        for kernel in Kernel::available() {
            let mut dst = vec![0xFF00_0000; 17];
            kernel.blend_mask(&mut dst, &[128; 17], 0xFFFF_FFFF);
            assert!(dst.iter().all(|pixel| pixel >> 24 == 0), "{kernel:?}");
            let mut dst = vec![0; 17];
            kernel.blend_color(&mut dst, &[0x8080_8080; 17]);
            assert!(dst.iter().all(|pixel| pixel >> 24 == 0), "{kernel:?}");
        }
    }
}
