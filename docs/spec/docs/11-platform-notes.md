# 11 — Platform Notes

Goal: identical behavior everywhere (NFR-PORT-01). Differences below are the only sanctioned ones. Platform code lives in `ferrum-platform` and `ferrum-pty` behind traits; everything else is portable.

## 1. Common

| Topic | Rule |
|---|---|
| Paths | `directories` crate: config/data/state/runtime dirs per OS; override with `FERRUM_CONFIG_DIR`, `FERRUM_DATA_DIR`, `FERRUM_STATE_DIR`, `FERRUM_RUNTIME_DIR` (tests, portable mode) |
| Portable mode | If a `ferrum-portable` marker file sits next to the executable, all dirs are subfolders of the executable directory |
| Shell detection | Login shell (`$SHELL`/`getent`/`dscl`), then installed shells from `/etc/shells`, then Windows: pwsh, Windows PowerShell, cmd, WSL distros (`wsl -l -q`), Git Bash |
| Signals / Ctrl+C | Handled by the PTY line discipline (Unix) / ConPTY (Windows); Ferrum sends the byte, never emulates signals |
| Process exit codes | Captured by the PTY backend; surfaced in the session event |
| Shell integration | Bash/zsh via sourced snippet injected with `--rcfile`/`ZDOTDIR`/`ENV` tricks documented per shell; fish via `XDG_DATA_DIRS`/`--init-command`; PowerShell via `-NoExit -Command` bootstrap. Never edit the user's rc files |
| Environment for children | `TERM=xterm-256color`, `COLORTERM=truecolor`, `TERM_PROGRAM=Ferrum`, `TERM_PROGRAM_VERSION`, `FERRUM_*` ids |

## 2. Linux

- **Display**: X11 and Wayland through `winit` + `softbuffer`. Wayland client-side decorations are provided by winit's built-in decorations; if they look poor, ship a minimal themed decoration (C-priority).
- **Memory**: Wayland `wl_shm` buffers are typically double-buffered — counted in the gate (`docs/03` §5).
- **Fonts**: `fontdb` scans `/usr/share/fonts`, `~/.local/share/fonts`, `~/.fonts`; no fontconfig dependency (fewer shared libraries resident). Preferences read from Ferrum settings; generic family fallbacks hard-coded (DejaVu Sans Mono, Noto, Liberation Mono).
- **Input**: XKB via winit; IME through winit's text-input (Wayland `text-input-v3`, X11 XIM); test with ibus and fcitx5.
- **Clipboard**: `arboard` (needs a running clipboard owner on Wayland: the daemon does not own the clipboard, the UI does; on UI exit the clipboard may vanish — acceptable, documented). Primary selection supported on both.
- **PTY**: `openpty`/`posix_openpt` via `portable-pty`/`rustix`; child spawned in a new session with controlling TTY; `TIOCSWINSZ` on resize; `SIGWINCH` implicitly.
- **cwd tracking**: OSC 7 first; fallback `/proc/<pid>/cwd` of the foreground process group leader on focus/change events only (no polling).
- **Own memory metric**: `/proc/self/smaps_rollup` (`Pss`).
- **Packaging**: `.deb` (+ `.desktop`, icons, AppStream metainfo, terminfo), `.rpm`, AppImage, `tar.gz`; AUR `PKGBUILD`; later Flatpak evaluation (sandbox conflicts with a terminal — likely not offered).
- **Autostart of the daemon**: spawned on demand by the UI as a detached child (`setsid`); optional systemd user unit is not required.
- **Min versions**: glibc 2.31+ (Ubuntu 20.04 equivalent) for official binaries; build on an old-glibc runner image.

## 3. macOS

