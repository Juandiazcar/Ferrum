# templates/

Files to copy into the repository during **WP-00**. Mapping:

| Template | Destination |
|---|---|
| `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `cliff.toml` | repo root |
| `Cargo.toml` | repo root (workspace manifest) |
| `.editorconfig`, `.gitignore`, `.gitattributes` | repo root |
| `.github/workflows/ci.yml` | `.github/workflows/ci.yml` (resolve action SHAs first) |
| `.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/*`, `.github/CODEOWNERS`, `.github/dependabot.yml` | same paths |
| `githooks/*` | `.githooks/` (executable; activated by `cargo xtask setup`) |

Placeholders to resolve in WP-00: `<SHA> # vX` for GitHub Actions, and a reconciliation of `deny.toml` with the installed `cargo-deny` schema. Versions in `Cargo.toml` were verified on 2026-11-20 — re-verify (`docs/07`).
