# 0026. Windows PTY backend: spike S4 measurements, decision and `PtyBackend` contract tests

- Status: accepted (maintainer decisions 2026-11-21: Windows backend and the `docs/11` §4 rewrite, recorded in `docs/14` §2 as Q-18)
- Date: 2026-11-21
- Spike: S4 (`docs/spec/docs/12-build-order.md`). Requirements touched: FR-SESS-009, NFR-REL, NFR-PORT-01, AC-MEM-02 (per-session PTY buffers, `docs/03` §2).

## Context

`docs/11` §4 delegates the Windows PTY to `portable-pty` and leaves to S4 the choice "keep it or call
ConPTY directly through `windows-sys`". `docs/02` §5 and ADR 0008 assume ConPTY pipes cannot be polled
and therefore need one blocking reader thread per session. R-04 (ConPTY quirks) and R-05
(`portable-pty` lightly maintained) are open. S4 exits with this ADR and a contract test list for
`PtyBackend` (input of WP-05).

## Method

One standalone package, `spikes/s4-windows-pty/` (own workspace and lockfile, excluded from the
product workspace). It follows the product lint policy (`docs/05`: same `[lints]` tables, `unsafe_code =
"forbid"`, no `unwrap`/`expect`/`println!`, functions of at most 40 lines, `cargo clippy -D warnings`
and `cargo fmt --check` clean) and depends only on `portable-pty` 0.9.0 and `anyhow` 1.0.104. One module
per concept: `session.rs` (spawn, reader thread, pumps), `stats.rs`, `helpers.rs` (helper children
`flood`, `sleep`, `exit`, `env`, `spawn-sleep`, `echo-bye`: the same binary, so nothing depends on shell
tricks), `shells.rs`, `bytes.rs`, `sysinfo.rs`, `out.rs`, and `exp1_echo.rs` to `exp8_close.rs`, one per
experiment. A reader thread per session is named `pty-read-<n>`, has a 256 KiB stack, reads 64 KiB and
feeds a bounded channel (32 chunks), as `docs/11` §4 prescribes.

Reproduce (any OS, repository root):

```bash
cd spikes/s4-windows-pty
cargo build --release --locked
S4_FLOOD_MB=64 ./target/release/s4-windows-pty all     # or one experiment: ... s4-windows-pty 2
cargo tree -p portable-pty -e normal --prefix none --target x86_64-pc-windows-msvc --locked
cp ../../deny.toml . && cargo deny check licenses bans; rm deny.toml
```

Runs (temporary workflow `.github/workflows/spike-s4.yml`, matrix windows/ubuntu/macos, deleted in the
last commit of the PR; logs stay on GitHub):

- Final run (the refactored harness), all three OS, all green: <https://github.com/Juandiazcar/Ferrum/actions/runs/44977495884>
  (jobs 137070556039 windows, 137070556045 ubuntu, 137070555874 macos). An earlier run of the same experiments before the refactor, <https://github.com/Juandiazcar/Ferrum/actions/runs/44976103587>, is quoted as a second sample where numbers vary.
- Windows: `windows-latest` = Windows Server 2025 build 10.0.26100.33828, `pwsh` 7.6.10. Ubuntu 26.04.1 (kernel 7.0 azure), macOS 26.x arm64 (Darwin 25.6.0).
- Local: Windows 11 Pro build 10.0.26300.9974, x86_64, `pwsh` not installed (only `cmd.exe` and `powershell.exe` rows). Local numbers vary with load (other spikes ran in parallel): the 64 MB flood ran at 6.4 MB/s in one run and 1.3 MB/s in a loaded one.

`portable-pty` check: 0.9.0 is the latest stable on crates.io on 2026-11-21 (`max_stable_version` 0.9.0,
released 2025-02-11, 31 M downloads; previous 0.8.1 in 2023-03). The upstream repository (`wezterm/wezterm`)
is active (commit 2026-11-20) but the crate has not been released for 21 months.

## Observed behavior (per item; W11 = local Windows 11, CI-W = Server 2025, U = Ubuntu CI, M = macOS CI)

### 1. Spawn, write, echo

| Shell | Result |
|---|---|
| `cmd.exe` | W11 and CI-W: typed text echoed, result marker 17-32 ms after the line was written; `exit` returns code 0 in 2-15 ms |
| `powershell.exe -NoProfile` | works; result marker after 121 ms (W11) and 1.40-2.27 s (CI-W, cold start, two runs) |
| `pwsh -NoProfile` | CI-W only: works, 51-91 ms; W11 not installed (spawn error `os error 2` returned as `Err`, no panic) |
| `sh -i`, `bash -i` | U and M: echo and result in 5-26 ms, `exit` code 0 |

