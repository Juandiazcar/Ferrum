# PROGRESS

Honest state of the build (`docs/spec/docs/12-build-order.md`). Numbers appear here only when measured.

## WP-03 `ferrum-proto` (stacked PRs #65 -> #66 -> #67)

Done: row encoding for damage (`encode_row`, `decode_row`; styles carry resolved colors and an
index into the block's `LinkTable`, which stores each URI once, never a session-local id, ADR
0028), `RowBlock` capped at `MAX_ROW_BLOCK_BYTES` so every `Snapshot`/`Rows`/`Damage` message
fits one frame (`Snapshot` carries the screen rows that fit, `Rows` the rest and the history), framing (`u32` little-endian length + `postcard`, the whole frame at most 4 MiB, checked
from the header), the message set of `docs/02` §4 (`SessionKind`, one reserved variant per
Phase 2 area; after `Welcome` the full state as `ProjectEvent::Created`/`SessionEvent::Created`
items, then `Synced`; `Killed` apart from `Exited`), versioning and the handshake
(`ProtocolVersion` 1.0, same major required, lower minor used; constant-time nonce equality; the
handshake bytes pinned for every major), length-capped lists without nesting, validated ranges
(`Percent`, `PerMille`), `ProjectId` as a UUID (maintainer decision Q-29), redacted `Debug` for
user content (`Text`, `Bytes`, rows, nonce), golden vectors for every variant at every nesting
level, the `decode` fuzz target in the nightly matrix, README. Dependencies: `postcard` 1.2.0
(bumped from the 1.1.3 of `docs/07`), `uuid` 1.29.3 (the workspace entry of WP-04,
no features in `ferrum-proto`; the same `Uuid` as `ferrum-project`'s `ProjectId`), `serde` 1.0.229 (re-verified), all latest stable on 2026-11-24.

| Criterion | Result |
|---|---|
| Golden vectors | `tests/golden.rs`: 24 client and 26 daemon frames plus 29 nested values (every enum variant, a row per underline style) encode to the committed bytes and decode back |
| Version negotiation | `tests/handshake.rs` (8 tests, including the pinned handshake bytes for another major) plus the `negotiate` doctest |
| Round trip | `tests/row_roundtrip.rs` (rows), `tests/message_roundtrip.rs` (every variant of both message enums) |
| Frame cap | `tests/frame_budget.rs`: a 1 024 x 512 screen with the `ferrum-vt` worst case per cell goes out in several `Damage` frames, and a full `Snapshot` with a 1 024-byte title fits one, each at most 4 MiB; `frame.rs` tests the cap at exactly 4 MiB with the header |
| dhat | `tests/alloc_row.rs`: 0 blocks over 100 passes of a 200-column row, a 60-row `RowBlock` and its `Damage` frame after warm-up (fails as intended when a buffer is re-created) |
| Fuzz | `decode`, 60 s local run on an Apple M2 on the final code: 11 326 808 runs, no crash; corpus = the golden frames |
| Benches (Apple M2, informational) | `proto_encode/row_200_cols` 1.38 us, `proto_encode/damage_200x60` 91.2 us; `docs/03` names no target, so no ceiling and no baseline yet (`benches/baselines/README.md`); nightly gates them against the previous commit |

Constants chosen (internal protocol and safety caps, Q-30): `PROTOCOL_VERSION` 1.0, `NONCE_LEN`
32 bytes, `MAX_LIST_ITEMS` 1 024, `ROW_BLOCK_HEADROOM` 4 KiB (reasons in the crate README).
`MAX_FRAME_LEN` 4 MiB is the spec's (header included, auditor's literal reading). No new open
question.

Open items: the reply messages of search and copy are not in `docs/02` §4 and arrive with WP-11
and WP-12 as additive variants; `Layout` tree-shape validation belongs to the daemon (WP-06/WP-10).
Next: WP-06 `ferrum-daemon` after WP-05.

## WP-04 `ferrum-config` and `ferrum-project` (stack #57 → #58 → #59 → #60)

