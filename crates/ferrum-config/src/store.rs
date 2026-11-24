//! Loading and saving versioned `TOML` files (`docs/02` §8).
//!
//! Reads are capped at [`MAX_FILE_BYTES`], tolerate unknown keys and migrate old versions in memory
//! (the next save writes the current version). Writes are atomic: a temporary file in the same
//! directory is written, flushed to disk (`fsync`) and renamed over the target, so a crash leaves
//! either the old or the new file, never a torn one. Paths are always arguments: this crate never
//! decides where files live.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;
use toml::Table;

use crate::error::ConfigError;
use crate::schema::Schema;

/// Largest configuration or project file read: 1 MiB. Real files are a few KiB; the cap bounds
/// memory against a corrupt or hostile file.
pub const MAX_FILE_BYTES: u64 = 1_024 * 1_024;

/// Suffix of the temporary file an atomic write goes through (hidden: a leading dot is added).
const TEMP_SUFFIX: &str = ".tmp";

/// Most numbered backup names tried (`.bad-<secs>`, `-1`, …). Anti-loop cap: it takes more than
/// 16 corrupt loads of one file within the same second to exhaust it, and then loading fails
/// with an error instead of overwriting a backup.
const MAX_BACKUP_ATTEMPTS: u32 = 16;

/// Where a loaded value came from.
#[derive(Debug)]
#[non_exhaustive]
pub enum Source {
    /// The file does not exist; the value is the default.
    Defaults,
    /// The file was read at the current schema version.
    File,
    /// The file was read at an older version and migrated in memory; saving writes the current
    /// version.
    Migrated {
        /// Version found in the file.
        from: u32,
    },
    /// The file was unusable: it was renamed to `backup` and the value is the default
    /// (`docs/02` §10 "Corrupt config file").
    Recovered {
        /// Where the unusable file now is.
        backup: PathBuf,
        /// Why it was unusable (for the UI notification).
        cause: ConfigError,
    },
}

/// A value read from a versioned file.
#[derive(Debug)]
pub struct Loaded<T> {
    /// The value.
    pub value: T,
    /// Where it came from.
    pub source: Source,
}

/// Parses `text`, migrates it to `schema.current` and deserializes it.
///
/// # Errors
/// [`ConfigError::Syntax`], the migration errors of [`Schema::migrate`], or
/// [`ConfigError::Invalid`] when the migrated table is not a valid `T`.
pub fn parse<T: DeserializeOwned>(text: &str, schema: &Schema) -> Result<(T, u32), ConfigError> {
    let table: Table = text.parse()?;
    let migrated = schema.migrate(table)?;
    let value = T::deserialize(migrated.table).map_err(|e| ConfigError::Invalid(e.to_string()))?;
    Ok((value, migrated.from))
}

/// Serializes `value` with `schema_version` set to `schema.current`.
///
/// # Errors
/// [`ConfigError::Serialize`] if `value` cannot be represented as a `TOML` table.
pub fn to_toml_string<T: Serialize>(value: &T, schema: &Schema) -> Result<String, ConfigError> {
    let table = Table::try_from(value)?;
    Ok(toml::to_string_pretty(&schema.stamp(table))?)
}

/// Loads `path`. A missing file yields `T::default()` with [`Source::Defaults`].
///
/// # Errors
/// I/O errors other than "not found", [`ConfigError::TooLarge`], and every error of [`parse`].
pub fn load<T>(path: &Path, schema: &Schema) -> Result<Loaded<T>, ConfigError>
where
    T: DeserializeOwned + Default,
{
    let Some(text) = read_capped(path)? else {
        return Ok(Loaded { value: T::default(), source: Source::Defaults });
    };
    let (value, from) = parse(&text, schema)?;
    let source = if from == schema.current { Source::File } else { Source::Migrated { from } };
    Ok(Loaded { value, source })
}

