# 0025. Render pipeline: spike S3 measurements

- Status: accepted (maintainer decisions of 2026-11-21 on the findings are recorded as Q-19, Q-20 and the Q-06 extension in `docs/14` §2; no S3 question stays open)
- Date: 2026-11-22
- Spike: S3 (`docs/spec/docs/12-build-order.md`), code in `spikes/s3-render-pipeline/`
- Refs: NFR-PERF-03, AC-MEM-01, AC-MEM-04, R-01, R-08, Q-06, ADR 0006, ADR 0015, ADR 0019

## Context

ADR 0006 chose CPU rendering through `softbuffer` with a purpose-built text blitter, and `tiny-skia`
for chrome only. `docs/03` gives the UI a 26 MB allowance and NFR-PERF-03 asks for a full-screen
repaint of 200×60 cells in at most 2.5 ms of CPU, but every UI figure was an estimate marked
"(to validate)". Q-06 chose native dialogs through `rfd` and said to load the dialog code lazily if
S3 shows an unacceptable resident cost. S3 produces the numbers and the two decisions.

## Method

One standalone binary (`spikes/s3-render-pipeline`, own `[workspace]`, own `Cargo.lock`, excluded from
the product workspace, product lint tables with `unsafe_code = "forbid"`, clippy clean). Versions
verified live on crates.io on 2026-11-21: `winit` 0.30.13 (0.31 is not a stable release yet),
`softbuffer` 0.4.8, `swash` 0.2.10, `fontdb` 0.24.0, `tiny-skia` 0.12.0 (optional feature), `rfd`
0.18.0 (optional feature); all are already in the `docs/07` tables, so the spike needs no
"Spike-only dependencies" row. Toolchain 1.100.0, release profile as the product (`opt-level = 3`, fat
LTO, one codegen unit, `strip = "symbols"`).

- **Window and surface.** `winit` window, `softbuffer` surface. The renderer writes cells directly
  into the `&mut [u32]` of `Surface::buffer_mut()` (0x00RRGGBB). There is no intermediate pixmap
  (`docs/02` §6). Windows are requested in *physical* pixels (`--physical`) so that the pixel count is
  exactly 1440×900 (1440×960 for the repaint run, so that 60 rows of 16 px fit) whatever the display
  scale. Every run reported `scale_factor=1`; no HiDPI display was available, so there is no
  separate "real scale" measurement. The 2880×1800 rows are the pixel-count equivalent of 1440×900
  at 2× with glyphs still at 12 px.
- **Font.** JetBrains Mono 2.304 variable (`JetBrainsMono[wght].ttf`, SIL OFL 1.1, 303 144 bytes,
  sha256 `662a196d58f1183bf2d77428b6d5283fe3f45161ab021bea4036bc98e5cac016`), downloaded from
  `download.jetbrains.com` at run time and not committed. It is loaded through `fontdb` (memory
  mapped) and rasterized by `swash` at 12 px, weight 400, hinting on, grayscale 8-bit coverage.
  Cell 7×16 px, so 200×55 cells are 1400×880 px and 200×60 cells are 1400×960 px.
- **Glyph cache.** Keyed by (glyph id, size), values are 8-bit coverage masks in one arena. The spike
  cache is unbounded (no LRU) and is warmed with the 94 printable ASCII glyphs before the surface is
  created. The text blitter fills backgrounds per run of equal colour and blends coverage over them.
  It is scalar code, not tuned or SIMD.
- **Grid content.** Deterministic xorshift generator. *Typical*: every row shows 20–119 visible
  columns (average about 70, as in the `docs/03` §1 fixture mix), 35 % of rows coloured in runs of 12
  columns, 3 % of cells with a highlighted background. *Dense*: all 200 columns of every row hold a
  glyph (the worst case: 12 000 glyphs per frame).
- **Repaint timings** (`--mode repaint`): 30 warm-up and 300 measured iterations, wall time of one
  thread around the draw call (`Instant`), reported as min / median / p95 / max in microseconds.
  "CPU time" is the wall time of a busy single thread. The run is repeated in three separate
  processes. Damage case: 4 rows (10..14) repainted, then `present_with_damage` with that rectangle.
- **Memory** (`--mode mem`): phases 0 process start, 1 window created, 2 font and glyph cache loaded,
  3 after four full presents, 4 after 5 s of idle (`ControlFlow::WaitUntil`, no redraw, no
  wake-ups). Four presents are needed because a double-buffered backend only allocates its second
  buffer on the second present. The first frame touches every pixel of the buffer, like a real
  window. Metric per `docs/03` §1:
  - Linux: **PSS** read by the process itself from `/proc/self/smaps_rollup`.
  - Windows: **Private Working Set** from the performance counter `\Process V2(<exe>:<pid>)\Working Set - Private`,
    sampled by `measure-windows.ps1` while the app waits for an acknowledgement on stdin (the spike
    has no `unsafe`, so it cannot call psapi).
  - macOS: **`phys_footprint`** from `footprint -p <pid> --format bytes`, sampled by `measure-macos.sh`.
  All memory values below are MiB (1 MiB = 1 048 576 bytes) unless stated.
- **Folder dialog** (`--dialog`, Windows): `rfd::FileDialog::pick_folder()` on its own thread; the sampler
  posts `WM_CLOSE` to the dialog windows of the process (the equivalent of Cancel), so no human click
  was needed. Samples: open (two points), closed +2 s and +6 s.
- **Hosts.** Windows: the maintainer's Windows 11 machine (build 26300, Intel Core i5-9400, 2.9 GHz),
  real display, physical session; other agent work (spikes S1, S4) ran on the same machine at times,
  so its timing carries more noise than a quiet machine. Linux: GitHub `ubuntu-latest` (4 vCPUs),
  X11 under Xvfb (`xvfb-run`, screen 3000×2000×24) and Wayland under `weston` 14.0.2
  `--backend=headless`; `backend=Xlib` and `backend=Wayland` are printed by the harness from the
  window handle. macOS: GitHub `macos-latest` (`Apple M1 (Virtual)`, 3 vCPUs in the run used, `backend=AppKit`).
  Runners are shared virtual machines.

