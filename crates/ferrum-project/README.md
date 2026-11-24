# ferrum-project

**Responsibility:** project model, .git/HEAD reader, cwd tracking logic and startup recipes.

**Status:** project model, layouts, startup recipes and the `.git/HEAD` reader implemented
(WP-04). The cwd tracking logic arrives with shell integration (WP-12).

| | |
|---|---|
| Allowed internal dependencies | ferrum-config (types only, if needed) |
| Must never depend on | ferrum-daemon, ferrum-ui, OS-specific code |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `ferrum-config` (types only: `Rgb`, `Schema`, `ProjectLimits` and its bounded types), `serde` (derive), `thiserror` (errors), `uuid` (project ids, Q-29), `unicode-segmentation` (single-glyph icons) |
| Dev-dependencies | `ferrum-testkit` (temporary directories for fixture repositories), `toml` (hand-built files) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.

## Model

- **`Project`** (`FR-PROJ-001`) is valid by construction: it is built from `ProjectParts` with
  `TryFrom` (deserialization goes through it too, so a file is checked at the boundary); to edit
  one, take `into_parts()`, change them and build again. Parts: `ProjectId` (a random UUID,
  Q-29), name, color (`#rrggbb`, default the accent `#E8590C`), optional icon (one grapheme
  cluster), at least one distinct absolute root directory (the first is the default for new
  sessions, `FR-PROJ-003`), sessions in sidebar order (`SessionEntry`: stable `SessionKey`,
  optional name and profile, root index, last absolute cwd for respawning after a daemon restart,
  `FR-SESS-008`), a layout and an optional recipe. Names and profiles are non-blank without
  control characters. `Project::new` names a project after its first root (`suggest_name`, the
  whole path for `/` or `C:\`, `FR-PROJ-002`).
- **Limits** (Q-30): files are held to hard caps, the upper bounds of the `[projects.limits]`
  settings (64 roots, 1 024-character names, 1 024 sessions, 256 recipe sessions);
  `Project::check_limits` applies the user's own limits (defaults 16, 128, 256, 32) when the
  user edits a project.
- **`Layout`** (`FR-PROJ-005`): `Pane { session }` or `Split { axis, ratio, first, second }` with
  the ratio in per mille (1–999). `Layout::equal` builds a balanced tree (any number of sessions,
  depth ⌈log2 n⌉). Validation checks depth (≤ 32), ratios, and that every pane shows a distinct
  known session, walking with an explicit stack; deserializing the tree is recursive, bounded by
  the `toml` parser's nesting limit and the 1 MiB file cap.
- **`Recipe`** (`FR-PROJ-007`, `docs/09` §3): sessions to open (name, profile, root index,
  relative sub-directory that may not leave its root, one-line startup command without control
  characters, at most 4 KiB) and an `Arrangement` (`single` or `side_by_side`, the two presets).
  Presets `one_terminal` and `front_and_back` name their sessions after their roots.
  `launches(roots)` resolves the start directories; `layout(keys)` builds the view.
- **`read_head`** (`FR-PROJ-004`): the branch without a git library (`gix` rejected, `docs/07`
  §3). From an absolute directory it walks up to the first `.git`: a directory (ordinary
  repository) or a file `gitdir: <path>` (linked worktree under `.git/worktrees/<name>`, submodule
  under `.git/modules/<name>`; relative paths are relative to the `.git` file's directory). It
  parses `HEAD` into `Head::Branch`, `Head::Detached` (40 or 64 hex digits, SHA-1 or SHA-256) or
  `Head::OtherRef`. Only regular files are opened (a FIFO would block), with a 4 KiB cap;
  anything git would not write (including `ref: refs/heads/` with no branch) is
  `GitError::Malformed`. Called on demand (focus or cwd change); there is no polling.

## Persistence

The daemon (WP-06) stores each project as `<data_dir>/ferrum/projects/<uuid>.toml` (`docs/02`
§8) with `ferrum_config::store::{save, load_or_recover}` and `PROJECT_SCHEMA` (version 1, no
migrations yet). An invalid file fails to load (`ConfigError::Invalid`) and is set aside like any
corrupt file. Anti-hostile constants: icon 64 bytes, command and paths 4 KiB, layout depth 32.
