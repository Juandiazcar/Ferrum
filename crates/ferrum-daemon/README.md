# ferrum-daemon

**Responsibility:** session manager, event loop, client server and layout persistence (library behind the ferrumd binary).

**Status:** skeleton (WP-00). No public API yet. Binary targets: see below.

| | |
|---|---|
| Allowed internal dependencies | ferrum-vt, ferrum-scrollback, ferrum-proto, ferrum-config, ferrum-project, ferrum-pty |
| Must never depend on | ferrum-ui, ferrum-render, ferrum-app, winit |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.

Binary: `ferrumd` (`src/main.rs`); the library holds all logic.
