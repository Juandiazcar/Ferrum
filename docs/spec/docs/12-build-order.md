# 12 — Build Order (Work Packages)

This is the ordered plan for a long, mostly unattended implementation. v1.0 is delivered as **one complete product** (no half-finished releases), but it is *built* bottom-up so that the foundation is proven before anything stands on it. The terminal is the product: if time runs out, the terminal and projects ship and Phase 2 waits.

Size: S ≈ hours · M ≈ 1 day · L ≈ 2–4 days · XL ≈ 1+ week of focused agent work.

Every WP ends with: all checks green (`cargo xtask check`), `PROGRESS.md` updated, docs updated, commits per `docs/06`.

## Stage 0 — Foundations

### WP-00 Repository bootstrap (S)
- Create repo skeleton per `docs/04`; copy `templates/` to real locations; resolve GitHub Actions versions (pin SHA); `cargo xtask setup/check` skeleton; hooks; `deny.toml` validated with the installed `cargo-deny`; empty crates compile; CI green on 3 OS.
- Copy specs to `docs/spec/`; create `PROGRESS.md`, `docs/manual-verification.md`, `docs/adr/0001-record-architecture-decisions.md`.
- **Exit:** CI green on an empty workspace; `cargo xtask check-deps` enforces layering.

### Spikes (each ends with an ADR holding the measurements; code may be thrown away)
- **S1 Memory model (M):** build a throwaway scrollback encoder + `vte` loop; feed the `memcheck` fixtures; measure bytes/line raw and compressed, and compare with a naive cell-grid scrollback and (if cheap) `alacritty_terminal`. **Exit:** measured per-session memory for 10 000 lines; confirms or corrects `docs/03` §2.
- **S2 Allocator (S):** system vs mimalloc vs jemalloc on S1's workload (steady-state PSS, throughput). **Exit:** ADR choosing the allocator per OS.
- **S3 Render pipeline (M):** `winit` + `softbuffer` window; draw a 200×55 cell grid with `swash` glyph cache; measure repaint time, framebuffer memory, idle memory, damage presentation on Linux (X11 under Xvfb + Wayland if available). Evaluate `rfd` memory/size. **Exit:** numbers for NFR-PERF-03 and the UI memory budget; decision on tiny-skia usage and the file-dialog approach.
- **S4 Windows PTY (M):** exercise `portable-pty` ConPTY on a Windows CI runner (spawn pwsh/cmd, resize, large output, exit codes, initial repaint behavior); decide keep vs direct `windows-sys`. **Exit:** ADR + contract test list for `PtyBackend`.

If a spike contradicts a target in `docs/03`, **stop and write the finding in `docs/14` (open question) with options**; do not silently relax the budget. Continue with the work that does not depend on the answer.

## Stage 1 — Pure core (no windows, no sockets)

### WP-01 `ferrum-scrollback` (L)
Encoding, style interning, chunking, LZ4 sealing, spill trait + file impl, random access by line index, memory accounting API, search iterator, property tests, fuzz target, `dhat` zero-alloc test, benches.
**Exit:** meets S1 numbers; round-trip property tests; fuzz 10 min clean.

### WP-02 `ferrum-vt` (XL)
Grid, cursor, modes, scroll regions, alt screen, charsets, SGR (incl. underline styles/colors), tabs, erase/insert/delete ops, OSC set from FR-TERM-009 (excluding graphics), DCS basics (DECRQSS safe subset, XTGETTCAP none), mouse/focus mode tracking state, bracketed paste state, synchronized output state, kitty keyboard mode stack state, grapheme/wide handling, hyperlinks, shell marks, selection model (logic only), reflow with scrollback integration, event sink trait (title, bell, cwd, clipboard request, …), damage tracking API (dirty rows).
**Exit:** imported ref-tests pass; vt fixtures for every implemented sequence; reflow property tests; fuzz 30 min clean; `dhat` zero steady-state allocations; parse benches hit NFR-PERF-02 or an ADR explains the gap and plan.

