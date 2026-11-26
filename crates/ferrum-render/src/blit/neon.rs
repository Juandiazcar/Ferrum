//! NEON kernels. `vld4`/`vst4` split eight pixels into one register per channel, so each channel
//! is blended on eight 16-bit lanes with the exact division of the parent module.
// perf: `render_repaint/*` (benches/repaint.rs) — 8 pixels per step instead of 1.

use std::arch::aarch64::{
    uint8x8_t, uint8x8x4_t, uint16x8_t, vaddq_u16, vdup_n_u8, vdupq_n_u16, vld1_u8, vld4_u8,
    vmlal_u8, vmull_u8, vmvn_u8, vqadd_u8, vshrn_n_u16, vsraq_n_u16, vst4_u8,
};

use super::{RGB_MASK, blend_color_scalar, blend_mask_scalar, blend_subpixel_scalar};

/// `round(t / 255)` of eight 16-bit lanes, narrowed to bytes.
#[target_feature(enable = "neon")]
fn div255(t: uint16x8_t) -> uint8x8_t {
    let t = vaddq_u16(t, vdupq_n_u16(128));
    vshrn_n_u16(vsraq_n_u16(t, t, 8), 8)
}

/// `round((s × a + d × (255 − a)) / 255)`.
#[target_feature(enable = "neon")]
fn mix(d: uint8x8_t, s: uint8x8_t, a: uint8x8_t) -> uint8x8_t {
    div255(vmlal_u8(vmull_u8(s, a), d, vmvn_u8(a)))
}

/// Callable only where NEON is enabled (`unsafe` at the call site; NEON is part of `aarch64`).
#[target_feature(enable = "neon")]
pub(super) fn blend_mask(dst: &mut [u32], mask: &[u8], color: u32) {
    let len = dst.len().min(mask.len());
    let (Some((dst, _)), Some((mask, _))) =
        (dst.split_at_mut_checked(len), mask.split_at_checked(len))
    else {
        return;
    };
    let [blue, green, red, _] = color.to_le_bytes();
    let (blue, green, red) = (vdup_n_u8(blue), vdup_n_u8(green), vdup_n_u8(red));
    let (blocks, pixels_rest) = dst.as_chunks_mut::<8>();
    let (covers, coverage_rest) = mask.as_chunks::<8>();
    for (block, cover) in blocks.iter_mut().zip(covers) {
        if *cover == [0; 8] {
            continue;
        }
        // SAFETY: `cover` is 8 bytes and `block` 8 `u32` (32 bytes), exactly what `vld1`, `vld4`
        // and `vst4` read and write; NEON loads and stores need no alignment.
        unsafe {
            let a = vld1_u8(cover.as_ptr());
            let d = vld4_u8(block.as_ptr().cast());
            let zero = vdup_n_u8(0);
            let out = uint8x8x4_t(mix(d.0, blue, a), mix(d.1, green, a), mix(d.2, red, a), zero);
            vst4_u8(block.as_mut_ptr().cast(), out);
        }
    }
    blend_mask_scalar(pixels_rest, coverage_rest, color);
}

/// As [`blend_mask`]: NEON only.
#[target_feature(enable = "neon")]
pub(super) fn blend_color(dst: &mut [u32], src: &[u32]) {
    let len = dst.len().min(src.len());
    let (Some((dst, _)), Some((src, _))) =
        (dst.split_at_mut_checked(len), src.split_at_checked(len))
    else {
        return;
    };
    let (blocks, pixels_rest) = dst.as_chunks_mut::<8>();
    let (sources, sources_rest) = src.as_chunks::<8>();
    for (block, source) in blocks.iter_mut().zip(sources) {
        // SAFETY: `block` and `source` are 8 `u32` each (32 bytes), what `vld4`/`vst4` access;
        // NEON loads and stores need no alignment.
        unsafe {
            let s = vld4_u8(source.as_ptr().cast());
            let d = vld4_u8(block.as_ptr().cast());
            let inverse = vmvn_u8(s.3);
            let over = |s: uint8x8_t, d: uint8x8_t| vqadd_u8(s, div255(vmull_u8(d, inverse)));
            let out = uint8x8x4_t(over(s.0, d.0), over(s.1, d.1), over(s.2, d.2), vdup_n_u8(0));
            vst4_u8(block.as_mut_ptr().cast(), out);
        }
    }
    blend_color_scalar(pixels_rest, sources_rest);
}

/// Per-channel coverage blend, NEON only (`unsafe` at the call site).
#[inline]
#[target_feature(enable = "neon")]
pub(super) fn blend_subpixel(dst: &mut [u32], coverage: &[u32], color: u32) {
    let len = dst.len().min(coverage.len());
    let (Some((dst, _)), Some((coverage, _))) =
        (dst.split_at_mut_checked(len), coverage.split_at_checked(len))
    else {
        return;
    };
    let [blue, green, red, _] = color.to_le_bytes();
    let (blue, green, red) = (vdup_n_u8(blue), vdup_n_u8(green), vdup_n_u8(red));
    let (blocks, pixels_rest) = dst.as_chunks_mut::<8>();
    let (covers, coverage_rest) = coverage.as_chunks::<8>();
    for (block, cover) in blocks.iter_mut().zip(covers) {
        if cover.iter().all(|&c| c & RGB_MASK == 0) {
            continue;
        }
        // SAFETY: `block` and `cover` are 8 `u32` each (32 bytes), what `vld4`/`vst4` access;
        // NEON loads and stores need no alignment.
        unsafe {
            let a = vld4_u8(cover.as_ptr().cast());
            let d = vld4_u8(block.as_ptr().cast());
            let zero = vdup_n_u8(0);
            let out =
                uint8x8x4_t(mix(d.0, blue, a.0), mix(d.1, green, a.1), mix(d.2, red, a.2), zero);
            vst4_u8(block.as_mut_ptr().cast(), out);
        }
    }
    blend_subpixel_scalar(pixels_rest, coverage_rest, color);
}
