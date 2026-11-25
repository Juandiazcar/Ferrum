# 02 — Architecture

## 1. Shape of the system

```
┌──────────────────────────────┐         local IPC (UDS / named pipe)        ┌─────────────────────────────────┐
│ ferrum  (GUI process)        │  ◄──── postcard frames, versioned ───────►  │ ferrumd (daemon, headless)      │
│                              │                                             │                                 │
│  winit window / events       │   Attach / Detach / Input / Resize /        │  SessionManager                 │
│  ferrum-ui   (widgets/layout)│   Scroll / Search / Snapshot / Damage       │   ├─ Session #1  PTY ─ VT ─ SB  │
│  ferrum-render (CPU raster)  │                                             │   ├─ Session #2  PTY ─ VT ─ SB  │
│  viewport cache (visible only)│                                            │   └─ …                          │
│  config + project model (ro) │                                             │  Project/Layout store (authoritative)│
└──────────────────────────────┘                                             └─────────────────────────────────┘
                                                                                      │ spawns
                                                                                      ▼
                                                                          shells / agent CLIs (children)
```

- **`ferrumd` is authoritative** for sessions, terminal state (grid + scrollback), projects, and layouts. It parses PTY output, so state survives UI exit.
- **`ferrum` is a thin renderer and input source.** It holds only what is visible (viewport rows) plus caches that are bounded.
- Two separate executables so the daemon never links windowing/graphics libraries (saves several MB of resident shared libraries).

### Why the daemon parses (and not the UI)

If the UI parsed raw bytes, reattaching would require replaying the whole byte stream, and background sessions would cost UI memory and CPU. With the daemon parsing, the UI receives *damage* for what it shows and ignores the rest. This is the tmux model, with the renderer on the other side of a socket.

## 2. Crates (summary; layout in `docs/04`)

| Crate | Kind | Responsibility | Platform-free | `unsafe` |
|---|---|---|---|---|
| `ferrum-vt` | lib | VT state machine on top of `vte`: grid, modes, charsets, selection model, hyperlinks, shell marks, image placements | yes | forbidden |
| `ferrum-scrollback` | lib | Compact line encoding, style interning, chunked LZ4 compression, disk spill, search index hooks | yes (spill behind trait) | forbidden |
| `ferrum-proto` | lib | IPC message types, framing, protocol versioning, row-damage encoding | yes | forbidden |
| `ferrum-pty` | lib | `PtyBackend` trait; Unix and ConPTY implementations; child process lifecycle | no | allowed, audited |
| `ferrum-daemon` | lib+bin(`ferrumd`) | Session manager, event loop, client server, persistence of layouts | no | forbidden |
| `ferrum-render` | lib | Font loading, glyph rasterization + cache, text layout for cells, blitters, damage-limited painting into a pixel buffer | yes | allowed in blit kernels (SIMD), audited |
| `ferrum-ui` | lib | Widget toolkit (minimal), layout engine, sidebar/splits/status bar/settings/palette, input mapping, theme application | yes | forbidden |
| `ferrum-config` | lib | Settings model, TOML (de)serialization, schema versions + migrations, export/import archive | yes | forbidden |
| `ferrum-project` | lib | Project model, `.git/HEAD` reader, cwd tracking logic, startup recipes | yes | forbidden |
| `ferrum-platform` | lib | Clipboard, drag-drop paths, default shells, directories, single-instance, own-RSS query, autostart of daemon | no | allowed, audited |
| `ferrum-app` | bin(`ferrum`) | winit glue, event loop, wiring of everything above | no | forbidden |
| `ferrum-testkit` | dev lib | Fixtures, golden harness, PTY test driver, fake clock | yes | forbidden |
| `xtask` | bin | `cargo xtask check|bench|memcheck|dist|…` | — | forbidden |

Dependency rule (enforced by `cargo xtask check-deps`): the exact allowed edges are the table in `docs/04` §2. In short: foundations → core → engines (`daemon`, `ui`) → composition roots (`ferrum-app`, `ferrumd` main). `platform` and `pty` are leaf services consumed through traits. **No cycles. The pure crates never import platform code.**

## 3. Terminal state model (`ferrum-vt`)

### 3.1 Live grid
- `Cell` is 16 bytes: `ch: u32` (scalar or index into a per-grid grapheme table), `fg: u32`, `bg: u32`, `attrs: u16`, `flags: u16`.
- Rows are `Vec<Cell>` in a ring buffer sized to the visible rows only (typically 30–70 rows × 80–250 cols → 40–280 KB per grid). Alt screen has its own grid, allocated lazily.
- Line flags: `wrapped` (soft wrap, needed for reflow), `prompt_mark`, `command_start/end` (OSC 133).

### 3.2 Scrollback (`ferrum-scrollback`)
When a row scrolls off the top it is **encoded and moved out of the grid** (never kept as 16-byte cells). Encoding (little-endian varints):

