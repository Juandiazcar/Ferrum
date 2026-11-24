//! Writing a `.ferrum` archive (`FR-DATA-002`).

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::path::{MAX_DEPTH, collision_key, is_hidden, is_portable_component};
use super::{
    ARCHIVE_FORMAT_VERSION, ArchiveLimits, CONFIG_ENTRY_MODE, MANIFEST_NAME, Manifest, Section,
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
    section: Section,
    archive_path: String,
    source: PathBuf,
    len: u64,
}

/// Writes the files of `sources` as a `.ferrum` archive to `out`. A missing directory exports
/// nothing for its section.
///
/// # Errors
/// [`ArchiveError::BadPath`] for a name that could not be imported on every OS (non-UTF-8,
/// reserved, forbidden characters, too long for a ustar header), [`ArchiveError::NameCollision`]
/// for two names equal up to case or Unicode normalization, [`ArchiveError::TooLarge`] beyond
/// `limits`, [`ArchiveError::Io`] when a source cannot be read, [`ArchiveError::Write`] when `out`
/// fails.
pub fn export_archive<W: Write>(
    sources: &ExportSources<'_>,
    out: W,
    limits: &ArchiveLimits,
) -> Result<ExportSummary, ArchiveError> {
    let mut walk = Walk {
        items: Vec::new(),
        file_keys: HashSet::new(),
        dir_keys: HashMap::new(),
        skipped: 0,
        total: 0,
        limits,
    };
    walk.root(sources.config_dir, Section::Config)?;
    walk.root(sources.data_dir, Section::Data)?;
    if let Some(dir) = sources.scrollback_dir {
        walk.root(dir, Section::Scrollback)?;
    }
    let manifest = Manifest {
        format_version: ARCHIVE_FORMAT_VERSION,
        includes_scrollback: sources.scrollback_dir.is_some(),
    };
    write_archive(out, manifest, &walk.items)?;
    let files = u32::try_from(walk.items.len()).map_err(|_| ArchiveError::TooLarge("entries"))?;
    Ok(ExportSummary { files, bytes: walk.total, skipped: walk.skipped })
}

struct Walk<'l> {
    items: Vec<Item>,
    /// Collision keys of the file paths.
    file_keys: HashSet<String>,
    /// Collision keys of the directory prefixes, with the exact prefix that claimed each.
    dir_keys: HashMap<String, String>,
    skipped: u32,
    total: u64,
    limits: &'l ArchiveLimits,
}

impl Walk<'_> {
    fn root(&mut self, dir: &Path, section: Section) -> Result<(), ArchiveError> {
        match fs::symlink_metadata(dir) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ArchiveError::io(dir, e)),
            Ok(_) => self.dir(dir, section, section.dir_name(), 0),
        }
    }

    fn dir(
        &mut self,
        dir: &Path,
        section: Section,
        prefix: &str,
        depth: usize,
    ) -> Result<(), ArchiveError> {
        let entries = fs::read_dir(dir).map_err(|e| ArchiveError::io(dir, e))?;
        let room = usize::try_from(self.limits.max_entries).unwrap_or(usize::MAX);
        let mut children = Vec::new();
        for entry in entries {
            // Bounded before sorting: a directory with millions of files stops here.
            if children.len() >= room.saturating_sub(self.items.len()).saturating_add(1) {
                return Err(ArchiveError::TooLarge("entries"));
            }
            children.push(entry.map_err(|e| ArchiveError::io(dir, e))?.path());
        }
        children.sort();
        for child in children {
            self.entry(&child, section, prefix, depth)?;
        }
        Ok(())
    }

    fn entry(
        &mut self,
        path: &Path,
        section: Section,
        prefix: &str,
        depth: usize,
    ) -> Result<(), ArchiveError> {
        let name = path.file_name().and_then(|n| n.to_str()).map(str::to_owned);
        let lossy = || ArchiveError::BadPath(ArchiveError::shown(&path.to_string_lossy()));
        let name = name.ok_or_else(lossy)?;
        if is_hidden(&name) {
            self.skipped = self.skipped.saturating_add(1);
            return Ok(());
        }
        if !is_portable_component(&name) {
            return Err(ArchiveError::BadPath(ArchiveError::shown(&name)));
        }
        let archive_path = format!("{prefix}/{name}");
        // symlink_metadata: a symbolic link is skipped, never followed.
        let meta = fs::symlink_metadata(path).map_err(|e| ArchiveError::io(path, e))?;
        if meta.is_dir() {
            if depth.saturating_add(1) >= MAX_DEPTH {
                return Err(ArchiveError::BadPath(ArchiveError::shown(&archive_path)));
            }
            return self.dir(path, section, &archive_path, depth.saturating_add(1));
        }
        if !meta.is_file() {
            self.skipped = self.skipped.saturating_add(1);
            return Ok(());
        }
        self.push(Item { section, archive_path, source: path.to_owned(), len: meta.len() })
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
        self.claim_names(&item.archive_path)?;
        // The ustar header must hold the path (at most 255 bytes, split at a `/`): import reads
        // headers raw and refuses GNU long-name entries.
        if tar::Header::new_ustar().set_path(&item.archive_path).is_err() {
            return Err(ArchiveError::BadPath(ArchiveError::shown(&item.archive_path)));
        }
        self.items.push(item);
        Ok(())
    }

    /// Refuses a path whose file name, or any directory on the way to it, equals another one up
    /// to letter case or Unicode NFC: a file `config/Themes` next to a directory `config/themes`,
    /// two files, or two spellings of one directory would collide on APFS or NTFS.
    fn claim_names(&mut self, path: &str) -> Result<(), ArchiveError> {
        let collision = || ArchiveError::NameCollision(ArchiveError::shown(path));
        for (end, _) in path.match_indices('/') {
            let dir = path.get(..end).ok_or_else(collision)?;
            let key = collision_key(dir);
            if self.file_keys.contains(&key) {
                return Err(collision());
            }
            match self.dir_keys.get(&key) {
                Some(exact) if exact != dir => return Err(collision()),
                Some(_) => {}
                None => {
                    let _new = self.dir_keys.insert(key, dir.to_owned());
                }
            }
        }
        let key = collision_key(path);
        if self.dir_keys.contains_key(&key) || !self.file_keys.insert(key) {
            return Err(collision());
        }
        Ok(())
    }
}

