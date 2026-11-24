//! Error types of the crate (`docs/05` §3: one `thiserror` enum per area).

use std::io;
use std::path::PathBuf;

/// Failure of loading, migrating or saving a versioned `TOML` file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The operating system refused an operation on the file.
    #[error("cannot access {path}")]
    Io {
        /// File or directory involved.
        path: PathBuf,
        /// Cause.
        #[source]
        source: io::Error,
    },
    /// The file is larger than the cap for configuration files.
    #[error("{path} is larger than {max} bytes")]
    TooLarge {
        /// The file.
        path: PathBuf,
        /// The cap in bytes.
        max: u64,
    },
    /// The file is not valid UTF-8 `TOML`.
    #[error("not valid TOML: {0}")]
    Syntax(#[from] toml::de::Error),
    /// The `TOML` is valid but does not describe a valid value (wrong type, out-of-range value).
    #[error("invalid value: {0}")]
    Invalid(String),
    /// `schema_version` is not a non-negative integer.
    #[error("schema_version is not a valid version number")]
    BadVersion,
    /// The file was written by a newer Ferrum; it is left untouched.
    #[error("{schema} schema version {found} is newer than the supported version {supported}")]
    TooNew {
        /// Name of the schema (file kind).
        schema: &'static str,
        /// Version in the file.
        found: u32,
        /// Newest version this build understands.
        supported: u32,
    },
    /// The file is older than the oldest version a migration exists for.
    #[error("{schema} schema version {found} is older than the oldest supported version {oldest}")]
    TooOld {
        /// Name of the schema (file kind).
        schema: &'static str,
        /// Version in the file.
        found: u32,
        /// Oldest version a migration chain starts from.
        oldest: u32,
    },
    /// A migration step rejected the document.
    #[error("migration of {schema} from version {from} failed: {reason}")]
    Migration {
        /// Name of the schema (file kind).
        schema: &'static str,
        /// Version the failing step starts from.
        from: u32,
        /// What the step could not handle.
        reason: String,
    },
    /// The schema declares more migrations than versions (a programming error, reported, never a
    /// panic).
    #[error("{0} schema declares more migrations than versions")]
    BadSchema(&'static str),
    /// The value could not be written as `TOML`.
    #[error("cannot serialize as TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
}

impl ConfigError {
    /// Whether the error means the file content is unusable (and may be set aside as a backup),
    /// as opposed to an I/O failure or a file from a newer Ferrum, which must be left untouched.
    pub fn is_corrupt_content(&self) -> bool {
        matches!(
            self,
            Self::TooLarge { .. }
                | Self::Syntax(_)
                | Self::Invalid(_)
                | Self::BadVersion
                | Self::TooOld { .. }
                | Self::Migration { .. }
        )
    }

    /// A short name of the error kind, safe to log: it never contains file content or paths.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io { .. } => "io",
            Self::TooLarge { .. } => "too_large",
            Self::Syntax(_) => "syntax",
            Self::Invalid(_) => "invalid",
            Self::BadVersion => "bad_version",
            Self::TooNew { .. } => "too_new",
            Self::TooOld { .. } => "too_old",
            Self::Migration { .. } => "migration",
            Self::BadSchema(_) => "bad_schema",
            Self::Serialize(_) => "serialize",
        }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}

/// Failure of exporting or importing a `.ferrum` archive.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ArchiveError {
    /// The operating system refused an operation on a file or directory.
    #[error("cannot access {path}")]
    Io {
        /// File or directory involved (empty for the archive stream itself).
        path: PathBuf,
        /// Cause.
        #[source]
        source: io::Error,
    },
    /// An entry path is absolute, escapes its section, or uses a name that is not portable.
    #[error("archive path {0:?} is not allowed")]
    BadPath(String),
    /// An entry is not a regular file (symlink, hard link, device, directory, …).
    #[error("archive entry {0:?} is not a regular file")]
    NotRegularFile(String),
    /// The same path appears twice.
    #[error("archive entry {0:?} appears more than once")]
    Duplicate(String),
    /// A file, the entry count or the total size is above the limits.
    #[error("archive exceeds the limit: {0}")]
    TooLarge(&'static str),
    /// The archive does not start with a valid manifest, or its format is unsupported.
    #[error("archive manifest is missing or invalid: {0}")]
    BadManifest(String),
    /// The staging directory for an import already exists.
    #[error("staging directory {0} already exists")]
    StagingExists(PathBuf),
}

impl ArchiveError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}
