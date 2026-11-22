# 09 — UI / UX Specification

Mockups are produced separately (`docs/design/`) and, where they differ from this file in visual details, the mockups win; behavior in this file wins.

## 1. Main window

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ ▍ferrum · Helpdesk                      ⌕ Search or run a command…     ─  ▢  ✕ │  ← custom titlebar (drag region)
├────────────────────┬──────────────────────────────────────────────────────────┤
│ PROJECTS           │  ● api  ·  ~/work/helpdesk/backend                       │  ← (optional) pane header per split
│ ───────────────────│──────────────────────────────────────────────────────────│
│ ▾ ◼ Helpdesk   3   │                                                          │
│   ● backend  main  │   $ cargo run                                            │
│   ○ frontend dev   │   …                                                      │
│   ○ logs           │                                                          │
│ ▾ ◼ Nokai      2   │                                                          │
│   ○ server         │                                                          │
│   ○ scratch        │                                                          │
│ ▸ ◼ CoreLab    1   │                                                          │
│                    │                                                          │
│ ＋ New project     │                                                          │
├────────────────────┴──────────────────────────────────────────────────────────┤
│ Helpdesk ⎇ main   ~/work/helpdesk/backend   4 sessions        ferrum 38 MB    │  ← status bar
└───────────────────────────────────────────────────────────────────────────────┘
```

- **Sidebar (left)**: default 248 px, resizable 160–420 px, collapsible to a 48 px icon rail (`Ctrl/Cmd+B`). Holds projects (groups) and sessions (items).
- **Terminal area (right)**: the active project's layout (tree of splits). One pane has focus (accent border, 1–2 px).
- **Status bar (bottom)**, left→right: project name · git branch · active session cwd · session count · *spacer* · Phase-2 slots (model usage meters) · Ferrum's own memory (live, 2 s refresh only while the window is visible).
- No tab strip at the top. No titlebar tabs.

### 1.1 Titlebar (custom, FR-UI-012)

- Height 36 px (38 px on macOS). Contents: app mark + project name on the left, a compact *Search or run a command* button (opens the palette) in the center-right, window controls on the right (Windows/Linux). On macOS the native traffic lights stay at the left (transparent titlebar, full-size content view) and the mark/project name follow them.
- The empty area is the drag region; double-click toggles maximize; right-click opens the window menu where the platform supports it.
- Edge and corner resize hit areas of 6 px; 1 px border in the theme's border color; no custom shadow (the compositor's own is used when available).
- Window controls are drawn by Ferrum (icon-only, 46×36 px hit areas on Windows/Linux, close turns danger-red on hover) and are reachable by keyboard (`Alt+Space` window menu on Windows/Linux).
- **Fallback:** Settings → Window → *Titlebar style: Custom / Native*. A launch flag `--native-titlebar` and automatic fallback when the compositor lacks required protocols (see `docs/11` §4.1).

## 2. Sidebar behavior

### Project group header
- Disclosure triangle, color chip (project color), name, session count, `…` menu on hover.
- Click toggles collapse; double-click or `F2` renames inline; right-click/`…` menu: Rename, Change color/icon, Add directory, Open directories in new session, Duplicate layout, Close all sessions, Remove project.
- Reorder by dragging the header; drop indicator is a 2 px accent line.

### Session item
- Status dot (running / idle-with-activity / bell / exited / failed), name (editable), optional right-aligned chips: git branch (if different from project default), running-command hint (from OSC 133 / title), exit code if exited.
- Click focuses (shows it in the active pane); middle-click closes; `F2`/double-click renames; drag to reorder or to another project; drop on the terminal area to place it into a split zone (left/right/top/bottom/center swap).
- Hover tooltip: full name, cwd, profile, PID, uptime.
- Name resolution order: user-set name → title set by the program (OSC 0/2, if "use program title" is on) → profile name + cwd basename.

### Empty states
- No projects: a centered card "Create your first project" with *Choose folder(s)…* and *Quick terminal (no project)* buttons. A synthetic project "Scratch" holds quick terminals.

## 3. Creating projects (v1.0)

1. `＋ New project` (or drop one/more folders on the sidebar, or palette: *New project*).
2. Dialog: name (prefilled from the first folder), directories list (add/remove/reorder), color, icon. Detected git repos show their branch.
3. Optional **startup recipe**: list of sessions to open (name, profile, cwd root, startup command). Presets: "One terminal", "Front + back (two terminals side by side)".
4. Creates project, opens its sessions, focuses the first.

Phase 2 adds "Describe your project…" (natural language) to generate roots + recipe + agents (`docs/13`).

## 4. Splits and layout

- Split right / down; resize by dragging the divider (4 px hit area, 1 px visible), double-click divider to equalize; keyboard resize.
- Move pane: drag its header grip or its sidebar item onto another pane's edge zone.
- Zoom pane (toggle): fills the area; sidebar remains.
- Layout persisted per project; opening a project restores it.
- Min pane size: 10 cols × 3 rows; below that the split refuses with a subtle shake feedback.

## 5. Terminal view details

- Cursor, selection, hyperlinks (underline on Ctrl/Cmd hover), search overlay (top-right of the pane, `Ctrl+Shift+F`/`Cmd+F`), scrollbar (thin overlay, appears on scroll/hover; shows search hits and prompt marks).
- Scroll: wheel, `Shift+PageUp/Down`, `Ctrl+Shift+Up/Down` jump to previous/next prompt (needs shell integration), "scroll to bottom" chip when scrolled up and new output arrives.
- Visual bell: 120 ms border pulse + sidebar dot; no sound by default.
- Padding configurable (default 8 px horizontal, 6 px vertical). Pane background may differ slightly by project color tint (optional, default off).
- Selection copy: `Ctrl+Shift+C` / `Cmd+C`; paste: `Ctrl+Shift+V` / `Cmd+V`; Linux: primary selection + middle-click; Windows: `Ctrl+V` pastes when no TUI has grabbed it (configurable "smart copy: Ctrl+C copies if selection exists, else sends SIGINT").
- Multiline paste confirmation (on by default when pasting > 1 line into a non-bracketed-paste application).
- Drop files: shell-quoted absolute paths separated by spaces, inserted as paste. Quoting follows the session's shell profile (POSIX single quotes, PowerShell single quotes with escaping, cmd double quotes).
- Paste image from clipboard: PNG saved to `runtime_dir/clips/<timestamp>.png` (auto-cleaned after 24 h or on daemon exit) and its path inserted; setting to disable or change dir.

## 6. Command palette and quick switch

- Palette (`Ctrl/Cmd+Shift+P`): fuzzy search over every action registered in `ferrum-ui`'s action table, showing the current shortcut. Executes immediately, supports arguments via a second step (e.g., *Rename session* → text input).
- Quick switch (`Ctrl+Shift+O` / `Cmd+P`): fuzzy over projects and sessions; Enter focuses; `Ctrl/Cmd+Enter` opens in a split.

## 7. Default shortcuts

Principle: on Linux/Windows, app shortcuts use `Ctrl+Shift+…` so they never collide with TUIs; on macOS, `Cmd+…`. All are rebindable in Settings → Keybindings; a *pass-through* toggle (`Ctrl+Shift+Esc` / `Cmd+Opt+Esc`) sends every key to the terminal for a pane that needs it.

| Action | Linux / Windows | macOS |
|---|---|---|
| New session (current project) | Ctrl+Shift+T | Cmd+T |
| New project | Ctrl+Shift+N | Cmd+Shift+N |
| Close session | Ctrl+Shift+W | Cmd+W |
| Next / previous session | Ctrl+Tab / Ctrl+Shift+Tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Go to session 1–9 | Alt+1…9 | Cmd+1…9 |
| Next / previous project | Ctrl+Alt+Down / Up | Cmd+Opt+Down / Up |
| Split right / down | Ctrl+Shift+D / Ctrl+Shift+E | Cmd+D / Cmd+Shift+D |
| Focus pane | Alt+Arrows | Cmd+Opt+Arrows |
| Zoom pane | Ctrl+Shift+Z | Cmd+Shift+Enter |
| Rename session | F2 | Enter (on focused sidebar item) / F2 |
| Command palette | Ctrl+Shift+P | Cmd+Shift+P |
| Quick switch | Ctrl+Shift+O | Cmd+P |
| Find in scrollback | Ctrl+Shift+F | Cmd+F |
| Copy / Paste | Ctrl+Shift+C / V | Cmd+C / V |
| Font bigger / smaller / reset | Ctrl+= / Ctrl+- / Ctrl+0 | Cmd+= / Cmd+- / Cmd+0 |
| Toggle sidebar | Ctrl+Shift+B | Cmd+B |
| Settings | Ctrl+, | Cmd+, |
| Fullscreen | F11 | Ctrl+Cmd+F |
| Prompt navigation | Ctrl+Shift+Up/Down | Cmd+Up/Down |

## 8. Settings (all in the UI)

Layout: left list of sections, right panel with controls, search box at the top that filters settings across sections. Changes apply live. A *Reset section* and *Reset all* exist. Each setting has a one-line description.

| Section | Settings |
|---|---|
| **Appearance** | Theme (list + preview; dark themes only), font family (system list + embedded), size, line height, letter spacing, bold-as-bright, ligatures, cursor style/blink/color, window padding, sidebar width/density, UI scale, project color tint, animations on/off, reduced motion, opacity (v1.x) |
| **Terminal** | Scrollback lines (with live RAM estimate), scroll speed, copy-on-select, smart copy, paste confirmation, bracketed paste, word characters, URL schemes allowed, bell (visual/audible/off), Option-as-Alt (macOS: left/right/both/none), kitty keyboard protocol on/off, OSC 52 policy (deny / write-only / ask), title reporting (off), image support (off / on with cache cap) |
| **Shells & profiles** | Default profile, list of profiles (name, command, args, env, cwd, icon), auto-detected profiles, shell integration on/off per profile |
| **Projects** | Default roots behavior, startup recipes, recent projects, "restore last layout on launch" |
| **Keybindings** | Searchable list; capture widget; conflict detection; import/export; reset |
| **Window** | Titlebar style (custom/native), sidebar default state, status bar items on/off, multi-window behavior, close behavior (keep daemon sessions: ask/always/never) |
| **Performance & memory** | Max FPS (30/60/120/auto), spill threshold, glyph cache size, image cache cap, "show live memory in status bar", diagnostics (open log, copy diagnostics) |
| **Import / Export** | Export `.ferrum` (choose contents), Import (merge/replace), Import theme (Alacritty/iTerm2/Windows Terminal/Kitty/base16) |
| **Updates** | Check automatically, channel (stable/beta), check now |
| **About** | Version, license, third-party notices |
| *(Phase 2)* **Agents & Providers** | Accounts, keys vault, usage meters, agent templates (`docs/13`) |

Config file backing exists (`settings.toml`) but is documented as an implementation detail; "Open config folder" is available under Advanced.

## 9. Theming

- Theme = TOML: `[meta] name/author/variant`, `[colors] foreground, background, cursor, cursor_text, selection_bg, selection_fg, ansi = [16 colors], bright_*`, `[ui] sidebar_bg, sidebar_fg, accent, border, status_bg, status_fg, danger, warning, success`, optional `[fonts]`.
- Built-in themes (v1.0, **all dark**): **Ferrum Dark** (default), **Ferrum Midnight** (cooler, deeper blue-black), Solarized Dark, Gruvbox Dark, Nord, Dracula, Catppuccin Mocha, Tokyo Night, One Dark. Check each theme's license before bundling; reimplement palettes only from permissively licensed sources.
- Design tokens (starting point; finalize from mockups): accent = rust orange `#E8590C`; Ferrum Dark background `#101214`, sidebar `#0B0D0F`, foreground `#D7DAE0`; contrast ratios ≥ 4.5:1 for text.
- **Typography (decided):** one embedded variable monospace family (JetBrains Mono) for both the terminal and the UI chrome, so only one font file is resident. Hierarchy comes from **weight steps**, not from a second family: 400 body/terminal, 500 session names, 600 labels and secondary headings, 700 project names and titles, 800 emphasis (brand wordmark, primary buttons). Sizes follow a small scale (11/12/13/15/18 px at 1× UI scale); italics only for terminal content. Variable-font axes are rasterized by `swash`; glyph cache keys include the weight.
- Spacing scale 4 px; corner radius 6 px; one elevation shadow style (cached tile).

## 10. Interaction and motion

- All transitions ≤ 120 ms and **optional**; no continuous animations; spinners only while a process is running and the window is focused (1 Hz tick, a single timer).
- Hover/pressed/focus states on every interactive element; focus ring visible for keyboard navigation.
- Everything reachable by keyboard; Tab order is logical; `Esc` closes overlays.

## 11. Accessibility and i18n

- v1.0: high-contrast themes, UI scale 75–200 %, reduced motion, keyboard-complete UI, IME correctness.
- v1.x: AccessKit integration evaluation (screen readers) — tracked as FR-UI-017.
- UI strings in a single `strings` module (English), structured for later localization (Spanish first). No string concatenation for sentences.

## 12. Error and edge-case UX

- Shell fails to start → the pane shows an inline error card (reason, command, "Edit profile", "Retry").
- Session exited → dim terminal, banner "Process exited (code N) — Restart · Close".
- Daemon unreachable → non-blocking bar "Reconnecting…", sessions shown frozen; automatic retry with backoff.
- Config corrupt → notification with "Open backup folder".
- Very large paste (> 1 MB) → confirmation with size.
