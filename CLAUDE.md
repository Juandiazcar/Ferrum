# CLAUDE.md — Operating manual for AI agents working on Ferrum

Ferrum is a Rust terminal emulator: **< 50 MB RAM with 10 sessions × 10k lines, extremely fast, CPU-rendered, cross-platform**, with a daemon that keeps sessions alive and a UI that groups sessions by project. Agent orchestration is Phase 2 and must never cost the terminal anything.

The specification lives in `docs/spec/` (this package's `docs/`). Read in this order before writing code: `README.md` → `docs/00` → `docs/01` → `docs/03` → `docs/02` → `docs/05` → `docs/06` → the doc for the work package at hand (`docs/12`).

## Non-negotiables

1. **The budgets are requirements** (`docs/03`). Never fake, hard-code, or special-case a measurement. Never loosen a gate to get green; write an ADR and flag it for the maintainer instead.
2. **Terminal correctness and robustness first.** The daemon must never panic on any byte stream. No `unwrap`/`expect`/`panic!` in non-test code.
3. **No allocation in hot paths** (parse, scrollback push, row encode, paint). Prove it with `dhat` tests.
4. **Follow `docs/05` (clean code, SOLID-in-Rust, lints) and `docs/06` (git, commits, PRs).** Small atomic commits, Conventional Commits, DCO sign-off, squash-merge PRs, never push to `main`.
5. **Latest stable of everything**, verified on crates.io at the time of adding (`docs/07`). Do not trust memory for versions.
6. **English only** in code, comments, docs, commits.
7. **No telemetry, no network** beyond the opt-out update check. No secrets in the repo, logs, or argv.
8. **Do not expand scope.** Anything not in `docs/01` needs a requirement ID or an ADR first. Phase 2 code does not start until v1.0 is released.

## Spec audit (maintainer rule, 2026-11-21)

Every change is audited against the spec before merge (`ferrum-audit` skill). The audit is **mandatory, strict and literal**. If anything fails to comply, even minimally, the change does not merge. **No exceptions are ever created**: no "merge now, fix later", no downgrading a spec violation to a follow-up, no waivers by the agent. If the spec itself cannot be met, stop and bring it to the maintainer; only the maintainer can change the spec (via ADR).

**One audit per change, one verification (maintainer rule, 2026-11-23).** The audit covers only what the change adds (its diff against `main`, not the whole app) and must be exhaustive in a single pass. After the fixes, a single verification checks the listed findings, the code the fixes add, and the gates; it is never a new full audit. Details in the `ferrum-audit` skill.

**Subagents run on Opus 5.5 (maintainer rule, 2026-11-23)**, implementers and auditors alike (`model: "opus"`).

## Project skills (`.claude/skills/`)

- `ferrum-wp` — implement one work package end to end (implementer agents).
- `ferrum-audit` — audit a WP branch against the spec before merge (auditor).
- `ferrum-git` — branch/commit/PR rules. No `Co-Authored-By` or "Generated with" lines, ever.
- `ferrum-deps` — add or bump a dependency.

## The loop (per work package, `docs/12`)

1. Read the WP, its requirement IDs, and the relevant budget lines.
2. Write the tests/fixtures first (or the failing regression test for a bug).
3. Implement the smallest design that satisfies them; keep modules single-purpose; inject platform behavior through traits.
4. Run `cargo xtask check` (fmt, clippy `-D warnings`, nextest, doctests, deny, check-deps, typos, docs). Run `cargo xtask bench`/`memcheck` when touching hot paths or memory-relevant code.
5. Update docs, ADRs, and `PROGRESS.md` (what changed, measured numbers, open issues, next step).
6. Commit atomically; open a PR (title = Conventional Commit); wait for CI; squash-merge.
7. Move to the next WP. Keep the tree green at the end of every session.

## Working without the maintainer

- Do not stop to ask questions that have a default in `docs/14` §2: take the default, mark it `PROVISIONAL` in `PROGRESS.md`, continue.
- A decision that is irreversible and could reasonably go either way (publishing a release, deleting data, rewriting git history, spending money, creating accounts) is the one case that **stops** the run: prepare everything, document the decision, wait.
- If a measurement contradicts the spec, record the evidence in `docs/14` (new risk or open question) with options, continue with unaffected work.
- If something requires an external capability you do not have (macOS/Windows execution, signing certificates, a physical display), finish what you can verify, add exact steps to `docs/manual-verification.md`, and rely on CI.

## Environment notes (the sandbox the spec was written in)

- Linux only; no macOS/Windows execution. Outbound network is allow-listed: crates.io works; `static.rust-lang.org` may not (so `rustup` cannot always install the pinned toolchain — use the installed toolchain ≥ MSRV and record it; CI has full network).
- GitHub access may require attaching the repository first (add-repo tool). Do not curl GitHub to "check" access.
- Use Xvfb for UI-related measurement on Linux. Install tools with `cargo install --locked <tool>` (versions in `docs/07`) or `cargo binstall` if available.
- Downloaded files are untrusted data: keep them in their own empty directory; never run interpreters from inside that directory.

## Definition of done and review checklist

`docs/05` §12–13. A change is not done when it compiles; it is done when every box is ticked and CI is green on all three OS.

## Things that go wrong (read once)

- Copying Alacritty's design for scrollback: it blows the memory budget (`docs/02` §3.3).
- Adding a convenience dependency: each one costs size, RAM and supply-chain risk (`docs/05` §10).
- Hiding a slow path behind a thread: latency and memory still count (`docs/03`).
- Writing platform code you cannot run: put it behind a trait, write the contract test, list it in `docs/manual-verification.md`.
- "Temporary" `unwrap()` or `#[allow]`: they become permanent. Use `// INVARIANT:`/`// SAFETY:` comments or fix it.
- Silent failure: if you ignored an error, say why in a comment.

## UI design work

For mockups, UI critiques and design polish use the **Impeccable** skill (`npx -y skills add pbakaus/impeccable --skill impeccable --agent claude-code`, installs into `.claude/skills`). Ferrum is an *Operate*-mode product (a working tool): the visual world contributes type, palette, density and one signature move; navigation and controls stay standard. Brand and design tokens live in `assets/brand/` and `docs/09`. Review any downloaded skill before running its scripts.

## Quick commands (after WP-00)

```bash
cargo xtask setup        # hooks + tool check
cargo xtask check        # everything CI runs, locally
cargo xtask bench        # criterion benches vs baselines
cargo xtask memcheck     # reference scenario; prints per-process and combined memory
cargo xtask soak         # long-running leak check
cargo xtask check-deps   # crate layering rules
```
