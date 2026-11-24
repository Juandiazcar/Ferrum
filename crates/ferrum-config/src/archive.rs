//! The `.ferrum` export/import archive (`FR-DATA-002`, `FR-DATA-003`, `docs/10` §4).
//!
//! An archive is a `tar` stream (ustar headers) compressed with `zstd`. Its first entry is
//! `manifest.toml` ([`Manifest`]); every other entry is a regular file under one of three
//! sections:
//!
//! ```text
//! manifest.toml
//! config/…        settings.toml, keybindings.toml, themes/*.toml   (Ferrum's config directory)
//! data/…          projects/*.toml (projects and layouts)            (Ferrum's data directory)
//! scrollback/…    only when the user chose "include scrollback"
//! ```
//!
//! Export walks the given directories without following symbolic links and writes entries with
//! fixed metadata (mode 0644, 0600 for scrollback; mtime 0; no owner names). It refuses names that
//! could not be imported everywhere: unportable names, paths too long for a ustar header, and
//! names that differ only in letter case or Unicode normalization.
//!
//! Import never trusts the archive. It reads entries raw, so GNU long-name and PAX extension
//! entries are rejected before their payload is read; it accepts only regular files with portable
//! relative names inside a known section; it enforces [`ArchiveLimits`] on the entries, on each
//! file, on the total and on the compressed stream; it bounds the `zstd` window; and it extracts
//! into a fresh staging directory created through [`StagingFs`] (owner-only on Unix when the
//! composition root passes `ferrum-platform`'s functions), removed again if anything fails.
//! Merging or replacing the live files and remapping project roots is the import dialog's job
//! (WP-14).

mod export;
mod import;
mod path;

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub use export::{ExportSources, ExportSummary, export_archive};
pub use import::{ImportSummary, import_archive};

pub use crate::error::MAX_SHOWN_PATH_CHARS;

use crate::settings::Settings;

/// File extension of an archive.
pub const ARCHIVE_EXTENSION: &str = "ferrum";

/// Name of the first entry of every archive.
pub const MANIFEST_NAME: &str = "manifest.toml";

/// Archive format written by this build; import accepts only this version.
pub const ARCHIVE_FORMAT_VERSION: u32 = 1;

/// Largest manifest accepted: 4 KiB. Anti-hostile cap: the manifest holds two short keys.
const MAX_MANIFEST_BYTES: u64 = 4 * 1_024;

/// `zstd` compression level: the library's own default, a good ratio for small TOML files at
/// low CPU cost.
const ZSTD_LEVEL: i32 = 3;

/// `zstd` window: 2^22 bytes = 4 MiB, set on the encoder and enforced as the maximum on the
/// decoder. Anti-hostile cap: a frame asking for a larger window (up to 2 GiB in the format) is
/// refused instead of making the decoder allocate it.
const ZSTD_WINDOW_LOG: u32 = 22;

/// Mode of exported configuration and data entries (owner read/write, others read), never taken
/// from the disk.
const CONFIG_ENTRY_MODE: u32 = 0o644;

/// Mode of exported scrollback entries: owner only, like spill files (`docs/10` §4).
const SCROLLBACK_ENTRY_MODE: u32 = 0o600;

/// Bytes one ustar entry may add to the stream besides its data: a 512-byte header and at most
/// 511 bytes of padding.
const TAR_ENTRY_OVERHEAD: u64 = 1_024;

/// Bytes of the two zero blocks that end a tar stream.
const TAR_END_BYTES: u64 = 1_024;

/// Bytes in a MiB.
const MIB: u64 = 1_024 * 1_024;

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

impl ArchiveLimits {
    /// The caps configured in `settings`: entries and total from Import / Export, the largest file
    /// is the per-session scrollback disk cap (the largest file Ferrum writes).
    pub fn from_settings(settings: &Settings) -> Self {
        let caps = settings.import_export;
        Self {
            max_entries: caps.max_entries.get(),
            max_file_bytes: u64::from(settings.performance.spill_disk_cap_mib.get()) * MIB,
            max_total_bytes: u64::from(caps.max_total_mib.get()) * MIB,
        }
    }

    /// Most bytes a valid tar stream within these caps can have, so the decompressed stream is
    /// cut off right after (no padding or extension entry can exceed it).
    fn max_stream_bytes(&self) -> u64 {
        let entries = u64::from(self.max_entries).saturating_add(1);
        self.max_total_bytes
            .saturating_add(MAX_MANIFEST_BYTES)
            .saturating_add(entries.saturating_mul(TAR_ENTRY_OVERHEAD))
            .saturating_add(TAR_END_BYTES)
    }
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self::from_settings(&Settings::default())
    }
}

/// How an import creates the staging directory and the files inside it. The composition roots
/// pass `ferrum-platform`'s owner-only functions (mode 0700 and 0600 on Unix, `docs/10` §4); this
/// crate stays free of OS-specific code. `create_file` must fail with
/// [`io::ErrorKind::AlreadyExists`] when the file exists, never replace it.
#[derive(Debug, Clone, Copy)]
pub struct StagingFs {
    /// Creates one directory; its parent exists; fails if it exists.
    pub create_dir: fn(&Path) -> io::Result<()>,
    /// Creates a new file for writing; fails if it exists.
    pub create_file: fn(&Path) -> io::Result<File>,
}

impl StagingFs {
    /// Plain `std` creation with the process defaults (umask on Unix, inherited ACLs on Windows).
    pub const STD: Self = Self { create_dir: create_dir_std, create_file: create_file_std };
}

fn create_dir_std(path: &Path) -> io::Result<()> {
    fs::create_dir(path)
}

fn create_file_std(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
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

    fn entry_mode(self) -> u32 {
        match self {
            Self::Config | Self::Data => CONFIG_ENTRY_MODE,
            Self::Scrollback => SCROLLBACK_ENTRY_MODE,
        }
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
