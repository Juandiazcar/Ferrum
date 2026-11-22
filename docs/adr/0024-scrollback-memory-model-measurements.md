# 0024. Scrollback memory model: S1 measurements

- Status: accepted for the measurements and for the conclusions each number supports (see
  "Decision"); the points the numbers do not support are not accepted here and are open in
  `docs/14` §2 (Q-S1-01, Q-S1-02)
- Date: 2026-11-21

## Context

`docs/12` Spike S1 asks for a throwaway scrollback encoder driven by `vte`, fed with the
reference scenario of `docs/03` §1, measuring bytes per line raw and compressed and comparing
with a naive cell-grid scrollback and `alacritty_terminal`. Its exit is the measured per-session
memory for 10 000 lines, confirming or correcting `docs/03` §2. ADR index rows 0004 (custom VT
on `vte`, reject `alacritty_terminal`) and 0005 (encoded text + style runs, chunk-compressed)
are marked "validated by S1"; this ADR holds that validation.

## Method

Code: `spikes/s1-memory-model/` (standalone package, own `[workspace]` and `Cargo.lock`, excluded
from the product workspace by the root `exclude = ["spikes"]`). It is throwaway; the generator
stays in the spike because it is not yet the reusable product fixture generator
(`fixtures/output/` is filled by WP-01 and WP-08).

- **Encoder**: exactly `docs/02` §3.2. `line := flags:u8 | text_len:varint | utf8 text (trailing
  default blanks trimmed) | run_count:varint | {style_id:varint, run_len:varint}*`, per-session
  style table (fg, bg, attrs interned, id 0 is default), hot chunk of at most 1 024 lines
  uncompressed, sealed chunks LZ4-block compressed with `lz4_flex` 0.14.2 (`Box<[u8]>`) plus a
  delta-encoded offsets index (varint line lengths). `run_len` counts Unicode scalars; wide
  characters are re-derived from the text. Eviction keeps exactly 10 000 lines (a partially
  skipped oldest chunk stays allocated until it is entirely outside the window, which is
  counted).
- **Driver**: `vte` 0.15.0 (parser only) into a minimal grid: 200 columns x 55 rows of 16-byte
  `Cell`s (`docs/02` §3.1) in a ring, printable text with width via `unicode-width`, CR/LF/BS/HT,
  SGR (16, 256 and truecolor), autowrap. Rows scrolling off the top go to the encoder.
- **Content** (deterministic, seeded, splitmix64): 12 000 logical lines, 10 000 rows retained
  (every run verified, see below). Rolled independently per line: 35 % colored (3 to 5 SGR runs,
  mean 4), 5 % non-ASCII (CJK, emoji, accented Latin), 2 % long (300 to 400 columns), others 10
  to 118 columns (mean about 64) so that the average is about 70. The achieved mix is measured
  and printed per run (table below), never assumed. Three text profiles:
  - `mixed`: synthetic log-like lines (timestamps, compiler lines, JSON, `ls -la`, git graph, hex
    dump) from a vocabulary of about 120 words. This is the reference fixture profile.
  - `corpus`: real prose, the `docs/spec` markdown files ASCII-sanitized (harsher: natural text).
  - `entropy`: uniformly random printable ASCII (lower bound for any compressor).
- **Accounting**: a counting global allocator (`src/alloc.rs`) sums the requested size of every
  live allocation. The per-session number is the live-byte delta around building the whole
  session (grid, parser, encoder, 64 KiB read buffer, style table, indices) after feeding
  12 000 lines. Capacity-based accounting (`Vec::capacity`, `Box<[u8]>::len`) is printed beside
  it and reconciles with the allocator to +84 bytes in every run. The shared LZ4 output scratch
  (about 171 KB) is excluded from the per-session figure because `docs/02` §5 compresses on one
  worker thread per daemon; the figure including it is also shown. The allocator counts
  requested bytes, not malloc headers or fragmentation (that is S2 and the 3 MB allocator slack
  line).
- **Correctness**: after measuring, the same seed is replayed into an independent reference sink
  that hashes each scrolled-off row straight from its cells; every one of the 10 000 retained
  rows is decoded from the store (decompressing sealed chunks) and its hash compared. All 18
  encoder runs printed `verify ok`.
