# 0016. Allocator choice: S2 measurements

- Status: accepted
- Date: 2026-11-22

## Context

`docs/03` §7 asks for a comparison of the system allocator (glibc `malloc` with `M_ARENA_MAX=1`
on Linux, the system allocators on macOS and Windows), `mimalloc` and `tikv-jemallocator` on the
reference workload, and the rule is: "choose the lowest steady-state memory with throughput within
10 % of the best". `docs/12` Spike S2 adds: "Exit: ADR choosing the allocator per OS". The
choice is made once, in the composition roots (`ferrum-app`, `ferrumd`), never in libraries.

## Method

Code: `spikes/s2-allocator/` (standalone package with its own `[workspace]` and `Cargo.lock`,
excluded from the product workspace by the root `exclude = ["spikes"]`; same lint tables as the
root `[workspace.lints]`, including `unsafe_code = "forbid"`, no `unsafe` anywhere in the spike).

- **Workload**: spike S1's, reused without copying: `src/main.rs` compiles S1's `content.rs`
  (generator), `grid.rs` (`vte` grid) and `scrollback.rs` (encoder) through `#[path]`
  (ADR 0024). Ten sessions (seeds 1 to 10), profile `mixed`, each fed 12 000 logical lines with
  10 000 rows retained (checked per session; the run fails otherwise), bounded hot-chunk growth
  (`reserve_exact`, 16 KiB steps, S1's requirement for WP-01), each with its 64 KiB read buffer, are
  held in one process. After the fill the process idles 10 s (steady state, `docs/03` §1), prints
  `STEADY pid=<pid>` and waits. It is single-threaded (as S1), has no PTY, no UI and no
  compression worker.
- **Allocator** is a cargo feature, one build per allocator: default = system, `mimalloc`
  (0.1.52, bundles mimalloc 2.3.2), `jemalloc` (`tikv-jemallocator` 0.7.0). Both versions were
  the latest stable on crates.io on 2026-11-21 (`docs/07` §2). The `#[global_allocator]` statics need
  no `unsafe` in our code. `glibc + M_ARENA_MAX=1` is the same binary as `system` run with
  the environment variable `MALLOC_ARENA_MAX=1` (no `mallopt`, no `unsafe`).
  `tikv-jemallocator` does not support Windows MSVC: its README gates the dependency with
  `cfg(not(target_env = "msvc"))`, the spike's `Cargo.toml` does the same, and
  `cargo build --features jemalloc` on Windows stops at a `compile_error!` ("not applicable").
  Jemalloc is therefore **not applicable on Windows**.
- **Metric** (`docs/03` §1), measured **externally** by `run-all.sh` (the spike calls no platform
  API and no `unsafe`): Linux `Pss` from `/proc/<pid>/smaps_rollup`; macOS `phys_footprint` in
  bytes from `footprint -f bytes <pid>` (the "Auxiliary data: phys_footprint" line, the Activity
  Monitor figure; plain `footprint` rounds to MB); Windows Private Working Set from the PDH
  counter `\Process(*)\Working Set - Private` through PowerShell
  `Get-Counter '\Process(*)\ID Process', '\Process(*)\Working Set - Private'`, matched to the PID
  (`probe-windows.ps1`; it falls back to WMI `Win32_PerfFormattedData_PerfProc_Process` on
  non-English Windows, and the fallback never triggered in any run reported here). This is one
  process, not the AC-MEM-01 sum of `ferrum` + `ferrumd`.
- **Baseline**: the same binary with `--sessions 0` (empty process, same 10 s idle), 3 runs per
  allocator, so "mean minus baseline" isolates what the workload costs under each allocator.
- **Throughput**: S1's parse + encode logical lines/s (`vte` + grid + encode + inline LZ4, without
  `dhat`, generation excluded), printed by the process. Two series: (a) the 3 memory runs
  per allocator (back-to-back, one allocator after the other); (b) `run-throughput.sh`, 10
  rounds in which the allocators run **interleaved**, one process each per round. (b) was added
  after the first CI runs showed run-to-run noise larger than the 10 % margin on macOS and
  Windows (see "Throughput noise"); the decision uses (b), and (a) is reported in full.
