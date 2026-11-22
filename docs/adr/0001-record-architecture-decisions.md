# 0001. Record architecture decisions

- Status: accepted
- Date: 2026-11-20 (specification), file created in WP-00

## Context

Ferrum has hard memory and performance budgets and a long, mostly unattended implementation.
Decisions that shape the architecture, budgets or rules must be traceable, with the evidence that
justified them, so that later work does not silently drift from them.

## Decision

Architecture-impacting decisions are recorded as Architecture Decision Records in `docs/adr/`,
one file per decision, named `NNNN-title.md`, with: context, decision, consequences, status
(proposed, accepted, superseded). Requirements, budgets and acceptance criteria change only
through an ADR that states the measurement or reason, approved by the maintainer
(`docs/spec/docs/14-risks-and-decisions.md` §4). The initial decision index is `docs/14` §3.

## Consequences

- A gate is never loosened without an ADR; spikes S1 to S4 each end with one.
- Reviewers can reject an architectural change that lacks an ADR.
- ADRs are append-only: a changed decision is a new ADR that supersedes the old one.
