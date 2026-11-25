//! SSE2 and AVX2 kernels. Pixels are unpacked to 16-bit lanes (two or four pixels per 128 bits),
//! blended with the exact division of the parent module, and packed back.
// perf: `render_repaint/*` (benches/repaint.rs) — 4 or 8 pixels per step instead of 1.

use std::arch::x86_64::{
    __m128i, __m256i, _mm_add_epi16, _mm_adds_epu8, _mm_and_si128, _mm_cvtsi32_si128,
    _mm_loadl_epi64, _mm_loadu_si128, _mm_mullo_epi16, _mm_packus_epi16, _mm_set1_epi16,
    _mm_set1_epi32, _mm_setzero_si128, _mm_shufflehi_epi16, _mm_shufflelo_epi16, _mm_srli_epi16,
    _mm_storeu_si128, _mm_sub_epi16, _mm_unpackhi_epi8, _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    _mm256_add_epi16, _mm256_adds_epu8, _mm256_and_si256, _mm256_cvtepu8_epi32, _mm256_loadu_si256,
    _mm256_mullo_epi16, _mm256_mullo_epi32, _mm256_packus_epi16, _mm256_set1_epi16,
    _mm256_set1_epi32, _mm256_setzero_si256, _mm256_shufflehi_epi16, _mm256_shufflelo_epi16,
    _mm256_srli_epi16, _mm256_storeu_si256, _mm256_sub_epi16, _mm256_unpackhi_epi8,
    _mm256_unpacklo_epi8,
};

use super::{RGB_MASK, Shape, blend_color_scalar, blend_mask_scalar, for_each_row};

/// Shuffle control that copies 16-bit lane 3 of every group of four (a pixel's alpha).
const ALPHA_LANES: i32 = 0b11_11_11_11;
/// `RGB_MASK` as the `i32` the `set1` intrinsics take (same bits).
#[allow(clippy::cast_possible_wrap, reason = "bit pattern, not a number")]
const RGB_MASK_I32: i32 = RGB_MASK as i32;
/// `0x01010101`: multiplying a byte by it repeats the byte in all four.
const REPEAT_BYTE: i32 = 0x0101_0101;

#[allow(clippy::cast_possible_wrap, reason = "bit pattern, not a number")]
const fn bits(pixel: u32) -> i32 {
    pixel as i32
}

/// `round((s × a + d × (255 − a)) / 255)` on eight 16-bit lanes.
#[target_feature(enable = "sse2")]
fn mix_128(d: __m128i, s: __m128i, a: __m128i) -> __m128i {
    let inverse = _mm_sub_epi16(_mm_set1_epi16(255), a);
    let t = _mm_add_epi16(_mm_mullo_epi16(s, a), _mm_mullo_epi16(d, inverse));
    let t = _mm_add_epi16(t, _mm_set1_epi16(128));
    _mm_srli_epi16(_mm_add_epi16(t, _mm_srli_epi16(t, 8)), 8)
}

/// `round(d × inverse / 255)` on eight 16-bit lanes.
#[target_feature(enable = "sse2")]
fn scale_128(d: __m128i, inverse: __m128i) -> __m128i {
    let t = _mm_add_epi16(_mm_mullo_epi16(d, inverse), _mm_set1_epi16(128));
    _mm_srli_epi16(_mm_add_epi16(t, _mm_srli_epi16(t, 8)), 8)
}

/// Coverage of four pixels, each byte repeated over its pixel's four channels.
#[target_feature(enable = "sse2")]
fn spread_4(coverage: [u8; 4]) -> __m128i {
    let m = _mm_cvtsi32_si128(i32::from_le_bytes(coverage));
    let m = _mm_unpacklo_epi8(m, m);
    _mm_unpacklo_epi16(m, m)
}