### Reproduction

```bash
# Windows (Git Bash), macOS, Linux with a display (Xvfb or a Wayland compositor).
curl -fsSL -o jb.zip https://download.jetbrains.com/fonts/JetBrainsMono-2.304.zip
unzip -q jb.zip 'fonts/variable/JetBrainsMono[[]wght].ttf' && mv 'fonts/variable/JetBrainsMono[wght].ttf' JBM-var.ttf
echo "662a196d58f1183bf2d77428b6d5283fe3f45161ab021bea4036bc98e5cac016  JBM-var.ttf" | sha256sum -c -

cd spikes/s3-render-pipeline
bash run.sh "$PWD/../../JBM-var.ttf" 3                # builds 4 variants, 3 runs of every measurement
RUN_DIALOG=1 bash run.sh "$PWD/../../JBM-var.ttf" 3   # Windows: also opens and closes the folder dialog
# Linux X11:     xvfb-run -a -s "-screen 0 3000x2000x24" bash run.sh <font> 3
# Linux Wayland: weston --backend=headless --socket=wl-s3 --width=3000 --height=2000 &
#                WAYLAND_DISPLAY=wl-s3 DISPLAY= bash run.sh <font> 3
# Ubuntu build packages: pkg-config libx11-dev libxcursor-dev libxrandr-dev libxi-dev
#                        libxkbcommon-dev libxkbcommon-x11-dev libx11-xcb-dev libxcb1-dev
```

`run.sh` prints `RESULT ...` lines; the tables below are those lines condensed. The Windows block
is from a local run; the Linux and macOS blocks are from the CI run listed under "Linux and macOS
runs".

## Results

Variants: `base` (window, surface, glyphs), `dialog` (`rfd` linked, dialog opened only with `--dialog`),
`skia` (`tiny-skia` linked and 28 chrome tiles built and blitted), `both`.

### Binary size (release, stripped, bytes)

| OS | base | + `tiny-skia` | + `rfd` | both |
|---|---|---|---|---|
| Windows x86_64 | 1 486 336 | 1 768 448 (+282 112, +19.0 %) | 1 495 040 (+8 704, +0.6 %) | 1 776 640 |
| Linux x86_64 (xdg-portal + Wayland features of `rfd`) | 3 192 424 | 3 464 728 (+272 304, +8.5 %) | 3 301 224 (+108 800, +3.4 %) | 3 573 464 |
| macOS arm64 | 1 320 992 | 1 552 816 (+231 824, +17.5 %) | 1 337 984 (+16 992, +1.3 %) | 1 586 320 |

The Linux binary is larger because `winit` and `softbuffer` carry both the X11 and the Wayland
backends (`wayland-dlopen`). These are sizes of a spike without any product code: the deltas, not the
totals, are the useful figures.

### Summary against the budget (idle, 1440×900 physical, base variant, three runs each)

| OS (metric) | idle total | surface part | note |
|---|---|---|---|
| Windows (Private Working Set) | 7.23 / 7.23 / 7.23 | 5.1 (one buffer, 4.94 MiB) | `softbuffer` uses one DIB section |
| Linux X11 (PSS) | 8.05 / 8.11 / 7.93 | 2.6 PSS (4.94 mapped) | the shared-memory segment is also mapped by the X server, so PSS charges half |
| Linux Wayland (PSS) | 11.00 / 8.49 / 8.50 | 5.0–7.5 PSS (9.9 mapped) | two `wl_shm` buffers after the second present; the PSS share depends on how weston maps them |
| macOS (`phys_footprint`) | 24.42 / 24.52 / 24.49 | window clamped to 1024×642 | see Q-20 in the findings; not comparable with the 1440×900 rows |

Per-run phase values are in the next sections.

### Windows 11 (local, Intel i5-9400, real display, `backend=Win32`)

Memory (Private Working Set, MiB; the three values are runs 1 / 2 / 3):

| scenario (window physical px) | 1 window created | 2 font + glyph cache | 3 after 4 presents | 4 idle 5 s | 5 dialog open | 6 dialog open | 7 closed +2 s | 8 closed +6 s |
|---|---|---|---|---|---|---|---|---|
| 1440x900 base | 1.79 / 1.80 / 1.80 | 2.12 / 2.13 / 2.14 | 7.23 / 7.23 / 7.23 | 7.23 / 7.23 / 7.23 |  |  |  |  |
| 1440x900 dialog | 1.80 / 1.80 / 1.79 | 2.13 / 2.13 / 2.11 | 7.23 / 7.23 / 7.09 | 7.23 / 7.23 / 7.09 |  |  |  |  |
| 1440x900 skia | 1.80 / 1.80 / 1.81 | 2.13 / 2.13 / 2.13 | 8.08 / 8.08 / 8.07 | 8.08 / 8.08 / 8.07 |  |  |  |  |
| 1440x900 both | 1.79 / 1.79 / 1.83 | 2.12 / 2.12 / 2.15 | 8.07 / 7.95 / 7.97 | 8.07 / 7.95 / 7.97 |  |  |  |  |
| 2880x1800 base | 1.78 / 1.79 / 1.81 | 2.11 / 2.11 / 2.14 | 22.04 / 21.92 / 21.94 | 22.04 / 21.92 / 21.94 |  |  |  |  |
| 1440x900 dialog + dialog opened | 1.81 / 1.80 / 1.79 | 2.14 / 2.12 / 2.11 | 7.11 / 7.09 / 7.09 | 7.11 / 7.09 / 7.09 | 13.43 / 13.30 / 13.52 | 13.43 / 13.30 / 13.52 | 12.16 / 12.05 / 12.23 | 12.16 / 12.05 / 12.23 |

Phases 5 to 8 belong to the run with the folder dialog opened and closed (`dialog_returned picked=false`, `windows_closed=2` in every run).

