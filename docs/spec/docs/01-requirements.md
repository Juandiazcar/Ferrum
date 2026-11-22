# 01 — Requirements

Priority: **M** = must for v1.0 · **S** = should for v1.0 (cut only with an ADR) · **C** = could (v1.x) · **P2** = Phase 2.

Every requirement has an ID. Tests and PRs reference IDs (e.g. `Refs: FR-TERM-012`).

## 1. Terminal emulation (FR-TERM)

| ID | Pri | Requirement |
|---|---|---|
| FR-TERM-001 | M | xterm-compatible emulation: VT100/VT220/VT520 core, xterm-256color, `TERM=xterm-256color` by default, optional `xterm-ferrum` terminfo shipped for extra capabilities |
| FR-TERM-002 | M | True color (24-bit), 256-color, 16-color palettes with theme-defined ANSI colors |
| FR-TERM-003 | M | Unicode: grapheme clusters, wide (CJK) characters, combining marks, emoji (color glyphs), correct width tables (`unicode-width`) |
| FR-TERM-004 | M | Alternate screen, scroll regions, origin mode, insert/replace mode, DECSCUSR cursor styles, line drawing charsets |
| FR-TERM-005 | M | Mouse reporting: X10, normal, button-event, any-event; SGR (1006) encoding; focus events (1004) |
| FR-TERM-006 | M | Bracketed paste (2004) with sanitization (see `docs/10`) |
| FR-TERM-007 | M | Synchronized output (2026) to avoid tearing during redraws |
| FR-TERM-008 | M | Kitty keyboard protocol (progressive enhancement) and legacy encodings; correct modifier handling incl. macOS Option-as-Alt setting |
| FR-TERM-009 | M | OSC support: 0/1/2 (title), 4/10/11/12 (colors, query+set), 7 (cwd), 8 (hyperlinks), 52 (clipboard, permission-gated), 133 (prompt/command marks), 1337 `File=` inline images (subset), 9;4 progress |
| FR-TERM-010 | M | Window resize with **reflow** of wrapped lines, including scrollback |
| FR-TERM-011 | M | Scrollback: configurable (default 10 000 lines, min 0, max 1 000 000); beyond 50 000 lines cold history spills to disk (mmap) |
| FR-TERM-012 | S | Kitty graphics protocol (subset: PNG/RGBA transmit, place, delete) with bounded image cache |
| FR-TERM-013 | S | Sixel (decode to the same bounded cache) |
| FR-TERM-014 | M | Selection: click, double-click (word), triple-click (line), rectangular (Alt+drag), shift-extend, auto-scroll while dragging |
| FR-TERM-015 | M | Search in scrollback (case, regex toggles), highlights, next/prev; implemented in the daemon so cold history is searchable without inflating UI memory |
| FR-TERM-016 | M | URL/path detection: Ctrl/Cmd+click opens hyperlinks (allow-listed schemes); detected file paths with `:line:col` can open in `$EDITOR` command template |
| FR-TERM-017 | M | Cursor: block/underline/bar, blink (configurable), unfocused-hollow |
| FR-TERM-018 | M | Bell: visual (tab indicator + subtle flash), audible optional, urgency hint optional |
| FR-TERM-019 | S | Ligatures (off by default, toggle per font); no cost when off |
| FR-TERM-020 | M | Conformance: passes the checklist in `docs/08` §6 (vttest subset + imported ref-tests) |

## 2. Sessions (FR-SESS)

| ID | Pri | Requirement |
|---|---|---|
| FR-SESS-001 | M | Sessions live in the daemon (`ferrumd`); closing the UI does not kill them |
| FR-SESS-002 | M | UI re-attaches to existing sessions at launch with full screen + scrollback restored |
| FR-SESS-003 | M | The daemon starts automatically on demand, exits when it has zero sessions and zero clients after a grace period (default 10 s) |
| FR-SESS-004 | M | Shell profiles: auto-detected (login shell, bash, zsh, fish, pwsh, cmd, WSL distros, Git Bash) + custom command, args, env, cwd |
| FR-SESS-005 | M | Shell integration injected for bash/zsh/fish/pwsh (opt-out): emits OSC 7 and OSC 133 so cwd and prompts are known |
| FR-SESS-006 | M | Session status: running / exited(code) / killed; exited sessions stay visible until closed, with "restart" action |
| FR-SESS-007 | M | Per-session environment: `TERM`, `COLORTERM=truecolor`, `FERRUM=1`, `FERRUM_SESSION_ID`, `FERRUM_PROJECT_ID` |
| FR-SESS-008 | S | After a daemon crash/OS reboot, the layout (not the processes) is restored and shells are respawned in their last cwd |
| FR-SESS-009 | M | A flood in one session never stalls others or the UI (per-session budgets, backpressure on that PTY only) |