fn write_archive<W: Write>(out: W, manifest: Manifest, items: &[Item]) -> Result<(), ArchiveError> {
    let mut encoder =
        zstd::stream::write::Encoder::new(out, ZSTD_LEVEL).map_err(ArchiveError::Write)?;
    encoder.window_log(ZSTD_WINDOW_LOG).map_err(ArchiveError::Write)?;
    encoder.include_checksum(true).map_err(ArchiveError::Write)?;
    let mut tar = tar::Builder::new(encoder);
    let manifest_text =
        toml::to_string(&manifest).map_err(|e| ArchiveError::Write(io::Error::other(e)))?;
    let manifest_bytes = manifest_text.as_bytes();
    let manifest_entry =
        EntryHeader { path: MANIFEST_NAME, mode: CONFIG_ENTRY_MODE, len: byte_len(manifest_bytes) };
    append(&mut tar, &manifest_entry, manifest_bytes).map_err(ArchiveError::Write)?;
    for item in items {
        let file = File::open(&item.source).map_err(|e| ArchiveError::io(&item.source, e))?;
        let contents = ExactReader::new(file, item.len);
        let entry = EntryHeader {
            path: &item.archive_path,
            mode: item.section.entry_mode(),
            len: item.len,
        };
        append(&mut tar, &entry, contents).map_err(|e| ArchiveError::io(&item.source, e))?;
    }
    let encoder = tar.into_inner().map_err(ArchiveError::Write)?;
    let mut out = encoder.finish().map_err(ArchiveError::Write)?;
    out.flush().map_err(ArchiveError::Write)
}

/// What the ustar header of one entry says.
struct EntryHeader<'a> {
    path: &'a str,
    mode: u32,
    len: u64,
}

fn append<W: Write>(
    tar: &mut tar::Builder<W>,
    entry: &EntryHeader<'_>,
    contents: impl Read,
) -> io::Result<()> {
    let mut header = tar::Header::new_ustar();
    header.set_path(entry.path)?;
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(entry.len);
    header.set_mode(entry.mode);
    header.set_mtime(0);
    header.set_cksum();
    tar.append(&header, contents)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_equal_up_to_case_collide_between_files_and_directories() {
        let limits = ArchiveLimits::default();
        let mut walk = Walk {
            items: Vec::new(),
            file_keys: HashSet::new(),
            dir_keys: HashMap::new(),
            skipped: 0,
            total: 0,
            limits: &limits,
        };
        assert!(walk.claim_names("config/themes/a.toml").is_ok());
        assert!(walk.claim_names("config/themes/b.toml").is_ok(), "same directory, new file");
        assert!(walk.claim_names("config/Themes").is_err(), "file vs directory");
        assert!(walk.claim_names("config/THEMES/c.toml").is_err(), "two spellings of a directory");
        assert!(walk.claim_names("data/Projects").is_ok());
        assert!(walk.claim_names("data/projects/x.toml").is_err(), "directory vs file");
        assert!(walk.claim_names("config/themes/A.toml").is_err(), "file vs file");
    }
}
