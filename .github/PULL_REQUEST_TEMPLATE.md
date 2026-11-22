## What
<!-- 1–3 lines. PR title must be a Conventional Commit. -->

## Why
<!-- Problem or requirement. Link issues. -->

## How
<!-- Approach and trade-offs. Mention alternatives rejected. -->

Refs: <!-- FR-…, NFR-…, AC-… -->

## Evidence
<!-- UI change → screenshot/recording. Hot path or memory-relevant change → before/after numbers
     from `cargo xtask bench` / `cargo xtask memcheck`. -->

## Risk and rollback
<!-- What could break? How to revert? -->

## Checklist (Definition of Done — docs/05 §12)
- [ ] `cargo xtask check` passes locally; CI green on Linux/macOS/Windows
- [ ] Tests added/updated (bug fix includes a regression test)
- [ ] No new allocation/lock/syscall in hot paths; benches and memcheck unchanged or improved
- [ ] Docs updated (rustdoc, crate README, user docs, ADR if architectural)
- [ ] No new `unsafe` without `// SAFETY:` + test; no new dependency without a docs/07 entry
- [ ] Commits are atomic, Conventional, signed-off (DCO)
- [ ] Platform differences documented (docs/11) or none
