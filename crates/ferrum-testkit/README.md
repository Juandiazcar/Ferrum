# ferrum-testkit

**Responsibility:** dev-only helpers: fixtures, golden harness, PTY test driver, fake clock.

**Status:** skeleton (WP-00). No public API yet.

| | |
|---|---|
| Allowed internal dependencies | any (from dev-dependencies only) |
| Must never depend on | being a normal dependency of any crate; it is publish = false |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
