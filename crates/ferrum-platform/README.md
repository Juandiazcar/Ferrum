# ferrum-platform

**Responsibility:** clipboard, directories, single-instance, own-process memory metric and daemon autostart behind traits.

**Status:** partial. Contents so far:

- `private_fs::{create_private_dir, create_private_file}` (WP-04): owner-only creation (0700 and
  0600 on Unix; inherited per-user ACL on Windows), passed by the composition roots to
  `ferrum_config::archive::StagingFs` for imports (`docs/10` §4).

| | |
|---|---|
| Allowed internal dependencies | none (leaf service) |
| Must never depend on | any other ferrum crate; pure crates never import it |
| `unsafe` | allowed only in audited modules (`#![deny(unsafe_code)]` + local `#[allow]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
