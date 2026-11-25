# RESUME — session handoff

Handoff document so a new AI session (or a human) can pick up the work exactly where it stopped.
It is updated at every milestone (merge, WP state change, agent launch) and before a session's
context runs out. `PROGRESS.md` remains the per-WP record; this file is the "how to resume" view.

Last update: 2026-11-25: WP-00..WP-06 merged; WP-07 `ferrum-render` in progress.

## 1. How to resume (paste into a new Claude Code session, working directory `Ferrum/`)

```
Read RESUME.md, CLAUDE.md, PROGRESS.md and docs/spec/ (README → docs/00 → 01 → 03 → 02 → 05 → 06 → 12).
Follow the workflow in RESUME.md §3 from the point in §5. You (Opus) audit and merge;
Opus subagents implement. Ask the maintainer only on real doubts.
```

## 2. Project

- Ferrum: Rust terminal emulator, CPU-rendered, < 50 MB RAM with 10 sessions × 10k lines.
  Daemon (`ferrumd`) owns sessions/VT/scrollback; thin UI (`ferrum`).
- Repository: https://github.com/Juandiazcar/Ferrum.
- Spec: `docs/spec/`. Work order: `docs/spec/docs/12-build-order.md`.
- Agent manual: `CLAUDE.md`. Project skills: `.claude/skills/ferrum-{wp,audit,git,deps}`.
- ADRs: `docs/adr/` (0001, 0016, 0022–0028). Resolved decisions: `docs/spec/docs/14-risks-and-decisions.md` §2
  (next free open-question number: Q-35).

## 3. Workflow (agreed with the maintainer)

1. **Opus 5.5 subagents implement** each WP/spike in `docs/12` order (Agent tool, `model: "opus"`,
   background, in a git worktree of this repo). Parallel only when WPs are independent. Auditor subagents
   also run on Opus. (Maintainer decision 2026-11-23; before that implementers ran on Sonnet.)
2. **Opus audits** every PR against the spec (`ferrum-audit` skill): reads the full diff, re-runs
   `cargo xtask check`, the MSRV check (1.97), benches/memcheck/dhat when relevant, checks commits and CI.
   Never accepts a number it did not see produced.
3. **One audit per change, one verification** (maintainer rule 2026-11-23, `ferrum-audit` skill): the audit
   covers only the change's diff against `main` (never the whole app) and is exhaustive in one pass (every file,
   line by line, all findings at once). Blockers go back to the subagent with file:line and the spec clause.
   After the fixes, ONE verification checks only the listed findings, the code the fix commits add, and the
   gates. It is never a new full audit; something it finds outside that scope is fixed in the same batch.
4. Zero blockers and CI green on all three OS → Opus squash-merges and posts the audit result on the PR.
5. Continue WP after WP autonomously. Stop and ask only for real doubts, spec conflicts,
   maintainer decisions, or irreversible actions.

### Maintainer hard rules

- **No `Co-Authored-By:` trailer and no "Generated with" line** in any commit or PR (subagents are told too).
  Commits use `git commit -s` (DCO).
- **The spec audit is mandatory, strict and literal. No exceptions, ever.** Any deviation blocks the merge.
  Only the maintainer changes the spec (ADR or `docs/14`).
- `main` is protected: branches + PR + squash merge only. Never push or force-push `main`.
- Maintainer personal data (e.g. email) is private: never send it to external services.
- Repository content is English only.
- **The spec is followed to the letter** (2026-11-24): never edit `docs/spec/` to add exceptions, deviations or
  rationale; the code changes instead. OS behavior is identical unless `docs/11` §5 lists the difference. Only
  real maintainer decisions are recorded (`docs/14` §2).
- Every user-facing value the spec does not fix is a setting; internal caps are named constants with a reason,
  all listed in the Q-30 row (Q-30).
- The maintainer does not run benches: agents measure on hosted runners (Q-27 amended).

### Technical rules checked in every audit

`forbid(unsafe_code)` except pty/platform/render · no unwrap/expect/panic/indexing panics in non-test code ·
no prints outside an output module · functions ≤ ~40 lines and ≤ 4 params, no bool flag params (enums) ·
files ≤ ~400 lines, no `mod.rs` · narrow `#[allow]` with a reason/INVARIANT · every dependency: latest stable
verified on crates.io, `default-features = false`, recorded in `docs/07` · internal path deps carry
`version = "0.1.0"` · CI uses `--locked` · atomic commits, Conventional Commits with a real scope ·
`Q-NN` only for real spec conflicts (no self-chosen defaults, no PROVISIONAL).

