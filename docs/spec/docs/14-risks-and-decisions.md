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
| Q-06 | Folder/file pickers | Native OS dialogs (`rfd`). If S3 shows an unacceptable resident cost, load the dialog code lazily and keep an in-app path input as fallback |
| Q-07 | Light theme | **None** in v1.0; dark themes only |
| Q-08 | Custom titlebar | **Yes in v1.0**, with a native fallback setting (R-15) |
| Q-09 | Crash reporting | Local crash file + "Copy diagnostics"; nothing is sent |
| Q-10 | Flatpak / Snap | Not offered (see note below) |
| Q-11 | Logo and icon | Designed by Claude: `assets/brand/`; approved by the maintainer 2026-11-20 |
| Q-16 | Mockups tooling | Impeccable skill (installed from `pbakaus/impeccable`) guides the design work |
| Q-17 | Sealed-chunk memory vs its 0.45 MB per-session allowance (spike S1, ADR 0024: 0.352–0.355 MB on the synthetic reference mix, 0.521–0.524 MB on real prose, 0.700–0.712 MB on random text; LZ4 ratio 2.02× / 1.35× / 1.0× instead of the expected 2.5–4×) | Maintainer decision 2026-11-21: **keep all allowances and LZ4 unchanged.** WP-01 re-measures with the product fixture generator; the WP-08 gate (reference scenario) decides. If WP-01 or WP-08 shows the reference scenario over budget, it goes back to the maintainer with numbers |
| Q-18 | `docs/11` §4 said ConPTY re-renders the screen on attach and the initial full repaint must be filtered (spike S4, ADR 0026: no repaint of existing content occurs at session start; with `INHERIT_CURSOR` the only startup output is the cursor query `ESC[6n`; every resize, even a no-op one, is followed by a viewport-only repaint that never includes scrollback) | Maintainer decision 2026-11-21: **`docs/11` §4 rewritten to the measured behavior** (answer the startup cursor query when `INHERIT_CURSOR` is used; viewport-only repaint after every resize; the "filter the initial full repaint" statement is removed). Windows backend decision (same day, ADR 0026, accepted): direct `windows-sys` ConPTY backend inside `ferrum-pty`, `portable-pty` kept for Unix/macOS, with the documented fallback if the direct backend cannot pass the `PtyBackend` contract suite |

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
| 0016 | Allocator choice | pending (S2) |
| 0017 | Custom (client-drawn) titlebar in v1.0 with a native-titlebar fallback | accepted |
| 0018 | Dark themes only in v1.0 | accepted |
| 0019 | One variable monospace font family for terminal and UI; hierarchy by weight | accepted |
| 0020 | Initial releases unsigned (ad-hoc on macOS); signing steps prepared behind flags | accepted |
| 0021 | macOS installers per architecture, not universal | accepted |
| 0022 | CI jobs arrive with the work package that creates what they measure (nightly WP-01, memcheck WP-06/WP-08, soak WP-08, release WP-18) | accepted |
| 0023 | `ferrum-daemon` may depend on `ferrum-platform` (traits in the library, real implementations wired only in the `ferrumd` bin) | accepted |
| 0024 | Scrollback memory model: S1 measurements (confirms ADR 0004/0005 design; LZ4 ratio corrected; sealed-chunk allowance question resolved as Q-17) | accepted |
| 0026 | Windows PTY backend (spike S4): ConPTY measured on Windows 11, Windows Server 2025, Ubuntu and macOS; direct `windows-sys` backend for Windows, `portable-pty` kept for Unix/macOS; `PtyBackend` contract test list (PT-01 to PT-25); ADR 0008 Windows part validated; resolves Q-18 (docs/11 §4 wording) | accepted |

## 4. Change control for this specification

- Requirements, budgets, and acceptance criteria change only through an ADR that states the measurement or reason; the maintainer approves.
- Typos and clarifications can be fixed in a normal PR.
- When a spike or implementation invalidates an estimate, update the document **and** the ADR in the same PR.
