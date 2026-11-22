# 03 — Memory and Performance Budget

This file defines the numbers that decide whether a change may merge. The figures marked **(to validate)** are design estimates; spikes S1–S3 in `docs/12` replace them with measurements in the first work packages, and this file is then updated with the measured values.

## 1. The reference scenario (`AC-MEM-01`)

| Item | Value |
|---|---|
| Processes counted | `ferrum` (UI) + `ferrumd` (daemon). **Child processes (shells, agent CLIs) are excluded.** |
| Sessions | 10, each sized to the reference window's grid (≈ 200×55); only 1–4 are visible at a time, the rest are background sessions |
| Scrollback | 10 000 lines per session, **filled** (feed 12 000 lines, then verify 10 000 retained) |
| Content | Deterministic fixture mix: 35 % colored lines (≈ 4 SGR runs), average 70 visible columns, 5 % non-ASCII incl. CJK/emoji, 2 % very long lines (300+ cols) |
| Window | 1440×900 logical pixels at scale factor 1.0 (Linux CI under Xvfb; manual on macOS/Windows) |
| State | Steady state: 10 s of idleness after the fill; no images; default theme and font |
| Metric | Sum of per-process **PSS** (Linux), **phys_footprint** (macOS, as shown by Activity Monitor), **Private Working Set** (Windows, as Task Manager) |
| Gate | **Sum < 50 MB.** CI warns at ≥ 45 MB and fails at ≥ 50 MB |

A second gate, `AC-MEM-04`, handles HiDPI honestly: the window surface (framebuffer) is owned by the OS compositor path and grows with the pixel count (1440×900 at 2× is ≈ 20.7 MB *per buffer*). Therefore the gate at scale ≥ 1.5 is: **total minus window-surface bytes < 42 MB**, and the report prints both numbers. Techniques to keep the surface small are in §5.

## 2. Budget table (design)

