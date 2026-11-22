# ferrum-render

**Responsibility:** font loading, glyph cache and damage-limited CPU painting into a pixel buffer.

**Status:** skeleton (WP-00). No public API yet.

| | |
|---|---|
| Allowed internal dependencies | none (no windowing; takes plain values, not config types) |
| Must never depend on | winit, softbuffer, ferrum-config and any other ferrum crate |
| `unsafe` | allowed only in audited modules (`#![deny(unsafe_code)]` + local `#[allow]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
