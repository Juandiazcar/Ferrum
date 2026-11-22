---
name: ferrum-wp
description: Implement one Ferrum work package (WP-xx or spike Sx) end to end per the spec. Use when assigned a WP from docs/spec/docs/12-build-order.md.
---

# Implement a Ferrum work package

Spec root in this repo: `docs/spec/` (the spec's `docs/NN` = `docs/spec/docs/NN-*.md`).

## Before code
1. Read `CLAUDE.md`, then spec `docs/01`, `docs/03`, `docs/02`, `docs/05`, `docs/06`, and the WP section in `docs/12`.
2. List the WP's requirement IDs (FR-/NFR-/AC-) and budget lines it touches. Write them at the top of your working notes.
3. Check `PROGRESS.md` for prior state, PROVISIONAL decisions and open issues.

## Work
1. Branch per `ferrum-git` skill (`<type>/<scope>-<desc>`), from fresh `main`.
2. Tests/fixtures first (failing), then the smallest design that passes them.
3. Respect crate dependency edges (`docs/04` §2). Pure crates never touch OS code. Platform behavior behind traits.
4. Hard rules: no `unwrap`/`expect`/`panic!`/`todo!` in non-test code; no allocation in hot paths (`dhat` test); `#![forbid(unsafe_code)]` except pty/platform/render-blit; every `unsafe` has `// SAFETY:`; `thiserror` in libs, `anyhow` only in bins; `tracing` only, no prints; public items documented; each crate has README.md.
5. New dependency → `ferrum-deps` skill first.
6. Never fake/hard-code a measurement, never loosen a gate, never disable a failing test. If a measurement contradicts the spec: record evidence in `docs/spec/docs/14` as a risk/open question with options and continue with unaffected work.
7. Decisions with a default in `docs/14` §2: take default, mark `PROVISIONAL` in `PROGRESS.md`.
8. Code you cannot run (macOS/Windows-only, Linux-only): put behind trait, contract test, add steps to `docs/manual-verification.md`.

## Before handing off
1. Run `cargo xtask check` (once it exists; before WP-00 finishes: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`). Paste real output tail in your report.
2. Hot path / memory touched → `cargo xtask bench` / `memcheck`; report numbers.
3. Update `PROGRESS.md`: what was done, measured numbers, open issues, PROVISIONAL items, next step. Update ADRs (`docs/adr/`) and crate READMEs.
4. Commit atomically and push per `ferrum-git`. Open the PR. Do NOT merge — the auditor merges.

## Report back (final message)
- Branch, PR URL, commits.
- Requirement IDs covered and how (file:line).
- Check/bench/memcheck output (real numbers).
- Deviations from spec, PROVISIONAL choices, anything not verified and why.