Four stacked PRs (merge in order): #57 `feat/config-settings-store` (settings model, keybindings
and theme models, versioned TOML store, migration framework, synthetic v0 → v1 fixtures, `load`
fuzz target, `ferrum_testkit::temp::TempDir`), #58 `feat/config-archive` (`.ferrum` export/import,
tar + zstd, owner-only staging through `ferrum-platform`), #59 `feat/project-model` (project
model, layouts, startup recipes, project limit settings, UUID ids), #60 `feat/project-git-head`
(`.git/HEAD` reader, this entry). The first audit found 32 blockers; all are fixed in the stack
(maintainer decisions Q-29 and Q-30 recorded in `docs/14` §2).

- **Settings** (`FR-UI-007`, `FR-DATA-001`): one table per `docs/09` §8 section, including the
  ones the spec lists that were missing at first (per-font ligatures, editor command template,
  bell urgency hint, pasted images and their directory, multi-window behavior, default root of new
  sessions, recent projects, spill disk cap, import/export caps, project limits, and title reporting
  fixed at off as `docs/09` §8 lists it). Unknown keys are
  ignored; numbers are range-checked. `keybindings.toml` and `themes/*.toml` have typed models
  and their own schemas.
- **Store and migrations** (`FR-DATA-004`): `schema_version` in every file (missing = 0); pure
  migration steps over the TOML table; reads capped at 1 MiB; corrupt files moved to
  `<name>.bad-<unix secs>` by hard link (a copy only where links are unsupported; never replacing
  a backup, non-UTF-8 names kept) and defaults loaded;
  only the error kind is logged; files from a newer version refused and left untouched; atomic
  saves (temporary sibling, `fsync`, rename). Version 0 of `settings.toml` is synthetic
  (`fixtures/config/README.md`); migrating it gives exactly the v1 fixture.
- **Archive** (`FR-DATA-002`/`003`): manifest first, then `config/`, `data/`, optional
  `scrollback/` (entries 0600). Export never follows symbolic links, writes ustar headers with
  fixed metadata, refuses names that collide up to case or NFC (files and every directory on the way) and paths a ustar header cannot
  hold. Import reads entries raw (GNU long-name and PAX entries refused before their payload is
  read), accepts only regular files with portable names in a known section, enforces entry, file,
  total and stream caps from the settings and a 4 MiB zstd window, and extracts into a fresh
  staging directory created through `StagingFs` (owner-only 0700/0600 functions in
  `ferrum-platform`), removed on any error. Merge/replace and root remapping: WP-14.
- **Projects** (`FR-PROJ-001`/`002`/`003`/`005`/`007`): `Project` is valid by construction
  (`TryFrom<ProjectParts>`, also on deserialization): UUID id (Q-29), non-blank names without
  control characters, single-grapheme icon, at least one distinct absolute root, sessions with
  stable keys, valid profiles and absolute last cwd, a layout whose panes are distinct known
  sessions (balanced `Layout::equal`, depth ≤ 32), a recipe with relative sub-directories and
  one-line commands (presets *One terminal* and *Front + back*, names derived from the roots).
  Files are held to hard caps; the user's limits from `[projects.limits]` apply on edit.
  Persisted by the daemon (WP-06) as `<data_dir>/ferrum/projects/<uuid>.toml`.
- **Git** (`FR-PROJ-004`): `read_head` walks up to the nearest `.git` (directory or `gitdir:`
  file), handles branches, unborn branches, detached SHA-1/SHA-256, other refs, linked worktrees
  and submodules; opens regular files only, 4 KiB cap; no polling.

Exit criteria evidence: `synthetic_v0_migrates_to_the_v1_fixture`,
`v0_file_loads_as_migrated_settings_and_saves_as_v1`, `migrating_a_migrated_table_changes_nothing`
(property); `export_then_import_reproduces_every_file` plus the hostile-archive tests in
`crates/ferrum-config/tests/archive.rs` (long-name and PAX bombs, zstd window, traversal, links,
caps, collisions); `crates/ferrum-project/tests/git_head.rs` (fixture repositories built in
temporary directories: branch, nested start, detached, linked worktree with absolute and relative
`gitdir`, submodule, broken repositories, FIFO `HEAD`). No sleeps, no real user directories, no git
binary.