### Stacked PR trap (learned)

GitHub closes a stacked PR when its base branch is deleted, and `gh pr merge` on an upper PR merges into its
stacked base, not `main`. Merge one at a time: before merging N run `gh pr edit <N+1> --base main`; after the
merge run `git rebase --onto origin/main <ORIGINAL tip of N> <N+1>` (snapshot every branch tip before the first
merge, never the rebased one), push, wait for CI, merge. A PR BEHIND `main` is rebased on `main` first; a branch
checked out in another worktree is detached there first. Never loop the chain blindly. Each subagent gets its own
scratch folder (shared ones got overwritten).

## 4. Status (2026-11-25)

Done and on `main` (CI green): WP-00 bootstrap, spikes S1–S4, WP-01 `ferrum-scrollback` (#9–#16), WP-02 `ferrum-vt`
(#36–#53, #55), WP-04 `ferrum-config` + `ferrum-project` (#57–#60), WP-05 `ferrum-pty` (#62–#64), WP-03
`ferrum-proto` (#65–#67), dhat harness fix (#89), WP-06 `ferrum-daemon` (#90–#96, last commit `a071f25`).

Measured: S1 0.68 MB/session · S2 system allocator on Linux/Windows, mimalloc on macOS · S3 repaint ~1.0 ms
typical / ~2.6–2.8 ms dense (i5-9400) · S4 direct ConPTY via windows-sys · WP-06 `memcheck --daemon-only`
10.5 / 10.7 / 10.3 MB (Linux / macOS / Windows, allowance 18 MB), flood echo p99 ≤ 2.8 ms (gate 10 ms).

Decisions taken in WP-02..WP-06: Q-28 (NFR-PERF-02 accepted as measured), Q-29 ProjectId = UUID, Q-30 settings vs
constants, Q-31 one overlapped `pty-io` thread on Windows, Q-32 daemon IPC on mio / `interprocess` only in the UI,
Q-33 Windows child exit on a named `pty-exit` thread, Q-34 flood gate p50 ±10 % (paired runs), p99 absolute.

## 5. Exact resume point

- **WP-07 `ferrum-render`**: Opus implementer in worktree `Ferrum-wt-wp07` (started 2026-11-25). Next: single-pass
  audit of its PRs → fixes → ONE verification → stacked merge.
- Then WP-08 `ferrum-ui` + `ferrum-app`. **Before WP-08: the maintainer measures on a real Mac (MV-S3-1, Q-20).**

## 6. What remains (21 WPs total; 13 left)

- Stage 3: WP-07 `ferrum-render` (2.5 ms dense on i5/CI — Q-19), WP-08 `ferrum-ui` + `ferrum-app`.
- Stage 4: WP-09 to WP-15. Stage 5: WP-16 to WP-20 (release candidate; WP-20 needs the maintainer).
- Known nit: the vt and scrollback fuzz crates' path deps lack `version = "0.1.0"`.

## 7. Initial maintainer instructions (translated from Spanish, intent preserved)

- Read the full Ferrum spec first, then clone https://github.com/Juandiazcar/Ferrum.git.
- Initialize `main` once; after that, work on branches. Read the whole spec, including binaries and templates.
- Develop with subagents running Sonnet; Opus audits everything they do against the spec. (Changed
  2026-11-23: all subagents run on Opus 5.5.)
- Opus merges; in every PR and commit it is forbidden to add itself as co-author.
- Create the project skills before starting development.
- General rule: the spec audit is 100% mandatory and strict; if anything fails even minimally, no exception is
  ever made — everything must comply to the letter.
- Keep going with this workflow, in parallel with subagents where possible; notify the maintainer only
  when there are doubts.
- 2026-11-23: one audit per change, scoped to what was uploaded (not the whole app), exhaustive; fix
  everything; one verification of the fixes that must not surface new findings.

Maintainer decisions already taken: CI jobs arrive with their WP (ADR 0022) · branch protection · daemon→platform
edge (ADR 0023) · Q-17 keep and re-measure · S4 direct windows-sys (ADR 0026) · S3 goal intact on worst case,
helper process for the dialog, maintainer measures on Mac · spill without mmap (ADR 0027) · `version` on
internal deps · cargo hack with `--locked`, without `--no-dev-deps`.