- **Architectures**: build `aarch64-apple-darwin` and `x86_64-apple-darwin`; ship **one `.dmg` per architecture** (decided).
- **Min version**: macOS 12 (check winit/softbuffer requirements at WP-08).
- **Menus**: `muda` provides the standard app menu (About, Settings, Hide, Quit) and an Edit menu (Copy/Paste/Select All) so system shortcuts work; terminal-specific actions live in the palette to keep the menu small.
- **Keyboard**: Option-as-Alt setting (left/right/both/none) because Option composes characters by default; `Cmd` combos reserved for the app; `Ctrl` combos go to the terminal.
- **Fullscreen**: native fullscreen via winit; safe-area/notch irrelevant for windowed mode.
- **Fonts**: `fontdb` scans `/System/Library/Fonts`, `/Library/Fonts`, `~/Library/Fonts`; emoji via Apple Color Emoji (sbix bitmaps, supported by swash).
- **HiDPI**: Retina scale 2×; framebuffer cost per `docs/03` §5. Use the layer-backed softbuffer path and test memory with `vmmap`/`footprint`.
- **Memory metric**: `proc_pid_rusage` → `ri_phys_footprint`.
- **PTY**: BSD `openpty`; login shells started with `-l` argv[0] convention when the profile says "login shell".
- **Signing (decided: no Apple Developer account yet)**: builds are **ad-hoc signed** (required for arm64 binaries to run; the Rust toolchain/linker applies it) but **not Developer-ID signed or notarized**. Gatekeeper will block the first launch of a downloaded `.dmg`; install docs show the two overrides (right-click → Open, or `xattr -d com.apple.quarantine /Applications/Ferrum.app`). Homebrew cask is the recommended install path. The release pipeline contains the notarization steps behind a flag that turns on when an Apple Developer ID and `notarytool` credentials are supplied (WP-18 documents the exact secrets). Entitlements stay minimal (no JIT, no network server).
- **Gatekeeper/TCC**: the app does not request Full Disk Access; folder access prompts come from the shells it launches.
- **Secure Keyboard Entry**: optional toggle (blocks other apps from observing keystrokes); off by default.
- **Distribution**: `.dmg` with drag-to-Applications layout; Homebrew cask in a tap.

## 4. Windows

- **Min version**: Windows 10 1809 (ConPTY). Windows 11 fully supported. x86_64 and aarch64.
- **PTY**: ConPTY (`CreatePseudoConsole`) through a direct `windows-sys` backend inside `ferrum-pty` (audited `unsafe`, `docs/05` §4); `portable-pty` is used on Linux and macOS only (ADR 0026). If the direct backend cannot pass the `PtyBackend` contract suite on a supported Windows build, the fallback is `portable-pty` plus the same workarounds. Measured ConPTY behavior to handle (ADR 0026; document and test each in `ferrum-pty`): (1) **startup cursor query**: when `PSEUDOCONSOLE_INHERIT_CURSOR` is used, the first and only output is `ESC[6n` and the child's output is withheld until the terminal answers (`ESC[1;1R` for a new session); (2) there is **no initial full repaint**: nothing is re-rendered when a session starts; (3) **every resize, including a no-op one, is followed by a viewport-only repaint** (`ESC[?25l ESC[H`, each visible row, `ESC[K`) that never includes scrollback; (4) child exit is observed on the process handle, not by reader EOF (the reader ends only when the pseudoconsole is closed), and `TerminateProcess` alone leaves descendants alive while closing the pseudoconsole ends them; (5) an exit code of 259 must not be mistaken for "still running"; (6) `TERM` is not inherited (see *Console quirks*).
- **I/O model** (maintainer decision Q-31): **one I/O thread serves the output of all ConPTY sessions** with overlapped I/O: each pseudoconsole writes to a named pipe whose end in Ferrum is opened with `FILE_FLAG_OVERLAPPED` and serviced by an I/O completion port (through `mio`) on that single thread, named `pty-io`, which wakes the daemon loop through the `PtyBackend` readiness model (`Readiness::Notify`). There is no reader thread per session. Writes use a per-session writer thread or overlapped I/O. The `PtyBackend` trait hides this; the daemon loop only sees "readable chunk" events.
- **Child exit** (maintainer decision Q-33): Windows: process exit is watched by a named `pty-exit` thread (WaitForMultipleObjects, one thread per 63 sessions); Unix: through mio on the loop.
- **IPC**: named pipes via `interprocess` with a restrictive DACL.
- **Input**: `winit` Win32; IME (TSF) via winit; AltGr handling (Ctrl+Alt) must not be treated as Ctrl+Alt shortcuts on layouts that need AltGr; test with German, Spanish, and US-International layouts.
- **Fonts**: `fontdb` scans `C:\Windows\Fonts` and the per-user fonts dir; emoji via Segoe UI Emoji (COLR/CBDT).
- **HiDPI**: Per-Monitor-v2 DPI awareness (manifest) — set in the embedded manifest and verified.
- **Memory metric**: Private Working Set via `GetProcessMemoryInfoEx` / PDH.
- **Paths**: long path support via manifest; UTF-16 handling at boundaries only; drag & drop yields `PathBuf` (never lossy strings).
- **Clipboard**: `arboard` text + images (CF_DIB/PNG); images saved as PNG.
- **Signing (decided: no certificate yet)**: releases are **unsigned**; SmartScreen shows "Windows protected your PC" until reputation builds or a certificate is added. Install docs explain *More info → Run anyway*; SHA-256 sums are published; winget/Scoop manifests pin hashes. WP-18 evaluates free open-source signing programs and keeps the Authenticode steps behind a flag.
- **Installers**: MSI (WiX) with per-user and per-machine options, `winget` manifest, Scoop manifest, portable zip. Add to PATH optional.
- **Defaults**: default profile = PowerShell 7 if present, else Windows PowerShell; right-click paste behavior configurable (Windows Terminal compatible default: right-click pastes when no selection; copies when selection exists).
- **Console quirks**: set `ENABLE_VIRTUAL_TERMINAL_PROCESSING` irrelevant (ConPTY); pass `TERM`-less environment (Windows tools ignore it) but still set `COLORTERM`.

