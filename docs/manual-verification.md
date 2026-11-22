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

## Later work packages

Rows for IME, HiDPI, compositor behavior, OS drag and drop, installers and signing are added with
WP-07, WP-11, WP-16 and WP-18 (`docs/spec/docs/12-build-order.md`).
