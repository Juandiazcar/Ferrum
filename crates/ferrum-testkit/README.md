# ferrum-testkit

**Responsibility:** dev-only helpers: fixtures, golden harness, PTY test driver, fake clock, and the
shared contract suites.

**Status:** partial. Contents so far:

- `spill_contract`: the shared suite every `SpillStore` implementation passes (WP-01).
- `temp::TempDir`: a fresh directory under the system temporary directory, removed on drop, for
  tests that touch files (`docs/08` §7: never the real user config or data directories; WP-04).
- `pty_contract`: PT-01 to PT-25 of ADR 0026, one public function per case, run on the native
  backend by `tests/pty_contract_native.rs` and on the fake by `tests/pty_contract_fake.rs` (WP-05).
- `fake_pty`: an in-memory `PtyBackend` without threads or timing, for daemon tests (WP-05).
- `ferrum-pty-helper` (binary): the child the PTY suite runs instead of shell tricks (`docs/08` §2).

| | |
|---|---|
| Allowed internal dependencies | any (from dev-dependencies only) |
| Must never depend on | being a normal dependency of any crate; it is publish = false |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
