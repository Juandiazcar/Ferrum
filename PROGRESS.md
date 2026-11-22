# PROGRESS

Honest state of the build (`docs/spec/docs/12-build-order.md`). Numbers appear here only when measured.

## Spike S4 Windows PTY (branch `spike/s4-windows-pty`, ADR 0026)

Throwaway harness in `spikes/s4-windows-pty/` (standalone package, `unsafe_code = "forbid"`, only
`portable-pty` 0.9.0). Method, per-OS tables, reproduction commands and CI run links:
`docs/adr/0026-windows-pty-backend.md`. Measured on Windows 11 (build 26300, local), Windows Server
2025, Ubuntu and macOS CI (run 44977495884 of the refactored harness; the temporary workflow was deleted in the last commit).

| Item | Result |
|---|---|
| Spawn/echo (`cmd`, `powershell`, `pwsh`, `sh`, `bash`) | all work; marker 5-32 ms after input (cold `powershell.exe` on CI 1.4-2.27 s) |
| Exit codes | 0/3/255 exact in 12-36 ms; **exit 259 never reported** by `portable-pty` on Windows; `clone_killer().kill()` returns `Err` on success; Windows reader gets no EOF until the master is closed (Unix: at exit) |
| Resize | 0 errors in 39-49 resizes during a 16 MB flood, no loss; viewport-only repaint (225-324 bytes) after every resize, even same size; scrollback never re-emitted; child sees the new size |
| 64 MB output | ConPTY 5.2-6.5 MB/s (1.3 under load) flood, 0.6-0.9 MB/s `cmd /c type`; Ubuntu 36-59, macOS 34-61 MB/s; reader memory flat; no stalls (max gap 9-85 ms) |
| Startup | `portable-pty` sets `INHERIT_CURSOR`: the first and only bytes are `ESC[6n` until answered `ESC[1;1R`; no repaint of existing content at start |
| I/O model | reader handle hidden and synchronous: one blocking reader thread per session is forced with `portable-pty`; 0.08-0.12 MiB working set (0.11-0.15 MiB private) per session, plus one `conhost.exe` (0.5-0.8 MB) outside the process |
| Env/cwd | `TERM` inherited unless removed; `COLORTERM`/Unicode values reach the child; default cwd is the home directory |

- Decision (maintainer, 2026-11-21; ADR 0026 accepted): direct `windows-sys` ConPTY backend in `ferrum-pty` for Windows (audited `unsafe`), `portable-pty` kept for Linux/macOS; fallback to `portable-pty` plus the documented workarounds if the direct backend cannot pass the 25-test `PtyBackend` contract list (PT-01 to PT-25) on a supported Windows build. ADR 0008 Windows part validated as written (reader thread per session, 0.08-0.12 MiB working set each).
- `docs/11` §4 PTY bullet rewritten to the measured behavior (startup cursor query, no initial repaint, viewport-only repaint after every resize); Q-18 recorded as resolved in `docs/14` §2; R-04 mitigation updated (incl. the ConPTY throughput finding), R-05 likelihood Medium -> High (reason in the row). No question from S4 stays open; `docs/07` rows for `portable-pty`, `windows-sys` and the spike-only `anyhow` updated.
- The spike obeys the product lint policy (docs/05): `unsafe_code = "forbid"`, no `unwrap`/`expect`/`println!`, functions of at most 40 lines, modules per concept, clippy `-D warnings` and fmt clean. A one-off direct-API experiment needed `unsafe` and was removed; ADR 0026 keeps its results only as "Unverified exploratory notes (not evidence; WP-05 must prove)".
- Not verified: Windows before build 26100 (MV-S4-1 to 3 in `docs/manual-verification.md`), ARM64, the repaint against the reflowing `ferrum-vt` grid, close right after a fast exit with output still in the pipe.
- Next: WP-05 `ferrum-pty` implements the traits, the direct ConPTY backend and the contract suite.

## Spike S1 Memory model (branch `spike/s1-memory-model`, ADR 0024)

Throwaway harness in `spikes/s1-memory-model/` (standalone package, excluded from the product
workspace). Full tables, method and reproduction commands: `docs/adr/0024-scrollback-memory-model-measurements.md`.
Heap-measured with `dhat` 0.3.3 (no `unsafe` in the spike, `unsafe_code = "forbid"`), 12 000
lines fed, 10 000 rows retained and all verified by round trip, 3 seeds each, Windows 11,
i5-9400. Throughput comes from a second build without `dhat`.

| Measure | Result (seeds 1 / 2 / 3) |
|---|---|
| Raw encoded bytes per row | 75.32 / 74.95 / 75.23 (mixed) |
| LZ4 bytes per row (+ index 1.04) | 37.45 / 37.14 / 37.28 (mixed); 55.5 (prose); 75.0 (random) |
| LZ4 ratio | 2.02x mixed, 1.35x prose, 1.0x random (docs/03 expected 2.5-4x: not supported) |
| Per session, 10 000 rows, incl. live grid, bounded hot growth | 684 566 / 682 981 / 684 887 B mixed; about 852 000 prose; about 1 035 000 random |
| Naive 16-byte cells x 200 cols | 32 503 680 B (all seeds) |
| `alacritty_terminal` 0.26.0 | 51 234 147 B (all seeds), 24-byte cells |
| Parse + encode throughput (information) | 519 654 to 666 315 lines/s mixed (no `dhat`; alacritty_terminal 338 427 to 340 886) |

- Confirms ADR 0004 and 0005 (design of `docs/02` §3.2 round-trips; 47x to 75x smaller than the alternatives).
- `docs/03` §2 per-session scrollback rows now hold measured values; allowances and gates untouched.
- **Over allowance, not relaxed:** sealed chunks 0.52 MB (prose) and 0.70 MB (random) vs 0.45 MB, random-text per-session sum 1.03-1.04 MB vs 1.0 MB. Resolved by the maintainer decision Q-17 (2026-11-21, `docs/14` §2): keep all allowances and LZ4 unchanged; WP-01 re-measures with the product fixture generator and the WP-08 gate decides; over budget goes back to the maintainer with numbers.
- WP-01 requirement (ADR 0024 Consequences): the hot chunk uses bounded growth and stays at or below 0.10 MB (plain `Vec` growth measured 0.088-0.152 MB; bounded 0.086 MB).
- Not measured: real PSS/private working set, allocator headers and fragmentation (S2), real terminal corpora, zstd or larger chunks, Linux and macOS.

## Previous: WP-00 Repository bootstrap (branch `chore/repo-bootstrap`)

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
