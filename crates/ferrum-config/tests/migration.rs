//! Settings migrations against the committed fixtures (`FR-DATA-004`, `docs/12` WP-04 exit:
//! synthetic v0 → v1) and migration idempotence (`docs/08` §2 property tests).

use ferrum_config::{Bell, CursorStyle, SETTINGS_SCHEMA, Settings, Source, store};
use ferrum_testkit::temp::TempDir;
use proptest::prelude::*;
use toml::Table;

const V0: &str = include_str!("../../../fixtures/config/v0/settings.toml");
const V1: &str = include_str!("../../../fixtures/config/v1/settings.toml");

#[test]
fn synthetic_v0_migrates_to_the_v1_fixture() {
    let migrated = SETTINGS_SCHEMA.migrate(V0.parse().unwrap()).unwrap();
    let expected = SETTINGS_SCHEMA.migrate(V1.parse().unwrap()).unwrap();
    assert_eq!(migrated.from, 0);
    assert_eq!(expected.from, 1);
    assert_eq!(migrated.table, expected.table);
}

#[test]
fn v0_file_loads_as_migrated_settings_and_saves_as_v1() {
    let dir = TempDir::new("config-migrate").unwrap();
    let path = dir.join("settings.toml");
    std::fs::write(&path, V0).unwrap();
    let loaded = store::load::<Settings>(&path, &SETTINGS_SCHEMA).unwrap();
    assert!(matches!(loaded.source, Source::Migrated { from: 0 }));
    let settings = loaded.value;
    assert_eq!(settings.appearance.theme, "Nord");
    assert_eq!(settings.appearance.font_size.get(), 14.5);
    assert_eq!(settings.appearance.cursor_style, CursorStyle::Bar);
    assert_eq!(settings.terminal.scrollback_lines.get(), 20_000);
    assert!(settings.terminal.copy_on_select);
    assert_eq!(settings.terminal.bell, Bell::Off);

    settings.save(&path).unwrap();
    let reloaded = store::load::<Settings>(&path, &SETTINGS_SCHEMA).unwrap();
    assert!(matches!(reloaded.source, Source::File));
    assert_eq!(reloaded.value, settings);
    let on_disk: Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    assert_eq!(on_disk.get("schema_version").and_then(toml::Value::as_integer), Some(1));
}

#[test]
fn v1_fixture_loads_without_migration() {
    let (settings, from) = store::parse::<Settings>(V1, &SETTINGS_SCHEMA).unwrap();
    assert_eq!(from, 1);
    assert_eq!(settings.appearance.font_family, "Iosevka");
}

/// Values a flat version 0 key may hold.
fn scalar() -> impl Strategy<Value = toml::Value> {
    prop_oneof![
        any::<bool>().prop_map(toml::Value::Boolean),
        any::<i64>().prop_map(toml::Value::Integer),
        "[a-z ]{0,12}".prop_map(toml::Value::String),
    ]
}

/// Keys a random table must not use as scalars: the version key and the version 1 sections.
const RESERVED: [&str; 3] = ["schema_version", "appearance", "terminal"];

/// Random version 0 tables: any subset of the moved keys plus unrelated keys.
fn v0_table() -> impl Strategy<Value = Table> {
    let key = prop_oneof![
        Just("theme".to_owned()),
        Just("font_size".to_owned()),
        Just("scrollback_lines".to_owned()),
        Just("bell".to_owned()),
        "[a-z_]{1,10}",
    ];
    prop::collection::btree_map(key, scalar(), 0..8)
        .prop_map(|map| map.into_iter().filter(|(k, _)| !RESERVED.contains(&k.as_str())).collect())
}

proptest! {
    #[test]
    fn migrating_a_migrated_table_changes_nothing(table in v0_table()) {
        let once = SETTINGS_SCHEMA.migrate(table).unwrap();
        let again = SETTINGS_SCHEMA.migrate(SETTINGS_SCHEMA.stamp(once.table.clone())).unwrap();
        prop_assert_eq!(again.from, SETTINGS_SCHEMA.current);
        prop_assert_eq!(again.table, once.table);
    }
}