### WP-03 `ferrum-proto` (M)
Messages, framing, versioning, row encoding for damage, round-trip + golden-bytes tests, fuzz target for decode.
**Exit:** golden vectors committed; version negotiation tests.

### WP-04 `ferrum-config` and `ferrum-project` (M)
Settings model, defaults, TOML load/save atomic, schema versions + migration framework with fixtures, export/import archive (tar+zstd), project model, `.git/HEAD` reader (branches, detached, worktrees, submodules), startup recipes.
**Exit:** migration tests for a synthetic v0→v1; export/import round-trip; git reader tested against fixture repos created in tests.

## Stage 2 — Process and daemon

### WP-05 `ferrum-pty` (L)
`PtyBackend` traits + shared contract suite; Unix implementation; ConPTY implementation per S4; fake backend for tests; child lifecycle, resize, exit codes, env injection, shell-integration bootstrap files.
**Exit:** contract suite green on Linux and (CI) macOS/Windows.

### WP-06 `ferrum-daemon` (XL)
Session manager, `mio` loop (Windows: woken by the `PtyBackend`'s single overlapped-I/O thread, Q-31), per-session fairness and backpressure, damage coalescing, attach/detach/snapshot, search worker, reflow worker, project/layout authority + persistence, client server (UDS/pipe) with security (`docs/10` §2), idle shutdown, panic isolation per session, `ferrumd` binary, structured logging.
**Exit:** integration tests: AC-SESS-01/02, flood isolation (daemon-level), 20-session scenario without UI; `memcheck --daemon-only` within the daemon allowance.

## Stage 3 — Rendering and UI

### WP-07 `ferrum-render` (XL)
Font loading (embedded + system), metrics, glyph cache, grayscale AA, color emoji, cell painter with row hashing and damage rects, cursor/selection/underline/strikethrough/curly underline drawing, box-drawing and block-element synthesis (pixel-perfect, bypassing the font), powerline glyph alignment, HiDPI, chrome painter helpers (rounded rects, shadows via cached tiles), blit kernels (scalar + SIMD with runtime detection and equivalence tests).
**Exit:** NFR-PERF-03; zero steady-state allocations; golden image tests.

### WP-08 `ferrum-ui` core + `ferrum-app` walking skeleton (XL)
Widget kit, theming, layout engine, input mapping (keyboard encoding per terminal modes incl. kitty protocol), mouse, IME preedit, text input widget, action system, `ferrum-app` wiring: one window with the custom titlebar, sidebar with one project, one terminal pane, status bar with live memory.
**Exit:** you can run a real shell, `vim`, `htop`; latency harness first numbers; `memcheck` with the UI works on Linux.

## Stage 4 — Product features

### WP-09 Projects and sidebar (L)
Project CRUD UI, groups, rename in place, color/icon, DnD reorder/move, drop folders to create, recipes, quick switch, empty states.
**Exit:** AC-PROJ-01, AC-UI-01 (logic part).

### WP-10 Splits and layouts (L)
Split tree, resize, move pane by DnD, zoom, persistence via daemon, keyboard navigation.
**Exit:** layout round-trip tests; manual checklist.

### WP-11 Selection, clipboard, drag & drop, images (L)
Selection modes + auto-scroll, copy/paste policies, bracketed paste sanitization, multiline confirm, file drop quoting per shell, clipboard image → file path, OSC 52 policy UI, hyperlinks + path click.
**Exit:** AC-UI-02, AC-SEC-01 (paste/OSC 52/hyperlinks).

### WP-12 Search, prompt navigation, shell integration (M)
Daemon search + UI overlay + scrollbar markers, prompt jump (OSC 133), integration scripts for bash/zsh/fish/pwsh, cwd tracking.
**Exit:** search bench ≤ 15 ms; scripts tested in CI per shell where available.

### WP-13 Settings UI, themes, keybindings (XL)
All panels in `docs/09` §8, live apply, theme import (Alacritty TOML/YAML, iTerm2, Windows Terminal JSON, Kitty conf, base16), keybinding editor with conflict detection, command palette.
**Exit:** every setting reachable and persisted; theme importers tested with fixtures.

### WP-14 Export/import and migrations in UI (S)
Dialogs, path remapping, merge/replace. **Exit:** AC-DATA-01.

### WP-15 Advanced protocols (L)
Kitty keyboard protocol full, kitty graphics subset, sixel, iTerm2 inline images subset, bounded image cache, undercurl/colored underlines, synchronized output tuned.
**Exit:** protocol fixtures; memory unaffected when unused.

## Stage 5 — Platform polish, hardening, release

### WP-16 Platform polish (L)
custom titlebar per OS with native fallback (`docs/11` §4.1); macOS menu + Option-as-Alt; notarization steps prepared but only enabled when an Apple account exists; Windows manifest (DPI, long paths), AltGr, right-click behavior; Linux Wayland/X11 specifics, IME checks, primary selection; per-OS manual verification checklist items executed by the maintainer.

### WP-17 Hardening and performance (L)
Fuzz corpus growth, 1 h fuzz per target, soak, memory tuning to land inside budgets with headroom, allocator finalization, startup time, binary size, idle wakeups audit, panic-isolation tests, `kill -9` tests.
**Exit:** AC-MEM-*, AC-PERF-*, AC-TERM-03 green.

### WP-18 Packaging, auto-update, release automation (L)
cargo-dist/cargo-packager decision (ADR), installers per OS, signing pipelines (documented secrets), update manifest + minisign, updater UI, winget/brew/scoop/AUR manifests, `release.yml` end to end with a dry run.
**Exit:** AC-DIST-01 on a pre-release tag.

### WP-19 Documentation (M)
User docs (install, quick start, projects, shortcuts, themes, FAQ, compatibility notes), contributor docs, website-ready README with screenshots, `CHANGELOG`.

### WP-20 Release candidate (M)
Run every acceptance criterion on all three OS; fill the manual checklist; Typometer numbers; tag `v1.0.0-rc.1`; fix; tag `v1.0.0`.

## Phase 2 packages (only after v1.0 ships)

| WP | Scope |
|---|---|
| P2-01 | ADR: networking model (blocking threads vs async); `ferrum-vault` (ChaCha20-Poly1305, Argon2, keychain master key, export with passphrase) |
| P2-02 | `SessionKind::Agent` for CLI agents; agent templates; per-agent worktrees |
| P2-03 | `ferrum-providers`: OpenAI-compatible client + Anthropic client; provider catalog (OpenRouter, MiniMax, Qwen, Z.ai/GLM, DeepSeek…); key management UI |
| P2-04 | `ferrum-usage`: adapters + precision labels; status-bar meters; alerts |
| P2-05 | `ferrum-mcp`: built-in MCP server on the local socket; auto-registration into agent CLIs; message bus |
| P2-06 | Agent workflows (user-defined chains) and cross-agent messaging UI |
| P2-07 | Natural-language project generation |

## Rules for unattended runs

1. Work strictly in WP order unless a dependency or blocker forces a documented swap.
2. After each WP: run `cargo xtask check`, update `PROGRESS.md` (what was done, numbers measured, open issues, next WP), commit, and (when a remote exists) open the PR.
3. Never fake or hard-code benchmark results; never loosen a gate without an ADR; never disable a failing test to proceed (quarantine with an issue number and a deadline instead).
4. If blocked on a decision that belongs to the maintainer (anything listed in `docs/14` §2), record it, choose the default stated there, mark it `PROVISIONAL` in `PROGRESS.md`, and continue.
5. If blocked on something only the maintainer can supply (signing certificates, Apple/Windows accounts, access to macOS/Windows machines), finish everything around it, document exactly what is needed in `docs/manual-verification.md`, and move on.
6. Keep the working tree green: never leave `main`/the branch in a non-building state at the end of a session.
