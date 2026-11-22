---
name: ferrum-audit
description: Audit a Ferrum work-package branch/PR against the spec before merging. Use after a subagent reports a WP done, or for "audit WP-xx".
---

# Audit a Ferrum WP against the spec

Goal: decide merge / request changes. Evidence over trust: re-run checks yourself, read the diff, never accept a reported number you did not see produced.

## Steps
1. `git fetch && git diff main...<branch> --stat`, then read the full diff (not only summaries).
2. Re-read the WP in `docs/spec/docs/12-build-order.md`; check every exit criterion is met with evidence.
3. Run `cargo xtask check` (or fmt/clippy -D warnings/test before WP-00 ends) on the branch. Run benches/memcheck if the WP touches hot paths or memory.
4. Check against the spec, in precedence order (`docs/01` > `docs/03` > `docs/02` > `docs/05`/`06`):

| Area | What to verify |
|---|---|
| Requirements | Each claimed FR/NFR/AC ID is actually implemented and tested |
| Budgets (`docs/03`) | No unbounded `Vec`/`HashMap` growth; caches capped; numbers measured, not hard-coded |
| Architecture (`docs/02`, `docs/04` §2) | Crate edges allowed; pure crates free of OS code; daemon owns state; UI holds only visible rows |
| Rules (`docs/05`) | No `unwrap`/`expect`/`panic`/indexing panics in non-test code; `unsafe` only where allowed with `// SAFETY:`; no prints; no magic numbers; no TODO without issue; docs on public items; README per crate |
| Hot paths | `vt::parse`, `scrollback::push_line`, `proto::encode_row`, `render::paint_rows`: no alloc, `dhat` test present |
| Tests (`docs/08`) | Fixtures/property/fuzz targets the WP requires; no sleeps; no real user dirs |
| Security (`docs/10`) | Caps on params/OSC/frames; IPC permissions; no secrets/content in logs |
| Deps (`docs/07`) | Version verified on crates.io, `default-features = false`, entry in docs/07, not on ban list |
| Git (`docs/06`, `ferrum-git`) | Conventional Commit, DCO sign-off, **no Co-Authored-By / "Generated with" lines**, atomic commits, PR template filled |
| Honesty | `PROGRESS.md` matches reality; gates not loosened; failing tests not disabled; PROVISIONAL items marked |

5. Classify findings: **blocker** (spec violation, failing gate, fake evidence), **fix-before-merge** (rule violation), **follow-up** (issue/PROGRESS note).
6. Blockers or fix-before-merge → send back to the implementing subagent with exact file:line and the spec clause. Re-audit after fixes.
7. Clean + CI green on all OS → `gh pr merge <n> --squash --delete-branch`. Record audit result (findings, numbers) in the PR as a comment.
