# ferrum-render

**Responsibility:** font loading, glyph cache and damage-limited CPU painting into a pixel buffer
(`docs/02` §6).

| | |
|---|---|
| Allowed internal dependencies | none (no windowing; takes plain values, not config types) |
| Must never depend on | winit, softbuffer, ferrum-config and any other ferrum crate |
| External dependencies | `swash` (shaping, rasterization), `fontdb` (system font discovery), `thiserror` |
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

System font files are memory-mapped by `fontdb` only while they are read; nothing is copied.

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
