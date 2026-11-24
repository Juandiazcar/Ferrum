# 14 — Risks, Open Questions, and Decision Log

## 1. Risk register

| ID | Risk | Likelihood | Impact | Mitigation | Owner/Trigger |
|---|---|---|---|---|---|
| R-01 | **HiDPI/large-window framebuffer makes < 50 MB physically impossible** (e.g., 2880×1800 = 20.7 MB per buffer) | High on Retina/4K | High | Gate defined at 1440×900@1x and a second gate excluding the surface (`docs/03` §5); damage-limited presentation; optional compact rendering; state it plainly in docs | S3 measurement |
| R-02 | Custom terminal core is large; conformance gaps | Medium | High | Fixtures from Alacritty ref-tests, vttest, fuzzing, smoke list with real TUIs; prioritize common sequences first | WP-02 |
| R-03 | Scrollback estimates are off (compression ratio, per-line overhead) | Medium | High | Spike S1 measures before building on it; adjust chunk size/format; disk spill as safety valve | S1 |
| R-04 | Windows ConPTY quirks (initial repaint, resize, I/O model) | High | Medium | Spike S4 done (ADR 0026): quirks measured on Windows 11 and Server 2025 and listed with workarounds and a `PtyBackend` contract test list; isolate in `ferrum-pty`; direct `windows-sys` implementation decided (ADR 0026). ConPTY throughput is conhost-bound per session (1.3-10 MB/s for a flood, 0.6-1.2 MB/s for `cmd /c type`; Unix PTYs 34-61 MB/s on CI): a finding recorded in ADR 0026, no spec target covers it. Likelihood stays High (quirks confirmed) | WP-05 |
| R-05 | `portable-pty` is lightly maintained (last release 2025-02) | High | Medium | Wrap behind `PtyBackend`; keep an escape hatch to `rustix`/`windows-sys`. Spike S4 (ADR 0026) raised the likelihood from Medium: 0.9.0 is still the latest release on 2026-11-21 (21 months old) and the harness measured two Windows defects in it (an exit code of 259 is never reported; `clone_killer().kill()` returns `Err` on success) plus fixed ConPTY flags. Escape hatch exercised: ADR 0026 decides a direct `windows-sys` ConPTY backend for Windows (Unix keeps `portable-pty`) | WP-05 |
| R-06 | Cannot execute macOS/Windows from the Linux sandbox | Certain | Medium | CI matrix; compile-checks; manual verification checklist; maintainer tests on his Mac/Windows machines | every WP |
| R-07 | Custom UI toolkit lacks polish/accessibility vs mature toolkits | Medium | Medium | Keep widget set minimal; thorough UI-logic tests; accessibility deferred to v1.x with AccessKit evaluation | WP-08 |
| R-08 | CPU rendering misses latency/fluidity targets on large windows | Medium | High | Row hashing + damage rects, SIMD blit, frame pacing, glyph cache; spike S3; adaptive FPS under floods | S3, WP-07 |
| R-09 | Signing/notarization requires accounts the agent cannot create | Certain | Medium | Prepare pipelines; document required secrets; release unsigned pre-releases with clear notes | WP-18 |
| R-10 | Usage meters depend on undocumented/unstable vendor behavior and vendor terms | High | Medium | Phase 2; precision labels; adapter isolation; "unavailable" is a valid state | P2-04 |
| R-11 | Crate ecosystem churn (winit 0.31, softbuffer API changes) | Medium | Low | Pin via lockfile; isolate in `ferrum-app`/`ferrum-platform`; monthly dependency PRs | ongoing |
| R-12 | Auto-update misconfiguration bricks installs | Low | High | Staged update, signature verification, rollback copy, dry-run in pre-release | WP-18 |
| R-13 | Scope creep (Warp-class feature set) | High | High | Non-goals list (`docs/00` §3); every feature needs an FR ID and a memory line item | continuous |
| R-14 | Unattended agent drifts from the rules or hides failures | Medium | High | `CLAUDE.md` rules; gates that cannot be edited without an ADR; `PROGRESS.md` with real numbers; periodic maintainer review of the diff stat and the gate history | continuous |
| R-15 | **Custom titlebar quirks** (Windows 11 snap layouts, Wayland shadows/resize, tiling WMs, macOS fullscreen) | High | Medium | Native-titlebar setting and launch flag always available and tested in CI; automatic fallback on known-bad environments; degraded behaviors documented (`docs/11` §4.1) | WP-16 |
| R-16 | **Unsigned releases create install friction** (Gatekeeper, SmartScreen) and weaker OS-level trust | Certain | Medium | Clear install docs; checksums + minisign signatures; hash-pinned Homebrew/Scoop/winget manifests; signing pipeline ready behind flags for when accounts exist | WP-18 |