### 2. Exit codes and kill

| Case | Windows (W11, CI-W) | Unix (U, M) |
|---|---|---|
| `cmd /c exit 3`, `sh -c 'exit 3'` | `ExitStatus { code: 3 }` seen 28-36 ms after spawn | code 3 in 1-9 ms |
| exit 0 / 255 | 0 / 255 in 12-27 ms | same, 1-6 ms |
| **exit code 259** (`STILL_ACTIVE`) | **never reported: `try_wait` returns `None` for 10 s** (`WinChild::is_complete` compares the code with `STILL_ACTIVE`) | code 3 (259 & 255), reported |
| `child.kill()` | `Ok(())`, exit seen 0.26-0.34 ms later, code 1 | `Ok(())`, exit seen 50-178 ms later, `signal: Hangup` |
| `clone_killer().kill()` | **returns `Err` ("The handle is invalid", stale last-error) although the process died** (inverted check in `WinChildKiller::kill`) | `Ok(())` |
| Reader EOF while the master is open | **never** (waited 2 s) | immediately (0 ns after exit) |
| Reader EOF after dropping master and writer | 1.6-10 ms | already delivered |
| Last output of a fast-exiting child (30 runs) | 30/30 arrived with the master still open, 0 lost | 30/30 |
| Grandchild after `child.kill()` | **still alive** (only the direct child is terminated) | dead |
| Grandchild 1 s after dropping the master | dead (the pseudoconsole close terminates attached processes) | dead |

Dropping the master with a child killed by the close gives exit code `0xC000013A` (`STATUS_CONTROL_C_EXIT`), 3-5 ms later.

### 3. Resize

- `resize` itself costs 7-27 µs and returned `Ok` in all 39-49 calls (0 errors).
- Storm: 16 MB flood while resizing 80x24 <-> 120x40 every 40 ms: all output arrived (16.84 MB received for 16.78 MB written, ratio 1.004 to 1.005, the surplus is the repaints), end marker seen, longest gap between chunks 9-13 ms. No stall or loss.
- Idle `cmd.exe`: every resize, **including a same-size one**, produces a repaint of the visible viewport only: 245-252 bytes (same size, back to 80x24), 275-282 (grow 100x30), 225-232 (shrink 60x20). Shape: `ESC[?25l ESC[H` then each visible row followed by `ESC[K`, rows joined by `\r\n`; no `ESC[2J`.
- After 60 printed lines in a 24-row pty, a resize to 100x30 re-emitted 22 rows (`LINE39` onward, 318-324 bytes): **scrollback is never re-emitted**, only the viewport.
- The child sees the new size (`mode con`: Lines 30, Columns 100; `stty size`: `30 100`) on all OSes.
- Unix: the kernel emits nothing on resize (ubuntu 0 bytes; macOS 109 bytes are the shell redrawing its prompt after `SIGWINCH`).

### 4. Large output (64 MB, reader keeps only a 64-byte tail)

| | Throughput | Chunk size | Stalls | Notes |
|---|---|---|---|---|
| W11 helper flood | 6.4 MB/s (1.3 MB/s under load) | mean 2.5 KiB, max 4 KiB (8 KiB on CI-W) | max gap 22-85 ms | 67 109 054 B received for 67 108 882 B sent, end marker seen |
| CI-W helper flood | 5.2-6.5 MB/s (two runs) | max 4-8 KiB | 7-9 ms | same |
| W11 `cmd /c type` of a 64 MB file | 0.9 MB/s | mean **72 B**, 923 k chunks | 38 ms | line-by-line console writes |
| CI-W `cmd /c type` | 0.6-0.9 MB/s | mean 72 B | 6-7 ms | 72-114 s for 64 MB |
| Ubuntu flood / `cat` | 36.2-46.6 / 56.1-58.6 MB/s | mean 0.55-0.67 KiB, max 21 KiB | 1-2 ms | 1.016 ratio |
| macOS flood / `cat` | 34.0-47.6 / 42.8-60.7 MB/s | exactly 1 KiB | 2-5 ms | 1.016 ratio |

