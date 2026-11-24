# 07 — Tech Stack and Versions

Versions below were **verified on 2026-11-20** against crates.io (`max_stable_version`) and the Rust blog (`/releases/latest` → 1.100.0, dated 2026-11-12). They are the starting point for `[workspace.dependencies]`.

**Rule:** before adding or bumping any dependency, re-check crates.io (`curl -s -A "ferrum" https://crates.io/api/v1/crates/<name>`) and update this table with the date. Do not rely on memory or on this table once it is more than a month old. Dependabot/Renovate keeps `Cargo.lock` current automatically; this file records *decisions*.

## 1. Language and toolchain

| Item | Choice | Version | Notes |
|---|---|---|---|
| Rust | stable, pinned in `rust-toolchain.toml` | **1.100.0** (2026-11-12) | Update the pin monthly via a dedicated `chore(deps)` PR |
| Edition | 2024 | — | `resolver = "3"` (MSRV-aware) |
| MSRV (`rust-version`) | 1.97 | — | The sandbox used to write the spec had 1.97.0; do not use APIs newer than the MSRV unless the toolchain pin and MSRV are bumped together |
| Components | `rustfmt`, `clippy`, `llvm-tools-preview` | — | |
| Targets | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` | — | Linux musl is out of scope |

## 2. Runtime dependencies (v1.0)

| Area | Crate | Version | Used by | Why / notes |
|---|---|---|---|---|
| Windowing | `winit` | 0.30.13 | app | Stable line at check time; 0.31 not stable yet. Re-check before WP-08 |
| Pixel presentation | `softbuffer` | 0.4.8 | app | CPU buffers, damage presentation; X11, Wayland, Win32, AppKit |
| Window handles | `raw-window-handle` | 0.6.2 | app | transitive; pinned by winit/softbuffer |
| 2D chrome drawing | `tiny-skia` | 0.12.0 | render | rounded rects, gradients, shadows on cached tiles only |
| Fonts: shaping + raster | `swash` | 0.2.10 | render | one crate for shaping and rasterization incl. color glyphs; avoids `cosmic-text` weight |
| Font discovery | `fontdb` | 0.24.0 | render | scans system fonts lazily; memory-maps files |
| Font parsing | `ttf-parser` | 0.25.1 | render | transitive via fontdb |
| VT parsing | `vte` | 0.15.0 (2026-11-22, latest stable) | vt | state machine only; Ferrum owns the grid/handlers. `default-features = false`: the `no_std` buffer mode with a 1-byte OSC buffer, because `ferrum-vt` captures OSC strings itself with a 1 MiB cap (ADR 0028) |
| Unicode widths | `unicode-width` | 0.2.3 (2026-11-22, latest stable) | vt | `default-features = false` (no `cjk` context widths) |
| Grapheme clusters | `unicode-segmentation` | 1.13.3 (2026-11-22, latest stable) | vt | `default-features = false` |
| PTY | `portable-pty` | 0.9.0 | pty | Linux and macOS PTY only (ADR 0026); last release Feb 2025 → wrapped behind `PtyBackend` so it can be replaced. Windows uses `windows-sys` directly |
| Windows APIs | `windows-sys` | 0.61.2 | pty, platform | required by the direct ConPTY backend in `ferrum-pty` (`portable-pty` was insufficient on Windows: ADR 0026); prefer `-sys` (no bloat) |
| Unix syscalls | `rustix` | 1.1.6 | pty, platform, daemon | safe wrappers; avoids hand-written `libc` unsafe |
| libc (fallback) | `libc` | 0.2.191 | platform | only for what `rustix` lacks (e.g., `mallopt`, `malloc_trim`) |
| Event loop I/O (Unix) | `mio` | 1.2.5 | daemon | no async runtime; Windows uses reader threads (see `docs/11`) |
| IPC sockets/pipes | `interprocess` | 2.4.9 | proto/daemon/app | UDS + Windows named pipes with a uniform API |
| Serialization | `serde` | 1.0.229 | all | derive only where needed |
| Wire format | `postcard` | 1.1.3 | proto | compact, no-std friendly, deterministic |
| Config format | `toml` | 1.2.2 (re-verified 2026-11-24, latest stable) | config, xtask | serde-based; atomic writes by us; `default-features = false` with `std`, `serde`, `parse`, `display` |
| Compression (scrollback) | `lz4_flex` | 0.14.2 | scrollback | pure Rust, `default-features = false`, `safe-encode`/`safe-decode` |
| mmap | `memmap2` | 0.9.12 | render | fonts only. The scrollback spill file uses plain positional reads and writes from `std` (ADR 0027), because mapping needs `unsafe` |
| Byte search | `memchr` | 2.8.3 (2026-11-22, latest stable) | vt, scrollback | `default-features = false`, `std`; `vt` uses `memmem` for `ESC ]` and `memchr` for ESC |
| POD casts | `bytemuck` | 1.25.2 | render, vt | `Cell` ↔ bytes where needed |
| Small vectors | `smallvec` | 1.16.6 | vt | only where profiling justifies |
| Fast hashing | `rustc-hash` | 2.1.3 (2026-11-22, latest stable) | vt, scrollback, render | interning tables (the scrollback style table too); `default-features = false`, `std` |
| Bit flags | `bitflags` | 2.13.2 (2026-11-22, latest stable) | vt | cell attrs, modes; `default-features = false` |
| Clipboard | `arboard` | 3.7.0 | platform | text + image; Wayland/X11 handled; features trimmed |
| Image decode | `image` | 0.25.10 | render | `default-features = false`, features: `png`, `jpeg`, `gif`, `webp` only |
| PNG (direct) | `png` | 0.18.1 | render | for kitty graphics PNG path if `image` is too heavy |
| Base64 | `base64` | 0.23.1 (2026-11-22, latest stable) | vt | OSC 52, kitty graphics; `default-features = false`, `std` (no `simd-unsafe`) |
| Directories | `directories` | 6.0.0 | platform | XDG / Library / AppData paths |
| Native menus (macOS) | `muda` | 0.21.2 | app (macOS only) | standard app menu so Cmd+Q/H/M and Edit actions work |
| File dialogs | `rfd` | 0.18.0 | app | folder picker for projects, export/import; evaluate memory/size (S3); fallback = in-app path input + drag & drop |
| Color parsing | `csscolorparser` | 0.9.2 | config | theme import (`#rrggbb`, `rgb()`, names) |
| Plist (iTerm themes) | `plist` | 1.10.4 | config | only for `.itermcolors` import; write a minimal parser if the crate proves heavy (measure at WP-13) |
| Archives (export) | `tar` | 0.4.46 | config | `.ferrum` = tar + zstd |
| Zstd | `zstd` | 0.14.4 | config | export archives only |
| CLI args | `lexopt` | 0.3.2 | app, daemon | tiny; `clap` is only used in `xtask` |
| xtask CLI | `clap` | 4.6.9 (verified 2026-11-21) | xtask only | derive parser for `cargo xtask`; never in shipped binaries |
| xtask graph check | `cargo_metadata` | 0.23.1 (verified 2026-11-21) | xtask only | typed `cargo metadata` for `check-deps`; brings `serde_json`, build tooling only |
| xtask bench baselines | `serde_json` | 1.0.154 (verified 2026-11-22, latest stable) | xtask only | reads criterion `estimates.json` and reads/writes `benches/baselines/*.json`; already compiled as a dependency of `cargo_metadata`, never in shipped binaries |
| Error types | `thiserror` | 2.0.21 (2026-11-23, latest stable) | libs | `default-features = false`, `std` |
| Error context (bins) | `anyhow` | 1.0.104 | app, daemon, xtask | |
| Logging | `tracing` | 0.1.44 (2026-11-23, latest stable) | all | `default-features = false`, `std` and `release_max_level_warn` (the `max_level_*` features compile out debug/trace in release) |
| Log sink | `tracing-subscriber` | 0.3.23 | app, daemon | `default-features = false`, `fmt` only; own tiny rotating file writer |
| Update download | `ureq` | 3.5.2 | app (updater) | blocking, small; `rustls` backend |
| TLS | `rustls` | 0.23.46 | app (updater) | no OpenSSL |
| Signature verify | `minisign-verify` | 0.3.0 | app (updater) | verify update manifests/artifacts |
| Hash | `sha2` | 0.11.1 | updater | |
| Allocator (ADR 0016) | system allocator on Linux and Windows; `mimalloc` 0.1.52 on macOS only (target dependency of app and daemon); `tikv-jemallocator` 0.7.0 evaluated, not chosen (not applicable on Windows MSVC) | — | app, daemon | decided by spike S2 per OS |