```
line := flags:u8 | text_len:varint | utf8_text (trailing default-blank cells trimmed)
        | run_count:varint | { style_id:varint | run_len:varint }*
```
- `style_id` indexes a per-session **style table** (fg, bg, attrs interned; typical sessions use < 200 styles).
- Wide characters and combining sequences are derived from the text at decode time (width table), not stored.
- Lines are appended to a **hot chunk** (the newest ≤ 1 024 lines, uncompressed). When full it is **sealed**: LZ4-block compressed (`lz4_flex`) and kept as a `Box<[u8]>` with an index of line offsets (delta-encoded).
- Beyond `spill_threshold` (default 50 000 lines per session) the oldest sealed chunks are written to a spill file in the run directory and dropped from RAM; reading copies one chunk back with a positional file read (no mmap, no `unsafe`: ADR 0027).
- Reflow decodes only the affected chunks, re-wraps using the `wrapped` flag, and re-encodes. Large reflows run on a worker with a time budget and yield.

### 3.3 Why not `alacritty_terminal`
Its grid stores every scrollback cell as a full cell struct (on the order of 20+ bytes per cell). 100 000 lines × 120 columns would be roughly 250–300 MB, an order of magnitude over budget, and the scroll-off path is not hookable. Ferrum therefore implements its own state machine on `vte` (parser only), and uses Alacritty's reference tests (Apache-2.0, attribution kept) as conformance fixtures. **Spike S1** validates this estimate by measuring before building on it (`docs/12`).

## 4. Protocol (`ferrum-proto`)

- Transport: Unix domain socket (Linux/macOS) / named pipe (Windows). The daemon serves it with `mio`'s own `UnixListener` / `NamedPipe` on its single I/O loop (maintainer decision Q-32, consistent with Q-31); the UI's blocking IPC thread uses `interprocess`. Owner-only permissions (see `docs/10`).
- Framing: `u32 length` + `postcard`-encoded message. Max frame 4 MiB. First frame is a handshake `{ protocol_version, client_kind, nonce }`; mismatched major versions are refused with a clear error and the UI offers to restart the daemon.
- Backward compatibility: additive changes only within a major protocol version; enums are `#[non_exhaustive]`.
- Key messages:

| Direction | Message | Notes |
|---|---|---|
| UI→D | `Attach{session_id, viewport{cols,rows}, want_scrollback_rows}` | Replies with `Snapshot` |
| UI→D | `Input{session_id, bytes}` | Keystrokes already encoded by the UI per terminal modes reported by D |
| UI→D | `Resize`, `Scroll{offset}`, `Select…`, `Search{query,opts}` | Search runs in the daemon |
| UI→D | `CreateSession/CloseSession/Restart`, `CreateProject/Update/Move/Reorder`, `SaveLayout` | Mutations are authoritative in D, echoed as events |
| D→UI | `Snapshot{rows, cursor, modes, title, …}` | Full screen for a viewport |
| D→UI | `Damage{session_id, rows:[{y, encoded_row}], cursor, scroll_delta}` | Incremental, coalesced per frame interval |
| D→UI | `SessionEvent{Title, Bell, CwdChanged, Exited, Activity, Progress}` | Sent for all sessions, tiny |
| D→UI | `ProjectEvent{…}`, `LayoutEvent{…}` | State changes |

- Only **visible** sessions get `Damage`. Others get `SessionEvent`s only. When a session becomes visible the UI sends `Attach` and receives a fresh `Snapshot`.
- Backpressure: the daemon coalesces damage and drops intermediate frames to the latest per session; it never queues unbounded data for a slow client.

## 5. Threading model

**Daemon** (no async runtime; `polling`/`mio`-based loop keeps RAM and binary small):
- 1 **I/O thread**: polls all PTY fds + client sockets. Reads PTY output in chunks (≤ 64 KiB), hands bytes to the owning session's parser *inline* (the parser is cheap), applies per-session fairness quantum (e.g. ≤ 256 KiB per session per loop turn) so a flooding session cannot starve others. **Windows:** the `PtyBackend` serves the output of every ConPTY session with overlapped I/O on one I/O thread of its own and wakes this loop when a session's output is ready (`docs/11` §4, maintainer decision Q-31); there is no thread per session, and the daemon's session logic is identical on all OSes because it only sees "chunk readable" events through the `PtyBackend` trait.
- Child exit (maintainer decision Q-33): Windows: process exit is watched by a named `pty-exit` thread (WaitForMultipleObjects, one thread per 63 sessions); Unix: through mio on the loop.
- 1 **worker thread** for CPU-heavy, interruptible work: reflow, search, compression of sealed chunks, spill I/O, layout persistence. Communicates through bounded channels.
- Per-session parser state is not shared across threads; sessions are owned by the I/O thread. Compression/search receive immutable chunk handles.