## 3. Projects (FR-PROJ)

| ID | Pri | Requirement |
|---|---|---|
| FR-PROJ-001 | M | A project has: id, name (renamable), color, icon (glyph), 1..N root directories, ordered sessions, layout |
| FR-PROJ-002 | M | Create project from the UI by choosing one or more directories (picker or drag a folder onto the sidebar); a name is suggested from the directory |
| FR-PROJ-003 | M | New session in a project starts in a chosen root (default: first root); sessions can be moved between projects by drag & drop |
| FR-PROJ-004 | M | Git awareness without a git library: read `.git/HEAD` (also worktrees and `gitdir:` files) to show the branch; refresh on focus/cwd change; no background polling |
| FR-PROJ-005 | M | A project has a **layout**: a tree of splits whose leaves are sessions; layouts persist |
| FR-PROJ-006 | S | "Quick switch" (Ctrl/Cmd+P) over projects and sessions with fuzzy search |
| FR-PROJ-007 | S | Optional per-project startup recipe: list of sessions (name, profile, cwd, startup command) created when the project is opened |
| FR-PROJ-008 | P2 | Natural-language project generation (agents) |

## 4. UI (FR-UI)

| ID | Pri | Requirement |
|---|---|---|
| FR-UI-001 | M | Vertical sidebar (left): projects as collapsible groups, sessions as items; collapsible to an icon rail |
| FR-UI-002 | M | Rename project/session in place (double-click or F2); names persist |
| FR-UI-003 | M | Drag & drop to reorder sessions/projects and to move sessions across projects |
| FR-UI-004 | M | Splits inside a project view: horizontal/vertical, resize by dragging, keyboard resize, swap/move panes, zoom a pane |
| FR-UI-005 | M | Status bar (bottom): project, branch, cwd, session count, Ferrum live memory; extensible item slots for Phase 2 meters |
| FR-UI-006 | M | Command palette (Ctrl/Cmd+Shift+P) for every action |
| FR-UI-007 | M | Settings entirely in UI (see `docs/09`): appearance, terminal, shells, keybindings, projects, import/export |
| FR-UI-008 | M | Themes: built-in dark set + user themes (no light theme in v1.0); import from Alacritty, iTerm2 (.itermcolors), Windows Terminal, Kitty, base16 formats |
| FR-UI-009 | M | Copy/paste: Ctrl+Shift+C/V (Linux/Windows), Cmd+C/V (macOS), copy-on-select option, middle-click paste on Linux, multiline-paste confirmation option |
| FR-UI-010 | M | Drag & drop of files/folders onto a terminal inserts shell-quoted paths (per shell) using bracketed paste |
| FR-UI-011 | M | Paste an **image** from the clipboard: saved to a temp file under the Ferrum run dir and its path inserted; configurable |
| FR-UI-012 | M | **Custom (client-drawn) titlebar** on all OSes: drag region, double-click to maximize, minimize/maximize/close controls, edge and corner resize; macOS keeps the native traffic lights. Setting "Use native titlebar" is the fallback (`docs/09` §1.1, `docs/11` §4.1) |
| FR-UI-013 | M | HiDPI: per-monitor scale factors, crisp text, fractional scaling |
| FR-UI-014 | M | Keyboard-first: every UI action has a rebindable shortcut; full mouse support too |
| FR-UI-015 | S | Pane/tab activity and bell indicators in the sidebar; "process running" spinner dot |
| FR-UI-016 | M | IME support (preedit rendering) on all platforms |
| FR-UI-017 | C | Accessibility via AccessKit |
| FR-UI-018 | M | Multiple windows: a window shows one project at a time; multiple windows may show different projects |

## 5. Data and portability (FR-DATA)

| ID | Pri | Requirement |
|---|---|---|
| FR-DATA-001 | M | Config + projects + layouts + themes + keybindings are stored in per-user directories (XDG / Library / AppData) as TOML |
| FR-DATA-002 | M | **Export**: single `.ferrum` archive (tar+zstd) with settings, projects, layouts, themes; optional "include scrollback" |
| FR-DATA-003 | M | **Import**: merge or replace; paths remapped when roots differ (UI prompts) |
| FR-DATA-004 | M | Config schema is versioned; migrations are automatic and tested |
| FR-DATA-005 | M | No network access in v1.0 except the update check (FR-DIST-002) |

