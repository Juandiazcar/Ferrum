//! The daemon's log file (`docs/05` §8): `ferrumd.log` in the state directory, rotated at
//! [`MAX_LOG_BYTES`] with [`KEPT_FILES`] files in all, never the terminal. What is logged never
//! contains terminal content, keystrokes, environment values or user paths (`docs/10` §4); the
//! call sites log kinds and ids only.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Size at which the log rotates (`docs/05` §8: 1 MB).
pub const MAX_LOG_BYTES: u64 = 1_000_000;
/// Log files kept: the current one and two rotated ones (`docs/05` §8: 1 MB × 3).
pub const KEPT_FILES: usize = 3;
/// File name of the current log.
pub const LOG_FILE: &str = "ferrumd.log";

/// An append-only log file that rotates by size.
#[derive(Debug)]
pub struct RotatingFile {
    path: PathBuf,
    file: File,
    written: u64,
}

impl RotatingFile {
    /// Opens (or creates) the log in `dir`, creating `dir` if needed.
    ///
    /// # Errors
    /// The I/O error of creating the directory or opening the file.
    pub fn open(dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(LOG_FILE);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata()?.len();
        Ok(Self { path, file, written })
    }

    fn rotated(&self, index: usize) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(format!(".{index}"));
        PathBuf::from(name)
    }

    /// `ferrumd.log.1` is the newest rotated file; the oldest is replaced.
    fn rotate(&mut self) -> io::Result<()> {
        for index in (1..KEPT_FILES).rev() {
            let from = if index == 1 { self.path.clone() } else { self.rotated(index - 1) };
            match std::fs::rename(&from, self.rotated(index)) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
        }
        self.file = OpenOptions::new().create(true).write(true).truncate(true).open(&self.path)?;
        self.written = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written > 0 && self.written.saturating_add(buf.len() as u64) > MAX_LOG_BYTES {
            self.rotate()?;
        }
        let written = self.file.write(buf)?;
        self.written = self.written.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_rotates_and_keeps_three_files() {
        let dir = std::env::temp_dir().join(format!("ferrumd-log-{}", std::process::id()));
        let _stale = std::fs::remove_dir_all(&dir);
        let mut log = RotatingFile::open(&dir).unwrap();
        let line = vec![b'x'; 600_000];
        for _ in 0..5 {
            log.write_all(&line).unwrap();
        }
        let files = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(files, KEPT_FILES);
        assert!(std::fs::metadata(dir.join(LOG_FILE)).unwrap().len() <= MAX_LOG_BYTES);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