**Values the spec does not fix (maintainer decision Q-30).** Every such value is a user setting
with its reason on its type, listed in `docs/14` Q-30: appearance (font size 13 pt 6–72, line
height 1.0 0.8–2.0, letter spacing 0 −2–10, padding bounds 0–64, bold-is-bright off, block cursor,
blink on, no ligature fonts, comfortable density, animations on, reduced motion off), terminal
(scroll speed 3 1–20, word characters `-_.~/:@`, copy-on-select off, smart copy off, editor
`{editor} {file}`, urgency hint off, Option-as-Alt none, kitty keyboard on, images on, pasted
images on), window (sidebar expanded, all status items, current window, ask on close), projects
(first root, 10 recent 0–50, restore layout on, limits 16 roots 1–64, 128-character names
16–1 024, 256 sessions 1–1 024, 32 recipe sessions 1–256), performance (spill threshold
1 024–1 000 000, spill disk cap 256 MiB 16–4 096, caches 1–64 MiB), updates (check on, stable),
import/export (10 000 entries 100–100 000, 5 184 MiB 64–65 536). Anti-hostile constants with
documented reasons: 1 MiB TOML read, 16 backup names, 4 KiB manifest, 4 MiB zstd window, archive
path depth 8 / component 255 bytes / 256 characters in errors, layout depth 32, per-mille ratios,
icon 64 bytes (longest emoji sequences are 35), 4 KiB `HEAD`/`.git` files, 1 KiB ref names, 64-character key and action names,
4 KiB commands and paths.

Dependencies (verified latest stable on crates.io 2026-11-24, `default-features = false`, docs/07
rows): `toml` 1.2.2, `tar` 0.4.46, `zstd` 0.14.4, new `unicode-normalization` 0.1.26 (config), new
in v1.0 `uuid` 1.29.3 (project, Q-29); `unicode-segmentation` now also used by project.

Gates (macOS arm64, Rust 1.100.0, stack rebased on `main` after the WP-02 merge): `cargo xtask
check` green; every commit of the stack passes clippy `-D warnings` and the WP-04 crates' tests
(`git rebase -x`); MSRV 1.97.0 (`cargo +1.97.0 test --locked -p ferrum-config -p ferrum-project
-p ferrum-platform -p ferrum-testkit`) green; Miri with the nightly job's flags green for
`ferrum-config` and `ferrum-project` (archive tests and the FIFO test cannot run under Miri: zstd is
C code and `mkfifo` is a process; the 70 000-pane layout case runs outside Miri only); fuzz `load`
60 s: 2 359 649 runs, no crash. Size impact of tar + zstd (cargo bloat, probe binary, stripped,
LTO): +728 KB, of which `zstd_sys` 216.7 KiB; it lands in the UI binary with WP-14.

Not in WP-04 (spec puts them later): cwd tracking logic (WP-12), import merge/replace and path
remapping dialogs (WP-14), built-in themes and importers (WP-13). Next: WP-05 `ferrum-pty`.

## WP-02 `ferrum-vt` (stack of PRs rebuilt on `main` after the audits of 2026-11-23)

Done: `ferrum-scrollback::{pop_front, skip_front, clear}`, a line over the size limits kept clipped
and counted, pending chunks sealed on the worker (`take_pending`, `ChunkSealer::seal`, `install`;
Q-21 amendment, `MAX_PENDING_CHUNKS` = 2 per Q-26 as amended, ADR 0028 decision 8); the crate (cell/color/
row/extras/graphemes, ring grid, modes, charsets, tabs, SGR with underline styles and colors,
ED/EL/ECH/ICH/DCH/IL/DL/SU/SD/REP, scroll regions, alternate screen, OSC 0/1/2/4/7/8/10/11/12/52/
104/110-112/133/9;4, DA/DSR/CPR/XTVERSION, DECRQM, DECRQSS safe subset, kitty keyboard stack per
screen, mouse/focus/paste/sync mode state, hyperlinks with released ids (history summarized at
push time, amortized sweeps), shell marks, damage tracking, event sink, selection model, resize
with streaming reflow); the sequence watchdog (cap at the exact byte, discard to the
terminator); `xtask import-alacritty-refs`; `docs/user/compat.md`; ADR 0028 (accepted, Q-21 to
Q-28 recorded in `docs/14`); README.

