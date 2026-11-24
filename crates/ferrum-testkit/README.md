# ferrum-testkit

**Responsibility:** dev-only helpers: fixtures, golden harness, PTY test driver, fake clock.

**Status:** partial. Contents so far:

- `spill_contract`: the shared suite every `SpillStore` implementation passes (WP-01).
- `temp::TempDir`: a fresh directory under the system temporary directory, removed on drop, for
  tests that touch files (`docs/08` §7: never the real user config or data directories; WP-04).

| | |
|---|---|
| Allowed internal dependencies | any (from dev-dependencies only) |
| Must never depend on | being a normal dependency of any crate; it is publish = false |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
