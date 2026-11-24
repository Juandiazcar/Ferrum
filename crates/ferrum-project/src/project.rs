//! The project model (`FR-PROJ-001` to `FR-PROJ-003`, `FR-PROJ-005`).
//!
//! A project is persisted by the daemon as `data_dir/projects/<id>.toml` through
//! `ferrum_config::store` with [`PROJECT_SCHEMA`]; after loading, the caller runs
//! [`Project::validate`] because the file is untrusted input.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use ferrum_config::{Rgb, Schema};
use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::layout::Layout;
use crate::recipe::Recipe;

/// Versions of a project file. Version 1 is the first; there is nothing to migrate yet.
pub const PROJECT_SCHEMA: Schema = Schema { name: "project", current: 1, migrations: &[] };

/// Directory below the per-user data directory that holds one file per project.
pub const PROJECTS_DIR: &str = "projects";

/// Longest project, session or recipe-session name in characters.
pub const MAX_NAME_CHARS: usize = 128;

/// Most root directories of one project.
pub const MAX_ROOTS: usize = 16;

/// Most sessions of one project (the sidebar and layout stay usable far below this).
pub const MAX_SESSIONS: usize = 256;

/// Longest icon glyph in bytes (one emoji sequence or symbol).
pub const MAX_ICON_BYTES: usize = 32;

/// Name used when no name can be derived from a directory.
pub const FALLBACK_NAME: &str = "Project";

/// Default project color: the accent color of `docs/09` §9.
pub const DEFAULT_COLOR: Rgb = Rgb { r: 0xE8, g: 0x59, b: 0x0C };

/// Identifies a project; assigned by the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub u64);

impl ProjectId {
    /// File name of this project inside [`PROJECTS_DIR`].
    pub fn file_name(self) -> String {
        format!("{}.toml", self.0)
    }
}

/// Identifies a session within its project's file, stable across daemon restarts (unlike the
/// daemon's runtime session ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionKey(pub u64);

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A session as remembered by its project, enough to respawn it after a daemon restart
/// (`FR-SESS-008`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SessionEntry {
    /// Key within the project.
    pub key: SessionKey,
    /// User-set name (`FR-UI-002`); `None` uses the program title or profile name.
    #[serde(default)]
    pub name: Option<String>,
    /// Shell profile name; `None` uses the default profile.
    #[serde(default)]
    pub profile: Option<String>,
    /// Index of the root it started in (`FR-PROJ-003`).
    #[serde(default)]
    pub root: u16,
    /// Last known working directory.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

impl SessionEntry {
    /// A session started in root `root`, without a name or profile.
    pub fn new(key: SessionKey, root: u16) -> Self {
        Self { key, name: None, profile: None, root, cwd: None }
    }
}

/// A named group of root directories and the sessions working in them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Project {
    /// Identifier.
    pub id: ProjectId,
    /// Display name (`FR-UI-002`: renamable).
    pub name: String,
    /// Sidebar color chip.
    #[serde(default = "default_color")]
    pub color: Rgb,
    /// Glyph shown next to the name.
    #[serde(default)]
    pub icon: Option<String>,
    /// Root directories, 1 to [`MAX_ROOTS`]; the first is the default for new sessions.
    pub roots: Vec<PathBuf>,
    /// Sessions in sidebar order.
    #[serde(default)]
    pub sessions: Vec<SessionEntry>,
    /// Split tree of the project view; `None` when no session is shown.
    #[serde(default)]
    pub layout: Option<Layout>,
    /// Sessions to create when the project is opened (`FR-PROJ-007`).
    #[serde(default)]
    pub recipe: Option<Recipe>,
}

fn default_color() -> Rgb {
    DEFAULT_COLOR
}

impl Project {
    /// A project over `roots` named after the first one (`FR-PROJ-002`), with no sessions.
    pub fn new(id: ProjectId, roots: Vec<PathBuf>) -> Self {
        let name = roots.first().map_or_else(|| FALLBACK_NAME.to_owned(), |r| suggest_name(r));
        Self {
            id,
            name,
            color: DEFAULT_COLOR,
            icon: None,
            roots,
            sessions: Vec::new(),
            layout: None,
            recipe: None,
        }
    }

    /// Root for a new session when none is chosen (`FR-PROJ-003`: the first root).
    pub fn default_root(&self) -> Option<&Path> {
        self.roots.first().map(PathBuf::as_path)
    }

    /// Checks every invariant of the model.
    ///
    /// # Errors
    /// The first [`ProjectError`] found.
    pub fn validate(&self) -> Result<(), ProjectError> {
        check_name(&self.name)?;
        if let Some(icon) = &self.icon
            && (icon.is_empty() || icon.len() > MAX_ICON_BYTES)
        {
            return Err(ProjectError::BadIcon { max: MAX_ICON_BYTES });
        }
        check_roots(&self.roots)?;
        let keys = self.check_sessions()?;
        if let Some(layout) = &self.layout {
            layout.validate(&keys)?;
        }
        if let Some(recipe) = &self.recipe {
            recipe.validate(self.roots.len())?;
        }
        Ok(())
    }

    fn check_sessions(&self) -> Result<HashSet<SessionKey>, ProjectError> {
        if self.sessions.len() > MAX_SESSIONS {
            return Err(ProjectError::TooManySessions { max: MAX_SESSIONS });
        }
        let mut keys = HashSet::with_capacity(self.sessions.len());
        for session in &self.sessions {
            if !keys.insert(session.key) {
                return Err(ProjectError::DuplicateSession(session.key));
            }
            check_root_index(session.root, self.roots.len())?;
            if let Some(name) = &session.name {
                check_name(name)?;
            }
        }
        Ok(keys)
    }
}

/// A project name derived from a directory: its last component, or [`FALLBACK_NAME`] for a root
/// such as `/` or `C:\` (`FR-PROJ-002`).
///
/// ```
/// use ferrum_project::suggest_name;
/// assert_eq!(suggest_name(std::path::Path::new("/work/helpdesk")), "helpdesk");
/// assert_eq!(suggest_name(std::path::Path::new("/")), "Project");
/// ```
pub fn suggest_name(dir: &Path) -> String {
    let name = dir.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let trimmed: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
    if trimmed.is_empty() { FALLBACK_NAME.to_owned() } else { trimmed }
}

pub(crate) fn check_name(name: &str) -> Result<(), ProjectError> {
    let chars = name.trim().chars().count();
    if chars == 0 || name.chars().count() > MAX_NAME_CHARS {
        return Err(ProjectError::BadName { max: MAX_NAME_CHARS });
    }
    Ok(())
}

pub(crate) fn check_root_index(index: u16, roots: usize) -> Result<(), ProjectError> {
    if usize::from(index) < roots { Ok(()) } else { Err(ProjectError::UnknownRoot(index)) }
}

fn check_roots(roots: &[PathBuf]) -> Result<(), ProjectError> {
    let bad = ProjectError::BadRoots { max: MAX_ROOTS };
    if roots.is_empty() || roots.len() > MAX_ROOTS || roots.iter().any(|r| r.as_os_str().is_empty())
    {
        return Err(bad);
    }
    let distinct: HashSet<&PathBuf> = roots.iter().collect();
    if distinct.len() == roots.len() { Ok(()) } else { Err(bad) }
}
