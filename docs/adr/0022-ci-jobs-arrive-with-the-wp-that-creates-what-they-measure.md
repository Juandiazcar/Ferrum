# 0022. CI jobs arrive with the work package that creates what they measure

- Status: accepted
- Date: 2026-11-21 (maintainer approved)

## Context

`docs/06` §5 and §6 list a `memcheck` job in `ci.yml`, a required `memcheck (linux)` status check,
and a `nightly.yml` workflow. In WP-00 there are no binaries, benches or fuzz targets, so none of
them can measure anything. A job that passes without measuring is a fake gate (`CLAUDE.md`
non-negotiable 1); a job that fails blocks every PR.

## Decision

Each scheduled or measuring job is created by the work package that produces what it measures.

| Job | Lands in | Content |
|---|---|---|
| `nightly.yml` | WP-01 | fuzz smoke (10 min per target), Miri on pure crates, ASan on Linux, full benches with baseline comparison, `cargo audit` |
| `memcheck` in `ci.yml` | WP-06 | `cargo xtask memcheck --daemon-only` |
| Full reference-scenario memcheck under Xvfb, and `memcheck (linux)` as a required status check | WP-08 | `cargo xtask memcheck` with both binaries |
| Soak job in `nightly.yml` | WP-08 | first point where both binaries exist; `cargo xtask soak` |
| `release.yml` | WP-18 | already in `docs/12` |

Rule: the audit of each listed work package blocks its merge if that package's job is missing.

Until then `cargo xtask memcheck` and `cargo xtask soak` exit 2 with "not implemented until WP-xx"
and print no numbers, and branch protection does not require `memcheck (linux)`.

## Consequences

- `docs/06` §5 and §6 carry a note pointing here; the `docs/14` §3 index has the row.
- `ci.yml` cites this ADR where the `memcheck` job would be.
- The `xtask soak` stub names WP-08 (it previously said WP-17).
- No gate is loosened: each job, once added, uses the budgets of `docs/03` unchanged.
