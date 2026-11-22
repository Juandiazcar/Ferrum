# ferrum-ui

**Responsibility:** minimal widget toolkit, layout engine, sidebar/splits/status bar, input mapping and theme application.

**Status:** skeleton (WP-00). No public API yet.

| | |
|---|---|
| Allowed internal dependencies | ferrum-render, ferrum-proto, ferrum-config, ferrum-project |
| Must never depend on | ferrum-daemon, ferrum-pty, winit, softbuffer |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
