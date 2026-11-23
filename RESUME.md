# RESUME — session handoff

Handoff document so a new AI session (or a human) can pick up the work exactly where it stopped.
It is updated at every milestone (merge, WP state change, agent launch) and before a session's
context runs out. `PROGRESS.md` remains the per-WP record; this file is the "how to resume" view.

Last update: 2026-11-23: WP-02 rebuilt as PR stack #36–#53 (round-2 fixes in progress); audit process changed to one audit + one verification; subagents on Opus.

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
- ADRs: `docs/adr/` (0001, 0016, 0022–0027). Resolved decisions: `docs/spec/docs/14-risks-and-decisions.md` §2
  (next free open-question number: Q-28).

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
merge run `git rebase --onto origin/main <old tip of N>`, push, wait for CI, merge. Never loop the chain blindly.

## 4. Status (2026-11-22)

Done and on `main` (CI green): WP-00 bootstrap, spikes S1–S4, RESUME.md (#17), WP-01 `ferrum-scrollback`
(PRs #9, #10, #14, #15, #16 — commit `98eb763`).

Measured: S1 0.68 MB/session (WP-01 crate side ~0.43 MB) · S2 system allocator on Linux/Windows, mimalloc on
macOS · S3 repaint ~1.0 ms typical / ~2.6–2.8 ms dense (i5-9400) · S4 direct ConPTY via windows-sys
(portable-pty has bugs), 5–6 MB/s.

## 5. Exact resume point

- **WP-02 `ferrum-vt` is stack #36 → #53** (18 stacked PRs; #36 `build(deps)` is based on `main`, each next
  PR on the previous branch, #53 `docs(vt)` carries ADR 0028). The old stack #18–#33 is closed (superseded).
- History: round-1 audit (44 blockers, 2026-11-23) → rework → round-2 audit (38 blockers; both lists are posted
  as comments on PR #53) → round-2 fixes **in progress** by an Opus subagent in worktree `Ferrum-wt-rework`
  (branch `wip/vt-stack-final`). The round-2 audit was a full re-audit; under the new rule (§3 item 3) the
  next step is ONLY the verification of the round-2 list + gates, then merge.
- Maintainer decisions on WP-02 (docs/14 §2): Q-21 NFR-PERF-02 measured WITH scrollback push, amended:
  LZ4 sealing of full chunks runs on the worker thread (docs/02 §5) · Q-22 reflow 10k lines ≤ 40 ms now ·
  Q-23 Alacritty ref-tests win everywhere · Q-24 extras table refuses when full + releases unreferenced ids ·
  Q-25 Alacritty literally, `selective_erasure` imported (DECSCA/DECSED/DECSEL removed) ·
  Q-26 `MAX_PENDING_CHUNKS = 1` with a docs/03 line keeping the session ≤ 1.0 MB · Q-27 the maintainer
  runs `cargo xtask bench` on the i5-9400 and saves the vt + `scrollback_seal` baselines before merge.
- Measured on Apple M2 (not the reference machine) at 9451268: `vt_parse/ascii_8mib` with push 510 MB/s,
  `sgr_heavy_8mib` 233 MB/s, reflow 10k lines 2.57 ms. 30-min CI fuzz of `parse`: clean on 2f0f7b8
  (run 45172536805); must be re-run on the final tip (temporary branch `ci/vt-fuzz-30min-*` with the
  workflow from commit 47d5652; delete those branches and `wip/*` after the merge).
- Next steps: verify the round-2 fixes → fuzz 30 min on the tip → maintainer i5 run (Q-27) → merge the
  stack one PR at a time with the stacked-PR procedure in §3 → WP-03.

## 6. What remains (21 WPs total; 19 left including WP-02)

- Stage 1: WP-03 `ferrum-proto`, WP-04 `ferrum-config` + `ferrum-project`.
- Stage 2: WP-05 `ferrum-pty` (direct windows-sys ConPTY, contract tests PT-01..PT-25), WP-06 `ferrum-daemon`
  (+ CI job `memcheck --daemon-only` per ADR 0022; wrapped-row search join; compression on the worker thread).
- Stage 3: WP-07 `ferrum-render` (must meet 2.5 ms dense on the i5-9400 and in CI — Q-19), WP-08 `ferrum-ui` +
  `ferrum-app` (full memcheck + required check + soak). **Before WP-08: the maintainer measures on a real Mac
  (MV-S3-1, Q-20).**
- Stage 4: WP-09 to WP-15. Stage 5: WP-16 to WP-20 (release candidate).

Maintainer to-do: recapture `benches/baselines/windows.json` on a quiet machine before release (it currently
reads +9–14% from machine state, not code).

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
