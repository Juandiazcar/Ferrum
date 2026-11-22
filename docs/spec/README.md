# Ferrum — Specification Package

> A cross-platform terminal written in Rust. Extremely fast, fluid with many sessions, and light on RAM.
> Sessions are grouped by project (one or more repositories). AI-agent orchestration comes later (Phase 2) and must never cost the terminal anything.

Spec date: 2026-11-20 · Status: **ready for implementation** · Maintainer decisions of 2026-11-20 applied (`docs/14` §2) · License of the project: MIT OR Apache-2.0

## Hard targets (the definition of "done")

| Target | Value | Where measured |
|---|---|---|
| Memory | **< 50 MB total** (UI process + daemon, PSS) with **10 sessions × 10 000 lines** of scrollback | `docs/03`, gate `AC-MEM-01` |
| Fluidity | No dropped frames / no input lag with 20+ sessions open and one flooding output | `docs/08`, `AC-PERF-*` |
| Compatibility | CPU rendering only (no GPU requirement); Linux (X11+Wayland), macOS (arm64+x64), Windows 10 1809+ / 11 | `docs/11` |
| Scope v1.0 | Terminal + projects + vertical tabs + customization. **No agents, no usage meters.** | `docs/00`, `docs/01` |

## Reading order

| # | File | What it answers |
|---|---|---|
| 00 | [vision-and-scope](docs/00-vision-and-scope.md) | What Ferrum is, what it is not, principles, decisions already taken |
| 01 | [requirements](docs/01-requirements.md) | Every functional and non-functional requirement, with IDs |
| 02 | [architecture](docs/02-architecture.md) | Daemon + UI split, crates, data flow, protocol, threading |
| 03 | [memory-and-performance-budget](docs/03-memory-and-performance-budget.md) | How < 50 MB is reached and measured, byte by byte |
| 04 | [repository-structure](docs/04-repository-structure.md) | Workspace layout, dependency rules, file organization |
| 05 | [development-rules](docs/05-development-rules.md) | Clean code, SOLID in Rust, error handling, unsafe policy, lints |
| 06 | [git-and-release-workflow](docs/06-git-and-release-workflow.md) | Branches, commits, PRs, CI, versioning, releases |
| 07 | [tech-stack-and-versions](docs/07-tech-stack-and-versions.md) | Every dependency, verified latest version, why it was chosen |
| 08 | [testing-and-acceptance](docs/08-testing-and-acceptance.md) | Test pyramid, fuzzing, benchmarks, acceptance criteria |
| 09 | [ui-ux-spec](docs/09-ui-ux-spec.md) | Layout, tabs, projects, settings, shortcuts, theming |
| 10 | [security-and-privacy](docs/10-security-and-privacy.md) | Threat model, escape-sequence hardening, supply chain |
| 11 | [platform-notes](docs/11-platform-notes.md) | Linux / macOS / Windows specifics, packaging, signing |
| 12 | [build-order](docs/12-build-order.md) | Ordered work packages with exit criteria (for long unattended runs) |
| 13 | [phase2-agents-and-usage](docs/13-phase2-agents-and-usage.md) | Agents, providers, usage meters, vault, MCP — designed now, built later |
| 14 | [risks-and-decisions](docs/14-risks-and-decisions.md) | Known risks, open questions, decision log (ADRs) |

Also included:

- [`CLAUDE.md`](CLAUDE.md) / [`AGENTS.md`](AGENTS.md) — operating rules for an AI agent that works unattended on this repository.
- [`assets/brand/`](assets/brand) — logo and app icon (SVG) designed for Ferrum, plus `docs/design/brand.md`.
- [`templates/`](templates) — ready-to-copy config files (toolchain, workspace manifest, lints, `deny.toml`, CI, PR/issue templates). Copy them into the new repo in work package WP-00.

## Precedence when documents disagree

1. `docs/01-requirements.md` (what must be true)
2. `docs/03-memory-and-performance-budget.md` (numbers)
3. `docs/02-architecture.md` (how)
4. `docs/05-development-rules.md` and `docs/06-git-and-release-workflow.md` (process)
5. Everything else

If a conflict cannot be resolved, write an ADR in `docs/14-risks-and-decisions.md` and continue with the highest-precedence reading.

## Language

All repository content (code, comments, docs, commits, issues) is written in **English**, because the project is open source. Conversation with the maintainer may happen in Spanish.