/// Callable only where SSE2 is enabled (`unsafe` at the call site; SSE2 is part of `x86_64`).
#[inline]
#[target_feature(enable = "sse2")]
pub(super) fn blend_mask_sse2(dst: &mut [u32], mask: &[u8], color: u32) {
    let len = dst.len().min(mask.len());
    let (Some((dst, _)), Some((mask, _))) =
        (dst.split_at_mut_checked(len), mask.split_at_checked(len))
    else {
        return;
    };
    let zero = _mm_setzero_si128();
    let rgb = _mm_set1_epi32(RGB_MASK_I32);
    let source = _mm_unpacklo_epi8(_mm_set1_epi32(bits(color)), zero);
    let (blocks, pixels_rest) = dst.as_chunks_mut::<4>();
    let (covers, coverage_rest) = mask.as_chunks::<4>();
    for (block, &cover) in blocks.iter_mut().zip(covers) {
        if cover == [0; 4] {
            continue;
        }
        let a = spread_4(cover);
        // SAFETY: `block` is 4 `u32`, exactly the 16 bytes read and written; unaligned access is
        // what `loadu`/`storeu` are for.
        unsafe {
            let d = _mm_loadu_si128(block.as_ptr().cast());
            let low = mix_128(_mm_unpacklo_epi8(d, zero), source, _mm_unpacklo_epi8(a, zero));
            let high = mix_128(_mm_unpackhi_epi8(d, zero), source, _mm_unpackhi_epi8(a, zero));
            let out = _mm_and_si128(_mm_packus_epi16(low, high), rgb);
            _mm_storeu_si128(block.as_mut_ptr().cast(), out);
        }
    }
    blend_mask_scalar(pixels_rest, coverage_rest, color);
}

/// As [`blend_mask_sse2`]: SSE2 only.
#[inline]
#[target_feature(enable = "sse2")]
pub(super) fn blend_color_sse2(dst: &mut [u32], src: &[u32]) {
    let len = dst.len().min(src.len());
    let (Some((dst, _)), Some((src, _))) =
        (dst.split_at_mut_checked(len), src.split_at_checked(len))
    else {
        return;
    };
    let zero = _mm_setzero_si128();
    let rgb = _mm_set1_epi32(RGB_MASK_I32);
    let full = _mm_set1_epi16(255);
    let (blocks, pixels_rest) = dst.as_chunks_mut::<4>();
    let (sources, sources_rest) = src.as_chunks::<4>();
    for (block, source) in blocks.iter_mut().zip(sources) {
        // SAFETY: `block` and `source` are 4 `u32` each, the 16 bytes read and written.
        unsafe {
            let s = _mm_loadu_si128(source.as_ptr().cast());
            let d = _mm_loadu_si128(block.as_ptr().cast());
            let alpha = |lanes: __m128i| {
                let lanes = _mm_shufflelo_epi16(lanes, ALPHA_LANES);
                _mm_sub_epi16(full, _mm_shufflehi_epi16(lanes, ALPHA_LANES))
            };
            let low = scale_128(_mm_unpacklo_epi8(d, zero), alpha(_mm_unpacklo_epi8(s, zero)));
            let high = scale_128(_mm_unpackhi_epi8(d, zero), alpha(_mm_unpackhi_epi8(s, zero)));
            let out = _mm_adds_epu8(s, _mm_packus_epi16(low, high));
            _mm_storeu_si128(block.as_mut_ptr().cast(), _mm_and_si128(out, rgb));
        }
    }
    blend_color_scalar(pixels_rest, sources_rest);
}

/// `round((s × a + d × (255 − a)) / 255)` on sixteen 16-bit lanes.
#[target_feature(enable = "avx2")]
fn mix_256(d: __m256i, s: __m256i, a: __m256i) -> __m256i {
    let inverse = _mm256_sub_epi16(_mm256_set1_epi16(255), a);
    let t = _mm256_add_epi16(_mm256_mullo_epi16(s, a), _mm256_mullo_epi16(d, inverse));
    let t = _mm256_add_epi16(t, _mm256_set1_epi16(128));
    _mm256_srli_epi16(_mm256_add_epi16(t, _mm256_srli_epi16(t, 8)), 8)
}

/// `round(d × inverse / 255)` on sixteen 16-bit lanes.
#[target_feature(enable = "avx2")]
fn scale_256(d: __m256i, inverse: __m256i) -> __m256i {
    let t = _mm256_add_epi16(_mm256_mullo_epi16(d, inverse), _mm256_set1_epi16(128));
    _mm256_srli_epi16(_mm256_add_epi16(t, _mm256_srli_epi16(t, 8)), 8)
}