- Reader-side memory stays flat: Windows private commit 1 452 -> 1 496 KiB (W11, flood), 1 740 -> 1 936 KiB (CI-W); Linux RSS 3 236 -> 3 236 KiB.
- Unix output is 1.6 % larger than written (`ONLCR` turns `\n` into `\r\n`, so `\r\n` becomes `\r\r\n`); ConPTY output is within 0.5 % but is a re-encoding. Tests must compare content (lines, markers), never bytes.
- The Windows ceiling is conhost, not the reader: 64 MB at 5-10 MB/s per session. No spec line covers PTY throughput, so this is a finding (noted in R-04), not a budget violation.

### 5. Bytes at startup (initial repaint)

`portable-pty` creates the pseudoconsole with `INHERIT_CURSOR | RESIZE_QUIRK | WIN32_INPUT_MODE` (source: `src/win/psuedocon.rs`). The first bytes the reader receives, for every child (`cmd.exe`, `powershell.exe`, `pwsh`, and a plain helper):

```
1b 5b 36 6e                              ESC [ 6 n      <- cursor position query, and NOTHING else
```

**Until the terminal answers `ESC[1;1R` the child's output is withheld**: 4 bytes after 2 s (all four children); the plain child that prints one line and exits was still running after 1.5 s. After the answer, `cmd.exe` (W11, 182 further bytes) emits:

```
ESC[?9001h  ESC[?1004h  ESC[m  ESC]0;C:\WINDOWS\system32\cmd.exe BEL  ESC[?25h  ESC[?25l
"Microsoft Windows [Version 10.0.26300.9974]\r\n(c) Microsoft Corporation. All rights reserved."
ESC[4;1H  "C:\Users\juand>"  ESC[?25h
hex: 1b 5b 3f 39 30 30 31 68 1b 5b 3f 31 30 30 34 68 1b 5b 6d 1b 5d 30 3b ...
```

`powershell.exe`: `ESC[?9001h ESC[?1004h ESC[m ESC]0;<title> BEL ESC[?25h "PS C:\Users\juand> "` (106 bytes). A plain child: the same mode sets, title, `ESC[?25h`, then its text. Findings:

- There is **no repaint of existing content** at session start (the session is new; nothing to repaint), and no `ESC[2J` when `INHERIT_CURSOR` is set. The only repaint in the lifetime of a session is the viewport rewrite on resize (item 3).
- The terminal must enable-track `?9001` (win32-input-mode) and `?1004` (focus events); plain VT input still works (echo test).
- The cursor query is answered correctly by any VT that implements `CSI 6 n`; a new session's cursor is at 1;1.
- Unix: `sh`/`bash` emit only their prompt (2-10 bytes), no queries, no mode sets.

### 6. I/O model and thread cost

- `try_clone_reader()` returns `Box<dyn Read + Send>` over a `CreatePipe` anonymous pipe (`filedescriptor::Pipe`, synchronous `ReadFile`). The handle is hidden: no `PeekNamedPipe`, no overlapped I/O, no wait handle can be obtained. Experiment 6a: for a child that is silent for 3 s the reader got 3 chunks in the first 12 ms and then **blocked for the rest of the 4.5 s with no EOF** after the child exited. The only way out is closing the master. Therefore, with `portable-pty`, **one blocking reader thread per session is forced** (confirms `docs/02` §5 for this backend). On Unix the reader sees EOF at child exit (3.00 s).
- `take_writer()` is a synchronous `WriteFile`. Writing 4 MiB to a child that never reads took more than 4 s on W11 (336-1 660 KiB written in 4 s) and 4+ s on CI-W (3 168-3 236 KiB at 4 s), so the writes cannot run on the daemon loop: a writer thread (or overlapped I/O) is needed. Unix: completed in 1-2 s.
- Cost of the model (named `pty-read-<n>` threads, 256 KiB stack, 64 KiB buffer, channel of 32):

