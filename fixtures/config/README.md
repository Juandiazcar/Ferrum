# fixtures/config

Every historical config schema, one directory per version (`v0/` to `vN/`), used by the migration
tests of `ferrum-config` (docs/08 §4, docs/12 WP-04). A fixture is never edited once a version has
shipped: a new schema version adds a new directory and a migration step.

| Version | Files | Status |
|---|---|---|
| `v0/` | `settings.toml` | **Synthetic.** Ferrum never shipped it. It is the flat, unversioned layout (no `schema_version`, no sections) that exists to exercise the migration framework end to end. Its keys `theme`, `font_family`, `font_size`, `cursor_style` move to `[appearance]`; `scrollback_lines`, `copy_on_select`, `bell` move to `[terminal]`; other keys are kept and ignored. |
| `v1/` | `settings.toml` | First real layout: `schema_version = 1` and one table per settings section (`docs/09` §8). `v1/settings.toml` is also the expected result of migrating `v0/settings.toml`. |

Project files (`ferrum-project`) start at schema version 1 and have no older versions yet.