## 6. Distribution (FR-DIST)

| ID | Pri | Requirement |
|---|---|---|
| FR-DIST-001 | M | Installers/packages: Linux (.deb, .rpm, AppImage, tar.gz), macOS (.dmg per architecture: arm64 and x86_64), Windows (.msi + portable .zip) |
| FR-DIST-002 | S | Update check against GitHub Releases at most once per 24 h, user-disableable; signature-verified download; applied on restart. Package-manager installs defer to the package manager |
| FR-DIST-004 | M | Until signing accounts exist, releases are unsigned (macOS/Windows); install docs explain the Gatekeeper/SmartScreen override and publish SHA-256 sums; package-manager channels (Homebrew cask, Scoop, winget) are the recommended path |
| FR-DIST-003 | S | winget, Homebrew cask, Scoop, AUR manifests maintained in-repo |

## 7. Non-functional requirements (NFR)

| ID | Pri | Requirement |
|---|---|---|
| NFR-MEM-01 | M | **< 50 MB** combined PSS (`ferrum` + `ferrumd`) with 10 sessions × 10 000 scrollback lines, reference scenario in `docs/03` |
| NFR-MEM-02 | M | Idle daemon with 1 empty session < 8 MB; idle UI with 1 session < 22 MB at the reference window size |
| NFR-MEM-03 | M | Memory is returned: after closing 9 of 10 loaded sessions, combined PSS drops by ≥ 70 % of those sessions' share within 10 s |
| NFR-MEM-04 | M | HiDPI honesty: at scale factor ≥ 1.5, combined memory **minus window-surface bytes** < 42 MB; both numbers are reported (`docs/03` §5) |
| NFR-PERF-01 | M | Keystroke-to-frame-ready latency (in-process, no display scan-out) p50 ≤ 4 ms, p99 ≤ 10 ms on the CI reference machine |
| NFR-PERF-02 | M | Parse throughput ≥ 400 MB/s on plain ASCII, ≥ 120 MB/s on SGR-heavy output, single core, reference machine |
| NFR-PERF-03 | M | Visible redraw of a full 200×60 screen ≤ 2.5 ms CPU (reference window) |
| NFR-PERF-04 | M | 20 sessions open, one running `yes` at full speed: other sessions' echo latency unchanged (p99 ≤ 10 ms), UI stays ≥ 60 fps when visible |
| NFR-PERF-05 | M | Cold start to first frame ≤ 120 ms (daemon already running), ≤ 250 ms (daemon cold) |
| NFR-PERF-06 | M | CPU when idle: 0.0 % (no timers firing when nothing changes; cursor blink only when focused and enabled) |
| NFR-SIZE-01 | S | Release binaries: `ferrum` ≤ 6 MB, `ferrumd` ≤ 3 MB (stripped) |
| NFR-REL-01 | M | The daemon never panics on any input byte stream (fuzz-verified); a bug in one session cannot take down other sessions |
| NFR-REL-02 | M | UI crash does not lose sessions; daemon crash is recoverable per FR-SESS-008 |
| NFR-SEC-01 | M | See `docs/10`: hardened escape sequences, owner-only IPC, no telemetry |
| NFR-PORT-01 | M | Identical behavior on Linux/macOS/Windows except where `docs/11` documents a platform difference |
| NFR-QUAL-01 | M | `cargo clippy -D warnings`, `cargo fmt --check`, `cargo deny check`, all tests green on all three OS before merge |
| NFR-DOC-01 | M | Every public item documented; every crate has a README describing responsibility and dependencies |

## 8. Phase 2 placeholders (not built in v1.0, but nothing may block them)

| ID | Requirement |
|---|---|
| FR-AGENT-001 | Agent sessions: a session of kind `Agent` running a CLI agent in a PTY, or an API agent with its own chat view |
| FR-AGENT-002 | Provider accounts: API keys for OpenRouter, Anthropic, OpenAI-compatible endpoints (MiniMax, Qwen, Z.ai/GLM, DeepSeek, etc.); official CLIs launched as-is |
| FR-AGENT-003 | Usage meters per provider with a precision label (exact / estimated / unknown) |
| FR-AGENT-004 | Inter-agent messaging through a built-in MCP server and a local message bus |
| FR-AGENT-005 | Encrypted key vault, exportable with a passphrase |
| FR-AGENT-006 | User-defined agent workflows (create, name, wire agents) |
| FR-AGENT-007 | Per-agent git worktrees |

See `docs/13`.
