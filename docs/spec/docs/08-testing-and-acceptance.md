# 08 — Testing and Acceptance

## 1. Philosophy

A terminal is a compatibility product: users notice the one sequence that renders wrong. Tests are therefore mostly **deterministic fixtures** (bytes in → state out), plus **fuzzing** for robustness, plus **budget gates** for memory and speed.

Test pyramid (approximate share of tests): unit 65 % · integration 25 % · end-to-end/system 8 % · manual 2 %.

## 2. Test types

| Type | Tool | Applies to | Notes |
|---|---|---|---|
| Unit | `cargo nextest`, built-in `#[test]` | all crates | pure, fast, deterministic; no sleeps; fake `Clock` |
| Property | `proptest` | `scrollback` (encode/decode/reflow round-trip), `proto` (round-trip), `vt` (resize invariants), `config` (migration idempotence) | persist failing seeds under `proptest-regressions/` and commit them |
| Golden/snapshot | `insta` | `vt` grid snapshots after fixture sequences; `ui` layout rects; `render` small PNG comparisons with tolerance | review snapshot diffs like code |
| Fuzz | `cargo-fuzz` | `vt::parse` (arbitrary bytes), `proto::decode`, `config::load`, `scrollback::decode_chunk`, OSC/kitty/sixel payloads | nightly 10 min/target; before release 1 h/target; corpus committed |
| Allocation | `dhat` | `vt::parse`, `scrollback::push_line`, `render::paint_rows` | steady-state allocations must be 0 |
| Benchmarks | `criterion` | parse, scrollback push/read, reflow, search, repaint, protocol encode | baselines in `benches/baselines/` |
| Contract | shared suites in `ferrum-testkit` | every `PtyBackend`, `SpillStore`, `Clock` implementation | same suite for real and fake impls |
| Integration | `ferrum-testkit` PTY driver | daemon ↔ client over a real socket with real PTYs running `sh`/`cmd` scripts | cross-platform; use portable helper binaries from `xtask` (e.g., `ferrum-fixture-gen`, `ferrum-echo`) instead of shell tricks |
| UI logic | headless | `ferrum-ui` action/layout/DnD logic with synthetic events | no window needed |
| Render | headless | `ferrum-render` into an in-memory buffer | bundled font for determinism |
| System | `xtask memcheck/soak/latency` | full `ferrum` + `ferrumd` | Linux under Xvfb in CI; macOS/Windows with real sessions on CI runners |
| Manual | checklist (`docs/manual-verification.md`) | IME, HiDPI, compositor behavior, drag & drop from the OS file manager, installers | recorded per release |

## 3. Coverage expectations (report-only, not a gate)

- `ferrum-vt`, `ferrum-scrollback`, `ferrum-proto`, `ferrum-config`: ≥ 90 % line coverage expected.
- `ferrum-ui`, `ferrum-daemon`: ≥ 80 %.
- Platform glue: covered by contract tests and manual checks.
- Coverage numbers inform review; never write tests only to raise numbers.

## 4. Fixtures

- `fixtures/vt/<topic>/<case>.in` (raw bytes, hex-annotated with comments in a `.md` alongside) → `<case>.snap` (insta snapshot of grid + cursor + modes).
- Imported from Alacritty's `ref` tests (Apache-2.0; keep attribution in `fixtures/vt/THIRD_PARTY.md`).
- Unicode: width and grapheme cases from the Unicode test data and the emoji sequences list.
- `fixtures/output/`: seeded generators for realistic content (compiler logs, `git log --graph`, JSON, `ls -la`, colored diffs, CJK text, very long lines) used by `memcheck` and benches. Generators are deterministic (seed in file name).
- `fixtures/config/v1/…vN/`: every historical config schema, used by migration tests.

## 5. Acceptance criteria (the release gates)

All of these must be green on Linux, macOS and Windows for a release candidate (memory gates use the OS metric defined in `docs/03`).

