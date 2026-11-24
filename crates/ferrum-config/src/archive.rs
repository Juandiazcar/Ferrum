//! The `.ferrum` export/import archive (`FR-DATA-002`, `FR-DATA-003`, `docs/10` §4).
//!
//! An archive is a `tar` stream compressed with `zstd`. Its first entry is `manifest.toml`
//! ([`Manifest`]); every other entry is a regular file under one of three sections:
//!
//! ```text
//! manifest.toml
//! config/…        settings.toml, keybindings.toml, themes/*.toml   (per-user config directory)
//! data/…          projects/*.toml (projects and layouts)            (per-user data directory)
//! scrollback/…    only when the user chose "include scrollback"
//! ```
//!
//! Export walks the given directories without following symbolic links and writes entries with
//! fixed metadata (mode 0644, mtime 0, no owner names). Import never trusts the archive: it accepts
//! only regular files with portable relative names inside a known section, enforces
//! [`ArchiveLimits`], bounds the decompression window, and extracts into a fresh staging directory
//! that is removed again if anything fails. Merging or replacing the live files from the staging
//! directory, and remapping project roots, is the import dialog's job (WP-14).

mod export;
mod import;
mod path;

use serde::{Deserialize, Serialize};

pub use export::{ExportSources, ExportSummary, export_archive};
pub use import::{ImportSummary, import_archive};

/// File extension of an archive.
pub const ARCHIVE_EXTENSION: &str = "ferrum";

/// Name of the first entry of every archive.
pub const MANIFEST_NAME: &str = "manifest.toml";

/// Archive format written by this build; import accepts only this version.
pub const ARCHIVE_FORMAT_VERSION: u32 = 1;

/// Largest manifest accepted: 4 KiB (it holds two keys).
const MAX_MANIFEST_BYTES: u64 = 4 * 1_024;

/// `zstd` compression level: the library default, a good ratio for small TOML files.
const ZSTD_LEVEL: i32 = 3;

/// `zstd` window: 2^22 bytes = 4 MiB, set on the encoder and enforced as the maximum on the
/// decoder so a hostile archive cannot make the decoder allocate a larger window.
const ZSTD_WINDOW_LOG: u32 = 22;

/// File mode written for every entry (owner read/write, others read); never taken from the disk.
const ENTRY_MODE: u32 = 0o644;

/// Default most entries in one archive besides the manifest.
pub const DEFAULT_MAX_ENTRIES: u32 = 10_000;

/// Default largest single file: 256 MiB, the per-session disk cap of scrollback
/// (`docs/10` §3 item 11).
pub const DEFAULT_MAX_FILE_BYTES: u64 = 256 * 1_024 * 1_024;

/// Default largest sum of all file sizes: 1 GiB.
pub const DEFAULT_MAX_TOTAL_BYTES: u64 = 1_024 * 1_024 * 1_024;

/// Size caps applied to both export and import, so whatever exports can be imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    /// Most entries besides the manifest.
    pub max_entries: u32,
    /// Largest single file in bytes.
    pub max_file_bytes: u64,
    /// Largest sum of file sizes in bytes.
    pub max_total_bytes: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        }
    }
}

/// Top-level directory of an archive entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// Files of the per-user config directory.
    Config,
    /// Files of the per-user data directory.
    Data,
    /// Scrollback dumps (optional).
    Scrollback,
}

impl Section {
    /// Every section, in archive order.
    pub const ALL: [Self; 3] = [Self::Config, Self::Data, Self::Scrollback];

    /// Directory name of the section inside the archive and the staging directory.
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Data => "data",
            Self::Scrollback => "scrollback",
        }
    }

    fn from_dir_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.dir_name() == name)
    }
}

/// Contents of `manifest.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Manifest {
    /// Archive format version ([`ARCHIVE_FORMAT_VERSION`]).
    pub format_version: u32,
    /// Whether the archive has a `scrollback/` section (`docs/10` §4: only when selected).
    pub includes_scrollback: bool,
}