### Spike-only dependencies (never in product crates)

Used only by throwaway packages under `spikes/` (own workspace and lockfile, excluded from the product workspace, CI lints and `deny.toml`). Versions already listed in this document (`vte`, `lz4_flex`, `unicode-width`, `anyhow`, `dhat`) are used at the same version.

| Crate | Version (verified) | Spike | Reason |
|---|---|---|---|
| `alacritty_terminal` | 0.26.0 (2026-11-21) | S1 | Measure its per-session scrollback memory against Ferrum's encoder (ADR 0024). Rejected for the product (§3) |
| `dhat` | 0.3.3 (2026-11-21, latest stable) | S1 | Safe heap accounting (`HeapStats`) instead of a hand-written allocator; no `unsafe` in the spike (§4 lists it for tests) |
| `portable-pty` | 0.9.0 (2026-11-21, latest stable; released 2025-02-11) | S4 | Measure ConPTY and Unix PTY behavior and decide keep vs direct `windows-sys` (ADR 0026). Same version as the product table above |
| `mimalloc` | 0.1.52 (2026-11-21, latest stable; released 2026-05-22; bundles mimalloc 2.3.2 through `libmimalloc-sys` 0.1.49) | S2 | Allocator candidate measured against the system allocator and jemalloc on the S1 workload (ADR 0016). Also in the product table above (macOS only) |
| `tikv-jemallocator` | 0.7.0 (2026-11-21, latest stable; released 2026-05-25) | S2 | Allocator candidate (ADR 0016); not applicable on Windows MSVC, so the spike resolves it on non-MSVC targets only. Not chosen for the product |
| `anyhow` | 1.0.104 (2026-11-21, latest stable) | S4 | Error context in the harness binary (`docs/05` §3: bins may use `anyhow`); same version as the product table |

