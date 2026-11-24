//! Error types of the crate (`docs/05` §3: one `thiserror` enum per area).

use std::io;
use std::path::PathBuf;

use crate::project::SessionKey;

/// Why a project (as loaded from a file or built by the UI) is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectError {
    /// A project, session or recipe name is empty or too long.
    #[error("name must have 1 to {max} characters")]
    BadName {
        /// Longest allowed name in characters.
        max: usize,
    },
    /// The icon is empty or too long.
    #[error("icon must be 1 to {max} bytes")]
    BadIcon {
        /// Longest allowed icon in bytes.
        max: usize,
    },
    /// The project has no root directory, too many, a relative one or the same one twice.
    #[error("a project needs 1 to {max} distinct, absolute root directories")]
    BadRoots {
        /// Most roots allowed.
        max: usize,
    },
    /// A session or recipe entry refers to a root index the project does not have.
    #[error("root index {0} does not exist")]
    UnknownRoot(u16),
    /// Too many sessions or recipe entries.
    #[error("more than {max} sessions")]
    TooManySessions {
        /// Most sessions allowed.
        max: usize,
    },
    /// Two sessions share a key.
    #[error("session {0} is listed twice")]
    DuplicateSession(SessionKey),
    /// The layout names a session the project does not hold.
    #[error("layout refers to unknown session {0}")]
    UnknownLayoutSession(SessionKey),
    /// The layout shows one session in two panes.
    #[error("session {0} appears twice in the layout")]
    DuplicateLayoutSession(SessionKey),
    /// The layout nests deeper than the limit.
    #[error("layout is nested deeper than {max} splits")]
    LayoutTooDeep {
        /// Deepest nesting allowed.
        max: usize,
    },
    /// A split ratio is outside 1..=999 per mille.
    #[error("split ratio {0} is outside 1..=999 per mille")]
    BadRatio(u16),
    /// A recipe sub-directory is absolute, leaves its root, or is too long.
    #[error("recipe directory must be relative and stay inside its root")]
    BadRecipeDir,
    /// A profile name is blank, too long or has control characters.
    #[error("profile name is not valid")]
    BadProfile,
    /// A session's last working directory is relative or has control characters.
    #[error("session working directory must be an absolute path")]
    BadCwd,
    /// A startup command has control characters (a newline would run a second command) or is
    /// too long.
    #[error("startup command must be one line without control characters")]
    BadCommand,
}

/// Failure of reading a repository's `HEAD` ([`read_head`](crate::read_head)).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GitError {
    /// The start directory is relative; the search needs an absolute path so it never depends
    /// on the process working directory.
    #[error("the start directory must be an absolute path")]
    RelativePath,
    /// A git file exists but could not be read (or a `gitdir:` target is missing).
    #[error("cannot read {path}")]
    Io {
        /// File involved.
        path: PathBuf,
        /// Cause.
        #[source]
        source: io::Error,
    },
    /// A `HEAD` or `.git` file does not have the format git writes, or is too large.
    #[error("{0} is not in git's format")]
    Malformed(PathBuf),
}
