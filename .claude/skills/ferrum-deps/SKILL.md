---
name: ferrum-deps
description: Add or bump a Rust dependency in Ferrum. Use before touching [workspace.dependencies] or any crate's [dependencies].
---

# Adding/bumping a dependency (spec docs/05 §10, docs/07)

1. Is it needed? Check docs/07 §3 "rejected" list and `deny.toml` bans (openssl, native-tls, tokio, async-std in v1.0). std or an existing dep first.
2. Verify the latest stable version live — never from memory:
   `curl -s -A "ferrum-dev" https://crates.io/api/v1/crates/<name> | jq -r '.crate.max_stable_version, .crate.updated_at'`
   (no jq: grep `max_stable_version`). Check MSRV compatibility with `rust-version` in root `Cargo.toml`.
3. Add version only to root `[workspace.dependencies]` with `default-features = false` + explicit features. Crates use `{ workspace = true }`.
4. Respect who may use it (docs/07 "Used by" column, docs/04 §2 edges).
5. Update docs/spec/docs/07 table: version + date verified. New crate not in docs/07 → justify in PR (what it replaces, size via `cargo tree -e normal`/`cargo bloat`, license, maintenance, unsafe footprint, MSRV).
6. Run `cargo deny check` and `cargo machete`. Commit `Cargo.lock`.
7. Major bump = its own PR with benchmarks.
