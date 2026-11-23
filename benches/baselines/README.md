# benches/baselines

Committed criterion baselines for `cargo xtask bench` on a **maintainer's machine** (docs/03 §6,
docs/08 §8: "full-precision numbers are taken on the maintainer's machines").

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
- `windows.json` was produced locally (Intel Core i5-9400, Windows 11, rustc 1.100.0, release
  profile). There is no `linux.json` or `macos.json` yet: a maintainer creates them on their own
  machines. Numbers measured on another machine (for example an Apple M2 during development) are
  never written here.
- Benches in the file: the seven `scrollback_*` benches of WP-01. The six `vt_*` benches of WP-02
  (`vt_parse/{ascii_8mib, sgr_heavy_8mib, unicode_8mib, ascii_8mib_no_history,
  sgr_heavy_8mib_no_history}` and `vt_reflow/10k_lines_200_to_100_columns`) have **no baseline
  yet**: the inputs come from the seeded `fixtures/output/` generator and the reference machine
  (the maintainer's i5-9400, Windows 11) has not run them on this code. A maintainer saves them
  with `cargo xtask bench --save-baseline` on that machine; until then the absolute ceilings
  above are the gate that matters for NFR-PERF-02 and `docs/03` §4, and `cargo xtask bench` on
  Windows reports the missing baselines instead of passing.

**Filled by:** WP-01 (`ferrum-scrollback`: push, read, spilled read, search, in `windows.json`)
and WP-02 (`ferrum-vt`: parse and reflow; their baselines come from the maintainer's run on the
i5-9400, maintainer decision Q-27).