### Phase 2 only (not in v1.0 builds)

| Area | Crate | Version | Notes |
|---|---|---|---|
| Secret storage (master key) | `keyring` | 4.2.0 | OS keychain / Credential Manager / Secret Service |
| AEAD | `chacha20poly1305` | 0.11.0 | vault file encryption |
| KDF | `argon2` | 0.6.0 | passphrase for export |
| Zeroing | `zeroize` | 1.9.2 | secrets in memory |
| Time | `jiff` | 1.0.1 | usage windows/resets |
| JSON | `serde_json` | 1.0.154 | provider APIs, MCP |
| IDs | `uuid` | 1.29.3 | |
| Async runtime | `tokio` 1.54.1 **or** blocking threads + `ureq` | — | decided in an ADR at P2-01; **banned in v1.0** (`deny.toml`) |

## 3. Evaluated and rejected

| Candidate | Reason |
|---|---|
| `alacritty_terminal` 0.26.0 | Scrollback stored as full cells → ~10× over the memory budget; no scroll-off hook (`docs/02` §3.3). Its ref-tests are used as fixtures instead |
| `wgpu` / any GPU path | Violates "CPU for max compatibility" and costs 30–100 MB of driver memory |
| `egui`, `iced`, `slint`, `gpui` | Each brings its own renderer/text stack and RAM floor; the UI surface is small enough to build directly |
| Tauri / Electron / webview | Memory floor far above 50 MB |
| `tokio` (v1.0) | Not needed; adds size and resident memory; Windows PTY needs threads anyway |
| `gix` 0.90.0 | Heavy; only the branch name is needed → read `.git/HEAD` directly |
| `cosmic-text` | Heavier than `swash` + our cell grid layout |
| `rustybuzz` 0.20.1 | Last release 2024-11; `swash` shapes adequately for ligatures |
| `notify` 8.2.2 | No file watching needed: settings change through the UI |
| `sysinfo` | Large; own-process memory is a 30-line platform function per OS |
| `reqwest` | Heavy; `ureq` suffices for the updater |
| `rusqlite` 0.43.0 | TOML files are enough for config/projects; revisit only if usage history in Phase 2 needs queries |
| `clap` in shipped binaries | ~hundreds of KB; `lexopt` is enough |
| `crossbeam-channel` 0.5.17 / `parking_lot` 0.13.0 | `std::sync::mpsc` (bounded `sync_channel`) and `std` locks are sufficient; add only with a benchmark proving need |

