# ferrum-vt

**Responsibility:** VT state machine on top of vte: grid, modes, charsets, selection model, hyperlinks, shell marks.

**Status:** skeleton (WP-00). No public API yet.

| | |
|---|---|
| Allowed internal dependencies | ferrum-scrollback |
| Must never depend on | ferrum-daemon, ferrum-pty, ferrum-platform, ferrum-proto, any OS or socket code |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