Binary size (bytes, stripped release)

| variant | bytes |
|---|---|
| base | 1486336 |
| skia | 1768448 |
| dialog | 1495040 |
| both | 1776640 |

CPU time per operation (microseconds, 300 iterations after 30 warm-up)

| run | measure | cells | min | median | p95 | max |
|---|---|---|---|---|---|---|
| 1 | typical | 200x60 | 955.3 | 1102.5 | 1504.3 | 9060.7 |
| 1 | typical:acquire | full | 0.0 | 0.0 | 0.1 | 0.1 |
| 1 | typical:present_full | full | 493.2 | 715.6 | 1116.9 | 3621.8 |
| 1 | dense | 200x60 | 2516.3 | 2686.5 | 5116.6 | 17076.2 |
| 1 | damage:acquire | 4 rows | 0.0 | 0.0 | 0.1 | 0.1 |
| 1 | damage:draw_4_rows | 4 rows | 60.0 | 76.1 | 111.5 | 1903.1 |
| 1 | damage:present_with_damage | 4 rows | 32.9 | 37.3 | 85.3 | 1145.8 |
| 2 | typical | 200x60 | 940.6 | 1148.8 | 1810.9 | 9179.5 |
| 2 | typical:acquire | full | 0.0 | 0.0 | 0.1 | 0.5 |
| 2 | typical:present_full | full | 477.0 | 742.8 | 1549.9 | 10755.5 |
| 2 | dense | 200x60 | 2488.6 | 2877.9 | 5758.2 | 17497.1 |
| 2 | damage:acquire | 4 rows | 0.0 | 0.0 | 0.1 | 0.1 |
| 2 | damage:draw_4_rows | 4 rows | 60.1 | 61.8 | 86.2 | 807.4 |
| 2 | damage:present_with_damage | 4 rows | 32.7 | 36.6 | 65.0 | 920.7 |
| 3 | typical | 200x60 | 919.0 | 982.9 | 1395.3 | 8821.8 |
| 3 | typical:acquire | full | 0.0 | 0.0 | 0.1 | 0.1 |
| 3 | typical:present_full | full | 390.3 | 612.9 | 1071.0 | 8404.1 |
| 3 | dense | 200x60 | 2511.8 | 2644.4 | 5300.5 | 19653.8 |
| 3 | damage:acquire | 4 rows | 0.0 | 0.0 | 0.1 | 0.1 |
| 3 | damage:draw_4_rows | 4 rows | 59.9 | 60.6 | 67.2 | 136.6 |
| 3 | damage:present_with_damage | 4 rows | 32.7 | 34.6 | 43.4 | 693.8 |

tiny-skia primitive cost (microseconds, 2000 iterations)

| run | op | tile | min | median | p95 | max |
|---|---|---|---|---|---|---|
| 1 | rrect_fill | 240x32 | 8.4 | 8.7 | 10.2 | 124.8 |
| 1 | rrect_stroke | 240x32 | 10.9 | 11.1 | 12.2 | 116.9 |
| 1 | rrect_gradient | 240x32 | 48.8 | 49.2 | 55.9 | 3167.2 |
| 1 | shadow_tile | 96x96 | 472.6 | 477.7 | 552.7 | 3695.0 |
| 2 | rrect_fill | 240x32 | 8.2 | 8.4 | 8.7 | 67.5 |
| 2 | rrect_stroke | 240x32 | 10.9 | 11.1 | 11.9 | 32.4 |
| 2 | rrect_gradient | 240x32 | 48.9 | 50.3 | 53.4 | 1935.9 |
| 2 | shadow_tile | 96x96 | 473.8 | 478.2 | 522.0 | 5799.6 |
| 3 | rrect_fill | 240x32 | 8.2 | 8.5 | 10.1 | 1479.6 |
| 3 | rrect_stroke | 240x32 | 10.9 | 11.1 | 11.9 | 913.0 |
| 3 | rrect_gradient | 240x32 | 48.9 | 49.2 | 51.7 | 1938.6 |
| 3 | shadow_tile | 96x96 | 472.4 | 476.9 | 519.8 | 8214.1 |

### Linux X11 (GitHub `ubuntu-latest`, Xvfb, `backend=Xlib`)

Memory (PSS, MiB; runs 1 / 2 / 3):

| scenario (window physical px) | 1 window created | 2 font + glyph cache | 3 after 4 presents | 4 idle 5 s |
|---|---|---|---|---|
| 1440x900 base | 4.61 / 4.71 / 4.59 | 5.45 / 5.52 / 5.33 | 8.05 / 8.11 / 7.93 | 8.05 / 8.11 / 7.93 |
| 1440x900 dialog | 4.73 / 4.64 / 4.72 | 5.54 / 5.50 / 5.59 | 8.13 / 8.10 / 8.19 | 8.13 / 8.10 / 8.19 |
| 1440x900 skia | 4.80 / 4.84 / 4.71 | 5.54 / 5.57 / 5.57 | 9.17 / 9.15 / 9.21 | 9.17 / 9.15 / 9.21 |
| 1440x900 both | 4.78 / 4.83 / 4.76 | 5.70 / 5.69 / 5.68 | 9.34 / 9.33 / 9.33 | 9.34 / 9.33 / 9.33 |
| 2880x1800 base | 4.76 / 4.75 / 4.65 | 5.50 / 5.50 / 5.45 | 15.51 / 15.51 / 15.47 | 15.51 / 15.51 / 15.47 |

Binary size (bytes, stripped release)

| variant | bytes |
|---|---|
| base | 3192424 |
| skia | 3464728 |
| dialog | 3301224 |
| both | 3573464 |

CPU time per operation (microseconds, 300 iterations after 30 warm-up)