**UI**:
- Main thread: winit event loop, input → protocol, damage → viewport cache → paint. Paint only on demand (event-driven; no continuous redraw).
- 1 **IPC thread**: blocking reads from the daemon socket, decodes frames, forwards via `EventLoopProxy`. No other threads unless profiling proves a need.

Rules: no locks in hot paths where ownership transfer works; bounded channels only; every thread named; no `static mut`; `tracing` spans for diagnostics (compiled out of release at `level = off` unless enabled).

## 6. Rendering (`ferrum-render`, CPU)

- Output target: a `&mut [u32]` (BGRA/ARGB per platform) provided by `softbuffer`. **No intermediate full-window pixmap** (avoids a second framebuffer). Chrome (rounded rects, shadows, gradients) uses `tiny-skia` on small, cached tiles; terminal text bypasses it with a purpose-built blitter.
- **Glyph cache**: key = (font_id, glyph_id, size_q, subpixel_q) → 8-bit coverage mask (or BGRA for color emoji) in a bounded LRU arena (default 3 MB). Rasterization and shaping with `swash`; fonts memory-mapped.
- **Damage tracking**: each viewport row carries a content hash; the renderer repaints only rows whose hash changed plus cursor/selection overlays, and passes the dirty rect to `softbuffer::Buffer::present_with_damage`.
- **Frame pacing**: render at most once per display refresh interval and only when something is dirty. During floods, parse continues at full speed while painting is capped (default 60 Hz; adaptive down to 30 Hz under sustained flood).
- Subpixel (LCD) antialiasing: off by default, grayscale AA default; optional LCD text on opaque backgrounds only.
- HiDPI: glyph size derived from scale factor; cache keyed by effective pixel size.

## 7. UI layer (`ferrum-ui`)

- Retained widget tree, but intentionally tiny: `Sidebar`, `ProjectGroup`, `SessionItem`, `SplitView`, `TerminalView`, `StatusBar`, `CommandPalette`, `SettingsPanels`, `Dialog`, `ContextMenu`, `TextInput`, `Toggle`, `Slider`, `Dropdown`, `ColorPicker`, `KeyCapture`.
- Pure functions: `layout(constraints) -> rects`, `paint(widget, painter)`, `handle(event) -> Vec<Action>`. No widget touches the network, files, or the socket; they emit `Action`s that `ferrum-app` interprets (Command pattern). This makes UI logic unit-testable without a window.
- Theming: a `Theme` value (colors, fonts, spacing, radii) is passed down; no globals.
- Text input widgets support IME preedit, selection, clipboard, undo (single level is enough).

## 8. Configuration and persistence

| Data | Owner | Location (per-user) | Format |
|---|---|---|---|
| Settings, keybindings, themes | UI writes, daemon reads shell profiles | `config_dir/ferrum/` | TOML (`settings.toml`, `keybindings.toml`, `themes/*.toml`) |
| Projects + layouts | **daemon** (authoritative) | `data_dir/ferrum/projects/*.toml` | TOML, atomic writes (write temp + fsync + rename) |
| Run files (socket, lock, spill, image temp) | daemon | `runtime_dir/ferrum/` | — |
| Window state (size, sidebar width) | UI | `state_dir/ferrum/window.toml` | TOML |

- All writes are atomic. All reads tolerate unknown keys (forward compat) and report errors in the UI without crashing.
- `schema_version` per file; migrations are pure functions with golden tests.

## 9. Extension points reserved for Phase 2 (no code in v1.0 except the seams)

- `SessionKind` enum (`Terminal` now; `Agent{…}` later) in `ferrum-proto` and `ferrum-daemon`.
- `StatusItem` trait objects in the status bar so Phase 2 can add meters without touching core.
- `ferrum-daemon` exposes an internal `EventBus` (typed, in-process) that Phase 2 crates subscribe to; in v1.0 it only carries session events.
- Protocol reserves message ranges for agent/usage/vault.
- Phase 2 code lives in new crates (`ferrum-agents`, `ferrum-providers`, `ferrum-usage`, `ferrum-vault`, `ferrum-mcp`) behind **runtime-enabled** composition in `ferrum-app`/`ferrum-daemon`. If disabled they are never instantiated.

## 10. Failure behavior

| Failure | Behavior |
|---|---|
| UI crashes | Daemon keeps sessions; next launch re-attaches |
| Daemon crashes | UI shows "reconnecting"; layouts restored from disk; shells respawned in last known cwd (processes are lost — documented) |
| One session's parser hits a bug | Panic isolated with `catch_unwind` at the session boundary; the session is marked failed with the error; others continue |
| Socket peer disappears | Daemon drops the client; UI retries with backoff |
| Disk full / spill fails | Spill disabled for that session; oldest lines are dropped; user notified once |
| Corrupt config file | Renamed to `*.bad-<timestamp>`, defaults loaded, UI notification with path |