- **Comparisons**: `naive` = every scrolled-off row kept as a full 200 x 16 B cell row.
  `alacritty` = `alacritty_terminal` 0.26.0 (latest on crates.io, verified 2026-11-21, released
  2026-04-06) `Term` with `scrolling_history = 10000` fed the same bytes through its own
  `vte::ansi::Processor`; history length verified equal to 10 000. Both measured by the same
  allocator delta. It was cheap, so it is measured rather than estimated.
- **Hot chunk growth**: `--hot-step 0` is plain `Vec` growth; `--hot-step 16384` reserves
  exactly (`reserve_exact`) in steps of 16 KiB. Both are reported because the capacity policy
  changes the hot-chunk figure.
- **Repetition**: each configuration was run with seeds 1, 2, 3 (one process per run, so 24
  runs). Memory figures are deterministic given the seed; throughput varies with the machine.
- **Machine**: Windows 11 Pro 10.0.26300, Intel Core i5-9400 (6 cores), 15.9 GB RAM, rustc
  1.100.0, `--release` (fat LTO, codegen-units 1). All processes ran sequentially.

Reproduce (repository root, Git Bash):

```bash
bash spikes/s1-memory-model/run-all.sh > s1-results.txt      # all 24 runs, about 1 minute
# single runs:
cargo build --release --manifest-path spikes/s1-memory-model/Cargo.toml
B=spikes/s1-memory-model/target/release/ferrum-spike-s1
$B --model ferrum --profile mixed --seed 1 --hot-step 16384
$B --model ferrum --profile corpus --corpus-dir docs/spec --seed 1
$B --model naive --seed 1
$B --model alacritty --seed 1
cargo test --release --manifest-path spikes/s1-memory-model/Cargo.toml   # encoder unit tests
```

`cargo clippy --release --all-targets -- -D warnings` and `cargo fmt --check` were run inside the
spike (they are not part of CI; the spike is excluded) and are clean with the workspace lint
policy, except for the one `unsafe impl GlobalAlloc` (see Deviations).

## Results

Sizes are bytes; "MB" in prose is 1 000 000 bytes. Columns are seeds 1 / 2 / 3.

### Achieved content mix (profile `mixed`; the other profiles are within 0.5 points)

| Property | Target (`docs/03` §1) | Measured |
|---|---|---|
| Colored lines | 35 % | 35.09 / 34.76 / 34.93 % |
| SGR runs per colored line | about 4 | 4.00 / 4.01 / 4.00 |
| Average visible columns | 70 | 69.25 / 68.91 / 69.15 |
| Non-ASCII lines | 5 % | 4.98 / 5.24 / 4.92 % |
| Lines of 300+ columns | 2 % | 1.81 / 1.76 / 1.77 % |
| Rows pushed for 12 000 lines (wrapping adds rows) | n/a | about 12 160, 10 000 retained, 9 sealed chunks, hot 904 lines (seed 1) |
| Styles interned | < 200 | 26 |

### Bytes per line (encoded rows, all rows pushed, retained or not)

| Profile | Raw bytes/row | LZ4 bytes/row | Index bytes/row | Ratio raw/LZ4 |
|---|---|---|---|---|
| mixed | 75.32 / 74.95 / 75.23 | 37.45 / 37.14 / 37.28 | 1.04 / 1.04 / 1.04 | 2.018 / 2.020 / 2.017 |
| corpus (prose) | 75.05 / 75.10 / 75.19 | 55.54 / 55.49 / 55.68 | 1.04 / 1.04 / 1.04 | 1.353 / 1.352 / 1.354 |
| entropy (random) | 74.53 / 74.92 / 76.11 | 74.83 / 75.30 / 76.29 | 1.03 / 1.04 / 1.04 | 0.996 / 0.996 / 0.996 |

