# ferrum-daemon

**Responsibility:** session manager, event loop, client server and layout persistence (library behind the ferrumd binary).

**Status:** WP-06. Binary: `ferrumd` (`src/main.rs`); the library holds all logic.

## Model

`Daemon` owns every session, project and client on one I/O thread polled with `mio`
(`docs/02` §5); one `daemon-worker` thread takes the heavy work. Platform behavior is injected
through `Ports` as traits (ADR 0023): the `PtyBackend` and `ExitWatch` of `ferrum-pty`, the IPC
`Listener` and `MemoryRelease` of `ferrum-platform`, a `SpillFactory` and a `Clock`. Only
`src/main.rs` names the real implementations (spill files in the run directory included).

| Module | What it does |
|---|---|
| `server` | the loop: accept clients, check the peer's user after its first bytes, the handshake within 5 s, per-client mutation rate limit, PTY output read in 64 KiB chunks into one shared buffer with a fairness quantum of 64 KiB and at most 8 reads per session per turn, input written without blocking when the PTY has room, damage frames, a bounded worker backlog (writes of a file coalesce), freed memory returned to the OS when idle (at most every 30 s), idle exit (FR-SESS-003) |
| `session` | one session: its `ferrum_vt::Terminal`, PTY parts, a bounded input buffer written on the loop (`docs/11` §5), status (`Failed` with its error after an isolated panic), `catch_unwind` around every use of its terminal state (`docs/02` §10) |
| `worker` | LZ4 seal of full history chunks (Q-21/Q-26), reflow of a lent terminal that yields after a 16 ms budget (doubled at each resume) so the session's output is read, and stops at a chunk boundary for a newer size, search over a history snapshot while the session runs, closing sessions, atomic project writes; bounded channels both ways |
| `bus` | the internal `EventBus` (`docs/02` §9): session events, to Phase 2 subscribers and the clients |
| `view`, `frame`, `encode` | per-client views with their own dirty rows and row hashes; `Snapshot`, `Rows` and `Damage` (with `scroll_delta`: rows a scroll brings are not resent), at most one frame per frame interval (`max_fps`) and session, leading edge, input included, a congested client gets the latest state; no allocation per frame (`tests/alloc_frame.rs`) |
| `search` | literal or regular-expression search (Unicode case folding, compiled on the worker with bounded sizes) over history and screen, soft-wrapped rows joined into logical lines matched in bounded windows that keep their left context |
| `projects`, `layout` | the authoritative project store (`data_dir/ferrum/projects/<id>.toml`, sidebar order in `project-order.toml`), layout conversion; a mutation is echoed only after its file is written; files set aside at start are reported to every client (`FileSetAside`) |
| `logging` | `ferrumd.log` in the state directory, 1 MB x 3, no content, paths or environment values |

Session ids are the session keys of the project files, so a restarted daemon (FR-SESS-008)
respawns each session under the same id in its last working directory (from OSC 7).

| | |
|---|---|
| Allowed internal dependencies | ferrum-vt, ferrum-scrollback, ferrum-proto, ferrum-config, ferrum-project, ferrum-pty, ferrum-platform (traits only in the library; real implementations are wired only in `src/main.rs`, ADR 0023) |
| Must never depend on | ferrum-ui, ferrum-render, ferrum-app, winit |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.

## Tests and benches

`tests/` runs a real `ferrumd` with real PTYs; the session child and the UI processes are the
test executable itself (`tests/support/`), so no installed shell is needed. `sessions.rs`:
AC-SESS-01/02 and the idle exit (never with a client connected, after the grace);
`flood.rs`: the 20-session scenario (hidden sessions get events only) and flood isolation
(NFR-PERF-04, p99 at most 10 ms, in the ordinary run; `cargo xtask flood --against <rev>`
compares with an earlier daemon, `docs/08` §8); `views.rs`: reflow on the worker that yields (output read meanwhile) and gives way to a newer size,
search across rewrapped rows, and the `ConPTY` resize repaint kept out of the history (ADR 0026
W6); `alloc_frame.rs`: damage frames allocate nothing (`dhat`); `memcheck.rs`: the reference
scenario run by `cargo xtask memcheck --daemon-only`. `benches/search.rs`: the search path over
10 000 lines (`docs/03` §4: at most 15 ms).
