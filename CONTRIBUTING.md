# Contributing to Ferrum

The rules are in the specification; this file is the short version. Read
[`CLAUDE.md`](CLAUDE.md) (it applies to people as well as AI agents) and
[`docs/spec/docs/05-development-rules.md`](docs/spec/docs/05-development-rules.md) and
[`06-git-and-release-workflow.md`](docs/spec/docs/06-git-and-release-workflow.md).

## Setup

1. Install Rust via `rustup`; `rust-toolchain.toml` pins the toolchain.
2. `cargo xtask setup` activates the git hooks and lists missing tools (versions in `docs/spec/docs/07`).
3. `cargo xtask check` runs everything CI runs. It must pass before you push.

## Workflow

- Branch from fresh `main`: `<type>/<scope>-<short-description>`; never push to `main`.
- Conventional Commits, atomic, signed off (`git commit -s`, DCO). The `commit-msg` hook checks it.
- One purpose per PR (about 400 changed lines at most, excluding fixtures and the lockfile); the PR
  title is a Conventional Commit and the PR template is filled in. PRs are squash-merged.
- Tests first; bug fixes include a regression test. No `unwrap`/`expect`/`panic!` outside tests.
- New dependencies follow `docs/spec/docs/05` §10 and `docs/07`: latest stable verified on crates.io,
  `default-features = false`, justified in the PR.
- English only in code, comments, docs and commits.

Budgets (memory, latency) are requirements, not goals: never loosen a gate to get green; write an ADR.
