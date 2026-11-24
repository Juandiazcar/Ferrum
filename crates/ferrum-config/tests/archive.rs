//! Export/import of the `.ferrum` archive (`FR-DATA-002`, `FR-DATA-003`, `docs/12` WP-04 exit:
//! export/import round-trip) and the import hardening (`docs/10`).
//!
//! `zstd` is C code, which Miri cannot execute, so these tests are skipped under Miri.
#![cfg(not(miri))]
#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::fs;
use std::path::Path;

use ferrum_config::archive::{
    ArchiveLimits, ExportSources, MANIFEST_NAME, Section, export_archive, import_archive,
};
use ferrum_config::{ArchiveError, Settings};
use ferrum_testkit::temp::TempDir;

/// A config and a data directory shaped like the real ones.
fn sample_home(root: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let config = root.join("config");
    let data_dir = root.join("data");
    Settings::default().save(&config.join("settings.toml")).unwrap();
    write(&config.join("keybindings.toml"), b"schema_version = 1\n");
    write(&config.join("themes/Tokyo Night.toml"), b"[meta]\nname = \"Tokyo Night\"\n");
    write(&data_dir.join("projects/helpdesk.toml"), b"schema_version = 1\nname = \"Helpdesk\"\n");
    write(&data_dir.join("projects/.helpdesk.toml.tmp"), b"half-written");
    (config, data_dir)
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

#[test]
fn export_then_import_reproduces_every_file() {
    let home = TempDir::new("archive-home").unwrap();
    let (config, data_dir) = sample_home(&home);
    let sources = ExportSources { config_dir: &config, data_dir: &data_dir, scrollback_dir: None };
    let mut bytes = Vec::new();
    let exported = export_archive(&sources, &mut bytes, &ArchiveLimits::default()).unwrap();
    assert_eq!((exported.files, exported.skipped), (4, 1), "the hidden temp file is skipped");

    let target = TempDir::new("archive-import").unwrap();
    let staging = target.join("staging");
    let imported = import_archive(bytes.as_slice(), &staging, &ArchiveLimits::default()).unwrap();
    assert_eq!((imported.files, imported.bytes), (exported.files, exported.bytes));
    assert!(!imported.manifest.includes_scrollback);
    for rel in ["settings.toml", "keybindings.toml", "themes/Tokyo Night.toml"] {
        assert_eq!(read(&staging.join("config").join(rel)), read(&config.join(rel)), "{rel}");
    }
    let project = "projects/helpdesk.toml";
    assert_eq!(read(&staging.join("data").join(project)), read(&data_dir.join(project)));
    assert!(!staging.join("data/projects/.helpdesk.toml.tmp").exists());
    let loaded = Settings::load(&staging.join("config/settings.toml"), std::time::UNIX_EPOCH);
    assert_eq!(loaded.unwrap().value, Settings::default());
}

#[test]
fn scrollback_is_included_only_when_asked_and_missing_dirs_export_nothing() {
    let home = TempDir::new("archive-home").unwrap();
    write(&home.join("scrollback/session-1.bin"), &[0, 1, 2, 255]);
    let missing = home.join("missing");
    let sources = ExportSources {
        config_dir: &missing,
        data_dir: &missing,
        scrollback_dir: Some(&home.join("scrollback")),
    };
    let mut bytes = Vec::new();
    let _summary = export_archive(&sources, &mut bytes, &ArchiveLimits::default()).unwrap();
    let staging = home.join("staging");
    let imported = import_archive(bytes.as_slice(), &staging, &ArchiveLimits::default()).unwrap();
    assert!(imported.manifest.includes_scrollback);
    assert_eq!(read(&staging.join("scrollback/session-1.bin")), [0, 1, 2, 255]);
}

#[cfg(unix)]
#[test]
fn export_skips_symbolic_links() {
    let home = TempDir::new("archive-home").unwrap();
    let (config, data_dir) = sample_home(&home);
    write(&home.join("secret"), b"outside");
    std::os::unix::fs::symlink(home.join("secret"), config.join("link.toml")).unwrap();
    let sources = ExportSources { config_dir: &config, data_dir: &data_dir, scrollback_dir: None };
    let summary = export_archive(&sources, &mut Vec::new(), &ArchiveLimits::default()).unwrap();
    assert_eq!((summary.files, summary.skipped), (4, 2));
}

#[test]
fn export_refuses_names_that_cannot_be_imported_and_respects_limits() {
    let home = TempDir::new("archive-home").unwrap();
    let (config, data_dir) = sample_home(&home);
    let sources = ExportSources { config_dir: &config, data_dir: &data_dir, scrollback_dir: None };
    let small = ArchiveLimits { max_entries: 3, ..ArchiveLimits::default() };
    let err = export_archive(&sources, &mut Vec::new(), &small).unwrap_err();
    assert!(matches!(err, ArchiveError::TooLarge("entries")), "{err}");
    let tiny = ArchiveLimits { max_file_bytes: 4, ..ArchiveLimits::default() };
    let err = export_archive(&sources, &mut Vec::new(), &tiny).unwrap_err();
    assert!(matches!(err, ArchiveError::TooLarge("file size")), "{err}");
    // Windows cannot create such a name at all, so only Unix can produce one to export.
    if cfg!(unix) {
        write(&config.join("a|b.toml"), b"x");
        let err = export_archive(&sources, &mut Vec::new(), &ArchiveLimits::default()).unwrap_err();
        assert!(matches!(err, ArchiveError::BadPath(_)), "{err}");
    }
}

/// Builds a compressed archive from raw entries, bypassing the `tar` crate's own path checks.
fn raw_archive(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    let manifest = b"format_version = 1\nincludes_scrollback = false\n";
    let all = std::iter::once((MANIFEST_NAME, tar::EntryType::Regular, &manifest[..]));
    for (path, kind, contents) in all.chain(entries.iter().copied()) {
        let mut header = tar::Header::new_gnu();
        let name = &mut header.as_old_mut().name;
        name.fill(0);
        name.get_mut(..path.len()).unwrap().copy_from_slice(path.as_bytes());
        header.set_entry_type(kind);
        header.set_size(u64::try_from(contents.len()).unwrap());
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, contents).unwrap();
    }
    zstd::encode_all(tar.into_inner().unwrap().as_slice(), 3).unwrap()
}

