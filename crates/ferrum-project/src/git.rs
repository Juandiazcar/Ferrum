//! Git awareness without a git library (`FR-PROJ-004`, `docs/07` §3: `gix` rejected).
//!
//! The branch is read from the repository's `HEAD` file. The repository is found by walking up
//! from a directory to the first `.git`:
//!
//! - a `.git` **directory**: an ordinary repository; `HEAD` is inside it;
//! - a `.git` **file** containing `gitdir: <path>`: a linked worktree (`<repo>/.git/worktrees/<name>`)
//!   or a submodule (`<super>/.git/modules/<name>`); `HEAD` is in that directory. A relative
//!   path is relative to the directory holding the `.git` file.
//!
//! `HEAD` holds either `ref: refs/heads/<branch>`, another symbolic ref, or a commit id (detached:
//! 40 hex digits for SHA-1 repositories, 64 for SHA-256). Reading is on demand only (on focus or
//! cwd change); there is no polling. Files are read with a small size cap.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::error::GitError;

/// Largest `HEAD` or `.git` file read. Anti-hostile cap: both are one line of at most a ref name
/// or a path (git writes under 300 bytes); 4 KiB also fits a 4 096-byte `gitdir:` path.
pub const MAX_GIT_FILE_BYTES: u64 = 4 * 1_024;

/// Longest ref name accepted from `HEAD`. Anti-hostile cap: git limits a ref path component to
/// the 255-byte file name limit and real ref names are far shorter; 1 KiB allows nesting.
const MAX_REF_BYTES: usize = 1_024;

/// Hex digits of a SHA-1 commit id.
const SHA1_HEX_DIGITS: usize = 40;

/// Hex digits of a SHA-256 commit id.
const SHA256_HEX_DIGITS: usize = 64;

const DOT_GIT: &str = ".git";
const HEAD_FILE: &str = "HEAD";
const GITDIR_PREFIX: &str = "gitdir:";
const REF_PREFIX: &str = "ref:";
const BRANCH_PREFIX: &str = "refs/heads/";

/// What `HEAD` points at.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Head {
    /// A local branch (also when the branch has no commit yet).
    Branch(String),
    /// A commit id (detached `HEAD`), lowercase hex.
    Detached(String),
    /// A symbolic ref outside `refs/heads/`, in full.
    OtherRef(String),
}

/// The `HEAD` of the repository containing a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GitHead {
    /// What `HEAD` points at.
    pub head: Head,
    /// Directory holding the `.git` entry (top of the working tree).
    pub work_tree: PathBuf,
    /// Directory holding `HEAD` (the `.git` directory or the `gitdir:` target).
    pub git_dir: PathBuf,
}

/// Finds the repository containing `start` and reads its `HEAD`. `Ok(None)` when no ancestor of
/// `start` has a `.git` entry.
///
/// # Errors
/// [`GitError::RelativePath`] for a relative `start`; [`GitError::Io`] when a git file exists but
/// cannot be read; [`GitError::Malformed`] when its content is not what git writes.
pub fn read_head(start: &Path) -> Result<Option<GitHead>, GitError> {
    if !start.is_absolute() {
        return Err(GitError::RelativePath);
    }
    for dir in start.ancestors() {
        let dot_git = dir.join(DOT_GIT);
        let meta = match fs::metadata(&dot_git) {
            Ok(meta) => meta,
            Err(e) if is_absent(&e) => continue,
            Err(e) => return Err(GitError::Io { path: dot_git, source: e }),
        };
        let git_dir = if meta.is_dir() {
            dot_git
        } else if meta.is_file() {
            resolve_gitdir_file(&dot_git, dir)?
        } else {
            continue;
        };
        let head = parse_head_file(&git_dir.join(HEAD_FILE))?;
        return Ok(Some(GitHead { head, work_tree: dir.to_owned(), git_dir }));
    }
    Ok(None)
}

