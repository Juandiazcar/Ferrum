# ferrum-render

**Responsibility:** font loading, glyph cache and damage-limited CPU painting into a pixel buffer
(`docs/02` §6).

| | |
|---|---|
| Allowed internal dependencies | none (no windowing; takes plain values, not config types) |
| Must never depend on | winit, softbuffer, ferrum-config and any other ferrum crate |
| External dependencies | `swash` (shaping, rasterization), `fontdb` (system font discovery), `rustc-hash` (cache maps), `thiserror` |
| `unsafe` | allowed only in the blit kernels (`#![deny(unsafe_code)]` + one local `#[allow]`) |

## Public API

- `font::Fonts`: the primary family (`Family::Embedded`, the compiled-in JetBrains Mono subset
  of `assets/fonts`, or `Family::System(name)`) plus the system color emoji fonts
  (`docs/07` §5). `Fonts::cell_metrics(&FontSpec)` gives the `CellMetrics` grid.
- `font::SystemFonts::scan()`: discovers the installed fonts with `fontdb` (no fontconfig,
  `docs/11` §2). It reads every font file, so the caller runs it off the UI thread.
- `font::FontSpec`: logical size, scale factor (HiDPI, fractional included), line height and
  letter spacing, all plain values the UI takes from the settings.

- `Glyphs`: the `Fonts`, the glyph cache and the blit kernels of one window.
  `Glyphs::draw_text(&mut Target, &TextRun)` draws a line on the cell grid (one cell per
  character, two for a wide glyph, none for a combining mark).
- `Painter::paint(&mut Glyphs, &mut Target, &Frame) -> &[Rect]`: paints one pane's `Grid` of
  `Row`s of `Cell`s (text or grapheme cluster, resolved colors, weight and slant, underline
  style and color, strikeout, overline, wide cells) with the `Overlay` (cursor: block,
  underline, bar, hollow; linear or block selection). Each row has a content hash; only rows
  whose hash differs from the frame the buffer holds (`Frame::buffer_age`, `softbuffer`'s
  `Buffer::age`) are repainted, and the dirty rectangles are returned for
  `present_with_damage`. Age 0, an age past the remembered frames or a new layout repaint
  everything, padding included. Steady-state painting allocates nothing (`tests/alloc_paint.rs`).
- `Target`: the caller's `&mut [u32]` (`0x00RRGGBB`, the `softbuffer` buffer) with its size;
  `Rect`, `Rgb`.

System font files are memory-mapped by `fontdb` only while they are read; nothing is copied.
A character the primary face lacks is found through the fallback chain (generic monospace
families of `docs/11` §2, the emoji fonts, then any system face whose recorded coverage may hold
it) and memoized.

Box drawing (U+2500–U+257F), block elements (U+2580–U+259F) and the powerline separators
(U+E0B0–U+E0B3) bypass the font: they are drawn on the cell's own pixel grid (light lines of the
font's underline thickness, heavy twice that), so lines meet exactly across cells; tests check
that every arm of every box-drawing character ends on the pixels of the matching straight line.

Grapheme clusters of several characters (emoji sequences, combining marks) are shaped with
`swash` by the face of their first character and memoized.

The glyph cache is a bounded LRU of grayscale coverage masks and premultiplied color images
(`docs/02` §6), capped in bytes by the caller (the glyph cache setting, default 3 MB). Blending
uses SSE2 or AVX2 (detected at run time) on `x86_64` and NEON on `aarch64`; tests check every
kernel against the scalar one bit for bit.

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.

## Tests and benches

- Golden images: `tests/golden.rs` paints small scenes with the embedded font (text styles,
  decorations, cursor shapes, selection, box drawing, blocks, powerline, at 1× and 2×) and
  compares them with the `insta` binary snapshots in `fixtures/render/`, per channel within a
  tolerance of 2.
- `tests/alloc_paint.rs` (`dhat`, `harness = false`): 0 allocations over 100 steady-state frames.
- `benches/repaint.rs`: `render_repaint/dense_200x60` and `typical_200x60` (NFR-PERF-03, full
  repaint, gated by `cargo xtask bench` at 5 ms), `render_repaint/damage_4_rows` (a frame with
  four changed rows).