## 4.1 Custom titlebar (all OSes, FR-UI-012)

| OS | Approach | Known limits and handling |
|---|---|---|
| Windows | `winit` window with decorations off; Ferrum draws the bar and controls; drag via `drag_window`, resize via `drag_resize_window` | Windows 11 *Snap layouts* on hover over the maximize button need native caption-button hit-testing, which winit does not expose: **degraded** (Win+Z and drag-to-edge snapping still work; documented). Per-monitor DPI and the 1 px border are handled in the window procedure only if S3 shows it is needed. Fallback: native titlebar |
| macOS | Transparent titlebar + full-size content view, native traffic lights kept | Native fullscreen hides/moves the controls — layout reacts to the fullscreen event; Stage Manager and tiling use native behavior. Fallback: native titlebar |
| Linux (X11) | Decorations off, client-drawn bar and controls, `_NET_WM_MOVERESIZE` via winit | Window managers with their own policies (tiling) may ignore client-side move/resize: fallback to native when the WM is a known tiling WM or via setting |
| Linux (Wayland) | Client-side decorations drawn by Ferrum (`xdg_toplevel` move/resize/show_window_menu through winit) | Without server-side decoration support there is no compositor shadow: use a 1 px border and square corners; if `xdg-decoration` is available prefer server-side when the user picks *Native* |

A custom titlebar costs no meaningful memory (it is a few widgets). It is an area of platform risk (R-15), which is why the native fallback is always available and tested in CI.

## 5. Cross-platform differences summary (the sanctioned list)

| Area | Linux | macOS | Windows |
|---|---|---|---|
| App shortcuts | Ctrl+Shift | Cmd | Ctrl+Shift |
| Clipboard extras | primary selection | — | right-click behavior |
| PTY | Unix PTY | Unix PTY | ConPTY |
| Daemon I/O | single-thread `mio` | single-thread `mio` | single-thread `mio` + one overlapped-I/O thread for all ConPTY sessions (Q-31) |
| Memory metric | PSS | phys_footprint | Private Working Set |
| Menus | none | native app menu | none |
| Decorations | server/client side | native | native |

## 6. CI matrix

| Runner | Targets built/tested |
|---|---|
| ubuntu (x86_64) | build + all tests + memcheck/soak (Xvfb) + fuzz smoke + packaging smoke |
| ubuntu (aarch64) | build + tests (when the runner type is available) |
| macos (arm64) | build + tests + memcheck (own metric) + packaging smoke |
| macos (x86_64) | build only (cross-compiled) + tests if runner available |
| windows (x86_64) | build + tests + memcheck (own metric) + packaging smoke |
| windows (aarch64) | cross-build only |

The agent working in a Linux sandbox cannot execute macOS/Windows binaries. For those platforms it writes code behind the trait, makes it compile (`cargo check --target …` when the target std is installable), covers it with contract tests, lists each unverified behavior in `docs/manual-verification.md`, and relies on CI for execution.