/// Only callable where the CPU supports AVX2 (`unsafe` at the call site).
#[inline]
#[target_feature(enable = "avx2")]
pub(super) fn blend_mask_avx2(dst: &mut [u32], mask: &[u8], color: u32) {
    let len = dst.len().min(mask.len());
    let (Some((dst, _)), Some((mask, _))) =
        (dst.split_at_mut_checked(len), mask.split_at_checked(len))
    else {
        return;
    };
    let zero = _mm256_setzero_si256();
    let rgb = _mm256_set1_epi32(RGB_MASK_I32);
    let source = _mm256_unpacklo_epi8(_mm256_set1_epi32(bits(color)), zero);
    let (blocks, pixels_rest) = dst.as_chunks_mut::<8>();
    let (covers, coverage_rest) = mask.as_chunks::<8>();
    for (block, cover) in blocks.iter_mut().zip(covers) {
        if *cover == [0; 8] {
            continue;
        }
        // SAFETY: `cover` is 8 bytes and `block` 8 `u32` (32 bytes), exactly what is read and
        // written; the loads and stores are unaligned ones.
        unsafe {
            let a = _mm256_cvtepu8_epi32(_mm_loadl_epi64(cover.as_ptr().cast()));
            let a = _mm256_mullo_epi32(a, _mm256_set1_epi32(REPEAT_BYTE));
            let d = _mm256_loadu_si256(block.as_ptr().cast());
            let low = mix_256(_mm256_unpacklo_epi8(d, zero), source, _mm256_unpacklo_epi8(a, zero));
            let high =
                mix_256(_mm256_unpackhi_epi8(d, zero), source, _mm256_unpackhi_epi8(a, zero));
            let out = _mm256_and_si256(_mm256_packus_epi16(low, high), rgb);
            _mm256_storeu_si256(block.as_mut_ptr().cast(), out);
        }
    }
    blend_mask_scalar(pixels_rest, coverage_rest, color);
}

/// Only callable where the CPU supports AVX2 (`unsafe` at the call site).
#[inline]
#[target_feature(enable = "avx2")]
pub(super) fn blend_color_avx2(dst: &mut [u32], src: &[u32]) {
    let len = dst.len().min(src.len());
    let (Some((dst, _)), Some((src, _))) =
        (dst.split_at_mut_checked(len), src.split_at_checked(len))
    else {
        return;
    };
    let zero = _mm256_setzero_si256();
    let rgb = _mm256_set1_epi32(RGB_MASK_I32);
    let full = _mm256_set1_epi16(255);
    let (blocks, pixels_rest) = dst.as_chunks_mut::<8>();
    let (sources, sources_rest) = src.as_chunks::<8>();
    for (block, source) in blocks.iter_mut().zip(sources) {
        // SAFETY: `block` and `source` are 8 `u32` each, the 32 bytes read and written.
        unsafe {
            let s = _mm256_loadu_si256(source.as_ptr().cast());
            let d = _mm256_loadu_si256(block.as_ptr().cast());
            let alpha = |lanes: __m256i| {
                let lanes = _mm256_shufflelo_epi16(lanes, ALPHA_LANES);
                _mm256_sub_epi16(full, _mm256_shufflehi_epi16(lanes, ALPHA_LANES))
            };
            let low =
                scale_256(_mm256_unpacklo_epi8(d, zero), alpha(_mm256_unpacklo_epi8(s, zero)));
            let high =
                scale_256(_mm256_unpackhi_epi8(d, zero), alpha(_mm256_unpackhi_epi8(s, zero)));
            let out = _mm256_adds_epu8(s, _mm256_packus_epi16(low, high));
            _mm256_storeu_si256(block.as_mut_ptr().cast(), _mm256_and_si256(out, rgb));
        }
    }
    blend_color_scalar(pixels_rest, sources_rest);
}

/// The rows of `shape`, SSE2 only (`unsafe` at the call site).
#[target_feature(enable = "sse2")]
pub(super) fn blend_mask_rect_sse2(dst: &mut [u32], mask: &[u8], color: u32, shape: Shape) {
    for_each_row(dst, mask, shape, |dst, mask| blend_mask_sse2(dst, mask, color));
}

/// The rows of `shape`, SSE2 only (`unsafe` at the call site).
#[target_feature(enable = "sse2")]
pub(super) fn blend_color_rect_sse2(dst: &mut [u32], src: &[u32], shape: Shape) {
    for_each_row(dst, src, shape, |dst, src| blend_color_sse2(dst, src));
}

/// The rows of `shape`, AVX2 only (`unsafe` at the call site).
#[target_feature(enable = "avx2")]
pub(super) fn blend_mask_rect_avx2(dst: &mut [u32], mask: &[u8], color: u32, shape: Shape) {
    for_each_row(dst, mask, shape, |dst, mask| blend_mask_avx2(dst, mask, color));
}

/// The rows of `shape`, AVX2 only (`unsafe` at the call site).
#[target_feature(enable = "avx2")]
pub(super) fn blend_color_rect_avx2(dst: &mut [u32], src: &[u32], shape: Shape) {
    for_each_row(dst, src, shape, |dst, src| blend_color_avx2(dst, src));
}