## 4. Development and CI tooling

| Tool | Version (2026-11-20) | Purpose |
|---|---|---|
| `cargo-nextest` | 0.9.152 | fast test runner |
| `cargo-deny` | 0.22.0 | licenses, bans, advisories, sources |
| `cargo-audit` | 0.23.0 | RustSec scan (nightly job) |
| `cargo-llvm-cov` | 0.9.2 | coverage (report only; no coverage gate) |
| `cargo-hack` | 0.6.46 | feature-matrix checks |
| `cargo-machete` | 0.10.1 | unused dependencies |
| `cargo-fuzz` | 0.13.2 | libFuzzer targets (needs nightly toolchain in that job only) |
| `libfuzzer-sys` | 0.4.13 (2026-11-22, latest stable) | fuzz targets only: the `fuzz/` package of a crate (own workspace, nightly in the fuzz job only), `default-features = false` + `link_libfuzzer`; never in product crates |
| `cargo-bloat` | 0.12.1 | binary size attribution (last release 2024-05; still works) |
| `criterion` | 0.8.2 | benchmarks |
| `proptest` | 1.12.0 (2026-11-23, latest stable) | property tests (`ferrum-scrollback`, `ferrum-vt`; `default-features = false`, `std`) |
| `insta` | 1.49.0 (2026-11-23, latest stable) | snapshot/golden tests (`default-features = false`; `ferrum-vt` fixtures) |
| `dhat` | 0.3.3 (2026-11-23, latest stable) | allocation counting in tests (`default-features = false`) |
| `terminal_size` | 0.4.4 (2026-11-24, latest stable; released 2026-03-23) | `ferrum-testkit` only: the `ferrum-pty-helper` child of the `PtyBackend` contract suite reports the terminal size it sees (PT-15) without `unsafe`; depends only on `rustix` (Unix) and `windows-sys` 0.61 (Windows), both already in this table (`default-features = false`) |
| `git-cliff` | 2.14.2 | changelog |
| `typos-cli` | 1.52.2 | typo check |
| `taplo-cli` | 0.10.0 | TOML formatting |
| `cargo-vet` | 0.10.3 | optional supply-chain audits |
| `cargo-dist` | 0.34.0 | evaluate for release automation (name/state may have changed; fallback is a hand-written matrix workflow) |
| `cargo-packager` | 0.11.8 | alternative packager to evaluate for .deb/.msi/.dmg |
| `vttest` | system package | manual VT conformance runs |
| GitHub Actions | resolved at WP-00 (2026-11-21), pinned by SHA in `.github/workflows/ci.yml` | `actions/checkout` v7.0.1, `Swatinem/rust-cache` v2.9.3, `taiki-e/install-action` v2.90.8, `dtolnay/rust-toolchain` (default-branch SHA, no releases); `actions/upload-artifact` v7.0.2 is resolved but unused until a job needs it; Dependabot keeps them current |

## 5. Fonts and assets

- Embedded font (decided): **JetBrains Mono as a variable font** (SIL OFL 1.1), subset to Latin + Latin-ext + box drawing + block elements + powerline (≤ 350 KB), used for both terminal and UI chrome with weight steps 400–800 (`docs/09` §9). Check the latest release at WP-07, subset it, ship the license text. System fonts are preferred when the user selects one.
- Symbols fallback: a small Nerd-Font-symbols subset is **not** bundled by default (size); offered as an optional download in settings in v1.x.
- Emoji: system emoji font (Noto Color Emoji / Apple Color Emoji / Segoe UI Emoji) via `fontdb`.

## 6. Toolchain bootstrap in restricted environments

If `rustup` cannot download the pinned toolchain (network allow-list), use the installed toolchain ≥ MSRV and record the situation in `PROGRESS.md`; CI (which has full network) is the source of truth for the pinned build. Do not edit `rust-toolchain.toml` to match a sandbox.
