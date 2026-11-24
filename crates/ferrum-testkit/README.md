# ferrum-testkit

**Responsibility:** dev-only helpers: fixtures, golden harness, PTY test driver, fake clock, and the
shared contract suites.

**Status:** the `SpillStore` contract suite (WP-01); the `PtyBackend` contract suite, its helper child
and the deterministic fake `PtyBackend` (WP-05).

- `spill_contract`: the suite every `SpillStore` implementation passes.
- `pty_contract`: PT-01 to PT-25 of ADR 0026, one public function per case, run on the native
  backend by `tests/pty_contract_native.rs` and on the fake by `tests/pty_contract_fake.rs`.
- `fake_pty`: an in-memory `PtyBackend` without threads or timing, for daemon tests.
- `ferrum-pty-helper` (binary): the child the PTY suite runs instead of shell tricks (`docs/08` §2).

| | |
|---|---|
| Allowed internal dependencies | any (from dev-dependencies only) |
| Must never depend on | being a normal dependency of any crate; it is publish = false |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