| Measure (10 sessions) | W11 | CI-W | Ubuntu | macOS |
|---|---|---|---|---|
| 10 bare blocked threads, working set / private commit delta | +116 to +240 / +300 to +472 KiB (3 local runs) | +84 / +128 KiB | +116 RSS / +3 476 `VmData` (the 10 x 256 KiB stacks, virtual) | +192 RSS |
| 10 real sessions, working set / private commit delta | +896 / +1 216 KiB (0.09 / 0.12 MiB each) | +856 / +1 100-1 208 KiB | +596 RSS | +704 RSS |
| extra `conhost.exe` processes (outside the Ferrum process) | +10 to +11, +5.1 to +6.1 MB working set in total | +10, +7.8 MB | n/a | n/a |

  Per session, in the Ferrum process: 0.08-0.12 MiB working set (about 0.11-0.15 MiB private on Windows), inside the 0.20 MB "per session misc" allowance of `docs/03` §2. The `docs/11` figure "10 sessions = 2.5 MB virtual" matches Linux `VmData`. Each ConPTY session also costs one `conhost.exe` of roughly 0.5-0.8 MB working set that no Ferrum budget counts (`docs/03` counts Ferrum processes); noted, not a violation of any line.

### 7. Environment and cwd

- `portable-pty` copies the parent environment (Windows: also merged with the registry environment). It sets neither `TERM` nor `COLORTERM`: a child under Git Bash saw `TERM=xterm-256color` because the parent had it. `env_remove("TERM")` works (child: `None`); `env("COLORTERM","truecolor")`, `TERM_PROGRAM`, `FERRUM_ID` and a non-ASCII value reach the child intact on all four OS.
- `cwd(...)` is honored. **Without it the default is the home directory** (`C:\Users\...` via `USERPROFILE`, `$HOME` on Unix), not the daemon's cwd.

### 8. What `portable-pty` lacks, footprint and licenses

Source review of 0.9.0 (`src/win/*.rs`, `src/unix.rs`) and measurements above:

| Gap or defect | Effect on Ferrum |
|---|---|
| Pseudoconsole flags are a constant: `INHERIT_CURSOR \| RESIZE_QUIRK \| WIN32_INPUT_MODE`; `PASSTHROUGH` is declared `dead_code` | Startup query hazard (item 5) cannot be turned off; no passthrough experiment |
| Reader and writer handles are hidden behind `Box<dyn Read/Write>` | Thread per session forced; no overlapped I/O, no `mio`/IOCP registration, no `CancelIoEx` |
| Exit detection compares the code with `STILL_ACTIVE` (259) | A child exiting with 259 looks alive forever (measured) |
| `WinChildKiller::kill` returns `Err` when `TerminateProcess` succeeds | `clone_killer().kill()` lies (measured); `WinChild::kill` swallows errors with `.ok()` |
| `kill` terminates only the direct child | Descendants live until the master is closed (measured) |
| `expect`/`unwrap` in library code: `load_conpty` panics on Windows older than 1809, 14 `unwrap`/`expect` lines in `win/*.rs` and `unix.rs`, mutex poisoning panics | Conflicts with "never panic" (`CLAUDE.md` rule 2); would need `catch_unwind` around every call |
| Default cwd is the home directory; environment inherited wholesale | Must always be set explicitly (item 7) |
| Prefers a side-loaded `conpty.dll`/`openconsole.exe` if present next to the executable | Security surface (DLL search); the product does not ship one |
| Unix kill sends `SIGHUP` only | Acceptable; behavior is what the contract must say |
| Dependencies: 23 transitive crates on Windows, 16 on Unix (`cargo tree -p portable-pty -e normal --prefix none`), among them `winapi` 0.3.9, `winreg`, `shared_library`, `lazy_static`, `serial2` (serial ports, unused), `nix` 0.28, `bitflags` 1.3.2 and 2.x, `thiserror` 1.x | Duplicates the workspace's `thiserror` 2 and `windows-sys`; `unsafe` lines: win 23, unix 15, lib 5, cmdbuilder 7 |
| Licenses | all MIT, Apache-2.0, BSD-2-Clause or MIT OR Apache-2.0; `cargo deny check licenses bans` passes with the repository `deny.toml` (only warning: duplicate `bitflags`) |

### Unverified exploratory notes (not evidence; WP-05 must prove)