| ID | Criterion | How |
|---|---|---|
| AC-MEM-01 | Combined memory < 50 MB in the reference scenario | `cargo xtask memcheck` |
| AC-MEM-02 | Idle daemon (1 empty session) < 8 MB; idle UI (1 session) < 22 MB | `memcheck --idle` |
| AC-MEM-03 | Memory returns after closing 9 of 10 loaded sessions (≥ 70 % of their share within 10 s) | `memcheck --release` |
| AC-MEM-04 | At scale ≥ 1.5: total − surface < 42 MB | `memcheck --hidpi` |
| AC-MEM-05 | Soak 60 min: growth ≤ 1 MB/h after warm-up | `xtask soak` (nightly) |
| AC-PERF-01 | Keystroke → frame-ready p50 ≤ 4 ms, p99 ≤ 10 ms | `xtask latency` |
| AC-PERF-02 | Parse ≥ 400 MB/s ASCII, ≥ 120 MB/s SGR-heavy | `cargo bench -p ferrum-vt` |
| AC-PERF-03 | Full repaint 200×60 ≤ 2.5 ms | `cargo bench -p ferrum-render` |
| AC-PERF-04 | 20 sessions with one flooding: others p99 ≤ 10 ms, UI ≥ 60 fps | `xtask flood` |
| AC-PERF-05 | Cold start ≤ 250 ms; warm ≤ 120 ms | `xtask startup` |
| AC-PERF-06 | Idle CPU 0.0 % (no wakeups in 30 s) | `xtask idle` |
| AC-TERM-01 | vttest subset + imported ref-tests pass (§6) | CI |
| AC-TERM-02 | Reflow round-trip: resizing 80→132→40→80 preserves logical text (property test) | CI |
| AC-TERM-03 | No panic/hang for 1 h of fuzzing per target on the release commit | fuzz job |
| AC-SESS-01 | Close UI, reopen: all sessions present with identical screen and scrollback hash | integration |
| AC-SESS-02 | `kill -9` of the UI process does not affect sessions; of the daemon: layout restored on restart | integration |
| AC-PROJ-01 | Project with two roots: sessions grouped, layout persisted across restart, branch shown | integration + manual |
| AC-UI-01 | Rename, reorder, move-between-projects via DnD, split, resize, zoom work (UI logic tests + manual on each OS) | tests + checklist |
| AC-UI-02 | Paste image → path inserted; drop file → quoted path inserted (per shell) | manual + unit for quoting |
| AC-DATA-01 | Export on OS A, import on OS B reproduces settings, themes, projects, layouts | manual + unit |
| AC-DIST-01 | Clean install, run, uninstall on each OS; update flow tested against a staging manifest | manual |
| AC-SEC-01 | Hardening tests in `docs/10` §3 pass | CI |

## 6. Conformance checklist (FR-TERM-020)

Automated:
1. All fixtures from Alacritty `ref` tests that apply to Ferrum's supported feature set.
2. Sequences from the xterm control-sequences document for every implemented code, each with at least one positive and one malformed-input test.
3. Unicode width/grapheme/emoji vectors.
4. Round-trip tests for mouse and keyboard encodings (legacy, SGR mouse, kitty keyboard protocol levels).

Manual (recorded in release notes), using `vttest`:
- Menu 1 (cursor movement), 2 (screen features), 3 (character sets), 6 (terminal reports), 8 (VT102), 11 (xterm/non-VT100) — subset that applies to a VT220+/xterm-class terminal.

Application smoke list (each launched, used for 2 minutes, resized, scrolled, closed): `vim`/`nvim`, `emacs -nw`, `nano`, `tmux` (nested), `less`, `htop`/`btop`, `fzf`, `lazygit`, `mc`, `ncdu`, `cmatrix` (flood), `ssh` into a remote host, `man`, `git log --graph --color`, `bat`, `ripgrep` with colors, `claude`/`codex` CLIs (they are heavy TUIs and the main real-world workload).

## 7. Test infrastructure rules

- Tests never depend on wall-clock sleeps; use fake clocks or event waits with generous timeouts and explicit failure messages.
- Tests never touch the real user config/data dirs: `ferrum-testkit` provides a temp "home" and sets `FERRUM_*_DIR` overrides.
- Tests are hermetic: no network, no dependency on installed shells beyond `sh` on Unix and `cmd` on Windows (helpers cover the rest).
- Flaky test policy: a test that flakes twice is quarantined with an issue within a day and fixed within a week; never retried silently in CI.
- Every bug fix starts with a failing test. Every fuzz crash becomes a regression test in the corpus and a unit test.

## 8. CI performance measurement caveats

Shared CI runners are noisy. Memory is deterministic enough to gate absolutely. Throughput/latency gates compare against the previous `main` on the same runner image with a 10 % tolerance and also have a loose absolute ceiling (2× target) that always fails. Full-precision benchmark numbers are taken on the maintainer's machines before each release and recorded in `docs/perf-history.md`.