/// Like [`load`], but an unusable file ([`ConfigError::is_corrupt_content`]) is moved to
/// `<name>.bad-<unix seconds of now>` (an existing backup is never replaced: `-1`, `-2`, … are
/// tried) and the default is returned with [`Source::Recovered`].
///
/// # Errors
/// I/O errors, a file from a newer Ferrum ([`ConfigError::TooNew`]: left untouched, never
/// overwritten with defaults), or a failure to rename the unusable file.
pub fn load_or_recover<T>(
    path: &Path,
    schema: &Schema,
    now: SystemTime,
) -> Result<Loaded<T>, ConfigError>
where
    T: DeserializeOwned + Default,
{
    match load(path, schema) {
        Err(cause) if cause.is_corrupt_content() => {
            let backup = set_aside(path, now)?;
            // Only the error kind: the error text can quote a line of the file (which may hold
            // profile environment values) or name a user path (docs/05 §8, docs/10 §4).
            tracing::warn!(kind = cause.kind(), "unusable configuration file set aside");
            Ok(Loaded { value: T::default(), source: Source::Recovered { backup, cause } })
        }
        other => other,
    }
}

/// Serializes `value` at `schema.current` and writes it atomically to `path`, creating the parent
/// directory if needed.
///
/// # Errors
/// [`ConfigError::Serialize`] or [`ConfigError::Io`].
pub fn save<T: Serialize>(path: &Path, value: &T, schema: &Schema) -> Result<(), ConfigError> {
    let text = to_toml_string(value, schema)?;
    write_atomic(path, text.as_bytes())
}

/// Writes `bytes` to `path` through a temporary sibling file, `fsync` and rename.
///
/// # Errors
/// [`ConfigError::Io`] naming the file that failed; the temporary file is removed on failure.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    let (dir, temp) = temp_path(path)?;
    fs::create_dir_all(&dir).map_err(|e| ConfigError::io(&dir, e))?;
    let written = write_synced(&temp, bytes).and_then(|()| fs::rename(&temp, path));
    written.map_err(|e| {
        // Best effort: the temporary file may not exist, and the original error matters more.
        let _cleanup = fs::remove_file(&temp);
        ConfigError::io(path, e)
    })
}

/// Reads `path` as UTF-8, `None` when it does not exist.
pub(crate) fn read_capped(path: &Path) -> Result<Option<String>, ConfigError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ConfigError::io(path, e)),
    };
    let mut bytes = Vec::new();
    let _len = file
        .take(MAX_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| ConfigError::io(path, e))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(ConfigError::TooLarge { path: path.to_owned(), max: MAX_FILE_BYTES });
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| ConfigError::Invalid(format!("{} is not UTF-8", path.display())))
}

fn write_synced(temp: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(temp)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn temp_path(path: &Path) -> Result<(PathBuf, PathBuf), ConfigError> {
    let not_a_file = || ConfigError::io(path, io::Error::from(io::ErrorKind::InvalidInput));
    let name = path.file_name().ok_or_else(not_a_file)?;
    let dir = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(name);
    temp_name.push(TEMP_SUFFIX);
    let temp = dir.join(temp_name);
    Ok((dir, temp))
}

fn set_aside(path: &Path, now: SystemTime) -> Result<PathBuf, ConfigError> {
    let secs = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    // OsString, not `display()`: a non-UTF-8 path must keep its exact bytes.
    let mut base = path.as_os_str().to_owned();
    base.push(format!(".bad-{secs}"));
    for attempt in 0..MAX_BACKUP_ATTEMPTS {
        let mut candidate = base.clone();
        if attempt > 0 {
            candidate.push(format!("-{attempt}"));
        }
        let candidate = PathBuf::from(candidate);
        match move_without_overwrite(path, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(ConfigError::io(path, e)),
        }
    }
    Err(ConfigError::io(path, io::Error::from(io::ErrorKind::AlreadyExists)))
}

/// Moves `from` to `to`, failing with `AlreadyExists` instead of replacing an existing `to`
/// (`rename` would silently replace it on Unix, and an `exists()` check before it races). A hard
/// link needs no space and no copy, so it works on a full disk and for any file size; only where
/// the file system has no hard links is the file copied (into a `create_new` target).
fn move_without_overwrite(from: &Path, to: &Path) -> io::Result<()> {
    match fs::hard_link(from, to) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(e),
        Err(_) => copy_without_overwrite(from, to)?,
    }
    fs::remove_file(from)
}

fn copy_without_overwrite(from: &Path, to: &Path) -> io::Result<()> {
    let mut source = File::open(from)?;
    let mut target = OpenOptions::new().write(true).create_new(true).open(to)?;
    let copied = io::copy(&mut source, &mut target).and_then(|_| target.sync_all());
    if copied.is_err() {
        // Best effort: the partial backup is useless and the original is still in place.
        let _cleanup = fs::remove_file(to);
    }
    copied
}
