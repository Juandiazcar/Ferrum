# 06 — Git and Release Workflow

## 1. Model

**Trunk-based development** with short-lived branches and squash merges. `main` is always releasable.

- `main` — protected. No direct pushes. Linear history. Every commit on `main` passes CI on all three OS.
- Working branches — one per change, live ≤ 3 days when possible, named `<type>/<scope>-<short-description>` in kebab-case, e.g. `feat/vt-osc-133`, `fix/scrollback-reflow-wide-chars`, `perf/render-row-hash`, `docs/adr-allocator`, `chore/deps-weekly`.
- Release branches only for patching an older released line: `release/0.4` (created from the tag when needed).
- Tags — `vMAJOR.MINOR.PATCH` (annotated, signed when possible).

Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`, `revert`.

## 2. Commit messages — Conventional Commits 1.0

```
<type>(<scope>)<!>: <imperative summary, ≤ 72 chars, no period>

<body: what and why, wrapped at 80 cols; mention trade-offs and measurements>

Refs: FR-TERM-009
BREAKING CHANGE: <only when applicable; explain migration>
Signed-off-by: Name <email>
```

- **Scopes** are crate names without the prefix: `vt`, `scrollback`, `proto`, `pty`, `daemon`, `render`, `ui`, `config`, `project`, `platform`, `app`, `testkit`, `xtask`, plus `deps`, `ci`, `release`, `docs`, `packaging`.
- Subject in imperative mood ("add", "fix", not "added"). The body explains *why*; the diff shows *what*.
- Performance commits include before/after numbers from `cargo xtask bench`.
- `!` after the scope or a `BREAKING CHANGE:` footer marks breaking changes (config schema, protocol, CLI).
- DCO: every commit carries `Signed-off-by` (`git commit -s`). Optional but recommended: sign commits (SSH or GPG).
- Commits and PRs made with AI assistance carry **no** `Co-Authored-By:` trailer and no "Generated with" line (maintainer decision, 2026-11-21).
- **Atomic commits**: each commit builds, passes tests, and does one logical thing. Refactors and behavior changes are separate commits. Use `git add -p`.
- `commit-msg` hook validates the format locally; CI re-validates the PR title (squash merge uses the PR title as the commit subject).

## 3. How to push a change (step by step)

```bash
# 0. one-time per clone
cargo xtask setup                 # installs git hooks (core.hooksPath=.githooks), checks toolchain & tools

# 1. start from fresh main
git switch main && git pull --ff-only
git switch -c feat/vt-osc-133

# 2. work in small, atomic commits
git add -p
git commit -s                     # message per §2

# 3. before pushing: everything CI will run
cargo xtask check                 # fmt, clippy -D warnings, tests, deny, check-deps, typos, docs
cargo xtask bench --quick         # only if you touched hot paths
cargo xtask memcheck --quick      # only if you touched vt/scrollback/render/daemon

# 4. keep up to date by rebasing (never merge main into your branch)
git fetch origin && git rebase origin/main

# 5. push and open a PR
git push -u origin feat/vt-osc-133            # after a rebase: git push --force-with-lease
gh pr create --fill --base main               # PR title must be a valid Conventional Commit

