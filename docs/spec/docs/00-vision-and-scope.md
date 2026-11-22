# 00 — Vision and Scope

## 1. What Ferrum is

Ferrum is a terminal emulator for people who keep many sessions open all day. It is built for three things, in this order:

1. **Be a great terminal.** Correct, fast, fluid, tiny.
2. **Organize work by project.** A project is a named group that holds one or more directories (typically one or more git repositories, e.g. `frontend/` + `backend/`) and all the terminal sessions belonging to it.
3. **(Phase 2) Host AI agents.** Run agent CLIs and API agents side by side, let them talk to each other, and show subscription usage. This must be additive: when disabled, it costs zero RAM and zero CPU.

## 2. Why it should exist

Pain points with Warp-style terminals reported by the maintainer: they become slow, and they ship features that are never used. Ferrum's answer is a hard resource budget and a small, deliberate feature set. **Every feature must justify its memory cost.**

## 3. Non-goals (v1.0)

- No GPU renderer. CPU rendering is a requirement for compatibility and for the memory budget.
- No account, no cloud sync, no telemetry, no AI features baked into the terminal core.
- No webview, no Electron/Tauri, no embedded scripting runtime, no plugin system.
- No block-based command UI that re-renders output as widgets (shell integration marks are enough).
- No tabs-in-titlebar; the vertical sidebar is the only tab strip.
- No light theme in v1.0 (dark themes only).

## 4. Principles

1. **Budget first.** Memory and latency numbers in `docs/03` are requirements, not aspirations. A change that breaks a gate is reverted.
2. **Terminal correctness before features.** Conformance bugs outrank new features.
3. **Boring core, sharp edges isolated.** Pure, platform-free crates hold the logic; platform code is thin and behind traits.
4. **Zero-cost abstractions or none.** No allocation in the parse loop or render loop in steady state.
5. **Local only.** Everything stays on the machine; export/import moves it to another machine.
6. **Customizable by UI.** Every setting is reachable from the UI; the file on disk is an implementation detail that is still human-readable.
7. **Small dependency surface.** Each dependency is a recurring cost (size, RAM, supply chain). See `docs/07`.

## 5. Decisions already taken (from the maintainer)

| Topic | Decision |
|---|---|
| Audience | Personal first; open source (MIT OR Apache-2.0) |
| Name | **Ferrum** |
| Platforms | Linux, macOS, Windows — all from the start |
| Rendering | CPU, no GPU requirement |
| Memory | < 50 MB total with 10 sessions × 10k lines |
| Layout | Vertical tabs on the left, terminal on the right, status bar at the bottom |
| Window chrome | Custom (client-drawn) titlebar in v1.0, with a "use native titlebar" fallback setting |
| Themes | Dark only; no light theme in v1.0 |
| Typography | One variable monospace family (JetBrains Mono) for terminal and UI; hierarchy through several bold weights |
| Releases | Per-architecture installers. No Apple or Windows signing accounts yet → unsigned builds at first (see `docs/11`, `docs/14` R-16) |
| Brand | Logo and icon designed by Claude (`assets/brand/`); maintainer approves the final version |
| Customization | Maximum, entirely through the UI: rename tabs, reorder, split, move, themes, fonts, keybindings |
| Projects | A project is a directory set (front+back or single); sessions are grouped under it |
| Sessions | Persist when the UI closes (background daemon) |
| Input niceties | Easy copy/paste, image paste, drag & drop of files |
| Persistence | Local; one-file export/import to move to another PC |
| Telemetry | None |
| Plugins | Not needed |
| Distribution | Auto-update + installers for all three platforms |
| Agents / usage meters | **Phase 2.** Optional; the terminal must not depend on them |
| Time pressure | None; quality over speed |

## 6. Scope by release

### v1.0 — "The terminal"
Terminal emulator + daemon-backed persistent sessions + projects + vertical tabs + splits + full UI customization + export/import + auto-update + packaging for 3 OS. Status bar shows project/branch/cwd and Ferrum's own live memory.

### v1.x — Quality of life
Search in scrollback, command palette polish, shell-integration prompt navigation, image protocol (kitty graphics) polish, more theme importers, accessibility (AccessKit) investigation.

### v2.0 — "The orchestra" (Phase 2)
Agent sessions (CLI + API), provider accounts and keys vault, usage meters in the status bar, inter-agent messaging via built-in MCP server, natural-language project generation. Designed in `docs/13`.

## 7. Success criteria

Ferrum is successful when the maintainer replaces their daily terminal with it on at least one OS for a full working week without going back, and every gate in `docs/08` is green on all three OS.
