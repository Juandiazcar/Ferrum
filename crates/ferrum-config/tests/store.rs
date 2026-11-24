//! Load/save behavior of versioned files: defaults, atomic save, corrupt-file recovery, caps
//! (`docs/02` §8 and §10, `FR-DATA-001`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ferrum_config::{ConfigError, SETTINGS_SCHEMA, Settings, Source, store};
use ferrum_testkit::temp::TempDir;

fn at(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

#[test]
fn missing_file_yields_defaults() {
    let dir = TempDir::new("config-store").unwrap();
    let loaded = Settings::load(&dir.join("settings.toml"), at(1)).unwrap();
    assert!(matches!(loaded.source, Source::Defaults));
    assert_eq!(loaded.value, Settings::default());
}

#[test]
fn default_settings_round_trip_through_a_file() {
    let dir = TempDir::new("config-store").unwrap();
    let path = dir.join("nested/dir/settings.toml");
    Settings::default().save(&path).unwrap();
    let loaded = Settings::load(&path, at(1)).unwrap();
    assert!(matches!(loaded.source, Source::File));
    assert_eq!(loaded.value, Settings::default());
    let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["settings.toml"], "the temporary file must be gone after the rename");
}

#[test]
fn save_replaces_an_existing_file() {
    let dir = TempDir::new("config-store").unwrap();
    let path = dir.join("settings.toml");
    Settings::default().save(&path).unwrap();
    let mut changed = Settings::default();
    changed.appearance.theme = "Dracula".to_owned();
    changed.save(&path).unwrap();
    assert_eq!(Settings::load(&path, at(1)).unwrap().value.appearance.theme, "Dracula");
}

#[test]
fn partial_file_and_unknown_keys_are_tolerated() {
    let text =
        "schema_version = 1\nfuture_key = 3\n[terminal]\ncopy_on_select = true\nfuture = \"x\"\n";
    let (settings, _) = store::parse::<Settings>(text, &SETTINGS_SCHEMA).unwrap();
    assert!(settings.terminal.copy_on_select);
    assert_eq!(settings.appearance, Settings::default().appearance);
}

#[test]
fn out_of_range_and_wrong_type_values_are_invalid() {
    for text in [
        "[terminal]\nscrollback_lines = 1000001",
        "[appearance]\nui_scale = 50",
        "[appearance]\nfont_size = \"big\"",
        "[appearance]\ncursor_color = \"red\"",
        "[terminal]\nbell = \"loud\"",
    ] {
        let err = store::parse::<Settings>(text, &SETTINGS_SCHEMA).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(_)), "{text}: {err}");
    }
}

#[test]
fn corrupt_file_is_set_aside_and_defaults_are_loaded() {
    let dir = TempDir::new("config-store").unwrap();
    let path = dir.join("settings.toml");
    std::fs::write(&path, "this is = = not toml").unwrap();
    let loaded = Settings::load(&path, at(1_700_000_000)).unwrap();
    let Source::Recovered { backup, cause } = loaded.source else { panic!("not recovered") };
    assert!(matches!(cause, ConfigError::Syntax(_)));
    assert_eq!(backup, dir.join("settings.toml.bad-1700000000"));
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "this is = = not toml");
    assert!(!path.exists());
    assert_eq!(loaded.value, Settings::default());

    std::fs::write(&path, "[appearance]\nui_scale = 1").unwrap();
    let again = Settings::load(&path, at(1_700_000_000)).unwrap();
    let Source::Recovered { backup, .. } = again.source else { panic!("not recovered") };
    assert_eq!(backup, dir.join("settings.toml.bad-1700000000-1"), "an existing backup is kept");
}

#[test]
fn file_from_a_newer_version_is_an_error_and_left_untouched() {
    let dir = TempDir::new("config-store").unwrap();
    let path = dir.join("settings.toml");
    std::fs::write(&path, "schema_version = 99\n").unwrap();
    let err = Settings::load(&path, at(1)).unwrap_err();
    assert!(matches!(err, ConfigError::TooNew { found: 99, supported: 1, .. }), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "schema_version = 99\n");
}

#[test]
fn oversized_and_non_utf8_files_are_corrupt() {
    let dir = TempDir::new("config-store").unwrap();
    let path = dir.join("settings.toml");
    let big = vec![b'#'; usize::try_from(store::MAX_FILE_BYTES).unwrap() + 1];
    std::fs::write(&path, big).unwrap();
    let err = store::load::<Settings>(&path, &SETTINGS_SCHEMA).unwrap_err();
    assert!(matches!(err, ConfigError::TooLarge { .. }) && err.is_corrupt_content());
    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    let err = store::load::<Settings>(&path, &SETTINGS_SCHEMA).unwrap_err();
    assert!(err.is_corrupt_content(), "{err}");
}

#[test]
fn write_atomic_into_a_path_without_file_name_fails_cleanly() {
    let dir = TempDir::new("config-store").unwrap();
    let err = store::write_atomic(&dir.join(".."), b"x").unwrap_err();
    assert!(matches!(err, ConfigError::Io { .. }));
}

#[test]
fn an_existing_backup_is_never_replaced() {
    let dir = TempDir::new("config-store").unwrap();
    let path = dir.join("settings.toml");
    std::fs::write(dir.join("settings.toml.bad-5"), "older backup").unwrap();
    std::fs::write(&path, "= broken").unwrap();
    let loaded = Settings::load(&path, at(5)).unwrap();
    let Source::Recovered { backup, .. } = loaded.source else { panic!("not recovered") };
    assert_eq!(backup, dir.join("settings.toml.bad-5-1"));
    assert_eq!(std::fs::read_to_string(dir.join("settings.toml.bad-5")).unwrap(), "older backup");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "= broken");
}

/// Linux file systems accept any bytes in a name (macOS APFS and Windows NTFS do not).
#[cfg(target_os = "linux")]
#[test]
fn corrupt_file_with_a_non_utf8_name_is_still_recovered() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = TempDir::new("config-store").unwrap();
    let path = dir.join(OsStr::from_bytes(b"settings-\xff.toml"));
    std::fs::write(&path, "= broken").unwrap();
    let loaded = Settings::load(&path, at(7)).unwrap();
    let Source::Recovered { backup, .. } = loaded.source else { panic!("not recovered") };
    assert_eq!(backup.as_os_str().as_bytes(), [path.as_os_str().as_bytes(), b".bad-7"].concat());
    assert!(backup.exists() && !path.exists());
}
