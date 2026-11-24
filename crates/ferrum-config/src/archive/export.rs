//! Writing a `.ferrum` archive (`FR-DATA-002`).

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::path::{MAX_DEPTH, is_hidden, is_portable_component};
use super::{
    ARCHIVE_FORMAT_VERSION, ArchiveLimits, ENTRY_MODE, MANIFEST_NAME, Manifest, Section,
    ZSTD_LEVEL, ZSTD_WINDOW_LOG,
};
use crate::error::ArchiveError;

/// Directories whose files go into the archive.
#[derive(Debug, Clone, Copy)]
pub struct ExportSources<'a> {
    /// Ferrum's per-user config directory (settings, keybindings, themes).
    pub config_dir: &'a Path,
    /// Ferrum's per-user data directory (projects and layouts).
    pub data_dir: &'a Path,
    /// Directory of scrollback dumps; `None` unless the user chose "include scrollback"
    /// (`docs/10` §4).
    pub scrollback_dir: Option<&'a Path>,
}

/// What an export wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportSummary {
    /// Files written (besides the manifest).
    pub files: u32,
    /// Sum of their sizes.
    pub bytes: u64,
    /// Entries left out: hidden names (temporary files) and anything that is not a regular file
    /// or directory (symbolic links are never followed).
    pub skipped: u32,
}

/// One file to archive.
struct Item {
    archive_path: String,
    source: PathBuf,
    len: u64,
}

/// Writes the files of `sources` as a `.ferrum` archive to `out`. A missing directory exports
/// nothing for its section.
///
/// # Errors
/// [`ArchiveError::BadPath`] for a file name that could not be imported on every OS (non-UTF-8,
/// reserved or with forbidden characters), [`ArchiveError::TooLarge`] beyond `limits`,
/// [`ArchiveError::Io`] for read or write failures.
pub fn export_archive<W: Write>(
    sources: &ExportSources<'_>,
    out: W,
    limits: &ArchiveLimits,
) -> Result<ExportSummary, ArchiveError> {
    let mut walk = Walk { items: Vec::new(), skipped: 0, total: 0, limits };
    walk.dir(sources.config_dir, Section::Config.dir_name(), 0)?;
    walk.dir(sources.data_dir, Section::Data.dir_name(), 0)?;
    if let Some(dir) = sources.scrollback_dir {
        walk.dir(dir, Section::Scrollback.dir_name(), 0)?;
    }
    let manifest = Manifest {
        format_version: ARCHIVE_FORMAT_VERSION,
        includes_scrollback: sources.scrollback_dir.is_some(),
    };
    write_archive(out, manifest, &walk.items).map_err(|e| ArchiveError::io("", e))?;
    let files = u32::try_from(walk.items.len()).map_err(|_| ArchiveError::TooLarge("entries"))?;
    Ok(ExportSummary { files, bytes: walk.total, skipped: walk.skipped })
}

struct Walk<'l> {
    items: Vec<Item>,
    skipped: u32,
    total: u64,
    limits: &'l ArchiveLimits,
}

impl Walk<'_> {
    fn dir(&mut self, dir: &Path, prefix: &str, depth: usize) -> Result<(), ArchiveError> {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound && depth == 0 => return Ok(()),
            Err(e) => return Err(ArchiveError::io(dir, e)),
        };
        let mut children = entries
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ArchiveError::io(dir, e))?;
        children.sort();
        for child in children {
            self.entry(&child, prefix, depth)?;
        }
        Ok(())
    }

    fn entry(&mut self, path: &Path, prefix: &str, depth: usize) -> Result<(), ArchiveError> {
        let name = path.file_name().and_then(|n| n.to_str()).map(str::to_owned);
        let name =
            name.ok_or_else(|| ArchiveError::BadPath(path.to_string_lossy().into_owned()))?;
        if is_hidden(&name) {
            self.skipped = self.skipped.saturating_add(1);
            return Ok(());
        }
        if !is_portable_component(&name) {
            return Err(ArchiveError::BadPath(name));
        }
        let archive_path = format!("{prefix}/{name}");
        // symlink_metadata: a symbolic link is skipped, never followed.
        let meta = fs::symlink_metadata(path).map_err(|e| ArchiveError::io(path, e))?;
        if meta.is_dir() {
            if depth.saturating_add(1) >= MAX_DEPTH {
                return Err(ArchiveError::BadPath(archive_path));
            }
            return self.dir(path, &archive_path, depth.saturating_add(1));
        }
        if !meta.is_file() {
            self.skipped = self.skipped.saturating_add(1);
            return Ok(());
        }
        self.push(Item { archive_path, source: path.to_owned(), len: meta.len() })
    }

    fn push(&mut self, item: Item) -> Result<(), ArchiveError> {
        if item.len > self.limits.max_file_bytes {
            return Err(ArchiveError::TooLarge("file size"));
        }
        self.total = self.total.saturating_add(item.len);
        if self.total > self.limits.max_total_bytes {
            return Err(ArchiveError::TooLarge("total size"));
        }
        if u64::try_from(self.items.len()).unwrap_or(u64::MAX) >= u64::from(self.limits.max_entries)
        {
            return Err(ArchiveError::TooLarge("entries"));
        }
        self.items.push(item);
        Ok(())
    }
}

fn write_archive<W: Write>(out: W, manifest: Manifest, items: &[Item]) -> io::Result<()> {
    let mut encoder = zstd::stream::write::Encoder::new(out, ZSTD_LEVEL)?;
    encoder.window_log(ZSTD_WINDOW_LOG)?;
    encoder.include_checksum(true)?;
    let mut tar = tar::Builder::new(encoder);
    let manifest_text = toml::to_string(&manifest).map_err(io::Error::other)?;
    append(&mut tar, MANIFEST_NAME, manifest_text.as_bytes(), byte_len(manifest_text.as_bytes()))?;
    for item in items {
        let file = File::open(&item.source)?;
        append(&mut tar, &item.archive_path, ExactReader::new(file, item.len), item.len)?;
    }
    let encoder = tar.into_inner()?;
    let mut out = encoder.finish()?;
    out.flush()
}

fn append<W: Write>(
    tar: &mut tar::Builder<W>,
    path: &str,
    contents: impl Read,
    len: u64,
) -> io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(len);
    header.set_mode(ENTRY_MODE);
    header.set_mtime(0);
    tar.append_data(&mut header, path, contents)
}

fn byte_len(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).unwrap_or(u64::MAX)
}

/// Reads exactly `len` bytes of a file, failing if it shrank since its size was taken (the tar
/// header already promised `len` bytes).
struct ExactReader {
    inner: io::Take<File>,
    remaining: u64,
}

impl ExactReader {
    fn new(file: File, len: u64) -> Self {
        Self { inner: file.take(len), remaining: len }
    }
}

impl Read for ExactReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n == 0 && self.remaining > 0 && !buf.is_empty() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "file shrank during export"));
        }
        self.remaining = self.remaining.saturating_sub(u64::try_from(n).unwrap_or(u64::MAX));
        Ok(n)
    }
}
