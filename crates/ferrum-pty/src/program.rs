//! The one program lookup rule of every backend (NFR-PORT-01):
//!
//! - a program with a directory part (`bin/tool`, `/usr/bin/tool`, `C:\tools\tool.exe`) is taken
//!   relative to the child's working directory;
//! - a bare name is searched in the absolute directories of the child's `PATH`, in order; the
//!   current directory is never searched;
//! - each candidate is tried as given, then with each executable extension (Windows: the child's
//!   `PATHEXT`; Unix has none), and the first executable file wins (Unix: a file with execute
//!   permission; Windows: a file whose extension is in `PATHEXT`).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::PtyError;

/// `PATHEXT` when the child's environment has none: the executable kinds of a default Windows
/// installation that `CreateProcessW` (directly or through `cmd.exe`) can start.
#[cfg(windows)]
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Where to look: the child's `PATH` and the executable extensions.
pub(crate) struct Search {
    pub(crate) path: Option<OsString>,
    pub(crate) extensions: Vec<OsString>,
}

impl Search {
    /// The search of a child whose environment has `path` and (Windows) `pathext`.
    pub(crate) fn new(path: Option<OsString>, pathext: Option<&OsStr>) -> Self {
        Self { path, extensions: extensions(pathext) }
    }
}

#[cfg(unix)]
fn extensions(_pathext: Option<&OsStr>) -> Vec<OsString> {
    Vec::new()
}

#[cfg(windows)]
fn extensions(pathext: Option<&OsStr>) -> Vec<OsString> {
    let text = pathext.and_then(OsStr::to_str).unwrap_or(DEFAULT_PATHEXT);
    text.split(';').filter(|ext| ext.starts_with('.')).map(OsString::from).collect()
}

/// Resolves `program` for a child running in `cwd` (see the module documentation).
///
/// # Errors
/// [`PtyError::ProgramNotFound`] when no candidate is an executable file.
pub(crate) fn resolve(program: &OsStr, cwd: &Path, search: &Search) -> Result<PathBuf, PtyError> {
    let path = Path::new(program);
    let bases: Vec<PathBuf> = if path.components().count() > 1 || path.is_absolute() {
        vec![cwd.join(path)]
    } else {
        let dirs = search.path.as_deref().map(std::env::split_paths).into_iter().flatten();
        dirs.filter(|dir| dir.is_absolute()).map(|dir| dir.join(path)).collect()
    };
    bases
        .into_iter()
        .flat_map(|base| {
            let suffixed =
                search.extensions.iter().map(|ext| with_suffix(&base, ext)).collect::<Vec<_>>();
            std::iter::once(base).chain(suffixed)
        })
        .find(|candidate| is_executable(candidate, &search.extensions))
        .ok_or(PtyError::ProgramNotFound)
}

fn with_suffix(base: &Path, ext: &OsStr) -> PathBuf {
    let mut text = base.as_os_str().to_owned();
    text.push(ext);
    PathBuf::from(text)
}

#[cfg(unix)]
fn is_executable(candidate: &Path, _extensions: &[OsString]) -> bool {
    candidate.is_file() && rustix::fs::access(candidate, rustix::fs::Access::EXEC_OK).is_ok()
}

#[cfg(windows)]
fn is_executable(candidate: &Path, extensions: &[OsString]) -> bool {
    let known = |ext: &OsStr| {
        extensions.iter().any(|known| {
            known
                .to_str()
                .zip(ext.to_str())
                .is_some_and(|(known, ext)| known.trim_start_matches('.').eq_ignore_ascii_case(ext))
        })
    };
    candidate.is_file() && candidate.extension().is_some_and(known)
}

/// Whether `program` is a batch file, which Windows starts through `cmd.exe`.
#[cfg(windows)]
pub(crate) fn is_batch(program: &Path) -> bool {
    program
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd"))
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!("ferrum-pty-program-{}", std::process::id()));
        for dir in ["a", "b", "cwd/bin"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for (file, mode) in [("a/tool", 0o644), ("b/tool", 0o755), ("cwd/bin/tool", 0o755)] {
            std::fs::write(root.join(file), b"").unwrap();
            std::fs::set_permissions(root.join(file), std::fs::Permissions::from_mode(mode))
                .unwrap();
        }
        root
    }

    #[test]
    fn follows_the_one_lookup_rule() {
        let root = tree();
        let path =
            std::env::join_paths([root.join("a"), root.join("b"), "relative".into()]).unwrap();
        let search = Search::new(Some(path), None);
        let cwd = root.join("cwd");
        // A bare name: the first executable file in PATH (a/tool is not executable).
        assert_eq!(resolve("tool".as_ref(), &cwd, &search).unwrap(), root.join("b/tool"));
        // A directory part: relative to the child's cwd.
        assert_eq!(resolve("bin/tool".as_ref(), &cwd, &search).unwrap(), cwd.join("bin/tool"));
        // Never the current directory, never a relative PATH entry.
        let none = Search::new(Some("relative".into()), None);
        assert!(matches!(resolve("tool".as_ref(), &cwd, &none), Err(PtyError::ProgramNotFound)));
        assert!(matches!(
            resolve("missing".as_ref(), &cwd, &search),
            Err(PtyError::ProgramNotFound)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