Audit rounds 1 (44 items) and 2 (38 items) are addressed in this rebuild; the maintainer
decisions Q-21 to Q-28 are in `docs/14` §2 and ADR 0028. Q-25: protected erase removed, all 45
Alacritty reference tests imported and passing. Q-26: one pending chunk missed the ASCII target on
the M2 (353 MB/s, 48 % inline seals) and was reported back; the amended decision is two
(0.18 MB, the sealed allowance lowered to 0.36 MB), which meets it there (below). Q-27: the
reference-machine run is the maintainer's (MV-WP02-1), pending. Q-28 (2026-11-24): absolute MB/s
gates on GitHub-hosted runners are meaningless (a different CPU each run); ASCII with the push is
optimized as far as reasonable and accepted as measured. Best with the push on: 292 MiB/s (EPYC 9V74,
Windows), 369 MiB/s (Xeon 6975P-C, Linux), 406 MiB/s / 426 MB/s (Apple M2); without history
541-607 MiB/s on x86. Kept from the x86 A/B (3 rounds, +4-7 %): ring index without a division,
last-style cache, ASCII scalar-count shortcut.

Evidence, Apple M2 (not the reference machine; shared with other builds, load average 5 to 11;
criterion medians, fixture inputs; the table is in ADR 0028):

| Criterion | Result |
|---|---|
| Q-21 NFR-PERF-02 with the scrollback push on, `MAX_PENDING_CHUNKS` = 2 | `vt_parse/ascii_8mib` 418.5 MiB/s (439 MB/s, target 400) with 5 286 of 64 550 chunks (8.2 %) sealed inline, `vt_parse/sgr_heavy_8mib` 211.5 MiB/s (222 MB/s, target 120): **met on the M2** (with one pending chunk ASCII was 336.5 MiB/s, 48 % inline: missed, Q-26 amended). Memory: `memory_report` sealed 0.338-0.343 MB, pending 0.086 MB, within the 0.36 / 0.18 MB allowances. x86 hosted runners and i5-9400: accepted as measured (Q-27 amended, Q-28) |
| Q-22 reflow of 10 000 lines (`vt_reflow/10k_lines_200_to_100_columns`) | 36.8 ms after the streaming rewrite, **3.44 ms** at the round-3 tip on the M2 (target 40 ms); x86 hosted runners 3.4 ms (Q-27 amended) |
| Worker seal | `scrollback_seal/fixture_chunks` 1.81 ms (451.7 MiB/s), ceiling 4.29 ms |
| Ceilings | `xtask/src/bench/baseline.rs` covers the with-push parse benches (2x target), the worker seal and the reflow; the vt and `scrollback_seal` baselines are **not** in `windows.json` (the maintainer saves them on the i5-9400, MV-WP02-1) |
| Alacritty ref-tests | 45 of 45 imported and passing (Q-25: `selective_erasure` included, protected erase not implemented) |
| vt fixtures | `tests/fixtures.rs` checks every sequence of `fixtures/vt/implemented.txt` has a positive and a real malformed-input case; boundary cases are named `edge_` |
| Unicode vectors | official `GraphemeBreakTest.txt`, `emoji-sequences.txt`, `emoji-zwj-sequences.txt` (Unicode 17.0.0, `fixtures/unicode/`) plus `tests/unicode_vectors.rs` |
| Property tests | `tests/reflow.rs`, `tests/chunking.rs`, `tests/fuzz_seeds.rs`, `ferrum-scrollback/tests/{pending,roundtrip}.rs`: failing seeds are saved under `proptest-regressions/` |
| dhat | `tests/alloc_parse.rs` (64 KiB reads like the daemon): 0 blocks per pass in steady state without history, with the push on, and with a worker that holds the handles while a 64-line history evicts them; sealing allocates exactly the blob; `tests/alloc_extras.rs`: a full extras table under new colors and refused links allocates nothing; the first use of a new cluster or link allocates once, bounded by the table caps |
| Fuzz | `crates/ferrum-vt/fuzz` target `parse`; the 30-minute CI run on 9451268 (the round-2 tip, run 45201563547) was clean, evidence for that commit only; the local run on the round-3 tip is in the PR |

Open items / not verified:
- The i5-9400 run and its baselines (Q-27, MV-WP02-1). The daemon wiring of the worker seal is
  WP-06 (ADR 0028 consequences).
- Linux and Windows runs of the benches; `vttest` (manual list is WP-17/WP-20); Miri and ASan on
  the new code run in the nightly workflow only.