| run | measure | cells | min | median | p95 | max |
|---|---|---|---|---|---|---|
| 1 | typical | 200x60 | 642.6 | 665.9 | 679.4 | 693.3 |
| 1 | typical:acquire | full | 409.8 | 441.3 | 686.8 | 1058.2 |
| 1 | typical:present_full | full | 0.9 | 1.9 | 2.1 | 22.1 |
| 1 | dense | 200x60 | 1576.0 | 1613.5 | 1728.6 | 1758.4 |
| 1 | damage:acquire | 4 rows | 41.3 | 113.1 | 120.3 | 294.5 |
| 1 | damage:draw_4_rows | 4 rows | 39.7 | 70.6 | 74.9 | 80.9 |
| 1 | damage:present_with_damage | 4 rows | 0.2 | 0.3 | 0.5 | 1.1 |
| 2 | typical | 200x60 | 641.1 | 663.9 | 679.9 | 772.2 |
| 2 | typical:acquire | full | 400.7 | 636.7 | 769.4 | 1144.8 |
| 2 | typical:present_full | full | 0.9 | 1.7 | 2.6 | 10.9 |
| 2 | dense | 200x60 | 1580.5 | 1700.6 | 1749.4 | 1799.7 |
| 2 | damage:acquire | 4 rows | 13.9 | 14.2 | 17.2 | 29.2 |
| 2 | damage:draw_4_rows | 4 rows | 34.1 | 34.3 | 38.3 | 48.1 |
| 2 | damage:present_with_damage | 4 rows | 0.2 | 0.3 | 0.5 | 0.6 |
| 3 | typical | 200x60 | 637.8 | 661.9 | 677.4 | 753.5 |
| 3 | typical:acquire | full | 440.8 | 660.2 | 818.1 | 963.0 |
| 3 | typical:present_full | full | 0.9 | 1.5 | 2.5 | 9.0 |
| 3 | dense | 200x60 | 1574.5 | 1599.0 | 1680.1 | 1798.3 |
| 3 | damage:acquire | 4 rows | 55.0 | 112.9 | 118.3 | 124.8 |
| 3 | damage:draw_4_rows | 4 rows | 56.9 | 70.5 | 77.9 | 83.1 |
| 3 | damage:present_with_damage | 4 rows | 0.2 | 0.2 | 0.5 | 0.6 |

tiny-skia primitive cost (microseconds, 2000 iterations)

| run | op | tile | min | median | p95 | max |
|---|---|---|---|---|---|---|
| 1 | rrect_fill | 240x32 | 4.3 | 4.6 | 11.5 | 46.6 |
| 1 | rrect_stroke | 240x32 | 6.5 | 6.8 | 8.8 | 21.8 |
| 1 | rrect_gradient | 240x32 | 37.3 | 37.8 | 42.2 | 73.4 |
| 1 | shadow_tile | 96x96 | 287.7 | 297.5 | 308.4 | 380.3 |
| 2 | rrect_fill | 240x32 | 4.3 | 4.5 | 4.7 | 22.9 |
| 2 | rrect_stroke | 240x32 | 6.5 | 6.7 | 6.9 | 22.1 |
| 2 | rrect_gradient | 240x32 | 37.2 | 37.7 | 42.2 | 51.2 |
| 2 | shadow_tile | 96x96 | 288.1 | 297.1 | 307.1 | 502.0 |
| 3 | rrect_fill | 240x32 | 4.3 | 4.5 | 4.7 | 22.3 |
| 3 | rrect_stroke | 240x32 | 6.5 | 6.7 | 6.9 | 25.8 |
| 3 | rrect_gradient | 240x32 | 37.3 | 38.1 | 43.2 | 57.5 |
| 3 | shadow_tile | 96x96 | 288.0 | 297.0 | 307.0 | 473.2 |

### Linux Wayland (GitHub `ubuntu-latest`, weston 14.0.2 headless, `backend=Wayland`)

Memory (PSS, MiB; runs 1 / 2 / 3):

| scenario (window physical px) | 1 window created | 2 font + glyph cache | 3 after 4 presents | 4 idle 5 s |
|---|---|---|---|---|
| 1440x900 base | 2.70 / 2.70 / 2.63 | 3.53 / 3.50 / 3.51 | 13.51 / 10.96 / 10.97 | 11.00 / 8.49 / 8.50 |
| 1440x900 dialog | 2.66 / 2.67 / 2.47 | 3.55 / 3.56 / 3.36 | 11.01 / 11.02 / 10.82 | 8.54 / 8.55 / 8.35 |
| 1440x900 skia | 2.77 / 2.62 / 2.55 | 3.54 / 3.54 / 3.51 | 11.97 / 11.97 / 12.07 | 9.50 / 9.50 / 9.60 |
| 1440x900 both | 2.76 / 2.76 / 2.73 | 3.65 / 3.72 / 3.69 | 12.15 / 12.22 / 12.12 | 9.68 / 9.75 / 9.65 |
| 2880x1800 base | 2.70 / 2.68 / 2.58 | 3.47 / 3.53 / 3.35 | 43.18 / 43.24 / 43.12 | 33.22 / 33.27 / 33.16 |

Binary size (bytes, stripped release)

| variant | bytes |
|---|---|
| base | 3192424 |
| skia | 3464728 |
| dialog | 3301224 |
| both | 3573464 |

CPU time per operation (microseconds, 300 iterations after 30 warm-up)