# 6. CI must be green on Linux/macOS/Windows; address review; re-run checks
# 7. merge with "Squash and merge" (the PR title becomes the commit message); delete the branch
```

Rules:
- `--force-with-lease` is allowed **only** on your own feature branch. Never force-push `main` or `release/*`.
- Never `git commit --no-verify` to bypass hooks, except to push a documented WIP branch that will not be merged as-is.
- Secrets never enter history. If one does, rotate it first, then rewrite history only with maintainer approval.
- Generated files, build output, binaries, screenshots > 500 KB, and fonts beyond `assets/fonts/` stay out of git (use release assets or Git LFS only with an ADR).
- One PR = one purpose, ideally ≤ 400 changed lines excluding fixtures/lockfile. Larger work lands as a stack of small PRs behind a disabled path.

## 4. Pull requests

Template (`.github/PULL_REQUEST_TEMPLATE.md`, see `templates/`):

- What / Why / How (3–6 lines)
- Requirement IDs
- Screenshots or terminal recordings for UI changes
- Performance and memory numbers (before/after) for hot-path changes
- Risk and rollback notes
- Checklist = Definition of Done (`docs/05` §12)

Review rules:
- At least 1 approval (maintainer) — for a solo project, the author self-reviews with the checklist, waits for CI, and may self-merge after 10 minutes of review of their own diff.
- All review threads resolved. CI green. Branch up to date with `main`.
- Changes touching `unsafe`, `ferrum-proto`, the config schema, or `deny.toml` require explicit maintainer approval (CODEOWNERS).

## 5. Branch protection (GitHub settings for `main`)

> **Note (ADR 0022):** the `memcheck (linux)` required check is added with WP-08, not before; until then branch protection requires the remaining checks only.

- Require pull request; require status checks: `fmt-clippy` (fmt + clippy), `test (ubuntu-latest)`, `test (macos-latest)`, `test (windows-latest)`, `deny-and-deps` (cargo deny + deps-graph + typos + machete), `memcheck (linux)`, `docs`, `pr-title`.
- Require linear history; require conversation resolution; dismiss stale approvals; block force-pushes and deletions; require signed commits (optional).
- Allow only squash merges.

## 6. Continuous integration

> **Note (ADR 0022):** the `memcheck` job (WP-06 `--daemon-only`, WP-08 full), `nightly.yml` (WP-01) and its soak job (WP-08) arrive with the work package that creates what they measure; `release.yml` arrives in WP-18. See `docs/adr/0022-ci-jobs-arrive-with-the-wp-that-creates-what-they-measure.md`.

| Workflow | Trigger | Jobs |
|---|---|---|
| `ci.yml` | every PR + push to `main` | fmt · clippy (`-D warnings`) · tests on ubuntu/macos/windows (`cargo nextest`) + doctests · `cargo deny` · deps-graph · typos · docs (`cargo doc -D warnings`) · `memcheck` (Linux, Xvfb) · PR-title lint · feature matrix (`cargo hack`) |
| `nightly.yml` | cron | soak test · fuzz smoke (10 min per target) · Miri on pure crates · ASan on Linux · full benches with baseline comparison · `cargo audit` |
| `audit.yml` | cron daily + `Cargo.lock` changes | RustSec advisories, license check |
| `release.yml` | tag `v*` | build artifacts for all targets, package, sign, checksums, SBOM, provenance attestation, GitHub Release with changelog, update manifests |

Pin GitHub Actions by commit SHA (with the version in a comment) and let Dependabot update them. **Look up the current major version of each action when writing the workflows** (`gh api repos/<owner>/<repo>/releases/latest`); the versions are intentionally not hard-coded in this spec because they could not be verified at writing time.

Caching: use a Rust cache action keyed on `Cargo.lock` + toolchain; never cache `target/` across toolchain bumps.

## 7. Versioning

- SemVer for the application. Before 1.0: `0.MINOR.PATCH` where **MINOR may break** (config, protocol, behavior) and PATCH is fixes only.
- Independent version numbers, each with its own migration policy:
  - `protocol_version` (IPC) — major bump = incompatible; the UI refuses an older daemon and offers a restart.
  - `config schema_version` (per file) — monotonic integer; migrations are automatic and tested with fixtures.
- Crates inside the workspace share the workspace version (`version.workspace = true`); none is published to crates.io in v1.0 (`publish = false`).
- The version lives in one place (`[workspace.package]`).

## 8. Releases

```bash
git switch main && git pull --ff-only
cargo xtask release prepare 0.5.0     # bumps version, runs full checks, regenerates CHANGELOG via git-cliff, opens a PR "chore(release): v0.5.0"
# merge that PR (squash), then:
git switch main && git pull --ff-only
git tag -s v0.5.0 -m "Ferrum v0.5.0"
git push origin v0.5.0                # release.yml takes over
```

`release.yml` outputs:
- Linux: `.deb`, `.rpm`, `.AppImage`, `.tar.gz` (x86_64 + aarch64)
- macOS: one `.dmg` per architecture (arm64, x86_64) — ad-hoc signed; notarized only once an Apple Developer ID exists
- Windows: `.msi` and portable `.zip` (x86_64 + aarch64) — unsigned until a certificate or signing service is available
- `SHA256SUMS`, detached signatures, SBOM (CycloneDX), build provenance attestation
- Update manifest consumed by the in-app updater (signed with the project's minisign key)

Release checklist (manual, in the release PR): all gates green · manual smoke on 3 OS · Typometer latency recorded · memory numbers for the reference scenario pasted in the notes · known issues listed.

Hotfix: branch from the tag (`release/0.5` → `fix/…`), cherry-pick the fix from `main` where possible, tag `v0.5.1`.

## 9. Changelog

Generated from Conventional Commits with `git-cliff` (`cliff.toml`): sections *Added, Changed, Fixed, Performance, Security, Breaking*. `docs`, `test`, `ci`, `chore` are omitted. Entries link PRs and requirement IDs.

## 10. Issues and project hygiene

- Labels: `type/*`, `area/*` (crate names), `os/*`, `priority/*`, `good first issue`, `needs-repro`.
- Issue templates: bug (version, OS, steps, expected/actual, `ferrum --diagnostics` output), feature request, conformance bug (sequence + expected behavior + reference terminal).
- `SECURITY.md`: private disclosure path; supported versions = latest minor.
- `CODEOWNERS`: maintainer owns everything; extra review required on `crates/ferrum-proto`, `crates/ferrum-pty`, `crates/ferrum-platform`, `deny.toml`, `.github/`.