- Behavior notes: a shell mark on a continuation row stays with the row open when it is fed
  (streaming reflow); RIS also empties the hyperlink table.
- Next: WP-03 `ferrum-proto`.

## WP-01 `ferrum-scrollback` (stack of PRs from `docs/adr-spill-without-mmap`)

Done: ADR 0027 (spill without mmap), the crate (encoding, style table, chunks, spill ring, search,
memory report, `decode_chunk`), property/spill/search/dhat tests, the `SpillStore` contract suite in
`ferrum-testkit`, the product fixture generator in `fixtures/output/`, criterion benches with
baselines, a real `cargo xtask bench`, the `decode_chunk` fuzz target with committed corpus, and
`.github/workflows/nightly.yml` (ADR 0022). Next: WP-02 `ferrum-vt` (grid, reflow on top of this
store; the live-grid line of `docs/03` §2 is re-measured there).

Per-session memory re-measured with the product fixture generator (Q-17; 12 000 logical lines fed,
10 000 rows retained, `dhat` requested bytes reconcile with `memory_report()` to 84 bytes, seeds 1-3):

| Content | Hot chunk | Sealed | Crate misc | Shared LZ4 scratch | Crate sum |
|---|---|---|---|---|---|
| Reference mix (`Mixed`) | 85 735-85 848 B | 338 264-342 854 B | 876 B | 106 594-106 698 B | 424 875-429 578 B |
| Random ASCII (`Entropy`) | 85 687-85 784 B | 706 238-708 977 B | 876 B | 106 594-106 698 B | 792 801-795 626 B |

- Reference mix: with the live grid 0.176 MB and read buffer 0.066 MB of ADR 0024 (the grid is
  WP-02, the buffer WP-06; not re-measurable here) a session is 0.667-0.672 MB, inside the 1.0 MB
  allowance; hot chunk within 0.10 MB (asserted by `tests/memory_budget.rs`). **No Q-21**: the
  reference scenario is within budget. Random ASCII is the already-recorded Q-17 over-allowance
  case (about 1.04 MB per session); the WP-08 gate decides.
- Generator achieved mix (counted by `tests/fixture_mix.rs`): colored 33-37 %, mean columns 66-74,
  non-ASCII 3.9-6.1 %, long lines 1.2-2.8 %, 4.0 runs per colored line, 12 160-12 260 rows.
- `push_line` allocation (dhat, `tests/alloc_push_line.rs`): 0 blocks inside a chunk; exactly 1
  block (the sealed blob) on the push that seals; the LZ4 hash table is reused (`CompressTable`).

Benches (`cargo xtask bench`, medians). The committed baseline `benches/baselines/windows.json` is
from an i5-9400 / Windows 11 desktop (for maintainer machines); the Linux column is one nightly run
on `ubuntu-latest`:

| Bench | Windows | Linux CI |
|---|---|---|
| push 12 256 fixture rows, retain 10 000 | 3.50 ms | 2.84 ms |
| read 10 000 lines sequentially | 2.62 ms | 1.92 ms |
| read 1 000 random lines (each decompresses its chunk) | 138.9 ms | 97.7 ms |
| read 20 480 spilled lines, memory fake / file ring | 7.87 / 8.07 ms | 5.77 / 5.81 ms |
| search 10 000 lines, literal / case-insensitive (target 15 ms, ceiling 30 ms) | 3.33 / 5.84 ms | 2.49 / 3.55 ms |

CI gate (docs/08 §8): the nightly job runs `cargo xtask bench --against HEAD~1`, which benches the
previous commit and the current one on the same runner in the same job (two interleaved rounds,
best median each) and fails above +10 % or above the 2x-target ceiling. On identical code it
measured -0.5 % to -0.1 % on all seven benches (run 45054066134), where comparing with numbers from
another run of the same runner class had differed by up to +18.9 %. The gate also failed as intended
on a doctored baseline (+270.8 %, exit 1).

