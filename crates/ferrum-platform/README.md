# ferrum-platform

**Responsibility:** clipboard, directories, single-instance, own-process memory metric and daemon autostart behind traits.

**Status:** skeleton (WP-00). No public API yet.

| | |
|---|---|
| Allowed internal dependencies | none (leaf service) |
| Must never depend on | any other ferrum crate; pure crates never import it |
| `unsafe` | allowed only in audited modules (`#![deny(unsafe_code)]` + local `#[allow]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
