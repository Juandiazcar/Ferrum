# ferrum-project

**Responsibility:** project model, .git/HEAD reader, cwd tracking logic and startup recipes.

**Status:** skeleton (WP-00). No public API yet.

| | |
|---|---|
| Allowed internal dependencies | ferrum-config (types only, if needed) |
| Must never depend on | ferrum-daemon, ferrum-ui, OS-specific code |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
