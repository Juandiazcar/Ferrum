//! The per-user directories (`docs/02` §8, `docs/11` §1): configuration, data, state and run
//! files.
//!
//! Each one is, in order: the `FERRUM_CONFIG_DIR`, `FERRUM_DATA_DIR`, `FERRUM_STATE_DIR` or
//! `FERRUM_RUNTIME_DIR` override (tests, portable setups), a subfolder of the executable's
//! directory when a `ferrum-portable` marker file sits next to it, or the OS location (XDG,
//! `Library`, `AppData`) with a `ferrum` subfolder. The run directory holds the socket, lock,
//! nonce and spill files, so [`Dirs::prepare_runtime`] creates it owner-only and refuses one that
//! another user owns or can enter (`docs/10` §2).

use std::io;
use std::path::{Path, PathBuf};

use crate::private_fs;

/// Name of the subfolder of every OS location.
const APP_DIR: &str = "ferrum";
/// Marker file next to the executable that turns on portable mode (`docs/11` §1).
pub const PORTABLE_MARKER: &str = "ferrum-portable";
/// Environment variable overriding the configuration directory.
pub const CONFIG_DIR_VAR: &str = "FERRUM_CONFIG_DIR";
/// Environment variable overriding the data directory.
pub const DATA_DIR_VAR: &str = "FERRUM_DATA_DIR";
/// Environment variable overriding the state directory.
pub const STATE_DIR_VAR: &str = "FERRUM_STATE_DIR";
/// Environment variable overriding the run directory.
pub const RUNTIME_DIR_VAR: &str = "FERRUM_RUNTIME_DIR";
/// Permission bits a run directory may have for anyone but its owner on Unix: none.
#[cfg(unix)]
const OTHERS_MASK: u32 = 0o077;

/// The four per-user directories of Ferrum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    /// Settings, keybindings, themes.
    pub config: PathBuf,
    /// Projects and layouts.
    pub data: PathBuf,
    /// Logs and window state.
    pub state: PathBuf,
    /// Socket, lock, nonce, spill files and image clips.
    pub runtime: PathBuf,
}

impl Dirs {
    /// Resolves the directories of the current user (see the module documentation). Nothing is
    /// created.
    ///
    /// # Errors
    /// [`io::ErrorKind::NotFound`] when the OS reports no home directory and no override is set.
    pub fn resolve() -> io::Result<Self> {
        let portable = portable_root();
        let pick = |var: &str, os: fn() -> Option<PathBuf>, sub: &str| -> io::Result<PathBuf> {
            if let Some(path) = std::env::var_os(var).filter(|value| !value.is_empty()) {
                return Ok(PathBuf::from(path));
            }
            if let Some(root) = &portable {
                return Ok(root.join(sub));
            }
            os().ok_or_else(|| io::ErrorKind::NotFound.into())
        };
        Ok(Self {
            config: pick(CONFIG_DIR_VAR, os_config, "config")?,
            data: pick(DATA_DIR_VAR, os_data, "data")?,
            state: pick(STATE_DIR_VAR, os_state, "state")?,
            runtime: pick(RUNTIME_DIR_VAR, os_runtime, "run")?,
        })
    }

    /// Creates the run directory owner-only if it is missing, and checks an existing one: it must
    /// be a directory owned by the current user that nobody else can enter (Unix). Parent
    /// directories are created with the default permissions.
    ///
    /// # Errors
    /// The I/O error of creating it, or [`io::ErrorKind::PermissionDenied`] for a directory that
    /// fails the checks.
    pub fn prepare_runtime(&self) -> io::Result<()> {
        prepare_private_dir(&self.runtime)
    }
}

/// See [`Dirs::prepare_runtime`].
///
/// # Errors
/// As [`Dirs::prepare_runtime`].
pub fn prepare_private_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match private_fs::create_private_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => check_private_dir(path),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn check_private_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    let is_owned = meta.uid() == rustix::process::geteuid().as_raw();
    if meta.is_dir() && is_owned && meta.mode() & OTHERS_MASK == 0 {
        return Ok(());
    }
    Err(io::Error::new(io::ErrorKind::PermissionDenied, "the run directory is not private"))
}

#[cfg(windows)]
fn check_private_dir(path: &Path) -> io::Result<()> {
    if std::fs::symlink_metadata(path)?.is_dir() && crate::win32::is_private(path)? {
        return Ok(());
    }
    Err(io::Error::new(io::ErrorKind::PermissionDenied, "the run directory is not private"))
}

/// The executable's directory when the portable marker sits next to it.
fn portable_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    dir.join(PORTABLE_MARKER).is_file().then(|| dir.to_path_buf())
}

fn base() -> Option<directories::BaseDirs> {
    directories::BaseDirs::new()
}

fn os_config() -> Option<PathBuf> {
    base().map(|dirs| dirs.config_dir().join(APP_DIR))
}

fn os_data() -> Option<PathBuf> {
    base().map(|dirs| dirs.data_dir().join(APP_DIR))
}

fn os_state() -> Option<PathBuf> {
    // Only Linux has a state directory (XDG); elsewhere the local data directory holds state.
    base().map(|dirs| dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir()).join(APP_DIR))
}

/// Linux: `$XDG_RUNTIME_DIR`, else `/tmp/ferrum-<uid>` (`docs/10` §2; [`prepare_private_dir`]
/// creates it exclusively and checks the owner).
#[cfg(target_os = "linux")]
#[allow(clippy::unnecessary_wraps, reason = "one signature for the lookups of every OS")]
fn os_runtime() -> Option<PathBuf> {
    if let Some(dir) = base().and_then(|dirs| dirs.runtime_dir().map(Path::to_path_buf)) {
        return Some(dir.join(APP_DIR));
    }
    let uid = rustix::process::geteuid().as_raw();
    Some(PathBuf::from(format!("/tmp/ferrum-{uid}")))
}

/// macOS: the per-user `$TMPDIR` (`docs/10` §2).
#[cfg(target_os = "macos")]
#[allow(clippy::unnecessary_wraps, reason = "one signature for the lookups of every OS")]
fn os_runtime() -> Option<PathBuf> {
    Some(std::env::temp_dir().join(APP_DIR))
}

/// Windows: the local application data directory (the IPC endpoint is a named pipe, not a file).
#[cfg(windows)]
fn os_runtime() -> Option<PathBuf> {
    base().map(|dirs| dirs.data_local_dir().join(APP_DIR).join("run"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_private_dir_is_created_and_reused() {
        let root = std::env::temp_dir().join(format!("ferrum-dirs-{}", std::process::id()));
        let _stale = std::fs::remove_dir_all(&root);
        let run = root.join("run");
        prepare_private_dir(&run).unwrap();
        prepare_private_dir(&run).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let open = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&run, open).unwrap();
            let error = prepare_private_dir(&run).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