| run | measure | cells | min | median | p95 | max |
|---|---|---|---|---|---|---|
| 1 | typical | 200x60 | 636.6 | 661.9 | 674.5 | 687.8 |
| 1 | typical:acquire | full | 3.6 | 34.4 | 53.8 | 182.0 |
| 1 | typical:present_full | full | 3.4 | 7.4 | 7.9 | 29.9 |
| 1 | dense | 200x60 | 1579.6 | 1604.7 | 1620.1 | 1636.0 |
| 1 | damage:acquire | 4 rows | 7.7 | 28.4 | 30.3 | 348.2 |
| 1 | damage:draw_4_rows | 4 rows | 33.8 | 34.5 | 44.8 | 51.7 |
| 1 | damage:present_with_damage | 4 rows | 1.9 | 2.8 | 3.2 | 4.7 |
| 2 | typical | 200x60 | 638.4 | 662.8 | 676.6 | 684.3 |
| 2 | typical:acquire | full | 3.8 | 27.7 | 29.6 | 222.7 |
| 2 | typical:present_full | full | 4.1 | 8.2 | 8.9 | 33.2 |
| 2 | dense | 200x60 | 1576.2 | 1602.2 | 1646.7 | 1921.1 |
| 2 | damage:acquire | 4 rows | 16.1 | 28.8 | 35.6 | 237.0 |
| 2 | damage:draw_4_rows | 4 rows | 33.8 | 34.4 | 42.1 | 65.6 |
| 2 | damage:present_with_damage | 4 rows | 1.6 | 2.8 | 3.2 | 11.5 |
| 3 | typical | 200x60 | 638.3 | 662.2 | 675.4 | 730.8 |
| 3 | typical:acquire | full | 4.4 | 41.2 | 197.7 | 298.2 |
| 3 | typical:present_full | full | 3.3 | 7.7 | 8.4 | 17.8 |
| 3 | dense | 200x60 | 1575.0 | 1599.0 | 1614.5 | 1685.9 |
| 3 | damage:acquire | 4 rows | 8.1 | 28.6 | 33.9 | 42.3 |
| 3 | damage:draw_4_rows | 4 rows | 33.9 | 34.5 | 44.0 | 59.6 |
| 3 | damage:present_with_damage | 4 rows | 1.6 | 2.9 | 3.3 | 11.8 |

tiny-skia primitive cost (microseconds, 2000 iterations)

| run | op | tile | min | median | p95 | max |
|---|---|---|---|---|---|---|
| 1 | rrect_fill | 240x32 | 4.3 | 4.5 | 4.6 | 38.7 |
| 1 | rrect_stroke | 240x32 | 6.5 | 6.7 | 6.8 | 17.8 |
| 1 | rrect_gradient | 240x32 | 37.3 | 37.6 | 42.2 | 51.7 |
| 1 | shadow_tile | 96x96 | 287.9 | 291.0 | 302.5 | 433.0 |
| 2 | rrect_fill | 240x32 | 4.3 | 4.5 | 4.6 | 29.0 |
| 2 | rrect_stroke | 240x32 | 6.5 | 6.7 | 6.8 | 20.6 |
| 2 | rrect_gradient | 240x32 | 37.3 | 37.7 | 42.0 | 49.9 |
| 2 | shadow_tile | 96x96 | 289.1 | 292.3 | 302.6 | 321.4 |
| 3 | rrect_fill | 240x32 | 4.3 | 4.5 | 4.7 | 31.7 |
| 3 | rrect_stroke | 240x32 | 6.5 | 6.6 | 6.8 | 24.4 |
| 3 | rrect_gradient | 240x32 | 37.2 | 37.7 | 42.4 | 117.9 |
| 3 | shadow_tile | 96x96 | 287.9 | 292.4 | 301.5 | 440.4 |

### macOS arm64 (GitHub `macos-latest`, `backend=AppKit`; window clamped to 1024×642)

macOS limited every requested window (1440×900, 1440×960 and 2880×1800) to 1024×642 pixels (`inner_px=1024x642`, 2 629 632 bytes per buffer), so the repaint rows draw only 146×40 of the 200×60 cells (`drawn_cells=146x40`, 5 840 of 12 000 cells) and the memory rows are not 1440×900 figures. The 2880×1800 row is therefore the same window as the 1440×900 rows. No 200×60 figure is derived from them.

Memory (`phys_footprint`, MiB; runs 1 / 2 / 3):

| scenario (window physical px) | 1 window created | 2 font + glyph cache | 3 after 4 presents | 4 idle 5 s |
|---|---|---|---|---|
| 1440x900 base | 10.61 / 10.64 / 10.66 | 10.89 / 10.91 / 10.92 | 16.52 / 16.47 / 16.47 | 24.42 / 24.52 / 24.49 |
| 1440x900 dialog | 10.74 / 10.66 / 10.72 | 11.00 / 10.97 / 11.02 | 16.56 / 16.55 / 16.60 | 24.55 / 24.64 / 24.42 |
| 1440x900 skia | 10.56 / 10.61 / 10.60 | 10.85 / 10.89 / 10.89 | 17.38 / 17.36 / 17.42 | 25.41 / 25.16 / 25.24 |
| 1440x900 both | 10.58 / 10.66 / 10.61 | 10.86 / 10.96 / 10.88 | 17.36 / 17.44 / 17.39 | 25.17 / 25.08 / 25.47 |
| 2880x1800 base | 10.63 / 10.64 / 10.64 | 10.91 / 10.92 / 10.94 | 16.49 / 16.50 / 16.52 | 24.52 / 24.49 / 24.44 |

Binary size (bytes, stripped release)

| variant | bytes |
|---|---|
| base | 1320992 |
| skia | 1552816 |
| dialog | 1337984 |
| both | 1586320 |

CPU time per operation (microseconds, 300 iterations after 30 warm-up)

