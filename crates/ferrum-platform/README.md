# ferrum-platform

**Responsibility:** clipboard, directories, single-instance, own-process memory metric and daemon autostart behind traits.

**Status:** partial. Contents so far:

- `private_fs::{create_private_dir, create_private_file}` (WP-04): owner-only creation (0700 and
  0600 on Unix; on Windows an access list for the user and `SYSTEM` only, inherited by the files
  inside), passed by the composition roots to
  `ferrum_config::archive::StagingFs` for imports (`docs/10` §4).
- `Dirs` (WP-06): the config, data, state and run directories (`FERRUM_*_DIR` overrides, the
  `ferrum-portable` marker, then XDG / `Library` / `AppData`; `docs/11` §1) and
  `prepare_private_dir`, which creates the run directory owner-only and refuses one another user
  owns or can enter (`docs/10` §2; on Windows: owned by the user, an access list naming only the
  user and `SYSTEM`; Linux falls back to `/tmp/ferrum-<uid>`).
- `instance` (WP-06): the single-instance lock (an OS file lock, released by the OS even after
  `kill -9`) and the 32-byte handshake nonce file, mode 0600.
- `ipc` (WP-06): the daemon endpoint behind the `Listener` and `Connection` traits (`mio`
  sources, maintainer decision Q-32): a Unix socket, mode 0600, or the named pipe
  `\\.\pipe\ferrum-<SID-hash>` with an owner-and-`SYSTEM` DACL, first instance, remote clients
  rejected. `Connection::is_peer_owner` checks the peer the same way on every OS once its first
  bytes arrived (`SO_PEERCRED`, `getpeereid`, the pipe client token by impersonation).
  `connect` is the blocking client of the tests and tools (the UI uses `interprocess`, Q-32); it
  checks that the pipe server runs as the user and is overlapped on Windows, so one thread can
  read while another writes.
- `memory` (WP-06): `MemoryRelease`, freed heap memory back to the OS (`docs/03` §3 rule 7):
  `malloc_trim(0)` on glibc, `mi_collect(true)` for `mimalloc` on macOS, `HeapCompact` on Windows.
- `default_shell` (WP-06): the shell used without a profile (`docs/11` §1, §4): `$SHELL`, the user
  database entry (`getpwuid_r`), the first installed shell of `/etc/shells`; on Windows `pwsh`,
  then Windows PowerShell, then `%COMSPEC%`.

| | |
|---|---|
| Allowed internal dependencies | none (leaf service) |
| Must never depend on | any other ferrum crate; pure crates never import it |
| `unsafe` | allowed only in audited modules (`#![deny(unsafe_code)]` + local `#[allow]`): `src/win32.rs`, `src/ipc/windows/sys.rs`, `src/ipc/windows/client.rs`, `getpeereid` in `src/ipc/unix.rs`, `src/shells/passwd.rs` and `src/memory.rs`, each block with `// SAFETY:` |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