/// `start` (or an ancestor) may be a file or missing: both just mean "no `.git` here".
fn is_absent(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory)
}

fn resolve_gitdir_file(dot_git: &Path, dir: &Path) -> Result<PathBuf, GitError> {
    let text = read_small(dot_git)?;
    let target = text
        .lines()
        .next()
        .and_then(|line| line.strip_prefix(GITDIR_PREFIX))
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .ok_or_else(|| GitError::Malformed(dot_git.to_owned()))?;
    Ok(dir.join(target))
}

fn parse_head_file(path: &Path) -> Result<Head, GitError> {
    let text = read_small(path)?;
    parse_head(&text).ok_or_else(|| GitError::Malformed(path.to_owned()))
}

/// Parses the first line of a `HEAD` file.
fn parse_head(text: &str) -> Option<Head> {
    let line = text.lines().next()?.trim_end();
    if let Some(target) = line.strip_prefix(REF_PREFIX) {
        let target = target.trim();
        let valid = !target.is_empty()
            && target.len() <= MAX_REF_BYTES
            && !target.chars().any(|c| c.is_control() || c.is_whitespace());
        if !valid {
            return None;
        }
        return match target.strip_prefix(BRANCH_PREFIX) {
            // `ref: refs/heads/` names no branch; git never writes it.
            Some("") => None,
            Some(branch) => Some(Head::Branch(branch.to_owned())),
            None => Some(Head::OtherRef(target.to_owned())),
        };
    }
    let is_id = matches!(line.len(), SHA1_HEX_DIGITS | SHA256_HEX_DIGITS)
        && line.bytes().all(|b| b.is_ascii_hexdigit());
    is_id.then(|| Head::Detached(line.to_ascii_lowercase()))
}

fn read_small(path: &Path) -> Result<String, GitError> {
    let io_error = |source| GitError::Io { path: path.to_owned(), source };
    // Only regular files: opening a FIFO or a device for reading can block forever.
    if !fs::metadata(path).map_err(io_error)?.is_file() {
        return Err(GitError::Malformed(path.to_owned()));
    }
    let mut bytes = Vec::new();
    let _len = File::open(path)
        .and_then(|file| file.take(MAX_GIT_FILE_BYTES.saturating_add(1)).read_to_end(&mut bytes))
        .map_err(io_error)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_GIT_FILE_BYTES {
        return Err(GitError::Malformed(path.to_owned()));
    }
    String::from_utf8(bytes).map_err(|_| GitError::Malformed(path.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_lines_parse_like_git_writes_them() {
        let branch = |b: &str| Some(Head::Branch(b.to_owned()));
        assert_eq!(parse_head("ref: refs/heads/main\n"), branch("main"));
        assert_eq!(parse_head("ref: refs/heads/feature/x\r\n"), branch("feature/x"));
        assert_eq!(parse_head("ref:refs/heads/main"), branch("main"));
        let other = Some(Head::OtherRef("refs/remotes/origin/main".to_owned()));
        assert_eq!(parse_head("ref: refs/remotes/origin/main\n"), other);
        let sha1 = "0123456789ABCDEF0123456789abcdef01234567";
        assert_eq!(parse_head(sha1), Some(Head::Detached(sha1.to_ascii_lowercase())));
        let sha256 = "a".repeat(SHA256_HEX_DIGITS);
        assert_eq!(parse_head(&format!("{sha256}\n")), Some(Head::Detached(sha256)));
    }

    #[test]
    fn malformed_head_lines_are_rejected() {
        let long_ref = format!("ref: refs/heads/{}", "b".repeat(MAX_REF_BYTES));
        for bad in [
            "",
            "\n",
            "ref: refs/heads/\n",
            "ref:",
            "ref: ",
            "ref: a b",
            "ref: a\u{7}",
            "0123",
            "z".repeat(40).as_str(),
            &long_ref,
        ] {
            assert_eq!(parse_head(bad), None, "{bad:?}");
        }
    }
}