| run | measure | cells | min | median | p95 | max |
|---|---|---|---|---|---|---|
| 1 | typical | 146x40 | 376.5 | 568.4 | 1294.5 | 1730.3 |
| 1 | typical:acquire | full | 16.9 | 25.8 | 68.2 | 370.0 |
| 1 | typical:present_full | full | 1.1 | 6.3 | 42.6 | 337.1 |
| 1 | dense | 146x40 | 758.7 | 825.7 | 1265.2 | 3562.4 |
| 1 | damage:acquire | 4 rows | 21.0 | 25.5 | 45.6 | 93.6 |
| 1 | damage:draw_4_rows | 4 rows | 28.7 | 30.0 | 97.8 | 249.6 |
| 1 | damage:present_with_damage | 4 rows | 0.9 | 1.0 | 10.1 | 39.3 |
| 2 | typical | 146x40 | 373.3 | 433.0 | 806.1 | 1378.0 |
| 2 | typical:acquire | full | 15.2 | 21.5 | 47.2 | 124.3 |
| 2 | typical:present_full | full | 0.9 | 4.8 | 24.9 | 106.8 |
| 2 | dense | 146x40 | 749.1 | 817.2 | 1062.3 | 1479.2 |
| 2 | damage:acquire | 4 rows | 17.2 | 20.9 | 26.2 | 80.8 |
| 2 | damage:draw_4_rows | 4 rows | 28.7 | 29.2 | 34.7 | 50.4 |
| 2 | damage:present_with_damage | 4 rows | 0.9 | 0.9 | 2.2 | 18.7 |
| 3 | typical | 146x40 | 374.5 | 389.5 | 456.7 | 582.8 |
| 3 | typical:acquire | full | 16.1 | 17.3 | 26.2 | 97.9 |
| 3 | typical:present_full | full | 1.0 | 1.3 | 17.5 | 67.0 |
| 3 | dense | 146x40 | 754.8 | 792.1 | 1525.5 | 3589.0 |
| 3 | damage:acquire | 4 rows | 18.8 | 22.8 | 28.4 | 55.3 |
| 3 | damage:draw_4_rows | 4 rows | 28.8 | 29.0 | 35.8 | 77.3 |
| 3 | damage:present_with_damage | 4 rows | 0.9 | 0.9 | 1.2 | 34.4 |

tiny-skia primitive cost (microseconds, 2000 iterations)

| run | op | tile | min | median | p95 | max |
|---|---|---|---|---|---|---|
| 1 | rrect_fill | 240x32 | 6.0 | 7.1 | 18.2 | 39418.9 |
| 1 | rrect_stroke | 240x32 | 11.1 | 13.3 | 36.8 | 4290.2 |
| 1 | rrect_gradient | 240x32 | 30.0 | 35.2 | 99.0 | 511.2 |
| 1 | shadow_tile | 96x96 | 330.4 | 387.7 | 849.5 | 13788.5 |
| 2 | rrect_fill | 240x32 | 5.9 | 6.5 | 16.1 | 242.1 |
| 2 | rrect_stroke | 240x32 | 11.0 | 11.5 | 29.5 | 2083.6 |
| 2 | rrect_gradient | 240x32 | 30.0 | 30.8 | 79.0 | 168.0 |
| 2 | shadow_tile | 96x96 | 329.4 | 344.2 | 519.6 | 993.0 |
| 3 | rrect_fill | 240x32 | 5.9 | 6.1 | 7.9 | 28.9 |
| 3 | rrect_stroke | 240x32 | 11.1 | 11.3 | 14.0 | 59.6 |
| 3 | rrect_gradient | 240x32 | 30.0 | 30.5 | 38.1 | 376.7 |
| 3 | shadow_tile | 96x96 | 329.3 | 333.9 | 395.5 | 954.2 |

Row names in the CPU tables: `typical` and `dense` are the full-repaint CPU time into the real buffer
(acquired, not presented); `typical:acquire` and `typical:present_full` are the other two parts of a
full typical frame (acquire the buffer, repaint, `present()`); `damage:*` are the three parts of the
4-row frame (`draw_4_rows` is the repaint, `present_with_damage` the presentation with a 1400×64
rectangle). `cells` is the number of cells drawn (200×60 unless the display clamped the window).

## Findings

### NFR-PERF-03 (full repaint of 200×60 cells, target at most 2.5 ms)

Median of the three runs (range of the three medians), unoptimized scalar blitter:

| Content | Windows i5-9400 | Linux X11 (CI) | Linux Wayland (CI) | macOS (CI, 146×40 cells only) |
|---|---|---|---|---|
| typical mix | 0.98–1.15 ms | 0.66 ms | 0.66 ms | 0.39–0.57 ms |
| dense text (12 000 glyphs) | **2.64–2.88 ms** | 1.60–1.70 ms | 1.60 ms | 0.79–0.83 ms |

- Typical content is within the target on every OS. **Dense text on the Windows reference machine is
  over 2.5 ms** (2.64–2.88 ms median, p95 5.1–5.8 ms), reported as is. The blitter is scalar and
  untuned. Maintainer decision (Q-19, 2026-11-21): the target stays at 2.5 ms and is defined on the
  strictest reading, a full 200×60 screen of dense text; WP-07 must meet it with the SIMD blitter
  on the maintainer's machine (i5-9400, Windows) and in CI, and goes back to the maintainer with
  numbers if it does not.
- The macOS rows draw 5 840 of 12 000 cells because the runner display clamped the window; no
  200×60 figure is derived from them.

### Damage-limited presentation

| | Windows | X11 (Xvfb) | Wayland (weston) | macOS |
|---|---|---|---|---|
| `buffer_age` over the first four presents | 0, 1, 1, 1 | 0, 1, 1, 1 | 0, 0, 2, 2 (double buffer) | **0, 0, 0, 0** |
| 4-row frame: repaint + `present_with_damage` | 61–76 + 35–37 µs | 34–71 + 0.3 µs (acquire 14–113 µs) | 34.5 + 2.8–2.9 µs (acquire 28–29 µs) | 29–30 + 0.9–1.0 µs (acquire 21–26 µs) |
| full typical frame: repaint, `present()` | 0.98–1.15 ms + 0.61–0.74 ms | 0.66 ms + 1.5–1.9 µs (acquire 0.44–0.66 ms) | 0.66 ms + 7.4–8.2 µs (acquire 28–41 µs) | 0.39–0.57 ms + 1.3–6.3 µs |
| `present_with_damage` of the full rectangle | 0.78–0.80 ms | 0.3–1.1 µs | 3.0–3.1 µs | 0.8 µs |

- On Windows the cost of presentation is proportional to the damaged rectangle (35–37 µs for 4 rows
  against 0.78–0.80 ms for the whole window): `present_with_damage` is honored and worth using.
- On X11 and Wayland the call itself is cheap (asynchronous); the cost moves to acquiring the next
  buffer, which after a full-window present took 0.44–0.66 ms on Xvfb.
