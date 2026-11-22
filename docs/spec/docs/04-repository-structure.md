# 04 — Repository Structure

## 1. Layout

```
ferrum/
├─ Cargo.toml                  # workspace root, [workspace.package], [workspace.lints], [workspace.dependencies]
├─ Cargo.lock                  # committed (this is an application)
├─ rust-toolchain.toml         # pinned stable toolchain
├─ rustfmt.toml
├─ clippy.toml
├─ deny.toml                   # cargo-deny policy
├─ typos.toml
├─ cliff.toml                  # git-cliff changelog config
├─ LICENSE-MIT  LICENSE-APACHE
├─ README.md  CHANGELOG.md  CONTRIBUTING.md  CODE_OF_CONDUCT.md  SECURITY.md
├─ CLAUDE.md  AGENTS.md        # rules for AI agents working in the repo
├─ .gitignore  .gitattributes  .editorconfig
├─ .github/
│  ├─ workflows/ ci.yml  nightly.yml  release.yml  audit.yml
│  ├─ ISSUE_TEMPLATE/  PULL_REQUEST_TEMPLATE.md  CODEOWNERS  dependabot.yml
├─ .githooks/                  # pre-commit, commit-msg, pre-push (activated by `cargo xtask setup`)
├─ crates/
│  ├─ ferrum-vt/
│  ├─ ferrum-scrollback/
│  ├─ ferrum-proto/
│  ├─ ferrum-pty/
│  ├─ ferrum-daemon/           # lib + bin `ferrumd`
│  ├─ ferrum-render/
│  ├─ ferrum-ui/
│  ├─ ferrum-config/
│  ├─ ferrum-project/
│  ├─ ferrum-platform/
│  ├─ ferrum-app/              # bin `ferrum`
│  └─ ferrum-testkit/          # dev-only helpers (publish = false)
├─ xtask/                      # automation: setup, check, bench, memcheck, soak, dist, release
├─ assets/
│  ├─ fonts/                   # embedded default mono font + license files
│  ├─ icons/                   # app icons (all sizes/formats) + UI glyph set
│  ├─ themes/                  # built-in themes (TOML)
│  ├─ shell-integration/       # bash, zsh, fish, pwsh scripts
│  └─ terminfo/                # xterm-ferrum source
├─ fixtures/
│  ├─ vt/                      # escape-sequence fixtures + expected grid snapshots
│  ├─ output/                  # deterministic content generators/samples for memcheck
│  └─ config/                  # old-schema configs for migration tests
├─ benches/baselines/          # criterion baselines (committed JSON)
├─ packaging/
│  ├─ linux/ (deb, rpm, appimage, desktop file, metainfo)
│  ├─ macos/ (Info.plist, entitlements, dmg layout)
│  ├─ windows/ (wix, manifest, winget, scoop)
│  └─ arch/ (PKGBUILD) · homebrew/ (cask)
└─ docs/
   ├─ spec/                    # this specification (00–14)
   ├─ adr/                     # architecture decision records, one file each: 0001-title.md
   ├─ design/                  # mockups and UI notes
   └─ user/                    # end-user documentation
```

> When the repository is created, copy this specification package into `docs/spec/` and the files from `templates/` to their real locations (WP-00 in `docs/12`).

## 2. Dependency rules (enforced)

The **only** allowed internal dependency edges (an edge not listed is forbidden):

| Crate | May depend on (internal) | Notes |
|---|---|---|
| `ferrum-scrollback` | — | foundation; defines the `SpillStore` trait it needs |
| `ferrum-proto` | — | foundation; message types only |
| `ferrum-config` | — | foundation; settings model + persistence helpers take paths as arguments |
| `ferrum-project` | `config` (types only, if needed) | project model + `.git/HEAD` reader |
| `ferrum-pty` | — | defines `PtyBackend` traits **and** hosts the OS implementations behind `cfg`; consumers use only the traits |
| `ferrum-platform` | — | clipboard, dirs, single-instance, memory metric; exposes traits for consumers that need injection |
| `ferrum-vt` | `scrollback` | pure terminal state machine |
| `ferrum-render` | — | no windowing; writes into `&mut [u32]`; takes plain values (colors, metrics), not config types |
| `ferrum-daemon` | `vt`, `scrollback`, `proto`, `config`, `project`, `pty` | uses `pty`/`platform` through traits; real implementations are injected by the `ferrumd` binary's `main` |
| `ferrum-ui` | `render`, `proto`, `config`, `project` | **never** `daemon`, `pty`, `winit` |
| `ferrum-app` | everything (+ `winit`, `softbuffer`) | composition root for the `ferrum` binary |
| `ferrum-testkit` | any | only from `[dev-dependencies]` |