## 2. Maintainer decisions (2026-11-20) and remaining open questions

### Resolved

| ID | Question | Decision |
|---|---|---|
| Q-01 | Repository name | `Ferrum` (owner: `Juandiazcar`, github.com/Juandiazcar/ferrum) |
| Q-02 | UI font strategy | One monospace family for everything (JetBrains Mono, variable); hierarchy through several bold weights (`docs/09` §9) |
| Q-03 | macOS installers | One `.dmg` per architecture |
| Q-04 | Windows signing | No certificate: unsigned releases at first (R-16) |
| Q-05 | macOS signing | No Apple Developer account: ad-hoc signed, not notarized, at first (R-16) |
| Q-06 | Folder/file pickers | Native OS dialogs (`rfd`). If S3 shows an unacceptable resident cost, load the dialog code lazily and keep an in-app path input as fallback. Extended 2026-11-21 per ADR 0025 (maintainer approved): the native dialog opens in a short-lived helper process (`ferrum --pick-folder`) so no resident cost stays in the UI process; the in-app path input and drag & drop remain the fallback |
| Q-07 | Light theme | **None** in v1.0; dark themes only |
| Q-08 | Custom titlebar | **Yes in v1.0**, with a native fallback setting (R-15) |
| Q-09 | Crash reporting | Local crash file + "Copy diagnostics"; nothing is sent |
| Q-10 | Flatpak / Snap | Not offered (see note below) |
| Q-11 | Logo and icon | Designed by Claude: `assets/brand/`; approved by the maintainer 2026-11-20 |
| Q-16 | Mockups tooling | Impeccable skill (installed from `pbakaus/impeccable`) guides the design work |
| Q-17 | Sealed-chunk memory vs its 0.45 MB per-session allowance (spike S1, ADR 0024: 0.352–0.355 MB on the synthetic reference mix, 0.521–0.524 MB on real prose, 0.700–0.712 MB on random text; LZ4 ratio 2.02× / 1.35× / 1.0× instead of the expected 2.5–4×) | Maintainer decision 2026-11-21: **keep all allowances and LZ4 unchanged.** WP-01 re-measures with the product fixture generator; the WP-08 gate (reference scenario) decides. If WP-01 or WP-08 shows the reference scenario over budget, it goes back to the maintainer with numbers. **WP-01 re-measurement (2026-11-22, product `ferrum-scrollback` + `fixtures/output/` generator, `dhat`-reconciled to 84 bytes, 3 seeds):** reference mix: hot 0.0857-0.0858 MB, sealed 0.338-0.343 MB, own misc 0.0009 MB, so 0.425-0.430 MB in the crate; with the live grid (0.176 MB) and read buffer (0.066 MB) of ADR 0024 (not re-measurable before WP-02 and WP-06) 0.667-0.672 MB per session, inside the 1.0 MB allowance: no new question. Random ASCII (not the reference scenario): sealed 0.706-0.709 MB, about 1.04 MB per session, the over-allowance case Q-17 already covers (the WP-08 gate decides) |
| Q-18 | `docs/11` §4 said ConPTY re-renders the screen on attach and the initial full repaint must be filtered (spike S4, ADR 0026: no repaint of existing content occurs at session start; with `INHERIT_CURSOR` the only startup output is the cursor query `ESC[6n`; every resize, even a no-op one, is followed by a viewport-only repaint that never includes scrollback) | Maintainer decision 2026-11-21: **`docs/11` §4 rewritten to the measured behavior** (answer the startup cursor query when `INHERIT_CURSOR` is used; viewport-only repaint after every resize; the "filter the initial full repaint" statement is removed). Windows backend decision (same day, ADR 0026, accepted): direct `windows-sys` ConPTY backend inside `ferrum-pty`, `portable-pty` kept for Unix/macOS, with the documented fallback if the direct backend cannot pass the `PtyBackend` contract suite |
| Q-19 | NFR-PERF-03 (full-screen repaint 200×60 ≤ 2.5 ms CPU): which content defines it? Spike S3 (ADR 0025), scalar blitter, direct into the `softbuffer` buffer: typical mix 0.98–1.15 ms median on Windows (i5-9400), 0.66 ms on Linux CI; dense text (all 12 000 cells hold a glyph) 2.64–2.88 ms on Windows (over 2.5 ms) and 1.60–1.70 ms on Linux CI | Maintainer decision 2026-11-21: **the target stays ≤ 2.5 ms, defined on the strictest reading: a full 200×60 screen of dense text (every cell holds a glyph).** WP-07 must meet it with the SIMD blitter on the maintainer's machine (i5-9400, Windows) and in CI. If it does not, it goes back to the maintainer with numbers |
| Q-20 | macOS UI memory: the S3 spike (window and surface only, no UI toolkit; the runner display clamped the window to 1024×642) reaches 10.6–10.7 MiB `phys_footprint` right after window creation and 24.4–24.5 MiB after 4 presents plus 5 s idle on a virtualized GitHub runner (ADR 0025), against a 4.5 MB process-base allowance | Maintainer decision 2026-11-21: **the allowance is unchanged.** The maintainer runs the spike on a real Mac (`docs/manual-verification.md` MV-S3-1) before WP-08 and that number decides; if it is over the allowance it returns to the maintainer |
| Q-21 | NFR-PERF-02 (parse at least 400 MB/s on plain ASCII and 120 MB/s on SGR-heavy output, single core): the first `ferrum-vt` build met it only without the scrollback push (160 / 84 MB/s with the push on the i5-9400, ADR 0028 proposed to move the push to the WP-06 worker) | Maintainer decision 2026-11-23: **the target applies with the scrollback push on, the product configuration** (`vt_parse/ascii_8mib` and `vt_parse/sgr_heavy_8mib`). The proposal to measure without the push and push from the worker is rejected. The encode and `Scrollback::push_line` are optimized until it is met; the absolute ceilings in `xtask/src/bench/baseline.rs` cover the with-push benches. If it cannot be met after real effort the numbers and the profile come back to the maintainer; nothing is relaxed. **Amended 2026-11-23 ("seal on the worker")** after 253 MB/s on the M2 with 44 % of the time in LZ4: the encode and `Scrollback::push_line` stay on the measured parse path (the I/O thread); only the LZ4 compression of a full hot chunk moves to the daemon worker, as §5 of `docs/02` already assigns it (immutable chunk handles, bounded channels). The full chunk becomes an immutable pending chunk with no copy and no steady-state allocation; at most `MAX_PENDING_CHUNKS` wait (3 when amended; 2 since maintainer decision Q-26, 2026-11-23, with a 0.18 MB "pending chunks" line in `docs/03` §2), then the push path seals inline (backpressure, counted); the parse benches run the compression on a second thread; `scrollback_seal/*` is gated by its own ceiling (ADR 0028 decision 8). Maintainer decision 2026-11-23 (with Q-26): a second (worker) thread compressing concurrently is the approved reading of "single core" in NFR-PERF-02 for the parse thread: the target is measured on the thread that parses, with the push on, while the worker compresses |
| Q-22 | Reflow of 10 000 lines took 58.7 ms against the 40 ms of `docs/03` §4 (i5-9400) | Maintainer decision 2026-11-23: **40 ms now**, measured by `vt_reflow/10k_lines_200_to_100_columns`; no deferral to WP-06/WP-17 and no interim 80 ms ceiling |
| Q-23 | Where the imported Alacritty reference tests disagree with a first reading of xterm | Maintainer decision 2026-11-23: **the Alacritty reference tests win everywhere** (ADR 0028 decision 5, RIS dropping the scrollback included); every such choice is listed in `docs/user/compat.md` with a test that pins it |
| Q-24 | Hyperlink and underline-color ids (`ExtraId`) travel inside scrollback style words, so the first build never reused one and refused new entries when the 4 095 slots were taken | Maintainer decision 2026-11-23: **refuse when full is approved, plus a cleanup**: ids that no row uses (screen, alternate screen, scrollback) are released, refusals are counted (`Extras::overflowed`), memory stays bounded, the parse path does not allocate, and a reused id can never re-point a live row (ADR 0028 decision 3, `tests/extras.rs`) |
| Q-25 | Amends Q-23: the Alacritty reference test `selective_erasure` was not imported because Ferrum implemented DECSCA, DECSED and DECSEL from xterm's documentation, while Alacritty has none | Maintainer decision 2026-11-23: **Alacritty's behavior, literally, protected erase included.** DECSCA, DECSED and DECSEL are removed (code, fixtures, `implemented.txt`), `selective_erasure` is imported and passes (45 of 45), and `docs/user/compat.md` says that protected erase is not supported, with the test. RIS resets exactly what Alacritty's `Term::reset_state` resets (cursor style and title included, no title event, palette kept) (ADR 0028 decision 5) |
| Q-26 | Amends the Q-21 amendment: how many full chunks may wait for the worker, and what they cost per session | Maintainer decision 2026-11-23: **`MAX_PENDING_CHUNKS` = 1**, with a `docs/03` §2 "pending chunk" line that keeps the per-session allowance at 1.0 MB, and a stop if the with-push target is missed. With one, the Apple M2 measured `vt_parse/ascii_8mib` 336.5 MiB/s (353 MB/s, under 400) with 48 % of its chunks sealed inline, and it was reported back. **Amended the same day: `MAX_PENDING_CHUNKS` = 2.** The "pending chunks" line is 0.18 MB (two chunk buffers, 0.172 MB at most, 0.086 MB measured in steady state); 0.09 MB came from misc (0.20 -> 0.11) and 0.09 MB from sealed chunks (0.45 -> 0.36; measured 0.338-0.343 MB on the reference mix with the product code), so 0.25 + 0.10 + 0.36 + 0.18 + 0.11 = 1.00 MB. With two, on the M2: `ascii_8mib` 418.5 MiB/s (439 MB/s) with 5 286 of 64 550 chunks (8.2 %) sealed inline, `sgr_heavy_8mib` 211.5 MiB/s (222 MB/s) with 7 of 16 261; `tests/memory_budget.rs` passes with the new allowances (ADR 0028 decision 8 and Measurements) |
| Q-27 | Who measures NFR-PERF-02, the reflow and the worker seal on the reference machine | Maintainer decision 2026-11-23: **the maintainer runs `cargo xtask bench` on the i5-9400 (Windows) before merge** and saves the `vt_*` and `scrollback_seal/*` baselines there (`docs/manual-verification.md` MV-WP02-1). Until then the verdict on the reference machine is "pending maintainer run"; no estimate is written as a verdict and no M2 number goes into `windows.json`. See Q-28 (2026-11-24): ASCII with the push is accepted as measured; the run still records it and saves the baselines |
| Q-28 | NFR-PERF-02 on plain ASCII with the scrollback push: GitHub-hosted runners give a different CPU on each run, so an absolute MB/s gate there cannot judge it | Maintainer decision 2026-11-24: **GitHub-hosted runners get a different CPU each run, so an absolute MB/s gate there is meaningless. NFR-PERF-02 ASCII-with-push: optimize as far as reasonable, then accept the measured result and move on** (best measured: 292 MiB/s EPYC 9V74 Windows, 369 MiB/s Xeon 6975P-C Linux, 406 MiB/s / 426 MB/s Apple M2; no-history 541-607 MiB/s on x86). SGR-heavy at least 120 MB/s and the reflow at most 40 ms remain met (ADR 0028) |