- `softbuffer` reports `buffer_age` 1 on Windows and X11 (previous contents are kept), 2 on Wayland
  once both buffers exist, and **0 on macOS, i.e. the contents are undefined**. A damage-limited
  renderer must therefore repaint everything when the age is 0 (or 2 and the previous damage is not
  tracked). On macOS that is cheap in practice (0.39–0.57 ms for 5 840 typical cells) but it means the
  per-row hash plus dirty-rect scheme of `docs/02` §6 saves presentation work there, not drawing work.
- Idle wake-ups were not measured (NFR-PERF-06); the idle phase only checks memory with
  `ControlFlow::WaitUntil`.

### UI memory

- Idle total of the spike at 1440×900: Windows 7.23 in all three runs (Private Working Set), X11
  7.93–8.11 (PSS), Wayland 8.49–11.00 (PSS), macOS 24.42–24.52 (`phys_footprint`, at a clamped
  1024×642 window).
- Process base is already above its 4.5 MB allowance on macOS before any UI exists: 10.6–10.7 MiB
  right after window creation, from 1.6 MiB at process start. Maintainer decision (Q-20,
  2026-11-21): the allowance is unchanged; the maintainer runs the spike on a real Mac
  (`docs/manual-verification.md` MV-S3-1) before WP-08 and that number decides, returning to the
  maintainer if it is over. Linux X11 4.6–4.7, Wayland 2.6–2.7 and Windows 1.8 are below the
  allowance, but those are lower bounds (no UI toolkit, no chrome).
- The window surface is one buffer on Windows (5.1 MiB) and X11 (4.94 MiB mapped, 2.6 PSS because
  the X server maps the same shared-memory segment), and two buffers on Wayland (9.9 MiB mapped
  after the second present; PSS 5.0–7.5 at idle and 7.5–10.0 right after the presents, the share
  charged to the client depending on how weston maps them). The 2880×1800 equivalent costs 19.8–19.9
  on Windows, 10.0 PSS on X11 and 29.7–29.8 (idle) to 39.7–39.8 (right after the presents) on
  Wayland. This confirms R-01: surface bytes dominate at HiDPI and `AC-MEM-04` is the right gate.
- Glyph cache: 10 032 bytes for 94 ASCII glyphs at one size and weight. Font: 0.26–0.88 MiB resident
  for the 0.29 MiB variable font plus that cache. Both are far below their allowances, but the real
  working set (CJK, emoji, several weights and sizes) is not measured and the spike cache has no LRU.
- Real scale factor: 1.0 on every machine used, so no separate "real scale" figure exists; HiDPI is
  represented by the 2880×1800 pixel-count rows.

### tiny-skia (ADR 0006)

| | Windows | Linux CI | macOS CI |
|---|---|---|---|
| rounded rect fill, 240×32 tile | 8.4–8.7 µs | 4.5–4.6 µs | 6.1–7.1 µs |
| rounded rect stroke | 11.1 µs | 6.6–6.8 µs | 11.3–13.3 µs |
| rounded rect with gradient | 49.2–50.3 µs | 37.6–38.1 µs | 30.5–35.2 µs |
| shadow tile 96×96 (12 stacked rounded rects, no blur in `tiny-skia`) | 477–478 µs | 291–298 µs | 334–388 µs |
| binary size delta | +282 112 B (+19.0 %) | +272 304 B (+8.5 %) | +231 824 B (+17.5 %) |

Linking `tiny-skia` changed no resident figure by itself (phase 2 is equal with and without it on
every OS); the resident delta is the 28 tiles, 0.84 MiB of pixels (0.64–1.28 MiB measured on Windows,
X11 and macOS; Wayland PSS swings with the compositor and is not used for this). Chrome primitives
cost 4.5–51 µs and a shadow tile 0.29–0.50 ms, all paid once when a tile is built, never per frame.
The binary-size cost is 0.23–0.28 MB. An alternative (a hand-written rounded-rect and shadow
blitter) was not built, so this evaluates the absolute cost only, not a replacement.

### Native folder dialog (`rfd`)

| | Windows | Linux | macOS |
|---|---|---|---|
| binary size delta | +8 704 B (+0.6 %) | +108 800 B (+3.4 %) | +16 992 B (+1.3 %) |
| idle memory with the dialog code linked, never opened | 7.23 / 7.23 / 7.09 (base 7.23 / 7.23 / 7.23) | 8.13 / 8.10 / 8.19 (base 8.05 / 8.11 / 7.93) | 24.55 / 24.64 / 24.42 (base 24.42 / 24.52 / 24.49) |
| dialog open | 13.43 / 13.30 / 13.52 (+6.2 to +6.4) | not measured | not measured |
| 2 s after closing | 12.16 / 12.05 / 12.23 (+5.0 to +5.1) | not measured | not measured |
| 6 s after closing | 12.16 / 12.05 / 12.23 (+5.0 to +5.1) | not measured | not measured |

Linked but unused, `rfd` costs nothing measurable in memory on the three OS (at most +0.26 MiB,
inside the run-to-run spread). **Opening the dialog once on Windows leaves about 5 MiB of Private
Working Set in the process** (shell and COM libraries stay loaded; observed for 6 s after closing,
no longer). On Linux the default `rfd` backend (xdg-portal through `pollster`, plus Wayland) pulls
neither `tokio` nor `async-std` (`cargo tree -i tokio` prints nothing, `async-std` is not in the
graph); opening a portal dialog needs a desktop session and was not measured. A dialog on macOS
needs a human click on the main thread and was not attempted. The Windows dialog was closed with
`WM_CLOSE` by the external sampler (`dialog_close windows_closed=2`, `dialog_returned picked=false`),
not clicked by a person; a dialog with a selection could allocate differently.

## Decisions

