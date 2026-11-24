# ferrum-config

**Responsibility:** settings model, TOML persistence, schema versions, migrations and export/import archive.

**Status:** implemented (WP-04): settings model, versioned TOML store, migration framework,
export/import archive.

| | |
|---|---|
| Allowed internal dependencies | none (foundation) |
| Must never depend on | any other ferrum crate, OS-specific code |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `serde` (derive), `toml` (file format), `tar` + `zstd` (archive), `unicode-normalization` (archive name collisions), `thiserror` (errors), `tracing` (one warning when a corrupt file is set aside) |
| Dev-dependencies | `ferrum-testkit` (temporary directories), `proptest` |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.

## Model

- **`Settings`** is the typed model of `settings.toml`, one table per section of `docs/09` §8:
  `appearance`, `terminal`, `shells`, `projects`, `window`, `performance`, `updates`,
  `import_export`. Every field
  has a default, so a partial file (or no file) is valid. Unknown keys are ignored (forward
  compatibility). Numeric settings are range-checked newtypes (`ScrollbackLines` 0–1 000 000,
  `UiScale` 75–200 %, …); an out-of-range value makes the file invalid instead of reaching the
  program.
- **`Keybindings`** (`keybindings.toml`, `KEYBINDINGS_SCHEMA`) holds the user's changes to the
  default shortcuts of `docs/09` §7: `[[bindings]] action = "…" keys = "Ctrl+Shift+T"` (no `keys`
  unbinds). Chords and action names are validated while parsing.
- **`Theme`** (`themes/<name>.toml`, `THEME_SCHEMA`) is the `docs/09` §9 format: `[meta]`
  (dark variant only in v1.0), `[colors]` with the 16-color `ansi` palette, `[ui]`, optional
  `[fonts]`.
- **`Schema`** versions one file kind. Every file has a top-level integer `schema_version`
  (missing = 0). A `Migration` is a pure function over the parsed `toml::Table` that upgrades
  version `n` to `n + 1`; `Schema::migrate` applies the steps in order before the typed value is
  deserialized, so old files never need old Rust types. Migrating a current table changes nothing
  (property-tested). `SETTINGS_SCHEMA` is at version 1; version 0 is the synthetic flat layout of
  `fixtures/config/README.md`.
- **`store`** reads files capped at 1 MiB (`MAX_FILE_BYTES`), migrates them in memory and reports
  where the value came from (`Source::Defaults`, `File`, `Migrated`, `Recovered`). An unusable
  file (bad TOML, invalid value, failed migration, too large) is moved to
  `<name>.bad-<unix seconds>` (never replacing an older backup; the name keeps non-UTF-8 bytes)
  and defaults are used (`docs/02` §10); only the error kind is logged, never file content or
  paths. A file from a newer Ferrum
  (`TooNew`) is an error and is never overwritten. Saves are atomic: write a hidden temporary
  sibling, `fsync`, rename over the target.
- **`archive`** writes and reads the `.ferrum` file (`FR-DATA-002`/`003`): `zstd`-compressed
  `tar` (ustar headers) whose first entry is `manifest.toml` (`format_version`,
  `includes_scrollback`), then `config/…`, `data/…` and, only when asked, `scrollback/…`. Export
  walks the directories without following symbolic links, skips hidden temporary files, writes
  fixed metadata (mode 0644, 0600 for scrollback, mtime 0, no owner) and refuses names that could
  not be imported everywhere: unportable names, paths too long for a ustar header, names equal up
  to letter case or Unicode NFC. Import trusts nothing: entries are read raw (GNU long-name and
  PAX entries are refused before their payload is read), only regular files with portable
  relative names under a known section (no `..`, absolute paths, drive letters, separators,
  hidden names or Windows device names; at most 8 levels), no duplicates, caps on entries, file
  size, total size and the decompressed stream, and a 4 MiB `zstd` window. Paths in errors are
  cut to 256 characters. `ArchiveLimits::from_settings` takes the caps from the Import / Export
  settings (defaults 10 000 entries, 5 184 MiB in total) and the per-session spill cap (largest
  file); export applies the same caps. Import extracts into a fresh staging directory created
  through `StagingFs` (the product passes `ferrum-platform`'s owner-only 0700/0600 functions) and
  removes it on any error; merging or replacing the live files and remapping project roots is the
  import dialog's job (WP-14).

Paths are always arguments; the per-user directories come from `ferrum-platform`. The current time
for backup names is an argument too, so tests use fixed times.

Values the spec does not fix are user settings (maintainer decision Q-30, `docs/14` §2): each
default and range is explained on its type, and the full list is in Q-30. Only anti-hostile input
caps (1 MiB file read, 16 backup names, 4 KiB manifest, 4 MiB `zstd` window, archive path
depth 8) stay constants, with their reason in the code.

## Tests and fuzzing

- `tests/migration.rs`: the synthetic v0 fixture migrates to the v1 fixture, loads as
  `Source::Migrated`, saves as v1; migration idempotence property; the v1 keybindings and theme
  fixtures load and invalid ones are rejected.
- `tests/store.rs`: defaults, atomic save, partial files, invalid values, corrupt-file recovery,
  newer-version refusal, size cap, backups never replaced, non-UTF-8 names (Linux).
- `tests/archive.rs`: export → import round trip of a config and data directory, optional
  scrollback, symbolic links and hidden files skipped, and hostile archives (path traversal,
  absolute and drive paths, links and special entries, long-name and PAX bombs, a `zstd` window
  above 4 MiB, duplicates, each size cap, bad manifest, corrupt or truncated streams); a failed
  import leaves nothing behind; entry modes; collisions (Linux) and ustar path length (Unix). Skipped under Miri (`zstd`
  is C code).
- `fuzz/` (own workspace, nightly): target `load` parses arbitrary bytes as `settings.toml` and
  checks that anything that loads saves and reloads to the same value. Run with
  `cargo +nightly fuzz run load` from this directory; the nightly workflow runs it 10 minutes.
