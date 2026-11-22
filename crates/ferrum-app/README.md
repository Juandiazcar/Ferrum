# ferrum-app

**Responsibility:** composition root of the ferrum binary: winit glue, event loop and wiring of all crates.

**Status:** skeleton (WP-00). No public API yet. Binary targets: see below.

| | |
|---|---|
| Allowed internal dependencies | ferrum-vt, ferrum-scrollback, ferrum-proto, ferrum-pty, ferrum-daemon, ferrum-render, ferrum-ui, ferrum-config, ferrum-project, ferrum-platform |
| Must never depend on | ferrum-testkit (except as a dev-dependency) |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.

Binary: `ferrum` (`src/main.rs`, no library target).