| Component | Allowance | Expected (to validate) | Notes |
|---|---|---|---|
| **Daemon** | **≤ 18 MB** | ≈ 11–14 MB | 16 MB of components + 2 MB reserve |
| – process base (binary pages, libc, thread stacks) | 3.0 | 2.5 | no async runtime, 2 threads |
| – per session live grid (≈ 200×55×16 B) | 0.25 | 0.18 | alt screen grid allocated lazily |
| – per session hot chunk (1 024 lines × ≈ 90 B) | 0.10 | 0.09 | uncompressed |
| – per session sealed chunks (≈ 9 000 lines) | 0.45 | 0.25–0.4 | LZ4; ratio 2.5–4× on typical text |
| – per session misc (style table, parser, PTY buffers, indices) | 0.20 | 0.15 | read buffer 64 KiB, write queue bounded |
| – 10 sessions subtotal (10 × 1.0) | 10.0 | ≈ 6.5–8 | |
| – allocator slack / fragmentation | 3.0 | 2–3 | measure allocators (S2) |
| **UI** | **≤ 26 MB** | ≈ 19–24 MB | 24 MB of components + 2 MB reserve |
| – process base (binary, libwayland/X11, xkb; no fontconfig) | 4.5 | 4.0 | |
| – window surface at 1440×900@1x (up to 2 buffers) | 10.5 | 5.2–10.4 | OS-owned; see §5 |
| – glyph cache | 3.0 | 1.5–2.5 | bounded LRU |
| – fonts (mmap'd, only touched pages count) | 1.5 | 1.0 | |
| – viewport caches (≤ 4 visible sessions) | 1.5 | 0.6 | encoded rows only |
| – UI chrome tiles + widget state | 1.5 | 1.0 | |
| – allocator slack | 1.5 | 1.2 | |
| **Image cache (elastic, off in the reference scenario)** | **+3 MB cap** | 0 | hard cap, LRU eviction; only grows when images are displayed |
| **Total allowance** | **≤ 44 MB** (≤ 47 MB with images in use) | ≈ 30–38 MB | 3–6 MB of headroom against the 50 MB gate |

If a component exceeds its allowance in a PR, the PR must either shrink something else or update this table with an ADR approved by the maintainer.

## 3. Design rules that make the budget possible

1. **Scrollback is never stored as cells.** It is encoded text + style runs, chunk-compressed (`docs/02` §3.2). This alone is the difference between ~8 MB and ~290 MB for 100 000 lines.
2. **The UI holds only visible rows.** Hidden sessions cost the UI nothing.
3. **One copy of each thing.** The daemon owns terminal state; the UI never mirrors scrollback.
4. **Every cache has a hard cap and an eviction policy** (glyphs, images, hyperlink tables, search results, layout caches). No unbounded `HashMap`/`Vec` that grows with session output.
5. **Interning.** Styles, hyperlink URIs, grapheme clusters, and cwd strings are interned per session.
6. **Small-by-default containers**: `SmallVec`, `CompactString` only where profiling shows wins; plain `Box<[T]>` for sealed data.
7. **Return memory.** After large operations (reflow, search, compression), shrink buffers (`shrink_to_fit` on pooled buffers above a threshold) and, on Linux/glibc, call `malloc_trim(0)` at most once per 30 s when the daemon is idle. If `mimalloc`/`jemalloc` wins in S2, use its purge APIs instead.
8. **No per-frame allocation.** Paint and parse loops reuse scratch buffers owned by long-lived structs. Verified by `dhat` tests (§6).
9. **Thread stacks are small and explicit** (256–512 KiB) because every thread has a name and a bounded job.
10. **Binary size is memory.** `opt-level = 3` only in hot crates (`ferrum-vt`, `ferrum-scrollback`, `ferrum-render`); `opt-level = "s"` for UI chrome and glue if it measurably reduces resident size without hurting latency; LTO fat, `codegen-units = 1`, `strip = "symbols"`.
11. **Optional features cost nothing when unused**: kitty graphics/sixel code paths allocate their caches on first use; spill files are created on first spill; Phase 2 crates are not instantiated unless enabled.

## 4. Performance budget (latency and throughput)

| ID | Metric | Target | Method |
|---|---|---|---|
| NFR-PERF-01 | Keystroke → frame-ready (in-process) | p50 ≤ 4 ms, p99 ≤ 10 ms | bench harness: write byte to fake PTY echo, time until renderer reports frame ready |
| NFR-PERF-02 | Parse throughput | ≥ 400 MB/s ASCII; ≥ 120 MB/s SGR-heavy | `criterion` bench on `ferrum-vt` over fixtures, single core |
| NFR-PERF-03 | Full-screen repaint 200×60 | ≤ 2.5 ms CPU | `criterion` bench on `ferrum-render` |
| NFR-PERF-04 | 20 sessions, one flooding (`yes`) | other sessions p99 ≤ 10 ms; UI ≥ 60 fps | scripted integration bench |
| NFR-PERF-05 | Cold start to first frame | ≤ 120 ms warm daemon / ≤ 250 ms cold | harness timestamps |
| NFR-PERF-06 | Idle CPU | 0.0 % (no wakeups) | `perf stat`/`powermetrics`-style check; assert no timers fire when idle |
| — | Scrollback search over 10 000 lines | ≤ 15 ms | bench |
| — | Reflow of 10 000 lines on resize | ≤ 40 ms, interruptible, UI keeps painting | bench |
| — | Scroll with wheel through 10 000 lines | every frame ≤ 4 ms | bench |

Relative reference: where an absolute target is hard to justify, the harness also reports the ratio against `alacritty` built from source on the same machine for `cat` of a 100 MB file; the goal is ≤ 1.5× Alacritty's wall time. This ratio is informational, not a gate.

**Honesty note:** end-to-end display latency (key press to photons) depends on compositor and monitor and cannot be gated in CI. It is measured manually with a Typometer-style tool before each minor release and recorded in the release notes.

## 5. Window-surface memory (the HiDPI problem)

Pixel buffers cost `width × height × 4` bytes per buffer:

| Window (physical px) | Per buffer | 2 buffers |
|---|---|---|
| 1440×900 | 5.2 MB | 10.4 MB |
| 1920×1080 | 8.3 MB | 16.6 MB |
| 2880×1800 (1440×900 @2x) | 20.7 MB | 41.5 MB |
| 3840×2160 | 33.2 MB | 66.4 MB |

Mitigations, in order:
1. Render **directly into the OS-provided buffer** (no separate pixmap).
2. Use damage-limited presentation (`present_with_damage`) so only dirty rects are written.
3. Prefer single-buffer paths where the platform allows it; where the platform double-buffers, nothing can be done and the gate accounts for it (`AC-MEM-04`).
4. Release/shrink the surface when the window is minimized or fully occluded.
5. Optional setting "Compact rendering" (off by default): render at reduced resolution and scale up; documented as a trade-off for 4K displays.

This is the one place where the "< 50 MB" promise can be physically impossible for very large windows, regardless of code quality. The docs and release notes must say so plainly.

## 6. How the numbers are enforced

| Check | Tool | Where |
|---|---|---|
| Combined PSS in the reference scenario | `cargo xtask memcheck` | CI on 3 OS (Linux gated automatically; macOS/Windows gated with their own metric) |
| 60-minute soak, random output + resizes + attach/detach | `cargo xtask soak` | nightly CI, fail if combined growth > 1 MB/h after warm-up |
| Per-frame/per-parse allocation count | `dhat` heap tests in `ferrum-vt` and `ferrum-render` | unit tests: steady-state alloc count must be 0 |
| Binary size | `cargo bloat`-style report in CI | informational, fail if > +5 % vs `main` without label `size-ok` |
| Throughput/latency regression | `criterion` + baseline file in `benches/baselines/` | fail if > +10 % slower than baseline (same runner class) |

CI timing numbers are noisy on shared runners: gates for latency use a *relative* comparison against the previous `main` commit run on the same job, plus loose absolute ceilings; memory gates are absolute because memory is deterministic.

## 7. Allocator decision (spike S2)

Candidates: system allocator (glibc `malloc` with `mallopt(M_ARENA_MAX=1)` on Linux, system allocators on macOS/Windows), `mimalloc` (0.1.52), `tikv-jemallocator` (0.7.0). Measure combined PSS in the reference scenario and parse throughput. Choose the lowest steady-state memory with throughput within 10 % of the best. Record the result in an ADR. Allocator choice is made once, in the composition roots (`ferrum-app`, `ferrumd`), never in libraries.
