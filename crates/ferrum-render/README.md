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
  `SystemFonts::scan_dir(path)` scans one folder instead (the tests use it with the fixture
  fonts of `fixtures/fonts/`, never the runner's own fonts).
- `font::FontSpec`: logical size, scale factor (HiDPI, fractional included), line height and
  letter spacing, all plain values the UI takes from the settings.

- `Glyphs`: the `Fonts`, the glyph cache and the blit kernels. **One per process**: `docs/03`
  §2 budgets one glyph cache, so every window and pane shares the same `Glyphs` (WP-08 wires it).
  `Glyphs::draw_text(&mut Target, &TextRun)` draws a line on the cell grid (one cell per
  character, two for a wide glyph, none for a combining mark).
- `Target`: the caller's `&mut [u32]` (`0x00RRGGBB`, the `softbuffer` buffer) with its size;
  `Rect`, `Rgb`.

System font files are memory-mapped by `fontdb` only while they are read; nothing is copied.
A character the primary face lacks is found through the fallback chain (generic monospace
families of `docs/11` §2, the emoji fonts, then any system face whose coverage may hold it; the
coverage of the system faces is read the first time a fallback is needed) and memoized. Emoji
presentation follows Unicode `Emoji_Presentation` and VS15/VS16. At most 64 fallback faces stay
loaded; the least recently used is dropped, with its memo entries and cached glyphs.

Box drawing (U+2500–U+257F), block elements (U+2580–U+259F) and the powerline separators
(U+E0B0–U+E0B3) bypass the font: they are drawn on the cell's own pixel grid (light lines of the
font's underline thickness, heavy twice that), so lines meet exactly across cells; tests check
that every arm of every box-drawing character ends on the pixels of the matching straight line,
and complementary blocks tile at odd sizes. A wide cell's synthesized glyph covers both cells.

The glyph cache is a bounded LRU of grayscale coverage masks, per-channel LCD coverage (the LCD
text setting, `Glyphs::set_antialias`) and premultiplied color images (`docs/02` §6), capped in
bytes by the caller (the glyph cache setting, default 3 MB); the cap counts pixels and the
cache's containers, and a glyph larger than the whole cache is drawn without being kept.
Blending uses SSE2 or AVX2 (detected at run time) on `x86_64` and NEON on `aarch64`; tests
check every kernel against the scalar one bit for bit.

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
