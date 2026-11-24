//! Migration of the synthetic version 0 of `settings.toml` to version 1.
//!
//! Version 0 never shipped: it is the flat, unversioned layout defined in
//! `fixtures/config/README.md` so the migration framework is exercised end to end
//! (`docs/12` WP-04 exit criterion). Version 1 groups the same keys into sections.

use toml::{Table, Value};

/// Flat version 0 key → (version 1 section, key).
const MOVED_KEYS: [(&str, &str, &str); 7] = [
    ("theme", "appearance", "theme"),
    ("font_family", "appearance", "font_family"),
    ("font_size", "appearance", "font_size"),
    ("cursor_style", "appearance", "cursor_style"),
    ("scrollback_lines", "terminal", "scrollback_lines"),
    ("copy_on_select", "terminal", "copy_on_select"),
    ("bell", "terminal", "bell"),
];

/// Moves the flat version 0 keys into their version 1 sections. A key already present in the
/// section wins over the flat one; other top-level keys are left alone (they are ignored on load).
pub(super) fn to_v1(table: &mut Table) -> Result<(), String> {
    for (old, section, key) in MOVED_KEYS {
        let Some(value) = table.remove(old) else { continue };
        let entry = table.entry(section).or_insert_with(|| Value::Table(Table::new()));
        let Value::Table(section_table) = entry else {
            return Err(format!("`{section}` is not a table"));
        };
        let _kept = section_table.entry(key).or_insert(value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_keys_move_into_sections_and_section_values_win() {
        let mut table: Table =
            "theme = \"Nord\"\nbell = \"off\"\nunknown = 1\n[terminal]\nbell = \"audible\"\n"
                .parse()
                .unwrap();
        to_v1(&mut table).unwrap();
        let expected: Table =
            "unknown = 1\n[appearance]\ntheme = \"Nord\"\n[terminal]\nbell = \"audible\"\n"
                .parse()
                .unwrap();
        assert_eq!(table, expected);
    }

    #[test]
    fn a_section_that_is_not_a_table_is_reported() {
        let mut table: Table = "theme = \"Nord\"\nappearance = 3\n".parse().unwrap();
        assert!(to_v1(&mut table).unwrap_err().contains("appearance"));
    }
}
