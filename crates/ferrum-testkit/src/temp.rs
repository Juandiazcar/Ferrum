//! Temporary directories for tests that touch the file system (`docs/08` §7: tests never touch
//! the real user config or data directories).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Distinguishes directories created by the same process.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// A fresh, empty directory under the system temporary directory, removed with its contents when
/// dropped.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates a new empty directory whose name contains `label`, the process id, a counter and
    /// the current time, so parallel tests and test processes never share one.
    ///
    /// # Errors
    /// The I/O error of creating the directory.
    pub fn new(label: &str) -> io::Result<Self> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
        let name = format!("ferrum-test-{label}-{}-{id}-{nanos}", std::process::id());
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir(&path)?;
        Ok(Self { path })
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `self.path().join(relative)`.
    pub fn join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.path.join(relative)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best effort: a leftover temporary directory must not fail the test that used it.
        let _removed = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_are_distinct_and_removed_on_drop() {
        let a = TempDir::new("unit").unwrap();
        let b = TempDir::new("unit").unwrap();
        assert_ne!(a.path(), b.path());
        std::fs::write(a.join("file"), b"x").unwrap();
        let kept = a.path().to_owned();
        drop(a);
        assert!(!kept.exists());
    }
}