*Note on Flatpak/Snap:* they are sandboxed Linux packaging formats. A terminal must reach the user's real shells, files and tools, and sandboxing fights that, so Ferrum ships `.deb`, `.rpm`, AppImage, tar.gz and an AUR package instead.

### Still open (the agent proceeds with the default and marks it PROVISIONAL)

| ID | Question | Default | Decide by |
|---|---|---|---|
| Q-13 | Start code signing (Apple Developer ID, Windows certificate or OSS signing service) | Unsigned until the maintainer supplies accounts | WP-18 |
| Q-14 | Website/domain | None; the README is the landing page | after v1.0 |

## 3. Decision log (ADR index)

ADR files live in `docs/adr/NNNN-title.md` (status: proposed / accepted / superseded). Initial decisions recorded by this specification:

| ADR | Decision | Status |
|---|---|---|
| 0001 | Record architecture decisions as ADRs | accepted |
| 0002 | Rust workspace; Rust 1.100.0 pinned; edition 2024; MSRV 1.97 | accepted |
| 0003 | Two executables: `ferrum` (UI) and `ferrumd` (daemon); daemon owns terminal state | accepted |
| 0004 | Custom VT state machine on `vte`; reject `alacritty_terminal` for memory reasons | accepted (validated by S1) |
| 0005 | Scrollback stored as encoded text + style runs, chunk-compressed, optional disk spill | accepted (validated by S1) |
| 0006 | CPU rendering via `softbuffer` + custom blitter; `tiny-skia` for chrome only | accepted (validated by S3) |
| 0007 | Custom minimal UI toolkit; no egui/iced/slint/webview | accepted |
| 0008 | No async runtime in v1.0; `mio` loop on Unix, reader threads on Windows | accepted (validated by S4) |
| 0009 | IPC via local socket/named pipe, `postcard` frames, versioned protocol | accepted |
| 0010 | Config/projects in TOML with schema versions; export as tar+zstd | accepted |
| 0011 | Trunk-based Git, Conventional Commits, squash merges, DCO sign-off | accepted |
| 0012 | License MIT OR Apache-2.0 | accepted |
| 0013 | Phase 2 uses official CLIs for subscriptions; no OAuth token reuse | accepted |
| 0014 | Agent-to-agent communication via built-in MCP server | proposed (confirm at P2-05) |
| 0015 | Memory gate defined at 1440×900@1x + surface-excluded gate for HiDPI | accepted |
| 0016 | Allocator choice per OS (spike S2): system allocator on Linux (glibc, no `M_ARENA_MAX`) and Windows, `mimalloc` on macOS; jemalloc not applicable on Windows MSVC; mimalloc costs +13.5 MB on Linux with THP `madvise` | accepted |
| 0017 | Custom (client-drawn) titlebar in v1.0 with a native-titlebar fallback | accepted |
| 0018 | Dark themes only in v1.0 | accepted |
| 0019 | One variable monospace font family for terminal and UI; hierarchy by weight | accepted |
| 0020 | Initial releases unsigned (ad-hoc on macOS); signing steps prepared behind flags | accepted |
| 0021 | macOS installers per architecture, not universal | accepted |
| 0022 | CI jobs arrive with the work package that creates what they measure (nightly WP-01, memcheck WP-06/WP-08, soak WP-08, release WP-18) | accepted |
| 0023 | `ferrum-daemon` may depend on `ferrum-platform` (traits in the library, real implementations wired only in the `ferrumd` bin) | accepted |
| 0024 | Scrollback memory model: S1 measurements (confirms ADR 0004/0005 design; LZ4 ratio corrected; sealed-chunk allowance question resolved as Q-17) | accepted |
| 0025 | Render pipeline: S3 measurements (confirms ADR 0006 as an architecture; tiny-skia kept for chrome tiles only; native folder dialog opened in a short-lived helper process, Q-06 extended; Q-19 and Q-20 resolved by the maintainer) | accepted |
| 0026 | Windows PTY backend (spike S4): ConPTY measured on Windows 11, Windows Server 2025, Ubuntu and macOS; direct `windows-sys` backend for Windows, `portable-pty` kept for Unix/macOS; `PtyBackend` contract test list (PT-01 to PT-25); ADR 0008 Windows part validated; resolves Q-18 (docs/11 §4 wording) | accepted |
| 0027 | Scrollback spill without mmap: the spill file uses plain positional reads and writes (no `unsafe`, no `memmap2` in `ferrum-scrollback`); FR-TERM-011, `docs/02` §3.2 and `docs/07` reworded | accepted |
| 0028 | `ferrum-vt` design decisions (strings captured or capped at 1 MiB and discarded to their terminator, fast paths in front of `vte`, extras in the color words with release of unused ids, streaming reflow, Alacritty reference behavior, grid size limit, worker seal) and parse throughput measured with the scrollback push (Q-21 to Q-28) | accepted |

## 4. Change control for this specification

- Requirements, budgets, and acceptance criteria change only through an ADR that states the measurement or reason; the maintainer approves.
- Typos and clarifications can be fixed in a normal PR.
- When a spike or implementation invalidates an estimate, update the document **and** the ADR in the same PR.
