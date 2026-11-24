# Manual verification

Checks that need a capability CI and agents do not have (real macOS/Windows machines, a physical
display, signing accounts). Each release candidate records the result per OS. Items are added by
the work package that introduces the unverifiable behavior.

## Repository bootstrap (WP-00)

| ID | Check | Linux | macOS | Windows |
|---|---|---|---|---|
| MV-00-1 | Fresh clone: `cargo xtask setup` activates hooks and reports tools correctly | pending | pending | verified 2026-11-21 (author machine) |
| MV-00-2 | `git commit` without `-s` or with a non-conventional subject is rejected by the hook | pending | pending | verified 2026-11-21 (author machine) |
| MV-00-3 | `.githooks/*` keep the executable bit after clone | pending | pending | index mode 100755 verified |

## Windows PTY (spike S4, ADR 0026)

CI covers Windows Server 2025 (build 26100) and the author machine covers Windows 11 (build 26300). Older ConPTY builds were not exercised; the WP-05 contract suite must be run there.

| ID | Check | Linux | macOS | Windows |
|---|---|---|---|---|
| MV-S4-1 | Run `cd spikes/s4-windows-pty && cargo run --release -- all` (needs `pwsh` for the pwsh rows) on Windows 10 1809, 21H2 and 22H2: compare experiments 2 (exit codes), 5 (startup bytes) and 8 (`drop(master)` with an unread flooding child must return, not hang) with ADR 0026 | n/a | n/a | pending |
| MV-S4-2 | Same run on Windows 11 24H2 and 25H2 retail builds | n/a | n/a | pending (build 26300 verified 2026-11-21) |
| MV-S4-3 | Windows on ARM64 (aarch64): experiments 1, 2, 5 | n/a | n/a | pending |

## Render pipeline (spike S3, ADR 0025)

CI covered X11 under Xvfb, headless Wayland (weston) and a virtual macOS runner whose display clamped the window to 1024×642; the author machine covered Windows 11 at scale 1.0. Run from `spikes/s3-render-pipeline` with `bash run.sh <font.ttf> 3` (see the ADR for the font and the packages).

| ID | Check | Linux | macOS | Windows |
|---|---|---|---|---|
| MV-S3-1 | Memory and repaint at 1440×900 with a window the display does not clamp (physical machine, scale 1.0): compare with the ADR tables, in particular macOS idle `phys_footprint` (Q-20) | pending (desktop X11 and Wayland) | pending | verified 2026-11-22 (author machine) |
| MV-S3-2 | The same on a display with scale factor 1.5 or 2.0, without `--physical`: record the surface-excluded and total figures of `AC-MEM-04` | pending | pending | pending |
| MV-S3-3 | Folder dialog resident cost (`--dialog`, a person selects a folder once and cancels once): xdg-portal under GNOME and KDE, `NSOpenPanel` on macOS; on Windows a selection instead of Cancel | pending | pending | Cancel verified 2026-11-22 (+5.0 to +5.1 MiB after closing) |
| MV-S3-4 | Full repaint of 200×60 dense text on a second Windows machine and on Apple silicon hardware, to compare with the SIMD blitter target (Q-19) | pending | pending | pending |

## Terminal state machine (WP-02, ADR 0028)

The author machine is an Apple M2; NFR-PERF-02, the reflow budget and the worker seal are judged on
the reference machine (maintainer decision Q-27; ASCII with the push is accepted as measured, Q-28).
Run on the i5-9400 (Windows 11), on a quiet machine, from the repository root at the tip of the WP-02 stack:

```
cargo xtask bench --quick
cargo xtask bench --save-baseline
git diff --stat benches/baselines/windows.json
```

The first run reports the `vt_*` and `scrollback_seal/*` benches as missing baselines (expected)
and fails if one is above its absolute ceiling (`xtask/src/bench/baseline.rs`: 2x the target).
The second writes the medians into `benches/baselines/windows.json`; commit that file in its own
PR with the printed numbers (MiB/s of `vt_parse/ascii_8mib` and `vt_parse/sgr_heavy_8mib` with the
push on, `vt_reflow/10k_lines_200_to_100_columns`, `scrollback_seal/fixture_chunks`, and the
"inline seals" lines the parse bench prints on stderr).

| ID | Check | Linux | macOS | Windows |
|---|---|---|---|---|
| MV-WP02-1 | NFR-PERF-02 with the scrollback push on (400 MB/s ASCII, accepted as measured per Q-28; 120 MB/s SGR-heavy), the reflow of 10 000 lines (40 ms) and the worker seal ceiling, measured with the commands above; baselines saved | not required | not required | pending maintainer run |

## Later work packages

Rows for IME, HiDPI, compositor behavior, OS drag and drop, installers and signing are added with
WP-07, WP-11, WP-16 and WP-18 (`docs/spec/docs/12-build-order.md`).
