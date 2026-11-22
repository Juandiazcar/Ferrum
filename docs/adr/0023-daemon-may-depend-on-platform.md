# 0023. `ferrum-daemon` may depend on `ferrum-platform`

- Status: accepted
- Date: 2026-11-21 (maintainer approved)

## Context

`docs/04` §2 says the real `platform` implementations are injected by the `ferrumd` binary's
`main`. `ferrumd` lives inside the `ferrum-daemon` crate (lib plus bin), but the same table forbade
the edge `ferrum-daemon -> ferrum-platform`, so the binary could not name the implementations it
must inject.

## Decision

Add the edge `ferrum-daemon -> ferrum-platform`.

- The library code of `ferrum-daemon` uses `ferrum-platform` only through traits.
- The real implementations are constructed and wired only in the `ferrumd` bin (`src/main.rs`).
- Updated in the same change: `docs/04` §2 (table row and note), `xtask/deps-rules.toml`, the
  `ferrum-daemon` README, and the docs of `lib.rs` and `main.rs`.

## Consequences

- `cargo xtask check-deps` accepts `ferrum-daemon -> ferrum-platform` and still rejects every other
  edge, including `ferrum-ui -> ferrum-platform`.
- The rule that library code touches platform only through traits is by review, not by tooling; the
  audit of WP-06 verifies that no concrete platform type appears outside `src/main.rs`.
