# benches/baselines

Committed criterion baselines for `cargo xtask bench` (docs/03 §6). Policy (maintainer decision
Q-27, amended): the agents measure them on GitHub-hosted runners, and every entry records the
runner, the rustc version and the run id it comes from. Entries from before the amendment record
the machine they were taken on.

- One file per OS: `<os>.json` (`windows`, `linux`, `macos`), because timings are only comparable on
  the same machine class. Content: the **median** time in nanoseconds of every bench id
  (`<group>/<bench>`), read from criterion's `estimates.json`.
- `cargo xtask bench` compares with this OS's file and fails if a bench is more than 10 % slower,
  if a baseline bench was not run, if a bench has no baseline, or if a bench is above an absolute
  ceiling (2x the budget target, `xtask/src/bench/baseline.rs`: scrollback search over 10 000 lines
  30 ms). A missing baseline file for the OS is a failure, never a pass.
- `cargo xtask bench --quick` uses criterion's quick mode (fewer samples; same rules).
- `cargo xtask bench --save-baseline` rewrites this OS's file. Do it deliberately, on a quiet
  machine, and explain the change in the PR (a slower baseline is a loosened gate and needs the
  reason).
- **CI does not use these files** (docs/08 §8: shared runners are too noisy for numbers measured
  elsewhere). The nightly job runs `cargo xtask bench --against HEAD~1`: the previous commit and the
  current one are benched in the same job on the same runner and compared (+10 %, same ceilings).
- The seven `scrollback_*` entries of `windows.json` were taken on an Intel Core i5-9400 with
  Windows 11 and rustc 1.100.0 (release profile), before the amendment. There is no `linux.json`
  or `macos.json` yet. Numbers from a development machine (for example an Apple M2) are never
  written here: an entry comes from the recorded hosted runner or reference machine only.
- Benches in the file: the seven `scrollback_*` benches of WP-01. The six `vt_*` benches of WP-02
  (`vt_parse/{ascii_8mib, sgr_heavy_8mib, unicode_8mib, ascii_8mib_no_history,
  sgr_heavy_8mib_no_history}` and `vt_reflow/10k_lines_200_to_100_columns`) have **no baseline
  yet**: Q-27 as amended keeps no `vt_*` baselines until a reference-machine run. Until then the
  absolute ceilings above are the gate that matters for NFR-PERF-02 and `docs/03` §4, and
  `cargo xtask bench` on Windows reports the missing baselines instead of passing.
- `scrollback_seal/fixture_chunks` (the compression the daemon worker does for the push path,
  Q-21 amendment in ADR 0028) has **no baseline yet** for the same reason; its absolute ceiling
  (4.29 ms: 2x the time to seal the 857 466 encoded bytes at NFR-PERF-02's 400 MB/s) gates it.

- `vt_parse/ascii_8mib_inline_seal` and `vt_parse/sgr_heavy_8mib_inline_seal` compress on the
  parse thread instead of the worker; they are informational (they show the gap the worker
  closes) and have no absolute ceiling. Like the other `vt_*` benches they get their baselines
  with the reference-machine run (Q-27 as amended) and are then gated the same way (+10 %);
  until then `cargo xtask bench` reports them as missing baselines.

- `proto_encode/row_200_cols` and `proto_encode/damage_200x60` (WP-03, `ferrum-proto`: encoding
  one 200-column row and a full 200x60 `Damage` frame) have **no baseline yet** and no absolute
  ceiling (`docs/03` §4 names no target for them). An agent adds them from a recorded hosted
  run, as for `daemon_search` below; until then `cargo xtask bench` without `--against` reports
  them as missing baselines, and the nightly `--against HEAD~1` job gates them (+10 %).

- `daemon_search/literal_insensitive_10k_lines` and `daemon_search/regex_insensitive_10k_lines`
  (WP-06, `ferrum-daemon`: the daemon's search path over 10 000 lines, `docs/03` §4 15 ms, ceiling
  30 ms) were measured on a GitHub-hosted Windows runner (Intel Xeon 6973P-C, rustc 1.100.0,
  release profile; run 45663852806), as the agents measure under Q-27 as amended: 3.92 ms and
  6.88 ms (medians).

- `render_repaint/dense_200x60` and `render_repaint/typical_200x60` (WP-07, `ferrum-render`: the
  full repaint of NFR-PERF-03, every cell a glyph per maintainer decision Q-19, and the typical
  mix; absolute ceiling 5 ms, 2x the 2.5 ms target) and `render_repaint/damage_4_rows` (a frame
  with four changed rows; no target in `docs/03` §4, so no ceiling) were measured on a
  GitHub-hosted Windows runner (AMD EPYC 9V45, rustc 1.100.0, release profile; run 45820450614,
  three runs, the median of their medians), as the agents measure under Q-27 as amended:
  0.659 ms, 0.324 ms and 55.3 µs.

**Filled by:** WP-01 (`ferrum-scrollback`: push, read, spilled read, search, in `windows.json`),
WP-02 (`ferrum-vt`: parse and reflow, no baseline until the reference-machine run, Q-27 as
amended), WP-06 (`ferrum-daemon`: search, from a hosted Windows runner) and WP-07
(`ferrum-render`: repaint).
