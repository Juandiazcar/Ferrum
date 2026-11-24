//! Reading a `.ferrum` archive into a staging directory (`FR-DATA-003`, `docs/10`).

use std::fs;
use std::io::{self, Read};
use std::path::Path;

use super::path::parse_entry;
use super::{
    ARCHIVE_FORMAT_VERSION, ArchiveLimits, MANIFEST_NAME, MAX_MANIFEST_BYTES, Manifest, Section,
    StagingFs, ZSTD_WINDOW_LOG,
};
use crate::error::ArchiveError;

/// What an import extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImportSummary {
    /// The archive's manifest.
    pub manifest: Manifest,
    /// Files extracted (besides the manifest).
    pub files: u32,
    /// Sum of their sizes.
    pub bytes: u64,
}

/// Extracts the archive read from `input` into `staging`, which must not exist yet; its parent
/// must. Directories and files are created through `fs` (pass `ferrum-platform`'s owner-only
/// functions in the product, [`StagingFs::STD`] elsewhere). Files land in `staging/config/…`,
/// `staging/data/…` and `staging/scrollback/…`. On any error `staging` is removed again, so a
/// failed import leaves nothing behind.
///
/// # Errors
/// [`ArchiveError::StagingExists`]; [`ArchiveError::BadManifest`] if the first entry is not a
/// supported manifest; [`ArchiveError::BadPath`], [`ArchiveError::NotRegularFile`] (including
/// GNU long-name and PAX extension entries), [`ArchiveError::Duplicate`] for an unsafe entry;
/// [`ArchiveError::TooLarge`] beyond `limits`; [`ArchiveError::Read`] for a corrupt stream or a
/// `zstd` window above 4 MiB; [`ArchiveError::Io`] for a write failure.
pub fn import_archive<R: Read>(
    input: R,
    staging: &Path,
    limits: &ArchiveLimits,
    fs: StagingFs,
) -> Result<ImportSummary, ArchiveError> {
    match (fs.create_dir)(staging) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ArchiveError::StagingExists(staging.to_owned()));
        }
        Err(e) => return Err(ArchiveError::io(staging, e)),
    }
    let result = extract(input, &Target { staging, fs }, limits);
    if result.is_err() {
        // Best effort: the extraction error is what the user needs; a leftover directory is
        // harmless and the next import uses a new staging name.
        let _cleanup = fs::remove_dir_all(staging);
    }
    result
}

/// Where and how files are written.
struct Target<'a> {
    staging: &'a Path,
    fs: StagingFs,
}

fn extract<R: Read>(
    input: R,
    target: &Target<'_>,
    limits: &ArchiveLimits,
) -> Result<ImportSummary, ArchiveError> {
    let mut decoder = zstd::stream::read::Decoder::new(input).map_err(ArchiveError::Read)?;
    decoder.window_log_max(ZSTD_WINDOW_LOG).map_err(ArchiveError::Read)?;
    let capped = CappedReader { inner: decoder, remaining: limits.max_stream_bytes() };
    let mut archive = tar::Archive::new(capped);
    // raw: GNU long-name/long-link and PAX entries come back as entries (and are refused) instead
    // of being read into memory by the tar crate before any limit applies.
    let mut entries = archive.entries().map_err(read_error)?.raw(true);
    let first =
        entries.next().ok_or_else(|| ArchiveError::BadManifest("archive is empty".into()))?;
    let manifest = read_manifest(first.map_err(read_error)?)?;
    let mut summary = ImportSummary { manifest, files: 0, bytes: 0 };
    for entry in entries {
        let entry = entry.map_err(read_error)?;
        if summary.files >= limits.max_entries {
            return Err(ArchiveError::TooLarge("entries"));
        }
        let size = entry.size();
        if size > limits.max_file_bytes {
            return Err(ArchiveError::TooLarge("file size"));
        }
        summary.bytes = summary.bytes.saturating_add(size);
        if summary.bytes > limits.max_total_bytes {
            return Err(ArchiveError::TooLarge("total size"));
        }
        extract_file(entry, target, manifest)?;
        summary.files = summary.files.saturating_add(1);
    }
    Ok(summary)
}

fn read_error(error: io::Error) -> ArchiveError {
    if error.kind() == io::ErrorKind::FileTooLarge {
        ArchiveError::TooLarge("archive stream")
    } else {
        ArchiveError::Read(error)
    }
}