Nightly (`nightly.yml`, run on `ubuntu-latest` from a temporary push-triggered copy of the
workflow, deleted afterwards; run https://github.com/Juandiazcar/Ferrum/actions/runs/45054066134):
fuzz `decode_chunk` 10 min, 50 912 728 runs, 0 crashes (cov 376, 251 corpus entries; earlier runs
did 74 347 460 and 34 716 106, runner speed varies 2x); Miri (unit tests and the single-line
property) green; ASan test run green; `cargo audit` green; benches vs previous commit green. Miri
cannot run the multi-thousand-line property tests in reasonable time (28 minutes without finishing),
so the workflow runs the smaller subset and says what is excluded.

Deviations / notes:
- Internal entries of `[workspace.dependencies]` carry `version = "0.1.0"` (cargo-deny rejects
  path-only entries); `cargo xtask release prepare` (WP-18) must bump them with the workspace
  version. `cargo hack` runs `--locked`, without `--no-dev-deps`.
- The fuzz package has its own committed `Cargo.lock`; `cargo fuzz run` has no `--locked`, so the nightly job
  runs `cargo fetch --locked` before it and `git diff --exit-code` on the lock after it.
- Spill I/O runs on the pushing thread; compression of full chunks runs wherever the owner calls
  `ChunkSealer::seal` (the worker in WP-06; ADR 0028 decision 8). The pending chunk costs at most
  two chunk buffers per session (`MAX_PENDING_CHUNKS` = 2, maintainer decision Q-26: 0.172 MB
  at most, 0.086 MB measured, 0.18 MB allowance in `docs/03` §2), reported as `MemoryReport::pending_bytes`.
- Search matches per row (a match spanning a soft wrap is not found here); regex is a
  `LineMatcher` implemented by the daemon (no regex dependency added).
- Column widths of wide characters are derived by the consumer from the stored scalars.

## Spike S2 Allocator (branch `spike/s2-allocator`, ADR 0016)

Throwaway harness in `spikes/s2-allocator/` (standalone package, `unsafe_code = "forbid"`, S1's
workload compiled in through `#[path]`): 10 sessions x (12 000 lines fed, 10 000 retained) in one
process, 10 s idle, then an external probe (Linux PSS from `smaps_rollup`, macOS `phys_footprint`
from `footprint -f bytes`, Windows Private Working Set from PDH `Get-Counter`). 3 runs per
allocator and OS plus 10 interleaved throughput rounds. Tables, method, reproduction and CI run
links: `docs/adr/0016-allocator-choice.md`.

| OS | Mean memory of the 10-session process, MB (system / mimalloc / jemalloc) | Decision |
|---|---|---|
| Linux (PSS, CI) | 8.350 / 21.831 / 9.675 (glibc + `MALLOC_ARENA_MAX=1`: 8.357; mimalloc with `MIMALLOC_ALLOW_THP=0`: 9.871) | system allocator |
| macOS (phys_footprint, CI) | 10.372 / 10.312 / 10.569 | mimalloc |
| Windows (Private WS, local / CI) | 8.800 / 10.547 and 8.928 / 10.648; jemalloc not applicable (MSVC) | system allocator |

- Throughput (S1 parse + encode, interleaved 10 rounds, mean lines/s) is within 10 % of the best for
  every candidate on every OS (worst: macOS system and mimalloc 3.8 % below jemalloc), so the
  `docs/03` §7 rule reduces to the lowest memory.
- mimalloc on Linux costs +13.5 MB: transparent huge pages (`AnonHugePages` 20 480 kB with THP
  `madvise`); with `MIMALLOC_ALLOW_THP=0` it is 9.871 MB.
- Back-to-back throughput on shared runners was noisier than the 10 % margin (identical binaries
  swapped order between two CI runs), hence the interleaved series; both are in the ADR.
- `MALLOC_ARENA_MAX=1` showed no effect, but the workload is single-threaded; purge/trim effects
  were not measured (they need `unsafe`). Both are left to the WP-08 gate.
- `docs/03` §2 slack rows not replaced (slack not isolable from whole-process numbers); no
  allowance or gate changed; no open question raised.

## Spike S3 Render pipeline (branch `spike/s3-render-pipeline`, ADR 0025)

Throwaway harness in `spikes/s3-render-pipeline/` (standalone package, product lint tables,
`unsafe_code = "forbid"`, clippy clean; `winit` 0.30.13, `softbuffer` 0.4.8, `swash` 0.2.10, `fontdb`
0.24.0, `tiny-skia` 0.12.0, `rfd` 0.18.0, JetBrains Mono 2.304 variable). Method, per-OS tables,
reproduction commands and CI run links: `docs/adr/0025-render-pipeline-measurements.md`. Windows 11
(i5-9400, local, real display) and CI run 45028410900 (Xvfb, headless weston, macOS runner; the
temporary workflow was deleted in the last commit). Three runs of everything.

| Item | Result (MiB for memory; ranges are the three runs) |
|---|---|
| Full repaint 200×60, typical mix (NFR-PERF-03, target 2.5 ms) | Windows 0.98–1.15 ms; Linux CI 0.66 ms; macOS 0.39–0.57 ms (clamped window, 146×40 cells only) |
| Full repaint 200×60, dense text (12 000 glyphs) | **Windows 2.64–2.88 ms (over 2.5 ms; Q-19: target kept, SIMD blitter required in WP-07)**; Linux CI 1.60–1.70 ms; macOS 0.79–0.83 ms (clamped) |
| 4-row damage frame (repaint + `present_with_damage`) | Windows 61–76 + 35–37 µs (full-rect present 0.78–0.80 ms); X11 34–71 + 0.3 µs; Wayland 34.5 + 2.8–2.9 µs; macOS 29–30 + 0.9–1.0 µs |
| `buffer_age` over the first four presents | Windows 0, 1, 1, 1; X11 0, 1, 1, 1; Wayland 0, 0, 2, 2 (double buffer); **macOS 0, 0, 0, 0 (contents undefined)** |
| Idle total at 1440×900 (spike only, no UI toolkit) | Windows 7.23 PWS; X11 7.93–8.11 PSS; Wayland 8.49–11.00 PSS; macOS 24.42–24.52 `phys_footprint` (window clamped to 1024×642) |
| Window surface | Windows 5.1 (one buffer); X11 2.6 PSS (4.94 mapped, shared with the X server); Wayland 5.0–7.5 PSS (two buffers, 9.9 mapped); 2880×1800 equivalent: Windows 19.8–19.9, X11 10.0 PSS, Wayland 29.7–29.8 PSS at idle |
| Process base (window created) | Windows 1.8, X11 4.6–4.7, Wayland 2.6–2.7, **macOS 10.6–10.7 (allowance 4.5; Q-20: unchanged, real-Mac run decides)** |
| Glyph cache, fonts | 10 032 B for 94 ASCII glyphs; font plus cache 0.26–0.88 resident (0.29 MiB file) |
| `tiny-skia` | rounded rect 4.5–9 µs, stroke 6.6–13 µs, gradient 30–50 µs, shadow tile 0.29–0.50 ms; binary +272–282 KB (+8.5 % Linux, +17.5–19.0 % Windows and macOS); tiles 0.84 MiB (0.64–1.28 resident delta) |
| `rfd` | binary +9 KB Windows, +109 KB Linux, +17 KB macOS; linked and unused: no measurable memory; Windows dialog open +6.2 to +6.4, still +5.0 to +5.1 six seconds after closing |

- `docs/03` §2 UI rows (process base, window surface, glyph cache, fonts, chrome tiles) now hold measured values; allowances and gates untouched.
- Decisions (ADR 0025): keep `tiny-skia` for chrome tiles only; native folder dialog through `rfd` run in a short-lived helper process, with the in-app path input as fallback (approved by the maintainer 2026-11-21, Q-06 extended); ADR 0006 validated as an architecture.
- **Over target, decided by the maintainer 2026-11-21 (no S3 question stays open):** Q-19, the target stays 2.5 ms on dense text and WP-07 must meet it with the SIMD blitter (spike scalar blitter: Windows 2.97–3.00 ms); Q-20, the macOS process-base allowance is unchanged and a real-Mac run of the spike (MV-S3-1) before WP-08 decides.
- ADR 0025 Consequences carry implementation requirements for WP-07/WP-08 (honor `buffer_age`, touch every pixel on the first frame, LRU for the glyph cache, X11 dev packages on the Linux build image).
- Not measured: HiDPI hardware (scale 1.0 everywhere), unclamped macOS window, idle CPU, Linux and macOS folder dialogs, real glyph working set (CJK, emoji, weights), a non-scalar blitter. Steps are in `docs/manual-verification.md` (MV-S3-1 to MV-S3-4).

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