1. **tiny-skia: keep it, for chrome tiles only (ADR 0006 stands).** Text and the terminal grid stay
   on the purpose-built blitter writing straight into the `softbuffer` buffer; `tiny-skia` draws
   rounded rectangles, borders, gradients and shadow tiles into small cached pixmaps that are
   composited into the same buffer. Its cost is 0.23–0.28 MB of binary, about 0.6–1.3 MiB resident for a
   generous tile set (inside the 1.5 allowance of "UI chrome tiles + widget state", of which widget
   state is not yet counted), and 4.5–500 µs per tile build. Revisit only if WP-08 shows the tile set
   or the binary delta threatens the UI allowance.
2. **ADR 0006 is validated as an architecture.** Direct writes into the `softbuffer` buffer work on
   Windows, X11, Wayland and macOS; typical-content repaint is 0.39–1.15 ms; damage presentation is
   honored where it matters (Windows) and cheap elsewhere. The two findings over their numbers were
   decided by the maintainer on 2026-11-21 and do not change the rendering design: dense text on
   Windows keeps the 2.5 ms target and WP-07 must meet it (Q-19); the macOS process-base allowance
   is unchanged and a real-Mac run decides (Q-20). The ADR index row of ADR 0006 keeps the status
   "accepted (validated by S3)".
3. **File dialog: keep native dialogs through `rfd`, but never in the UI process.** The resident cost
   of one dialog (about 5 MiB on Windows, kept after closing) is more than twice the 2 MB reserve
   of the UI allowance, so Q-06's lazy-loading rule applies. Loading the code lazily inside the UI
   process cannot unload the shell libraries, so the native dialog opens in a short-lived helper
   process: the UI re-executes its own binary (`ferrum --pick-folder`), the helper opens the `rfd`
   dialog, prints the chosen path and exits, and the memory returns to the OS, so no resident cost
   stays in the UI process. **Approved by the maintainer on 2026-11-21 (Q-06 extended in
   `docs/14` §2).** The in-app path text input plus drag and drop (`docs/07`) remain the fallback
   when the helper or the dialog is unavailable. The helper's own cost and the Linux and macOS
   dialogs were not measured; WP-09 measures them. `AC-MEM-01` counts `ferrum` and `ferrumd` in
   steady state, and the helper exists only while a dialog is open.

## Consequences (implementation requirements, not open questions)

- **WP-07 requirement (Q-19, maintainer decision 2026-11-21).** The full repaint of a 200×60 screen
  of dense text (every cell holds a glyph) must take at most 2.5 ms CPU, measured by the
  `ferrum-render` bench on the maintainer's machine (i5-9400, Windows) and in CI, with the SIMD
  blitter. If it does not, the result goes back to the maintainer with numbers. The scalar spike
  blitter measured 2.64–2.88 ms there, so the SIMD blitter is required, not optional.
- **WP-08 precondition (Q-20, maintainer decision 2026-11-21).** The macOS process-base allowance
  stays at 4.5 MB. Before WP-08 the maintainer runs the spike on a real Mac (MV-S3-1); that number
  decides, and if it is over the allowance it returns to the maintainer.
- **WP-09.** Implement the folder picker as the helper process described in decision 3, with the
  in-app path input as fallback, and measure the helper and the Linux and macOS dialogs
  (MV-S3-3).
- `ferrum-render` must honor `Buffer::age()`: repaint everything when it is 0, and track damage over
  the last `age` frames otherwise. The renderer cannot assume that unpainted rows keep their pixels
  (macOS reports 0 every time).
- The first frame, and every frame after a resize, must touch every pixel of the buffer; untouched
  pages are not resident and would hide the real surface cost.
- `softbuffer` presentation, buffer acquisition and the renderer stay on the UI thread; present
  after the whole frame, not per row.
- Linux build needs the X11 and xkbcommon development packages listed under "Reproduction"
  (`tiny-xlib` requires `x11-xcb`); `docs/11` and the CI image for the real UI WP must install them.
- The glyph cache in `ferrum-render` needs its LRU and a cap; the spike's 10 KB figure is for 94
  glyphs only.
- The tiles of the chrome are built once and cached; no `tiny-skia` call may run on the repaint path.
- `docs/03` §2 UI rows now carry the measured values; allowances and gates are unchanged.

## Limits of this measurement (nothing here is estimated)

- Windows timings come from one machine with a busy background; Linux and macOS from shared virtual
  machines whose host (and CPU model, printed in the log: `Apple M1 (Virtual)`, 3 vCPUs for macOS in
  the run used here) can differ from run to run, so CI timings are indicative, not comparable across
  runs.
- macOS windows were clamped to 1024×642 by the runner display, so macOS memory and repaint rows are
  not the 1440×900 and 200×60 figures; no real macOS hardware was used.
- Wayland is a headless weston, not a desktop compositor with decorations; Xvfb has no window manager.
- No HiDPI display (scale factor 1.0 everywhere); 2880×1800 rows are pixel-count equivalents.
- Idle CPU and wake-ups were not measured. The Linux and macOS folder dialogs were not measured.
- The spike cache has no LRU and holds ASCII only; fonts other than JetBrains Mono were not compared.
- Windows memory is sampled from a performance counter at the moment the app waits for the
  acknowledgement; values are stable to about 0.1 MiB between runs.

## Linux and macOS runs

Temporary workflow `spike-s3.yml` (added on the spike branch and deleted in the last commit of the
branch; the runs stay visible in the Actions tab):

- Run `45028410900`, on the commit that adds the temporary workflow, whose `spikes/` tree is the
  one that is merged: https://github.com/Juandiazcar/Ferrum/actions/runs/45028410900
  - linux-x11 (Xvfb): https://github.com/Juandiazcar/Ferrum/actions/runs/45028410900/job/137226392945
  - linux-wayland (weston headless): https://github.com/Juandiazcar/Ferrum/actions/runs/45028410900/job/137226393120
  - macos: https://github.com/Juandiazcar/Ferrum/actions/runs/45028410900/job/137226393091
- The Windows block comes from a local run of the same `spikes/` tree on the author machine.