fn read_manifest<R: Read>(entry: tar::Entry<'_, R>) -> Result<Manifest, ArchiveError> {
    let bad = |reason: &str| ArchiveError::BadManifest(reason.to_owned());
    if !is_regular(&entry) || entry.path_bytes().as_ref() != MANIFEST_NAME.as_bytes() {
        return Err(bad("the first entry is not manifest.toml"));
    }
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(bad("manifest is too large"));
    }
    let mut text = String::new();
    let _len = entry.take(MAX_MANIFEST_BYTES).read_to_string(&mut text).map_err(read_error)?;
    let manifest: Manifest = toml::from_str(&text).map_err(|e| bad(e.message()))?;
    if manifest.format_version != ARCHIVE_FORMAT_VERSION {
        return Err(bad("unsupported archive format version"));
    }
    Ok(manifest)
}

fn extract_file<R: Read>(
    mut entry: tar::Entry<'_, R>,
    target: &Target<'_>,
    manifest: Manifest,
) -> Result<(), ArchiveError> {
    let path = entry_path(&entry)?;
    if !is_regular(&entry) {
        return Err(ArchiveError::NotRegularFile(ArchiveError::shown(&path)));
    }
    let bad_path = || ArchiveError::BadPath(ArchiveError::shown(&path));
    let (section, parts) = parse_entry(&path).ok_or_else(bad_path)?;
    if section == Section::Scrollback && !manifest.includes_scrollback {
        return Err(bad_path());
    }
    let dest = create_parents(target, section, &parts)?;
    let mut file = match (target.fs.create_file)(&dest) {
        Ok(file) => file,
        // A second entry with the same path (or the same path on a case-insensitive file system)
        // is refused instead of overwriting the first one.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ArchiveError::Duplicate(ArchiveError::shown(&path)));
        }
        Err(e) => return Err(ArchiveError::io(&dest, e)),
    };
    let size = entry.size();
    let copied = io::copy(&mut (&mut entry).take(size), &mut file).map_err(read_error)?;
    if copied != size {
        return Err(ArchiveError::Read(io::ErrorKind::UnexpectedEof.into()));
    }
    Ok(())
}

/// Creates the directories of `section/parts[..last]` below the staging directory, one level at a
/// time through the staging file functions, and returns the file path.
fn create_parents(
    target: &Target<'_>,
    section: Section,
    parts: &[&str],
) -> Result<std::path::PathBuf, ArchiveError> {
    let mut dir = target.staging.join(section.dir_name());
    let (file, dirs) = parts.split_last().ok_or_else(|| ArchiveError::BadPath(String::new()))?;
    for next in std::iter::once(None).chain(dirs.iter().map(Some)) {
        if let Some(name) = next {
            dir.push(name);
        }
        match (target.fs.create_dir)(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() => {}
            Err(e) => return Err(ArchiveError::io(&dir, e)),
        }
    }
    Ok(dir.join(file))
}

/// The entry path as text; a non-UTF-8 path is rejected.
fn entry_path<R: Read>(entry: &tar::Entry<'_, R>) -> Result<String, ArchiveError> {
    let bytes = entry.path_bytes();
    String::from_utf8(bytes.to_vec())
        .map_err(|_| ArchiveError::BadPath(ArchiveError::shown(&String::from_utf8_lossy(&bytes))))
}

fn is_regular<R: Read>(entry: &tar::Entry<'_, R>) -> bool {
    entry.header().entry_type() == tar::EntryType::Regular
}

/// Fails with [`io::ErrorKind::FileTooLarge`] once more than `remaining` bytes are read, so the
/// decompressed stream can never exceed what valid entries within the limits need.
struct CappedReader<R> {
    inner: R,
    remaining: u64,
}

impl<R: Read> Read for CappedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        let n_bytes = u64::try_from(n).unwrap_or(u64::MAX);
        self.remaining = self.remaining.checked_sub(n_bytes).ok_or_else(|| {
            io::Error::new(io::ErrorKind::FileTooLarge, "archive stream above the limits")
        })?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_reader_fails_past_its_limit() {
        let mut reader = CappedReader { inner: &[7_u8; 10][..], remaining: 4 };
        let mut out = Vec::new();
        let err = reader.read_to_end(&mut out).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::FileTooLarge);
        assert!(matches!(read_error(err), ArchiveError::TooLarge("archive stream")));
        let mut exact = CappedReader { inner: &[7_u8; 4][..], remaining: 4 };
        assert_eq!(exact.read_to_end(&mut out).unwrap(), 4);
    }
}
