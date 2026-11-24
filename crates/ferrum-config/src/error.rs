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

    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}
