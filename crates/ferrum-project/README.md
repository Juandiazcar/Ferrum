# ferrum-project

**Responsibility:** project model, .git/HEAD reader, cwd tracking logic and startup recipes.

**Status:** project model, layouts and startup recipes implemented (WP-04). The cwd tracking logic
arrives with shell integration (WP-12).

| | |
|---|---|
| Allowed internal dependencies | ferrum-config (types only, if needed) |
| Must never depend on | ferrum-daemon, ferrum-ui, OS-specific code |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `ferrum-config` (types only: `Rgb`, `Schema`), `serde` (derive), `thiserror` (errors) |
| Dev-dependencies | `ferrum-testkit` (temporary directories) |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.

## Model

- **`Project`** (`FR-PROJ-001`): `ProjectId`, name, color (`#rrggbb`, default the accent
  `#E8590C`), optional icon glyph, 1 to 16 distinct root directories (the first is the default
  for new sessions, `FR-PROJ-003`), sessions in sidebar order (`SessionEntry`: stable
  `SessionKey`, optional name and profile, root index, last known cwd for respawning after a daemon
  restart, `FR-SESS-008`), a layout and an optional recipe. `Project::new` names a project after
  its first root (`suggest_name`, `FR-PROJ-002`).
- **`Layout`** (`FR-PROJ-005`): `Pane { session }` or `Split { axis, ratio, first, second }` with
  the ratio in per mille (1–999). `Layout::equal` builds equal splits; validation checks depth
  (≤ 32), ratios, and that every pane shows a distinct known session. Validation walks the tree
  with an explicit stack (no recursion on file input).
- **`Recipe`** (`FR-PROJ-007`, `docs/09` §3): sessions to open (name, profile, root index,
  relative sub-directory that may not leave its root, startup command) and an `Arrangement`
  (`single`, `side_by_side`, `stacked`). Presets `one_terminal` and `front_and_back`.
  `launches(roots)` resolves the start directories; `layout(keys)` builds the view.

## Persistence

The daemon (WP-06) stores each project as `data_dir/projects/<id>.toml` with
`ferrum_config::store::{save, load_or_recover}` and `PROJECT_SCHEMA` (version 1, no migrations
yet). Files are untrusted: after loading, call `Project::validate`. Caps: name 128 characters,
16 roots, 256 sessions, 32 recipe sessions, icon 32 bytes.