Reading order from the bottom up: foundations (`scrollback`, `proto`, `config`, `pty`, `platform`, `render`) → core (`vt`, `project`) → engines (`daemon`, `ui`) → composition roots (`ferrum-app`, `ferrumd` main). No cycles. Pure crates (`vt`, `scrollback`, `proto`, `config`, `project`, `ui`, `render`) never import platform code, and every OS-specific line lives in `pty` or `platform`.

`cargo xtask check-deps` parses `cargo metadata`, verifies the graph against `xtask/deps-rules.toml` (this table in machine-readable form), and fails on violations or cycles. Add an ADR when changing the rules.

## 3. Crate conventions

- Name: `ferrum-<area>`; Rust identifiers use `ferrum_<area>`.
- Every crate has `README.md` (responsibility, public API overview, dependency list, what it must never depend on).
- `lib.rs` starts with `//!` crate docs and the lint header (`#![forbid(unsafe_code)]` etc. as per `docs/05`).
- Modules: one concept per file; `mod.rs` is not used (use `foo.rs` + `foo/`), files ≤ ~400 lines (soft), functions ≤ ~40 lines (soft; see `docs/05` for the exception process).
- Public API surface is small: `pub(crate)` by default, `pub` only for things other crates need; re-export a curated API from `lib.rs`.
- Tests: unit tests in the same file (`#[cfg(test)] mod tests`), integration tests in `crates/<name>/tests/`, benches in `crates/<name>/benches/`, fixtures in `fixtures/`.
- Features: avoid. Allowed only for (a) optional heavy deps (`kitty-graphics`, `sixel`), (b) platform shims. Every feature is documented in the crate README and tested with `cargo hack --each-feature` in CI.
- `[dependencies]` entries always reference `{ workspace = true }`; versions live only in the root `[workspace.dependencies]`, with `default-features = false` and explicit features.

## 4. Naming

| Thing | Convention | Example |
|---|---|---|
| Types, traits, enums | `UpperCamelCase`, nouns | `SessionManager`, `PtyBackend` |
| Functions, methods | `snake_case`, verbs | `attach_session`, `encode_row` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_FRAME_LEN` |
| IDs | newtype wrappers | `SessionId(u64)`, `ProjectId(Uuid)` |
| Booleans | predicate names | `is_visible`, `has_focus` |
| Conversions | `as_` (cheap borrow), `to_` (expensive), `into_` (consuming), `from_` | |
| Modules | short, lowercase nouns | `grid`, `scrollback`, `osc` |
| Errors | `<Area>Error` enum | `ProtoError`, `ConfigError` |
| Actions/events | past tense for events, imperative for actions | `SessionExited`, `CloseSession` |
| Test names | behavior sentences | `reflow_keeps_wrapped_lines_joined` |
| Branches/commits | see `docs/06` | `feat/vt-osc-133` |

No abbreviations except widely known ones (`pty`, `osc`, `csi`, `id`, `cwd`, `ui`, `ipc`).

## 5. Where does X go? (quick decision list)

| I am adding… | It goes in |
|---|---|
| A new escape sequence | `ferrum-vt` (`parser/` handler + `fixtures/vt/` test) |
| A way to store/read lines | `ferrum-scrollback` |
| A new IPC message | `ferrum-proto` (+ bump minor protocol version + test) |
| OS-specific code | `ferrum-platform` or `ferrum-pty` behind a trait |
| A setting | `ferrum-config` (model + migration) + `ferrum-ui` (panel) |
| A widget | `ferrum-ui` |
| Pixel-level drawing | `ferrum-render` |
| Wiring of the above | `ferrum-app` / `ferrum-daemon` main |
| A dev script | `xtask` (never loose shell scripts, except in `packaging/`) |
