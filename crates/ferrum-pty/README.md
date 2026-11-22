# ferrum-pty

**Responsibility:** PtyBackend traits plus Unix and ConPTY implementations and child process lifecycle.

**Status:** skeleton (WP-00). No public API yet.

| | |
|---|---|
| Allowed internal dependencies | none (leaf service) |
| Must never depend on | any other ferrum crate; consumers use only its traits |
| `unsafe` | allowed only in audited modules (`#![deny(unsafe_code)]` + local `#[allow]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
