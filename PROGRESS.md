# PROGRESS

Honest state of the build (`docs/spec/docs/12-build-order.md`). Numbers appear here only when measured.

## Current: WP-00 Repository bootstrap (branch `chore/repo-bootstrap`)

### Done
- Workspace `Cargo.toml` (from `docs/spec/templates/`), 12 skeleton crates under `crates/` (each with
  `//!` docs, lint header and README listing responsibility, allowed and forbidden dependencies),
  `xtask/`. `ferrum-daemon` has lib plus bin `ferrumd`; `ferrum-app` has bin `ferrum`;
  `ferrum-testkit` is `publish = false`.
- Templates copied to their real locations; `docs/spec/templates/` left untouched.
- `cargo xtask`: `setup`, `check` (and `--fast`), `check-deps` (rules in `xtask/deps-rules.toml`, 6 unit
  tests for the checker). `bench`, `memcheck`, `soak` are stubs that print "not implemented until
  WP-xx" (soak: WP-08) and exit with code 2; they never print numbers.
- GitHub Actions pinned by SHA (table in the PR and in `docs/07` §4); Rust is installed by the local
  composite action `.github/actions/setup-rust`, which reads the channel from `rust-toolchain.toml`.
- `deny.toml` reconciled with cargo-deny 0.22.0; `cargo deny check` passes.
- `audit.yml` (daily and on lockfile changes: cargo audit, cargo deny advisories and licenses); layout directories from `docs/04` §1 each with a purpose README.
- Docs: licenses, README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, CHANGELOG, ADR 0001,
  `docs/manual-verification.md`, `typos.toml`.

### Decisions and deviations (read these)
- **ADR 0022 (maintainer approved 2026-11-21):** CI jobs arrive with the work package that creates what they measure: `nightly.yml` in WP-01, `memcheck --daemon-only` in WP-06, full `memcheck` plus the required `memcheck (linux)` check and the nightly soak job in WP-08, `release.yml` in WP-18. The audit of each of those WPs blocks its merge if the job is missing. Until then `cargo xtask memcheck`/`soak` exit 2 and print no numbers. See [`docs/adr/0022-ci-jobs-arrive-with-the-wp-that-creates-what-they-measure.md`](docs/adr/0022-ci-jobs-arrive-with-the-wp-that-creates-what-they-measure.md).
- **ADR 0023 (maintainer approved 2026-11-21):** `ferrum-daemon` may depend on `ferrum-platform` (traits only in the library; real implementations wired only in `ferrumd`'s `src/main.rs`). See [`docs/adr/0023-daemon-may-depend-on-platform.md`](docs/adr/0023-daemon-may-depend-on-platform.md).
- **unsafe lint:** `[workspace.lints.rust] unsafe_code = "forbid"` as in the template. A crate cannot
  inherit the workspace table and override one entry, so `ferrum-pty`, `ferrum-platform` and
  `ferrum-render` repeat the lint tables with `unsafe_code = "deny"` (`docs/05` §4). Keep them in sync.
- **`cargo xtask check` includes `machete` and `cargo hack`** because CI runs them (docs/06 §3 says
  "everything CI runs"). Missing tools fail the check; they are never skipped.
- **Unused workspace dependencies:** the template's `[workspace.dependencies]` is kept whole as the
  single source of versions; cargo's `unused_workspace_dependencies` lint is set to `allow`.
- **New dev-tool crates** `clap` 4.6.9 and `cargo_metadata` 0.23.1 (xtask only) and `toml` bumped to
  1.2.2, verified on crates.io 2026-11-21 and recorded in `docs/07`.
- **`git add --renormalize .`** produced no change (the index already held LF content), so there is no
  renormalize commit.
- **dtolnay/rust-toolchain** publishes no releases: pinned to the default-branch tip
  (`df2a88e9fe4098bad09cb42c3dec7187f15861fc`, 2026-11-15).

### Environment notes
- Toolchain 1.100.0 (the pin) installed via rustup; MSRV 1.97 is declared but not separately
  verified (no 1.97 toolchain was installed).
- Local machine is Windows 11. Smart App Control initially blocked freshly built executables (os error 4551); the maintainer disabled it, and `cargo xtask check` was re-run from a clean `target/` with no retries: all steps passed.
- `cargo-machete` could not be built in release mode locally (Application Control blocked a dependency build script), so it was installed with `--debug`.
  CI installs prebuilt tools via `taiki-e/install-action`.

### Open / next
- Next: spikes S1 to S4 and WP-01 (`ferrum-scrollback`).
- Branch protection (`docs/06` §5) is a GitHub setting for the maintainer.
- Measured numbers: none (no code to measure yet).
