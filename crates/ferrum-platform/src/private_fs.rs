//! Owner-only file creation (`docs/10` §4: files holding user content, such as an imported
//! archive or scrollback, are readable by their owner only).
//!
//! On Unix new directories get mode 0700 and new files 0600, set at creation (no window where
//! they are more open). On Windows a new directory gets an access list for the user and `SYSTEM`
//! only, which the files created inside inherit. The signatures use `std` types only, so pure
//! crates (`ferrum_config::archive::StagingFs`) receive these as function pointers from the
//! composition roots without depending on this crate.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// Mode of directories created by [`create_private_dir`] on Unix.
#[cfg(unix)]
const PRIVATE_DIR_MODE: u32 = 0o700;

/// Mode of files created by [`create_private_file`] on Unix.
#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;

/// Creates one directory readable by the owner only. Fails if it exists.
///
/// # Errors
/// The I/O error of creating it.
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut builder = std::fs::DirBuilder::new();
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, PRIVATE_DIR_MODE);
        builder.create(path)
    }
    #[cfg(windows)]
    {
        crate::win32::create_private_dir(path)
    }
}

/// Creates a new file for writing, readable by the owner only. Fails with
/// [`io::ErrorKind::AlreadyExists`] if it exists; never replaces a file.
///
/// # Errors
/// The I/O error of creating it.
pub fn create_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    let _same = options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, PRIVATE_FILE_MODE);
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_owner_only_entries_and_never_replaces_a_file() {
        let root = std::env::temp_dir().join(format!("ferrum-private-fs-{}", std::process::id()));
        let _stale = std::fs::remove_dir_all(&root);
        create_private_dir(&root).unwrap();
        let file = root.join("clip.bin");
        drop(create_private_file(&file).unwrap());
        assert_eq!(create_private_file(&file).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(create_private_dir(&root).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&root), PRIVATE_DIR_MODE);
            assert_eq!(mode(&file), PRIVATE_FILE_MODE);
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
