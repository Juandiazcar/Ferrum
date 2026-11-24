//! Reading a `.ferrum` archive into a staging directory (`FR-DATA-003`, `docs/10`).

use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

use super::path::parse_entry;
use super::{
    ARCHIVE_FORMAT_VERSION, ArchiveLimits, MANIFEST_NAME, MAX_MANIFEST_BYTES, Manifest, Section,
    ZSTD_WINDOW_LOG,
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
/// must. Files land in `staging/config/…`, `staging/data/…` and `staging/scrollback/…`. On any
/// error `staging` is removed again, so a failed import leaves nothing behind.
///
/// # Errors
/// [`ArchiveError::StagingExists`]; [`ArchiveError::BadManifest`] if the first entry is not a
/// supported manifest; [`ArchiveError::BadPath`], [`ArchiveError::NotRegularFile`],
/// [`ArchiveError::Duplicate`] for an unsafe entry; [`ArchiveError::TooLarge`] beyond `limits`;
/// [`ArchiveError::Io`] for a corrupt stream or a write failure.
pub fn import_archive<R: Read>(
    input: R,
    staging: &Path,
    limits: &ArchiveLimits,
) -> Result<ImportSummary, ArchiveError> {
    match fs::create_dir(staging) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ArchiveError::StagingExists(staging.to_owned()));
        }
        Err(e) => return Err(ArchiveError::io(staging, e)),
    }
    let result = extract(input, staging, limits);
    if result.is_err() {
        // Best effort: the extraction error is what the user needs; a leftover directory is
        // harmless and the next import uses a new staging name.
        let _cleanup = fs::remove_dir_all(staging);
    }
    result
}

fn extract<R: Read>(
    input: R,
    staging: &Path,
    limits: &ArchiveLimits,
) -> Result<ImportSummary, ArchiveError> {
    let stream_error = |e| ArchiveError::io("", e);
    let mut decoder = zstd::stream::read::Decoder::new(input).map_err(stream_error)?;
    decoder.window_log_max(ZSTD_WINDOW_LOG).map_err(stream_error)?;
    let mut archive = tar::Archive::new(decoder);
    let mut entries = archive.entries().map_err(stream_error)?;
    let first =
        entries.next().ok_or_else(|| ArchiveError::BadManifest("archive is empty".into()))?;
    let manifest = read_manifest(first.map_err(stream_error)?)?;
    let mut summary = ImportSummary { manifest, files: 0, bytes: 0 };
    for entry in entries {
        let entry = entry.map_err(stream_error)?;
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
        extract_file(entry, staging, manifest)?;
        summary.files = summary.files.saturating_add(1);
    }
    Ok(summary)
}

fn read_manifest<R: Read>(entry: tar::Entry<'_, R>) -> Result<Manifest, ArchiveError> {
    let bad = |reason: &str| ArchiveError::BadManifest(reason.to_owned());
    if entry_path(&entry)? != MANIFEST_NAME || !is_regular(&entry) {
        return Err(bad("the first entry is not manifest.toml"));
    }
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(bad("manifest is too large"));
    }
    let mut text = String::new();
    let _len = entry
        .take(MAX_MANIFEST_BYTES)
        .read_to_string(&mut text)
        .map_err(|e| ArchiveError::io(MANIFEST_NAME, e))?;
    let manifest: Manifest = toml::from_str(&text).map_err(|e| bad(e.message()))?;
    if manifest.format_version != ARCHIVE_FORMAT_VERSION {
        return Err(bad("unsupported archive format version"));
    }
    Ok(manifest)
}

fn extract_file<R: Read>(
    mut entry: tar::Entry<'_, R>,
    staging: &Path,
    manifest: Manifest,
) -> Result<(), ArchiveError> {
    let path = entry_path(&entry)?;
    if !is_regular(&entry) {
        return Err(ArchiveError::NotRegularFile(path));
    }
    let (section, parts) = parse_entry(&path).ok_or_else(|| ArchiveError::BadPath(path.clone()))?;
    if section == Section::Scrollback && !manifest.includes_scrollback {
        return Err(ArchiveError::BadPath(path));
    }
    let mut dest = staging.join(section.dir_name());
    dest.extend(&parts);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| ArchiveError::io(parent, e))?;
    }
    // create_new: a second entry with the same path (or the same path on a case-insensitive file
    // system) is refused instead of overwriting the first one.
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&dest) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ArchiveError::Duplicate(path));
        }
        Err(e) => return Err(ArchiveError::io(&dest, e)),
    };
    let size = entry.size();
    let copied = io::copy(&mut (&mut entry).take(size), &mut file)
        .map_err(|e| ArchiveError::io(&dest, e))?;
    if copied != size {
        return Err(ArchiveError::io(&dest, io::ErrorKind::UnexpectedEof.into()));
    }
    Ok(())
}

/// The entry path as text; a non-UTF-8 path is rejected.
fn entry_path<R: Read>(entry: &tar::Entry<'_, R>) -> Result<String, ArchiveError> {
    let bytes = entry.path_bytes();
    String::from_utf8(bytes.to_vec())
        .map_err(|_| ArchiveError::BadPath(String::from_utf8_lossy(&bytes).into_owned()))
}

fn is_regular<R: Read>(entry: &tar::Entry<'_, R>) -> bool {
    entry.header().entry_type() == tar::EntryType::Regular
}
