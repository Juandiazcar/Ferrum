# ferrum-config

**Responsibility:** settings model, TOML persistence, schema versions, migrations and export/import archive.

**Status:** implemented (WP-04): settings model, versioned TOML store, migration framework.

| | |
|---|---|
| Allowed internal dependencies | none (foundation) |
| Must never depend on | any other ferrum crate, OS-specific code |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `serde` (derive), `toml` (file format), `thiserror` (errors), `tracing` (one warning when a corrupt file is set aside) |
| Dev-dependencies | `ferrum-testkit` (temporary directories), `proptest` |

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.

## Model

- **`Settings`** is the typed model of `settings.toml`, one table per section of `docs/09` §8:
  `appearance`, `terminal`, `shells`, `projects`, `window`, `performance`, `updates`. Every field
  has a default, so a partial file (or no file) is valid. Unknown keys are ignored (forward
  compatibility). Numeric settings are range-checked newtypes (`ScrollbackLines` 0–1 000 000,
  `UiScale` 75–200 %, …); an out-of-range value makes the file invalid instead of reaching the
  program. Keybindings and themes are separate files modeled with the settings UI (WP-13).
- **`Schema`** versions one file kind. Every file has a top-level integer `schema_version`
  (missing = 0). A `Migration` is a pure function over the parsed `toml::Table` that upgrades
  version `n` to `n + 1`; `Schema::migrate` applies the steps in order before the typed value is
  deserialized, so old files never need old Rust types. Migrating a current table changes nothing
  (property-tested). `SETTINGS_SCHEMA` is at version 1; version 0 is the synthetic flat layout of
  `fixtures/config/README.md`.
- **`store`** reads files capped at 1 MiB (`MAX_FILE_BYTES`), migrates them in memory and reports
  where the value came from (`Source::Defaults`, `File`, `Migrated`, `Recovered`). An unusable
  file (bad TOML, invalid value, failed migration, too large) is renamed to
  `<name>.bad-<unix seconds>` and defaults are used (`docs/02` §10). A file from a newer Ferrum
  (`TooNew`) is an error and is never overwritten. Saves are atomic: write a hidden temporary
  sibling, `fsync`, rename over the target.

Paths are always arguments; the per-user directories come from `ferrum-platform`. The current time
for backup names is an argument too, so tests use fixed times.

Defaults that the spec does not fix (chosen here, changeable in the settings UI): font size 13 pt,
line height 1.0, cursor blink on, scroll speed 3 lines, word characters `-_.~/:@`, images on.

## Tests and fuzzing

- `tests/migration.rs`: the synthetic v0 fixture migrates to the v1 fixture, loads as
  `Source::Migrated`, saves as v1; migration idempotence property.
- `tests/store.rs`: defaults, atomic save, partial files, invalid values, corrupt-file recovery,
  newer-version refusal, size cap.
- `fuzz/` (own workspace, nightly): target `load` parses arbitrary bytes as `settings.toml` and
  checks that anything that loads saves and reloads to the same value. Run with
  `cargo +nightly fuzz run load` from this directory; the nightly workflow runs it 10 minutes.
