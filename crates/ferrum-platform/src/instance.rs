//! Single-instance lock and handshake nonce of the daemon (`docs/10` §2).
//!
//! The lock is an exclusive OS file lock on `ferrumd.lock` in the run directory: the OS releases
//! it when the process ends, even after `kill -9`, so a stale file never blocks a restart. The
//! nonce is 32 random bytes from the OS in `ferrumd.nonce` (mode 0600 on Unix); clients read it
//! and present it in their `Hello`.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::private_fs;

/// File name of the lock in the run directory.
pub const LOCK_FILE: &str = "ferrumd.lock";
/// File name of the nonce in the run directory.
pub const NONCE_FILE: &str = "ferrumd.nonce";
/// Bytes of the nonce (`ferrum_proto::NONCE_LEN`; this crate depends on no other ferrum crate).
pub const NONCE_BYTES: usize = 32;

/// Holds the single-instance lock while alive.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

/// Why the lock could not be taken.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LockError {
    /// Another daemon of this user holds it.
    #[error("another daemon is already running for this user")]
    Held,
    /// The lock file could not be opened or locked.
    #[error("the lock file could not be used")]
    Io(#[source] io::Error),
}

impl InstanceLock {
    /// Takes the lock in `runtime_dir` without blocking.
    ///
    /// # Errors
    /// [`LockError::Held`] when another process holds it, [`LockError::Io`] otherwise.
    pub fn acquire(runtime_dir: &Path) -> Result<Self, LockError> {
        let path = runtime_dir.join(LOCK_FILE);
        let file = match private_fs::create_private_file(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                OpenOptions::new().write(true).open(&path).map_err(LockError::Io)?
            }
            Err(error) => return Err(LockError::Io(error)),
        };
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(LockError::Held),
            Err(TryLockError::Error(error)) => Err(LockError::Io(error)),
        }
    }
}

/// Writes a fresh random nonce to `runtime_dir`, replacing a stale one, and returns it. Call it
/// only while holding the [`InstanceLock`].
///
/// # Errors
/// The I/O error of writing the file, or of the OS random source.
pub fn write_nonce(runtime_dir: &Path) -> io::Result<[u8; NONCE_BYTES]> {
    let mut nonce = [0; NONCE_BYTES];
    getrandom::fill(&mut nonce).map_err(|error| io::Error::other(error.to_string()))?;
    let path = nonce_path(runtime_dir);
    match std::fs::remove_file(&path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut file = private_fs::create_private_file(&path)?;
    file.write_all(&nonce)?;
    file.sync_all()?;
    Ok(nonce)
}

/// Reads the nonce a running daemon wrote to `runtime_dir` (what a client presents).
///
/// # Errors
/// The I/O error of reading it, or [`io::ErrorKind::InvalidData`] when it has the wrong size.
pub fn read_nonce(runtime_dir: &Path) -> io::Result<[u8; NONCE_BYTES]> {
    let mut bytes = Vec::with_capacity(NONCE_BYTES + 1);
    // One byte more than a nonce is enough to tell a wrong size.
    File::open(nonce_path(runtime_dir))?.take(NONCE_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    bytes.try_into().map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad nonce file"))
}

fn nonce_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(NONCE_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_is_exclusive_and_the_nonce_round_trips() {
        let dir = std::env::temp_dir().join(format!("ferrum-instance-{}", std::process::id()));
        let _stale = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let lock = InstanceLock::acquire(&dir).unwrap();
        assert!(matches!(InstanceLock::acquire(&dir), Err(LockError::Held)));
        drop(lock);
        let _again = InstanceLock::acquire(&dir).unwrap();
        let first = write_nonce(&dir).unwrap();
        assert_eq!(read_nonce(&dir).unwrap(), first);
        assert_ne!(write_nonce(&dir).unwrap(), first, "every daemon start draws a new nonce");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