fn import_err(bytes: &[u8], limits: &ArchiveLimits) -> ArchiveError {
    let dir = TempDir::new("archive-hostile").unwrap();
    let staging = dir.join("staging");
    let err = import_archive(bytes, &staging, limits).unwrap_err();
    assert!(!staging.exists(), "a failed import must leave no staging directory ({err})");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0, "nothing written outside staging");
    err
}

#[test]
fn hostile_paths_are_rejected() {
    let regular = tar::EntryType::Regular;
    for path in [
        "config/../../escape.toml",
        "/etc/passwd",
        "config/..\\..\\escape.toml",
        "config/C:/x.toml",
        "unknown/a.toml",
        "scrollback/a.bin",
        "config/.hidden",
        "config/NUL",
    ] {
        let err = import_err(&raw_archive(&[(path, regular, b"x")]), &ArchiveLimits::default());
        assert!(matches!(err, ArchiveError::BadPath(_)), "{path}: {err}");
    }
}

#[test]
fn links_and_special_entries_are_rejected() {
    for kind in [
        tar::EntryType::Symlink,
        tar::EntryType::Link,
        tar::EntryType::Directory,
        tar::EntryType::Char,
        tar::EntryType::Fifo,
    ] {
        let err =
            import_err(&raw_archive(&[("config/a.toml", kind, b"")]), &ArchiveLimits::default());
        assert!(matches!(err, ArchiveError::NotRegularFile(_)), "{kind:?}: {err}");
    }
}

#[test]
fn duplicates_and_size_caps_are_rejected() {
    let regular = tar::EntryType::Regular;
    let twice = raw_archive(&[("config/a.toml", regular, b"1"), ("config/a.toml", regular, b"2")]);
    assert!(matches!(import_err(&twice, &ArchiveLimits::default()), ArchiveError::Duplicate(_)));
    let three = raw_archive(&[
        ("config/a.toml", regular, b"123"),
        ("config/b.toml", regular, b"456"),
        ("data/c.toml", regular, b"789"),
    ]);
    let cases = [
        (ArchiveLimits { max_entries: 2, ..ArchiveLimits::default() }, "entries"),
        (ArchiveLimits { max_file_bytes: 2, ..ArchiveLimits::default() }, "file size"),
        (ArchiveLimits { max_total_bytes: 8, ..ArchiveLimits::default() }, "total size"),
    ];
    for (limits, what) in cases {
        assert!(
            matches!(import_err(&three, &limits), ArchiveError::TooLarge(w) if w == what),
            "{what}"
        );
    }
}

#[test]
fn bad_manifest_and_corrupt_streams_are_rejected() {
    let limits = ArchiveLimits::default();
    let mut no_manifest = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_cksum();
    no_manifest.append_data(&mut header, "config/a.toml", &b"x"[..]).unwrap();
    let bytes = zstd::encode_all(no_manifest.into_inner().unwrap().as_slice(), 3).unwrap();
    assert!(matches!(import_err(&bytes, &limits), ArchiveError::BadManifest(_)));

    let empty = zstd::encode_all(&[0_u8; 1024][..], 3).unwrap();
    assert!(matches!(import_err(&empty, &limits), ArchiveError::BadManifest(_)));
    assert!(matches!(import_err(b"not zstd at all", &limits), ArchiveError::Io { .. }));
    let truncated = raw_archive(&[("config/a.toml", tar::EntryType::Regular, &[7; 4096])]);
    let truncated = &truncated[..truncated.len() / 2];
    assert!(matches!(import_err(truncated, &limits), ArchiveError::Io { .. }));
}

#[test]
fn unsupported_format_version_is_rejected() {
    let mut tar = tar::Builder::new(Vec::new());
    let manifest = b"format_version = 2\nincludes_scrollback = false\n";
    let mut header = tar::Header::new_gnu();
    header.set_size(u64::try_from(manifest.len()).unwrap());
    header.set_cksum();
    tar.append_data(&mut header, MANIFEST_NAME, &manifest[..]).unwrap();
    let bytes = zstd::encode_all(tar.into_inner().unwrap().as_slice(), 3).unwrap();
    assert!(matches!(import_err(&bytes, &ArchiveLimits::default()), ArchiveError::BadManifest(_)));
}

#[test]
fn existing_staging_directory_is_refused() {
    let dir = TempDir::new("archive-staging").unwrap();
    let bytes = raw_archive(&[]);
    let err = import_archive(bytes.as_slice(), dir.path(), &ArchiveLimits::default()).unwrap_err();
    assert!(matches!(err, ArchiveError::StagingExists(_)));
    assert_eq!(Section::ALL.map(Section::dir_name), ["config", "data", "scrollback"]);
}
