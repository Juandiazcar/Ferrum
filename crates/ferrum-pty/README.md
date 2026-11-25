# ferrum-pty

**Responsibility:** the `PtyBackend` traits plus the Unix and ConPTY implementations and the child
process lifecycle (ADR 0026).

**Status:** WP-05. Traits, per-session environment (FR-SESS-007), Unix
backend on `portable-pty`, direct ConPTY backend on `windows-sys`, shell integration bootstrap
files (FR-SESS-005, scripts in `assets/shell-integration/`, module `shell_integration`).

## Model

A `PtyBackend` turns a `SpawnSpec` (program, arguments, explicit cwd, environment changes, size)
into a `PtySession` of four objects: `PtyReader` (output; `Readiness::Fd` on Unix for the daemon's
poller; on Windows `Readiness::Notify` from the backend's single I/O thread, Q-31; never a thread
per session), `PtyWriter` (may block:
write from your own thread), `PtyResize`, and `PtyChild` (exit status, truthful idempotent `kill`,
a cloneable `PtyKiller`, and `close`, which ends every process on the terminal and lets the reader
reach the end of the output).

Behavior is the same on every OS (NFR-PORT-01): one program lookup rule (the child's `PATH`,
never the current directory; Windows adds `PATHEXT` and runs batch files through `cmd.exe`), the
output ends at `close` and never at the child's exit, `kill` ends the child at once, `close` ends
every process attached to the terminal, a killed child reports `ExitStatus::Killed`, writes after
`close` fail with `PtyError::Closed`, and dropping the writer sends nothing.

| Platform | Backend | Notes |
|---|---|---|
| Linux, macOS | `UnixBackend` (`portable-pty` 0.9.0 opens the pseudo terminal, `rustix` for the rest) | the child is spawned by `std::process::Command` as a session leader with the terminal as controlling TTY; `close` finds the processes attached to the terminal in the kernel's process table (`/proc` on Linux, `proc_pidinfo` on macOS), without starting any program |
| Windows | `ConptyBackend` (`windows-sys` 0.61.2, `mio` 1.2.5) | pseudoconsole with `PSEUDOCONSOLE_INHERIT_CURSOR` (`docs/11` §4): the first output is `ESC[6n`, which the session layer answers (W1); the output of every session is an overlapped named pipe served by one I/O thread, `pty-io` (Q-31); exit read from the process handle (259 is an exit) |

`PtyWriter::try_write` (WP-06) never blocks: Unix writes the non-blocking master once and reports
`Readiness::Fd` (the reader's descriptor, watched for writability); `ConPTY` writes an overlapped
named pipe served by `pty-io` and reports `Readiness::Notify` (ADR 0026 W4: no thread per session
for writes either).

`ExitWatch` (`native_exit_watch`, WP-06) tells which children exited without a thread per child
or a timer (NFR-PERF-06): Linux watches `pidfd`s in one `epoll` set and macOS uses one `kqueue`
with `EVFILT_PROC`, both a single descriptor for the daemon's poller (`Readiness::Fd`); on Windows a
named `pty-exit` thread waits on the process handles with `WaitForMultipleObjects` (one thread per
63 children, slot 0 a wake event for changes; Q-33) and calls the notification
(`Readiness::Notify`) from that thread. The caller then reads the status
with `PtyChild::try_wait`.

Every backend passes the contract suite `ferrum_testkit::pty_contract` (PT-01 to PT-25); the
deterministic fake is `ferrum_testkit::fake_pty::FakeBackend`.

| | |
|---|---|
| Allowed internal dependencies | none (leaf service) |
| Must never depend on | any other ferrum crate; consumers use only its traits |
| `unsafe` | only in `src/conpty/sys.rs` (with `src/conpty/sys/security.rs`), `src/unix/sys.rs`, `src/exit_watch/macos.rs` and `src/exit_watch/windows.rs` (`#![deny(unsafe_code)]` + a local `#[allow]`), each block with `// SAFETY:` |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
