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
        /// File or directory involved.
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
    /// The archive stream could not be read or decompressed (corrupt, truncated, or its `zstd`
    /// window is larger than allowed).
    #[error("cannot read the archive stream")]
    Read(#[source] io::Error),
    /// The archive could not be written to the output.
    #[error("cannot write the archive")]
    Write(#[source] io::Error),
    /// Two files would get the same name on a case-insensitive or normalization-insensitive file
    /// system (macOS, Windows), so the archive could not be imported there (`AC-DATA-01`).
    #[error("{0:?} differs from another file only in letter case or Unicode normalization")]
    NameCollision(String),
}

/// Longest path quoted in an [`ArchiveError`]; longer ones are cut, so a hostile archive cannot
/// make error messages (and the UI or logs showing them) arbitrarily large.
pub const MAX_SHOWN_PATH_CHARS: usize = 256;

impl ArchiveError {
    /// `path` cut to [`MAX_SHOWN_PATH_CHARS`] characters, with `…` when it was longer.
    pub(crate) fn shown(path: &str) -> String {
        let mut chars = path.chars();
        let mut out: String = chars.by_ref().take(MAX_SHOWN_PATH_CHARS).collect();
        if chars.next().is_some() {
            out.push('…');
        }
        out
    }

    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shown_paths_are_cut_to_the_limit() {
        let short = "config/settings.toml";
        assert_eq!(ArchiveError::shown(short), short);
        let exact = "x".repeat(MAX_SHOWN_PATH_CHARS);
        assert_eq!(ArchiveError::shown(&exact), exact);
        let long = "é".repeat(MAX_SHOWN_PATH_CHARS + 50);
        let cut = ArchiveError::shown(&long);
        assert_eq!(cut.chars().count(), MAX_SHOWN_PATH_CHARS + 1);
        assert!(cut.ends_with('…'));
    }
}