A row averages 2.02 to 2.05 runs (all runs); 64.4 to 65.4 % of rows are a single default-style run,
which cost 1.95 to 1.98 bytes per row (averaged over all rows) on `run_count|style_id|run_len` (informational: encoding "no
runs" for them would save about 2.6 % of raw bytes; not done, the spike follows the spec).

### Per-session memory for 10 000 retained lines (allocator-measured)

Per-session = everything except the shared LZ4 scratch. "Incl. scratch" adds it back.

| Profile / hot growth | Total per session (seeds 1 / 2 / 3) | Spread | Incl. scratch (1 / 2 / 3) |
|---|---|---|---|
| mixed, bounded (`--hot-step 16384`) | 684 566 / 682 981 / 684 887 | 1 906 | 855 608 / 851 461 / 850 679 |
| mixed, plain `Vec` growth | 750 272 / 701 675 / 687 433 | 62 839 | 921 314 / 870 155 / 853 225 |
| corpus, bounded | 851 500 / 853 747 / 854 961 | 3 461 | 1 019 592 / 1 025 735 / 1 024 017 |
| corpus, plain growth | 917 239 / 872 364 / 857 292 | 59 947 | 1 085 331 / 1 044 352 / 1 026 348 |
| entropy, bounded | 1 030 583 / 1 035 275 / 1 042 498 | 11 915 | 1 200 251 / 1 202 305 / 1 215 468 |
| entropy, plain growth | 1 096 449 / 1 053 982 / 1 044 874 | 51 575 | 1 266 117 / 1 221 012 / 1 217 844 |

Component breakdown (capacity-based, bounded growth; allocator minus capacity sum is +84 in all
runs):

| Component (`docs/03` §2 line) | mixed | corpus | entropy | Allowance |
|---|---|---|---|---|
| Live grid, 200 x 55 x 16 B | 176 000 | 176 000 | 176 000 | 250 000 |
| Hot chunk data + 4 096 B line-offset index | 85 846 / 85 754 / 85 518 | 85 813 / 85 831 / 85 733 | 85 686 / 85 741 / 85 688 | 100 000 |
| Sealed chunks: LZ4 data + delta index + structs | 353 920 / 352 427 / 354 569 | 520 887 / 523 116 / 524 428 | 700 097 / 704 734 / 712 010 | 450 000 |
| Misc: 64 KiB read buffer 65 536 + style table 860 + line scratch 2 320 | 68 716 | 68 716 | 68 716 | 200 000 |
| Sum | about 684 000 | about 852 000 | about 1 035 000 | 1 000 000 |

Plain `Vec` doubling leaves the hot chunk at 151 552 / 104 448 / 88 064 bytes (seeds 1 / 2 / 3,
data capacity 147 456 / 100 352 / 83 968 plus the index). The hot chunk's content never exceeds
81 188 bytes (`hot_len_max_bytes`, all runs); 1 024 rows at 75 B is the 0.077 MB the design
expected. Misc excludes what the spike does not have: the PTY write queue, alt-screen grid,
hyperlink table, parser state (`vte::Parser` lives on the stack, no heap).

### Comparison (same bytes, same 200 x 55 grid, 10 000 history rows)

| Model | Bytes per session | Per history row | Seeds / spread | Throughput (lines/s) |
|---|---|---|---|---|
| Ferrum encoder, mixed, bounded | 684 566 / 682 981 / 684 887 | 38.3 B compressed incl. index | 1 906 | 600 504 / 640 933 / 636 875 (plain growth: 649 864 / 638 009 / 625 039) |
| Naive 16-byte cells x 200 cols | 32 503 680 on all 3 seeds | 3 200 B | 0 | 621 845 / 634 518 / 564 302 |
| `alacritty_terminal` 0.26.0 (24-byte cell) | 51 234 147 on all 3 seeds | about 5 123 B | 0 | 294 080 / 323 371 / 284 427 |

Naive and Alacritty sizes do not depend on content (fixed-size rows), hence zero spread. Ratios
against the Ferrum encoder (mixed, bounded): naive 47.5x, Alacritty 74.8x. Alacritty's
`Cell` is 24 bytes (`size_of`, measured), the figure `docs/02` §3.3 called "20+".

### Encode throughput (information only, not a gate)

Whole pipeline `vte` parse + grid + encode + inline LZ4 (generation excluded), mixed profile:
600 504 to 649 864 logical lines/s over the 6 runs above. Encoder only (row encode and sealing,
timed inside `push_row`): 970 074 to 1 026 899 rows/s with plain growth, 972 006 to 1 012 156
with bounded growth. Corpus and entropy runs are in the same range (502 082 to 743 342 lines/s,
718 958 to 1 268 021 rows/s) on a noisy desktop; the spread between repetitions (up to 30 %)
shows these are not stable enough to be baselines. Compression runs inline in the spike; the
design runs it on the worker thread.

## Decision

Confirmed by the numbers:

1. **ADR 0004 holds, strongly.** Storing scrollback as cells costs 32.5 MB (16 B cells) to
   51.2 MB (`alacritty_terminal`) per session for 10 000 lines at 200 columns; the encoded store
   costs 0.68 to 1.04 MB including the live grid. Ten Alacritty-style sessions would be about
   512 MB, ten times the whole 50 MB gate. Alacritty is also about 2 times slower on this
   workload in the spike (it does more per cell; informational). `docs/02` §3.3 claim confirmed:
   its cells are 24 bytes.
2. **ADR 0005 holds**: the encoding of `docs/02` §3.2 round-trips (10 000 of 10 000 rows verified
   in 18 runs) at about 75 raw bytes per row, which is better than the 90 estimated.
3. **Per-session memory fits the 1.0 MB per-session allowance for the reference fixture mix**:
   0.683 to 0.685 MB (mixed, bounded hot growth), 10 sessions 6.84 MB, inside the design's
   6.5 to 8 MB expectation. The live grid (0.176 MB), hot chunk and misc lines are inside their
   allowances.

Corrected by the numbers:

4. **The expected LZ4 ratio of 2.5 to 4x is not supported.** Measured 2.02x on the synthetic
   fixture mix, 1.35x on real prose and 1.0x on random text; per-row sealed cost 38.3, 56.6 and
   about 76 bytes including the index. `docs/03` §2 is updated with measured values.
5. **The sealed-chunk line is not within its 0.45 MB allowance for every content type**:
   0.352 to 0.355 MB (mixed) but 0.521 to 0.524 MB (prose) and 0.700 to 0.712 MB (random), and the
   per-session sum for random text is 1.03 to 1.04 MB against 1.0 MB. No allowance or gate is
   changed by this ADR; the evidence and options are in `docs/14` §2 (Q-S1-01). Which content
   the real `memcheck` fixtures resemble is decided in WP-01/WP-08 with the product fixture
   generator.
6. **The hot chunk needs a bounded growth policy.** With plain `Vec` doubling its capacity ends at
   88 064 to 151 552 bytes, above the 0.10 MB allowance in 2 of 3 seeds; with exact 16 KiB
   reservations it is 85 518 to 85 846 bytes in every run and throughput is unchanged within
   noise. WP-01 must implement bounded growth (Q-S1-02, default taken, no allowance change).

Not decided here: the `docs/03` §2 daemon totals, the allocator slack line (S2), the UI side (S3),
any change to the 1 024-line chunk size or to LZ4, and spill behavior (not exercised; 10 000 is
below the 50 000 threshold).

## Consequences

- `docs/03` §2: per-session scrollback rows now carry measured values citing this ADR; no
  allowance or gate edited. `docs/14` §2 gains Q-S1-01 and Q-S1-02; §3 has the 0024 row.
- WP-01 starts from these numbers and must reproduce them with the product fixture generator and
  `dhat`; it must not copy the spike code (counting allocator, inline compression, `HashMap`
  style table are spike shortcuts).
- The spike may be deleted after WP-01; the numbers stay here.

## Deviations and limits (stated plainly)

- The counting allocator needs one `unsafe impl GlobalAlloc` (with `// SAFETY:`), allowed in
  `src/alloc.rs` of the excluded spike only; `docs/05` §4 would otherwise forbid it. It is not in
  any product crate.
- The spike uses `anyhow`, `std::HashMap`, and `print`-free reporting through `writeln!`;
  `alacritty_terminal` is a spike-only dependency in the spike's own lockfile, not in the
  product workspace, `docs/07` or `deny.toml` scope.
- The content is synthetic or documentation prose; no real terminal session corpus (compiler
  output, `git log`, build logs of real repositories) was measured. LZ4 ratio depends strongly on
  that content, so the sealed-chunk figures are a range, not a point.
- Measured: requested allocation bytes. Not measured: allocator headers and fragmentation, real
  process PSS/private working set, idle behavior, reflow, search, spill, alt screen, zstd or
  larger chunks as alternatives, macOS and Linux (only Windows 11 was run; allocation sizes are
  platform independent but throughput is not).
- Compression is inline in the spike (so throughput includes it); the shared LZ4 scratch is
  excluded from the per-session figure and shown separately.
- Lines are 12 000 logical lines; wrapping makes about 12 160 rows, of which 10 000 are
  retained; "10 000 lines" in `docs/03` is read as rows.
