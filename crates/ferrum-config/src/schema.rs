//! Schema versions and the migration framework (`FR-DATA-004`, `docs/06` §7).
//!
//! Every persisted file carries a top-level integer `schema_version`. A file without it is version
//! 0. Migrations are pure functions over the parsed `TOML` table, one per version step, applied in
//! order until the table is at [`Schema::current`]; the typed value is deserialized only after
//! that, so old files never need old Rust types.

use toml::{Table, Value};

use crate::error::ConfigError;

/// Key that holds the schema version at the top level of every versioned file.
pub const VERSION_KEY: &str = "schema_version";

/// One migration step: rewrites a table of version `n` into version `n + 1`.
///
/// It must be pure (no I/O, no clock) and must not set [`VERSION_KEY`]; the framework does. An
/// `Err` carries a short reason for the user.
pub type Migration = fn(&mut Table) -> Result<(), String>;

/// The versions of one file kind and the steps between them.
#[derive(Debug, Clone, Copy)]
pub struct Schema {
    /// Short name of the file kind, used in errors (e.g. `"settings"`).
    pub name: &'static str,
    /// Version written by this build.
    pub current: u32,
    /// Steps ending at `current`: `migrations[i]` upgrades version `oldest + i` where
    /// `oldest = current - migrations.len()`.
    pub migrations: &'static [Migration],
}

/// A table brought to the current schema version.
#[derive(Debug, Clone, PartialEq)]
pub struct Migrated {
    /// The table without [`VERSION_KEY`], at [`Schema::current`].
    pub table: Table,
    /// Version found in the input (equal to `current` when nothing was migrated).
    pub from: u32,
}

impl Schema {
    /// Oldest version a migration chain starts from.
    ///
    /// # Errors
    /// [`ConfigError::BadSchema`] if the schema declares more steps than versions.
    pub fn oldest(&self) -> Result<u32, ConfigError> {
        u32::try_from(self.migrations.len())
            .ok()
            .and_then(|steps| self.current.checked_sub(steps))
            .ok_or(ConfigError::BadSchema(self.name))
    }

    /// Removes [`VERSION_KEY`] from `table` and applies every step up to [`Schema::current`].
    ///
    /// Migrating a table that is already current returns it unchanged (idempotent).
    ///
    /// # Errors
    /// [`ConfigError::BadVersion`], [`ConfigError::TooNew`], [`ConfigError::TooOld`] for a version
    /// outside the supported range; [`ConfigError::Migration`] when a step rejects the table.
    pub fn migrate(&self, mut table: Table) -> Result<Migrated, ConfigError> {
        let from = take_version(&mut table)?;
        let oldest = self.oldest()?;
        if from > self.current {
            return Err(ConfigError::TooNew {
                schema: self.name,
                found: from,
                supported: self.current,
            });
        }
        let first_step = from.checked_sub(oldest).ok_or(ConfigError::TooOld {
            schema: self.name,
            found: from,
            oldest,
        })?;
        let steps = self.migrations.iter().skip(usize::try_from(first_step).unwrap_or(usize::MAX));
        for (version, step) in (from..).zip(steps) {
            step(&mut table).map_err(|reason| ConfigError::Migration {
                schema: self.name,
                from: version,
                reason,
            })?;
        }
        Ok(Migrated { table, from })
    }

    /// Returns `table` with [`VERSION_KEY`] set to [`Schema::current`] (used when saving).
    pub fn stamp(&self, mut table: Table) -> Table {
        let _previous = table.insert(VERSION_KEY.to_owned(), Value::Integer(self.current.into()));
        table
    }
}

fn take_version(table: &mut Table) -> Result<u32, ConfigError> {
    match table.remove(VERSION_KEY) {
        None => Ok(0),
        Some(Value::Integer(n)) => u32::try_from(n).map_err(|_| ConfigError::BadVersion),
        Some(_) => Err(ConfigError::BadVersion),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::unnecessary_wraps, reason = "must match the Migration signature")]
    fn rename_a_to_b(table: &mut Table) -> Result<(), String> {
        if let Some(v) = table.remove("a") {
            let _old = table.insert("b".to_owned(), v);
        }
        Ok(())
    }

    fn double_b(table: &mut Table) -> Result<(), String> {
        match table.get_mut("b") {
            Some(Value::Integer(n)) => {
                *n = n.checked_mul(2).ok_or("b overflows")?;
                Ok(())
            }
            _ => Err("b is missing".to_owned()),
        }
    }

    const TWO_STEPS: Schema =
        Schema { name: "test", current: 2, migrations: &[rename_a_to_b, double_b] };

    fn table(text: &str) -> Table {
        text.parse().unwrap()
    }

    #[test]
    fn steps_run_in_order_from_the_file_version() {
        let out = TWO_STEPS.migrate(table("a = 21")).unwrap();
        assert_eq!(out.from, 0);
        assert_eq!(out.table, table("b = 42"));
        let out = TWO_STEPS.migrate(table("schema_version = 1\nb = 5")).unwrap();
        assert_eq!((out.from, out.table), (1, table("b = 10")));
    }

    #[test]
    fn current_table_is_unchanged_and_migration_is_idempotent() {
        let once = TWO_STEPS.migrate(table("a = 3")).unwrap();
        let twice = TWO_STEPS.migrate(TWO_STEPS.stamp(once.table.clone())).unwrap();
        assert_eq!(twice.from, 2);
        assert_eq!(twice.table, once.table);
    }

    #[test]
    fn versions_outside_the_chain_are_rejected() {
        let too_new = TWO_STEPS.migrate(table("schema_version = 3"));
        assert!(matches!(too_new, Err(ConfigError::TooNew { found: 3, supported: 2, .. })));
        let one_step = Schema { name: "t", current: 2, migrations: &[double_b] };
        assert!(matches!(
            one_step.migrate(table("b = 1")),
            Err(ConfigError::TooOld { oldest: 1, .. })
        ));
        for bad in ["schema_version = -1", "schema_version = \"1\"", "schema_version = 5000000000"]
        {
            assert!(matches!(TWO_STEPS.migrate(table(bad)), Err(ConfigError::BadVersion)), "{bad}");
        }
    }

    #[test]
    fn failing_step_reports_its_version() {
        let err = TWO_STEPS.migrate(table("schema_version = 1")).unwrap_err();
        assert!(matches!(err, ConfigError::Migration { from: 1, .. }), "{err}");
    }

    #[test]
    fn schema_with_more_steps_than_versions_is_an_error_not_a_panic() {
        let broken = Schema { name: "broken", current: 1, migrations: &[rename_a_to_b, double_b] };
        assert!(matches!(broken.migrate(Table::new()), Err(ConfigError::BadSchema("broken"))));
    }
}
