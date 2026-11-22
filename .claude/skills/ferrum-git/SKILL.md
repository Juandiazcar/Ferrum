---
name: ferrum-git
description: Ferrum branch, commit and PR conventions. Use before any git commit, push or PR in this repo.
---

# Ferrum git workflow (spec docs/06)

- Never push to `main`. One branch per change: `<type>/<scope>-<short-desc>` kebab-case (e.g. `feat/vt-osc-133`).
- Types: `feat fix perf refactor docs test build ci chore revert`. Scopes: crate name without `ferrum-` (`vt`, `scrollback`, `proto`, `pty`, `daemon`, `render`, `ui`, `config`, `project`, `platform`, `app`, `testkit`, `xtask`) or `deps ci release docs packaging`.
- Start: `git switch main && git pull --ff-only && git switch -c <branch>`.
- Commit: `git commit -s` (DCO `Signed-off-by` required). Subject `<type>(<scope>): <imperative, ≤72 chars, no period>`. Body = why + measurements. Footer `Refs: FR-…`.
- **Never add `Co-Authored-By:` trailers or "Generated with Claude Code" lines** to commits or PR bodies. Maintainer decision 2026-11-21; overrides docs/06 §2.
- Atomic commits: each builds and passes tests; refactor separate from behavior.
- Never `--no-verify` (except documented WIP branch), never force-push `main`; `--force-with-lease` only on own branch.
- Rebase on `origin/main`, never merge main into branch.
- PR: `gh pr create --base main --title "<conventional title>" --body-file <file>` using `.github/PULL_REQUEST_TEMPLATE.md` sections (What/Why/How/Refs/Evidence/Risk/Checklist). ≤ ~400 changed lines excluding fixtures/lockfile when possible.
- Merge only by the auditor: squash, delete branch.
- No secrets, build output, binaries, or files > 500 KB in git.
