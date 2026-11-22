# Manual verification

Checks that need a capability CI and agents do not have (real macOS/Windows machines, a physical
display, signing accounts). Each release candidate records the result per OS. Items are added by
the work package that introduces the unverifiable behavior.

## Repository bootstrap (WP-00)

| ID | Check | Linux | macOS | Windows |
|---|---|---|---|---|
| MV-00-1 | Fresh clone: `cargo xtask setup` activates hooks and reports tools correctly | pending | pending | verified 2026-11-21 (author machine) |
| MV-00-2 | `git commit` without `-s` or with a non-conventional subject is rejected by the hook | pending | pending | verified 2026-11-21 (author machine) |
| MV-00-3 | `.githooks/*` keep the executable bit after clone | pending | pending | index mode 100755 verified |

## Windows PTY (spike S4, ADR 0026)

CI covers Windows Server 2025 (build 26100) and the author machine covers Windows 11 (build 26300). Older ConPTY builds were not exercised; the WP-05 contract suite must be run there.

| ID | Check | Linux | macOS | Windows |
|---|---|---|---|---|
| MV-S4-1 | Run `cd spikes/s4-windows-pty && cargo run --release -- all` (needs `pwsh` for the pwsh rows) on Windows 10 1809, 21H2 and 22H2: compare experiments 2 (exit codes), 5 (startup bytes) and 8 (`drop(master)` with an unread flooding child must return, not hang) with ADR 0026 | n/a | n/a | pending |
| MV-S4-2 | Same run on Windows 11 24H2 and 25H2 retail builds | n/a | n/a | pending (build 26300 verified 2026-11-21) |
| MV-S4-3 | Windows on ARM64 (aarch64): experiments 1, 2, 5 | n/a | n/a | pending |

## Later work packages

Rows for IME, HiDPI, compositor behavior, OS drag and drop, installers and signing are added with
WP-07, WP-11, WP-16 and WP-18 (`docs/spec/docs/12-build-order.md`).
