# ferrum-pty

**Responsibility:** the `PtyBackend` traits plus the Unix and ConPTY implementations and the child
process lifecycle (ADR 0026).

**Status:** WP-05. Traits, per-session environment (FR-SESS-007), blocking reader thread, Unix
backend on `portable-pty`, direct ConPTY backend on `windows-sys`, shell integration bootstrap
files (FR-SESS-005, scripts in `assets/shell-integration/`, module `shell_integration`).

## Model

A `PtyBackend` turns a `SpawnSpec` (program, arguments, explicit cwd, environment changes, size)
into a `PtySession` of four objects: `PtyReader` (output; `Readiness::Fd` on Unix for the daemon's
poller, `Readiness::Blocking` on Windows, read on `spawn_reader_thread`), `PtyWriter` (may block:
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
| Linux, macOS | `UnixBackend` (`portable-pty` 0.9.0 opens the pseudo terminal, `rustix` for the rest) | the child is spawned by `std::process::Command` as a session leader with the terminal as controlling TTY; `close` finds the processes attached to the terminal (`/proc` on Linux, `/bin/ps -t` on macOS) |
| Windows | `ConptyBackend` (`windows-sys` 0.61.2) | pseudoconsole with `PSEUDOCONSOLE_INHERIT_CURSOR` (`docs/11` §4); the reader answers the startup cursor query; exit read from the process handle (259 is an exit) |

Every backend passes the contract suite `ferrum_testkit::pty_contract` (PT-01 to PT-25); the
deterministic fake is `ferrum_testkit::fake_pty::FakeBackend`.

| | |
|---|---|
| Allowed internal dependencies | none (leaf service) |
| Must never depend on | any other ferrum crate; consumers use only its traits |
| `unsafe` | only in `src/conpty/sys.rs` and `src/unix/sys.rs` (`#![deny(unsafe_code)]` + a local `#[allow]`), each block with `// SAFETY:` |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