- **Runs**: 3 per configuration and OS for memory (processes), 3 + 10 for throughput. Linux and
  macOS on GitHub-hosted runners through a temporary workflow (`.github/workflows/spike-s2.yml`,
  on `spike/s2-*` pushes only, deleted in the last commit of the PR): run
  [45025810885](https://github.com/Juandiazcar/Ferrum/actions/runs/45025810885) (all tables
  below except local Windows). An earlier complete run with the same harness minus the
  interleaved series, [45024902437](https://github.com/Juandiazcar/Ferrum/actions/runs/45024902437),
  gave the same memory results within 0.05 MB (Linux system 8.326, mimalloc 21.830, mimalloc
  `MIMALLOC_ALLOW_THP=0` 9.871, jemalloc 9.718, arena1 8.333; macOS system 10.350, mimalloc 10.285,
  jemalloc 10.574; Windows CI system 8.866, mimalloc 10.665 MB).
- **Machines**: Linux `ubuntu-26.04` runner (kernel 7.0.0-azure, glibc 2.43, 4 cores, 15.6 GB, THP
  `enabled` = `madvise`); macOS `macos-26-arm64` runner (Apple M1 virtual, 3 cores, 7 GB); Windows
  CI `windows-2025-vs2026` runner; Windows local: Windows 11 Pro 10.0.26300, Intel Core i5-9400,
  15.9 GB (the S1 machine). rustc 1.100.0, `--release` (fat LTO, codegen-units 1) everywhere.
  Shared CI runners are virtual machines; their throughput is noisy.

Reproduce (repository root, Git Bash on Windows; needs a C compiler on Linux/macOS for the
vendored allocators):

```bash
bash spikes/s2-allocator/run-all.sh 3          # memory + back-to-back throughput + baseline, all configs for this OS
bash spikes/s2-allocator/run-throughput.sh 10  # interleaved throughput
# one process by hand (feature: none | mimalloc | jemalloc):
cargo build --release --manifest-path spikes/s2-allocator/Cargo.toml --features mimalloc
spikes/s2-allocator/target/release/ferrum-spike-s2 --idle-secs 10 --hold-secs 120   # prints STEADY pid=<pid>
# S1 live bytes quoted below (dhat, seeds 1 to 10):
for s in 1 2 3 4 5 6 7 8 9 10; do spikes/s1-memory-model/target/release/ferrum-spike-s1 --model ferrum --profile mixed --seed $s --hot-step 16384; done
cargo test --release --manifest-path spikes/s2-allocator/Cargo.toml
```

## Results

Sizes in bytes unless "MB" (1 000 000 bytes). "mean minus baseline" is the metric of the
10-session process minus the metric of the empty process of the same allocator. For scale, the
S1 `dhat`-requested live bytes of exactly these ten sessions (seeds 1 to 10, including S1's per
session LZ scratch that this spike also keeps per session) add up to 8 547 746 bytes.

### Linux (PSS), CI run 45025810885

| Allocator | PSS, 3 full runs (B) | mean (MB) | empty-process baseline, 3 runs (B) | baseline mean (MB) | mean minus baseline (MB) |
|---|---|---|---|---|---|
| system (glibc) | 8384512 8327168 8339456 | 8.350 | 401408 393216 405504 | 0.400 | 7.950 |
| system + `MALLOC_ARENA_MAX=1` | 8384512 8359936 8327168 | 8.357 | 389120 434176 405504 | 0.410 | 7.948 |
| mimalloc | 21840896 21828608 21824512 | 21.831 | 5047296 5039104 5043200 | 5.043 | 16.788 |
| mimalloc, `MIMALLOC_ALLOW_THP=0` (variant) | 9876480 9876480 9860096 | 9.871 | 1065984 1074176 1078272 | 1.073 | 8.798 |
| jemalloc | 9728000 9658368 9637888 | 9.675 | 1110016 1110016 1077248 | 1.099 | 8.576 |

| Allocator | back-to-back runs 1 / 2 / 3 (lines/s) | mean |
|---|---|---|
| system | 852687 845463 856644 | 851598 |
| system + arena 1 | 861714 857779 831150 | 850214 |
| mimalloc | 831282 856197 850651 | 846043 |
| mimalloc, no THP | 818902 829033 838461 | 828799 |
| jemalloc | 846212 840975 848835 | 845341 |

| Allocator | interleaved rounds 1 to 10 (lines/s) | mean | vs best mean |
|---|---|---|---|
| system | 844024 858155 844510 858568 852167 851584 863546 848590 852319 747144 | 842061 | -0.8 % |
| system + arena 1 | 849586 853616 846237 856893 855692 854370 866486 849493 851780 803889 | 848804 | +0.0 % |
| mimalloc | 838448 827642 833718 855339 859313 831903 859407 845774 851538 865169 | 846825 | -0.2 % |
| jemalloc | 835893 838818 847360 842640 851701 843003 846445 849299 838822 839019 | 843300 | -0.6 % |

mimalloc's 21.8 MB is transparent huge pages: the raw `smaps_rollup` of the three full mimalloc runs shows
`AnonHugePages` of 20 480 kB (4 096 kB in the empty process; 0 for all other configurations) with THP `enabled = madvise`, which is what mimalloc 2.3.2 does when it asks for THP with
`madvise(MADV_HUGEPAGE)` on its arenas (`src/prim/unix/prim.c`; I did not trace which option path triggers it). With its own option
`MIMALLOC_ALLOW_THP=0` (environment variable only) it drops to 9.871 MB. That variant is
reported as extra information; it is not a different candidate (the product would have to set
the option, and the stock configuration is what `docs/03` §7 names).

### macOS (`phys_footprint`, `footprint -f bytes`), CI run 45025810885

| Allocator | phys_footprint, 3 full runs (B) | mean (MB) | empty-process baseline, 3 runs (B) | baseline mean (MB) | mean minus baseline (MB) |
|---|---|---|---|---|---|
| system (libsystem malloc) | 10371968 10371968 10371968 | 10.372 | 918336 918336 918336 | 0.918 | 9.454 |
| mimalloc | 10323008 10290176 10323008 | 10.312 | 1410048 1410048 1410048 | 1.410 | 8.902 |
| jemalloc | 10552192 10568640 10585024 | 10.569 | 1360768 1360768 1344384 | 1.355 | 9.213 |

| Allocator | back-to-back runs 1 / 2 / 3 (lines/s) | mean |
|---|---|---|
| system | 879477 643491 216993 | 579987 |
| mimalloc | 861682 870366 733644 | 821897 |
| jemalloc | 1026449 974961 901592 | 967667 |

| Allocator | interleaved rounds 1 to 10 (lines/s) | mean | vs best mean |
|---|---|---|---|
| system | 686843 977982 916589 1020104 986768 900746 1048492 1045729 1048854 1048516 | 968062 | -3.8 % |
| mimalloc | 811768 928802 916657 881509 999534 957970 1047715 1043730 1049129 1041647 | 967846 | -3.8 % |
| jemalloc | 986496 958168 973118 978573 926711 1043584 1047945 1048394 1049391 1051163 | 1006354 | +0.0 % |

### Windows (Private Working Set), local machine (i5-9400); jemalloc not applicable

| Allocator | Private WS, 3 full runs (B) | mean (MB) | empty-process baseline, 3 runs (B) | baseline mean (MB) | mean minus baseline (MB) |
|---|---|---|---|---|---|
| system (HeapAlloc) | 8802304 8798208 8798208 | 8.800 | 397312 385024 389120 | 0.390 | 8.409 |
| mimalloc | 10547200 10543104 10551296 | 10.547 | 880640 880640 876544 | 0.879 | 9.668 |

| Allocator | back-to-back runs 1 / 2 / 3 (lines/s) | mean |
|---|---|---|
| system | 561477 526416 520395 | 536096 |
| mimalloc | 600667 580579 607741 | 596329 |

| Allocator | interleaved rounds 1 to 10 (lines/s) | mean | vs best mean |
|---|---|---|---|
| system | 566172 579810 573818 576187 561446 580488 467189 553823 590100 556454 | 560549 | -4.2 % |
| mimalloc | 606541 552334 606864 601705 592893 515591 576983 608538 611252 577741 | 585044 | +0.0 % |

### Windows (Private Working Set), CI run 45025810885

| Allocator | Private WS, 3 full runs (B) | mean (MB) | empty-process baseline, 3 runs (B) | baseline mean (MB) | mean minus baseline (MB) |
|---|---|---|---|---|---|
| system | 8949760 8843264 8990720 | 8.928 | 430080 430080 466944 | 0.442 | 8.486 |
| mimalloc | 10678272 10629120 10637312 | 10.648 | 880640 884736 897024 | 0.887 | 9.761 |

| Allocator | back-to-back runs 1 / 2 / 3 (lines/s) | mean |
|---|---|---|
| system | 814327 788932 798044 | 800434 |
| mimalloc | 821894 844997 841718 | 836203 |

| Allocator | interleaved rounds 1 to 10 (lines/s) | mean | vs best mean |
|---|---|---|---|
| system | 806583 827796 826937 842912 834883 830540 819207 820627 826814 836939 | 827324 | -2.6 % |
| mimalloc | 865081 862223 845798 853108 805129 850468 861245 863649 847743 838429 | 849287 | +0.0 % |

### Throughput noise (why the decision uses the interleaved series)

On the first complete CI run ([45024902437](https://github.com/Juandiazcar/Ferrum/actions/runs/45024902437),
back-to-back only) the macOS means were system 785 940, mimalloc 919 488, jemalloc 779 094 lines/s;
in run 45025810885 the same builds gave system 579 987, mimalloc 821 897, jemalloc 967 667
(single runs from 216 993 to 1 026 449). The ranking of identical binaries reverses between runs,
so back-to-back means on shared runners measure the order and the neighbours, not the
allocator. On the local Windows machine the back-to-back means were system 536 096 against
mimalloc 596 329 (system 10.1 % below); the interleaved series gives system 4.2 % below, which
removes the order effect. Applied to the back-to-back means alone, the local Windows result
would have excluded the system allocator and the macOS result of run 45025810885 would have
selected jemalloc; both reverse with the interleaved series or with the other run, which is why
they are not used. The memory metric does not have this problem: its spread is at most 0.15 MB
(Windows CI system) and 0.06 MB elsewhere.

## Decision (`docs/03` §7 rule, applied per OS)

Rule: among the allocators whose mean throughput is within 10 % of the best mean, take the one
with the lowest mean steady-state memory. Throughput = interleaved series.

| OS | Within 10 % of best throughput | Lowest memory among them | Decision |
|---|---|---|---|
| Linux | all (worst is -0.8 %) | system 8.350 MB (arena 1: 8.357, jemalloc 9.675, mimalloc 21.831, mimalloc no-THP variant 9.871) | **system allocator (glibc `malloc`)**, without `MALLOC_ARENA_MAX=1`: it did not lower memory in this workload (8.357 against 8.350 MB, overlapping ranges) |
| macOS | all (worst is -3.8 %) | mimalloc 10.312 MB (system 10.372, jemalloc 10.569); the ranges do not overlap | **`mimalloc`** |
| Windows | both (system -4.2 % local, -2.6 % CI) | system 8.800 MB local, 8.928 MB CI (mimalloc 10.547 and 10.648) | **system allocator (Windows heap)**; jemalloc not applicable |

## Consequences

- **Wiring** (`ferrum-app` and `ferrumd` mains, per `docs/03` §7): Linux and Windows add nothing
  (no `#[global_allocator]`). macOS declares `mimalloc::MiMalloc` as the global allocator in both
  mains, with `mimalloc` as a macOS-only target dependency of those two crates (`docs/07` §2). No
  library crate selects an allocator. Neither `mimalloc` nor `tikv-jemallocator` is added to a product
  crate by this PR; no product code changes.
- **Linux**: `tikv-jemallocator` and `mimalloc` are not used. `M_ARENA_MAX=1` is not set; whether it
  matters with the daemon's real threads (reader, compressor) was not measured here (the spike is
  single-threaded) and is left to the WP-08 `memcheck` gate, which measures the real process.
  The glibc trim policy of `docs/03` §3 rule 7 stays as written (`malloc_trim(0)` at most once per
  30 s when idle); it needs a libc call, so it belongs behind a trait in `ferrum-platform`
  (`unsafe` allowed there by `docs/05` §4, ADR 0023), not in the `forbid(unsafe)` mains.
- **macOS**: rule 7 says "if `mimalloc` wins, use its purge APIs". Those APIs (`mi_collect`) are
  `unsafe` calls (`libmimalloc-sys`), so the same placement applies (behind a trait in
  `ferrum-platform`); whether any explicit purge is needed (mimalloc also purges by itself after a
  delay) was not measured and is left to WP-08. The margin over the system allocator on macOS is
  0.06 MB (0.6 %): the choice follows the rule literally, and it costs one dependency on one OS.
- **Windows**: system allocator; the only allocator-related measurement left is WP-08's.
- `docs/03` §2 rows "allocator slack / fragmentation" are **not** replaced by a number: this spike
  measures whole-process cost, and the workload's requested bytes (8.55 MB, `dhat`) are not
  comparable with resident bytes (memory that was requested but never touched is not resident), so
  the slack cannot be isolated honestly. Allowances and gates are unchanged. What is now
  measured is recorded in the ADR and noted in the table. Rows cite this ADR.
- The mimalloc result on Linux is a warning for any later change: with THP `madvise` (the Ubuntu
  default) mimalloc costs 13.5 MB more than glibc for this workload.
- The spike is throwaway; the numbers stay here. The temporary workflow is removed in the last commit.

## Deviations and limits (stated plainly)

- Purge/trim effect was **not measured**: `mi_collect`, `malloc_trim` and jemalloc's
  `arena.<i>.purge` (`mallctl`) are all `unsafe` calls, and the spike forbids `unsafe`. Only
  the allocators' automatic behavior during the 10 s idle is in the numbers.
- Single-threaded workload: glibc arena effects (`M_ARENA_MAX=1`) are not exercised, so "no
  effect" holds for this workload only.
- One process, not the AC-MEM-01 sum; no PTY, UI, compression worker, alt screen, resize or
  reflow; content is S1's synthetic `mixed` profile.
- CI runners are shared virtual machines: memory is deterministic enough (spread at most 0.15 MB),
  throughput is not (see the noise section); the decision depends on interleaved means, not on
  single runs. Linux CPU model of the run was not captured. macOS arm64 only (no Intel). glibc 2.43
  only (no musl).
- jemalloc on Windows MSVC: not applicable (not built, not measured).
- Results that came from code that is not in the repository (a macOS `footprint` option probe in a
  temporary workflow, a throwaway jemalloc build test) are not used as evidence.