S4's rules forbid `unsafe` in spikes, so a one-off experiment that called `CreatePseudoConsole` and
friends through `windows-sys` was run and **removed from the PR**. Nobody can reproduce it from this
repository (its output survives only in the Windows job log of run
<https://github.com/Juandiazcar/Ferrum/actions/runs/44975639196>, job 137064797652). Nothing in the
Decision, the workarounds or the contract tests rests on it. It only suggests what WP-05 could try:

- Pipes built as named pipes with `FILE_FLAG_OVERLAPPED` on Ferrum's end and synchronous on ConPTY's end seemed to work.
- Creating the pseudoconsole with flags 0 (no `INHERIT_CURSOR`) seemed to remove the startup cursor query (startup was `ESC[?9001h ESC[?1004h ESC[?25l ESC[2J ESC[m ESC[H` plus content). `PASSTHROUGH` (0x8) was accepted with no visible change in output or throughput.
- A single thread seemed able to serve ten sessions (`WaitForMultipleObjects` over overlapped reads, and `mio::windows::NamedPipe` with a `Waker`): a possible future optimization that would remove the per-session reader threads. The spec's reader-thread model stays as written.
- Waiting on the process handle seemed to detect exit code 259, and `ClosePseudoConsole` seemed not to block with a flooding child and nobody reading.

## Decision

1. **Windows: implement ConPTY directly with `windows-sys` 0.61.2 inside `ferrum-pty`** (the one crate allowed `unsafe` besides platform/render, `docs/05` §4), behind `PtyBackend`. Justification, by evidence:
   - Two correctness defects in `portable-pty` (exit 259 never reported, `clone_killer().kill()` result inverted) cannot be fixed without forking; panics in library code conflict with `CLAUDE.md` rule 2.
   - Fixed flags: `INHERIT_CURSOR` makes every session depend on answering `ESC[6n` before any byte of output (measured), a needless hazard for a headless daemon; a direct backend can pass flags chosen by Ferrum.
   - Hidden handles: the reader and writer are `Box<dyn Read/Write>`, so a backend owning the handles can also own cancellation and I/O strategy (source review; no behavior is claimed).
   - R-05: 21 months without a release; a direct backend removes the Windows dependency set (23 crates, `winapi`, `winreg`, `shared_library`, `lazy_static`, `serial2`) from the product.
   - Cost: roughly the spawn, attribute-list, pipe and wait code that `portable-pty` has in `win/psuedocon.rs` (about 150 lines of `unsafe`-adjacent code), plus command-line quoting, PATH search and environment block that `portable-pty` also provides. Every `unsafe` gets `// SAFETY:` and a contract test.
2. **Unix and macOS: keep `portable-pty` 0.9.0** behind the same traits for v1.0. Every item above passed on Ubuntu and macOS (exit codes, EOF at exit, resize, 64 MB, env, cwd, no lost final output, children die on kill). `rustix` stays the escape hatch (R-05).
3. **Fallback if WP-05 cannot make the direct backend pass the contract suite on a supported Windows build** (Windows 10 1809 and later: not exercised here, `docs/manual-verification.md` MV-S4-1 to 3): use `portable-pty` on Windows with the workarounds W1-W8 below and record it in a new ADR.
4. The decision rests only on reproducible results: the two measured `portable-pty` defects, the maintenance status and the measured behavior above. The maintainer accepted it on 2026-11-21; WP-05 proves the direct backend with the contract suite.

ADR 0008 (Windows part): **validated as written.** One blocking reader thread per session (named `pty-read-<id>`, 256 KiB stack, 64 KiB buffer, bounded channel) works with `portable-pty`, where it is forced because the reader handle is hidden and a blocked read ends only when the master is closed (item 6), and it is affordable: 0.08-0.12 MiB working set per session, inside the 0.20 MB per-session allowance. `docs/02` §5 and `docs/11` §4 (I/O model) therefore stay unchanged. Spec text changed by this ADR (maintainer decision 2026-11-21): the PTY bullet of `docs/11` §4 (measured ConPTY behavior replaces "re-renders the screen on attach"; Q-18), and the `docs/07` rows for `portable-pty` (Linux and macOS only) and `windows-sys` (required for the direct ConPTY backend).

## Required ConPTY workarounds (document and test each in `ferrum-pty`, `docs/11` §4)

| ID | Workaround | Evidence |
|---|---|---|
| W1 | When the pseudoconsole is created with `INHERIT_CURSOR`, the session layer answers the first `ESC[6n` with `ESC[1;1R` before anything else (a new session's cursor is at 1;1); without the answer no output arrives | item 5 |
| W2 | Do not wait for reader EOF to detect exit: wait on the process handle (never compare the code with 259); then drain the reader until it is quiet for a bounded time, then close the pseudoconsole, then read to EOF | item 2 |
| W3 | Close the pseudoconsole (not only `TerminateProcess`) to end a session; it ends every attached process; `kill` alone leaves descendants | item 2 |
| W4 | Writes run on a writer thread (or overlapped I/O) with a bounded queue; the daemon loop never blocks on `WriteFile` | item 6 |
| W5 | Reader on a named thread `pty-read-<id>`, 256 KiB stack, 64 KiB buffer, bounded channel + `mio::Waker` (as specified) | item 6 |
| W6 | Treat the resize repaint (`ESC[?25l ESC[H` + rewritten rows with `ESC[K`) as a viewport rewrite that must not enter scrollback; verify against the reflowing grid of `ferrum-vt` in WP-05/WP-06 (conhost does not reflow) | item 3 |
| W7 | Track `?9001` and `?1004` set/reset in the VT; keep sending plain VT input unless WP-02/WP-07 decide to implement win32-input-mode | item 5 |
| W8 | Environment is explicit: remove `TERM`, set `COLORTERM=truecolor`, `TERM_PROGRAM`, `FERRUM_*`; cwd is always set; spawn errors are values, never panics | item 7 |

## Proposed trait shape (signatures only; ISP per `docs/05` §2)

```rust
pub struct PtySize { pub cols: NonZeroU16, pub rows: NonZeroU16 }
pub enum EnvOp { Set(OsString), Remove }
pub struct SpawnSpec {
    pub program: OsString, pub args: Vec<OsString>,
    pub cwd: PathBuf,                       // always explicit
    pub env: Vec<(OsString, EnvOp)>,        // applied over the inherited environment
    pub size: PtySize,
}
#[non_exhaustive] pub struct ExitStatus { pub code: Option<u32>, pub signal: Option<i32> }

pub trait PtyBackend: Send + Sync {
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtySession, PtyError>;
}
pub struct PtySession {
    pub reader: Box<dyn PtyReader>, pub writer: Box<dyn PtyWriter>,
    pub resize: Box<dyn PtyResize>, pub child: Box<dyn PtyChild>,
}
/// How the daemon learns that `read` has data.
pub enum Readiness { Blocking /* reader thread */, Fd(RawFd) /* Unix: register with mio */, Handle(RawHandle) /* reserved: a backend that owns its handles */ }
pub trait PtyReader: Send {
    fn readiness(&self) -> Readiness;
    /// `Ok(0)` only after the session is closed. Never reports child exit.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError>;
}
pub trait PtyWriter: Send {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), PtyError>; // may block; caller owns the thread
}
pub trait PtyResize: Send + Sync {
    fn resize(&self, size: PtySize) -> Result<(), PtyError>;       // idempotent; may be followed by a repaint
    fn size(&self) -> PtySize;
}
pub trait PtyChild: Send {
    fn pid(&self) -> Option<u32>;
    fn try_wait(&mut self) -> Result<Option<ExitStatus>, PtyError>;
    fn exit_source(&self) -> ExitSource;                           // Pid / Handle the loop can wait on
    fn kill(&mut self) -> Result<(), PtyError>;                    // truthful result, idempotent
    fn close(self: Box<Self>) -> Result<(), PtyError>;             // ends all descendants, unblocks the reader
}
```

## `PtyBackend` contract tests (every implementation: Unix, ConPTY, fake, runs the same suite in `ferrum-testkit`)

"Spike" names the experiment that already showed the behavior; the test itself is written in WP-05.

| ID | Test name | Assertion | Spike |
|---|---|---|---|
| PT-01 | `spawn_echo_roundtrip` | typed line is echoed and the command output arrives within 5 s | 1 |
| PT-02 | `spawn_missing_program_is_error` | `Err`, no child, no panic; also empty program, NUL in argument, invalid cwd | 1 |
| PT-03 | `spawn_applies_cwd` | child reports the requested cwd; there is no default cwd | 7 |
| PT-04 | `spawn_env_set_and_remove` | `COLORTERM`, `TERM_PROGRAM`, `FERRUM_ID`, a non-ASCII value arrive; a removed `TERM` is absent even if the parent has it | 7 |
| PT-05 | `exit_code_zero_nonzero_and_255` | codes 0, 3, 255 are reported exactly | 2 |
| PT-06 | `exit_code_259_is_reported` | a child exiting with 259 is reported as exited (Windows); Unix reports `code & 255` | 2 |
| PT-07 | `exit_detected_without_reader_eof` | exit is observable within 1 s while the reader is still open | 2 |
| PT-08 | `reader_ends_after_close` | after `close`, `read` returns `Ok(0)` within 2 s, on every OS | 2 |
| PT-09 | `final_output_survives_fast_exit` | 30 runs of "print a line and exit": the line always arrives, with the reader drained before and also with `close` called right after exit | 2 (before-close variant) |
| PT-10 | `kill_is_truthful_and_idempotent` | `kill` returns `Ok` when the child died, a second `kill` and a `kill` after exit return `Ok`; exit observed within 1 s; also through a cloned killer | 2 |
| PT-11 | `close_ends_descendants` | a grandchild is gone within 2 s after `close` | 2 |
| PT-12 | `close_does_not_hang_with_unread_output` | `close` returns within 1 s while a child floods and nobody reads | 8 |
| PT-13 | `startup_cursor_query_is_answered` | if the backend emits `ESC[6n`, the standard reply `ESC[1;1R` releases the output and a plain child's first output arrives within 2 s | 5 |
| PT-14 | `no_output_lost_before_first_reply` | everything the child wrote while the query was unanswered (including its final line when it exits) arrives after the reply | 5 |
| PT-15 | `resize_updates_child_size` | the child sees the new rows and columns | 3 |
| PT-16 | `resize_same_size_is_ok` | no-op resize returns `Ok`; any repaint is at most `rows` lines and never earlier lines | 3 |
| PT-17 | `resize_storm_during_flood` | 16 MB flood with a resize every 40 ms: end marker arrives, no `Err`, longest read gap < 1 s | 3 |
| PT-18 | `large_output_integrity` | 64 MB: end marker arrives, content compared by lines not bytes, reader memory delta < 1 MiB; throughput is recorded, not gated (ConPTY is conhost-bound, item 4) | 4 |
| PT-19 | `read_respects_buffer_size` | no read returns more than the buffer given (64 KiB buffer used by the daemon) | 4 |
| PT-20 | `large_input_completes` | 4 MiB written to a child that never reads completes within 30 s while output is drained, and never blocks `read` | 6 |
| PT-21 | `reader_blocks_until_data_or_close` | on the blocking model a silent child does not make `read` return early or error | 6 |
| PT-22 | `no_panic_on_hostile_input` | huge environment, 32 KiB argument, closed handles, double `close`: errors only | 2, 8 |
| PT-23 | `ten_sessions_memory` | 10 sessions: process private commit/RSS delta < 0.20 MB x 10 including threads | 6 |
| PT-24 | `reader_thread_name_and_stack` | the reader thread is named `pty-read-<id>` with a 256 KiB stack and the 64 KiB buffer fits | 6 |
| PT-25 | `fake_backend_parity` | the deterministic fake passes PT-01, 05, 07, 08, 10, 15, 22 | n/a |

## Consequences

- WP-05 implements the traits above, the Unix backend on `portable-pty` 0.9.0, the ConPTY backend on `windows-sys` 0.61.2 (new `ferrum-pty` dependency edge; `unsafe` confined to one module with `// SAFETY:` and tests), the fake backend, and the suite PT-01 to PT-25 in `ferrum-testkit`.
- `docs/manual-verification.md` gains MV-S4-1 to 3: Windows 10 1809/21H2/22H2, Windows 11 24H2/25H2 and ARM64 were not exercised (only 10.0.26100 and 10.0.26300).
- `docs/14`: R-04 mitigation updated (with the ConPTY throughput finding), R-05 likelihood raised to High (`portable-pty` unreleased for 21 months, two defects measured), Q-18 resolved (the `docs/11` §4 rewrite), ADR index row 0026; no budget in `docs/03` is relaxed and no question stays open from S4.
- `docs/11` §4 PTY bullet rewritten to the measured behavior; `docs/07` rows for `portable-pty` and `windows-sys` updated.
- ADR 0008 stays accepted; its Windows part is validated as written (above).

## Not verified

- Windows before build 26100 (ClosePseudoConsole blocking with unread output is a known behavior of older conhost: not reproduced on 26100/26300, so PT-12 must run on older builds); Windows on ARM64; `pwsh` locally.
- Resize interplay with the reflowing grid of `ferrum-vt` (needs WP-02): W6 is a requirement, not a result.
- Whether data is lost when `close` is called immediately after exit while output is still in the pipe (PT-09 second variant).
- Everything in the exploratory notes; `PASSTHROUGH` effects with a terminal that implements it.
- Windows memory counters are read through PowerShell (`WorkingSet64`, `PrivateMemorySize64`), not a PSS-equivalent metric (`docs/03` §5 uses its own).
