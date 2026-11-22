# ferrum-config

**Responsibility:** settings model, TOML persistence, schema versions, migrations and export/import archive.

**Status:** skeleton (WP-00). No public API yet.

| | |
|---|---|
| Allowed internal dependencies | none (foundation) |
| Must never depend on | any other ferrum crate, OS-specific code |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
