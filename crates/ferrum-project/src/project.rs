//! The project model (`FR-PROJ-001` to `FR-PROJ-003`, `FR-PROJ-005`).
//!
//! A project is persisted by the daemon as `<data_dir>/ferrum/projects/<id>.toml` (`docs/02` §8)
//! through `ferrum_config::store` with [`PROJECT_SCHEMA`]. A [`Project`] can only exist valid:
//! it is built from [`ProjectParts`] with `TryFrom`, which deserialization goes through too, so a
//! file is checked at the boundary (parse, don't validate). To change a project, take its parts,
//! edit them and build it again.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use ferrum_config::{MaxNameChars, MaxRoots, MaxSessions, ProjectLimits, Rgb, Schema};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::error::ProjectError;
use crate::layout::Layout;
use crate::recipe::{Recipe, is_clean_text};

/// Versions of a project file. Version 1 is the first; there is nothing to migrate yet.
pub const PROJECT_SCHEMA: Schema = Schema { name: "project", current: 1, migrations: &[] };

/// Directory below Ferrum's data directory (`<data_dir>/ferrum/`) with one file per project.
pub const PROJECTS_DIR: &str = "projects";

/// Longest icon in bytes. Anti-hostile cap: one grapheme cluster could otherwise carry thousands
/// of combining marks. The longest recommended emoji sequences (a kiss or couple with two skin
/// tones) are 35 bytes in UTF-8, so 64 accepts every real glyph with room to spare.
pub const MAX_ICON_BYTES: usize = 64;

/// Default project color: the accent color of `docs/09` §9.
pub const DEFAULT_COLOR: Rgb = Rgb { r: 0xE8, g: 0x59, b: 0x0C };

/// Identifies a project. A random UUID (maintainer decision Q-29, `docs/04` §4), so projects
/// exported on one machine never collide with projects of another when imported (`AC-DATA-01`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub Uuid);

impl ProjectId {
    /// A new random (version 4) id.
    pub fn new_random() -> Self {
        Self(Uuid::new_v4())
    }

    /// File name of this project inside [`PROJECTS_DIR`].
    pub fn file_name(self) -> String {
        format!("{}.toml", self.0.hyphenated())
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(f)
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
/// (`FR-SESS-008`). Checked as part of [`Project`].
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
    /// Last known working directory (absolute).
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

impl SessionEntry {
    /// A session started in root `root`, without a name, profile or cwd.
    pub fn new(key: SessionKey, root: u16) -> Self {
        Self { key, name: None, profile: None, root, cwd: None }
    }
}

/// The editable form of a project; [`Project::try_from`] checks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProjectParts {
    /// Identifier.
    pub id: ProjectId,
    /// Display name (`FR-UI-002`: renamable).
    pub name: String,
    /// Sidebar color chip.
    #[serde(default = "default_color")]
    pub color: Rgb,
    /// One glyph (a single grapheme cluster) shown next to the name.
    #[serde(default)]
    pub icon: Option<String>,
    /// Absolute root directories, at least one; the first is the default for new sessions.
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

/// A named group of root directories and the sessions working in them; always valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ProjectParts", into = "ProjectParts")]
pub struct Project(ProjectParts);

impl Project {
    /// A project over `roots` named after the first one (`FR-PROJ-002`), with no sessions.
    ///
    /// # Errors
    /// [`ProjectError`] when the roots are not valid (none, relative, repeated, too many).
    pub fn new(id: ProjectId, roots: Vec<PathBuf>) -> Result<Self, ProjectError> {
        let name = roots.first().map(|root| suggest_name(root)).unwrap_or_default();
        Self::try_from(ProjectParts {
            id,
            name,
            color: DEFAULT_COLOR,
            icon: None,
            roots,
            sessions: Vec::new(),
            layout: None,
            recipe: None,
        })
    }

    /// Identifier.
    pub fn id(&self) -> ProjectId {
        self.0.id
    }

    /// Display name.
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// Color.
    pub fn color(&self) -> Rgb {
        self.0.color
    }

    /// Icon glyph.
    pub fn icon(&self) -> Option<&str> {
        self.0.icon.as_deref()
    }

    /// Root directories (at least one).
    pub fn roots(&self) -> &[PathBuf] {
        &self.0.roots
    }

    /// Sessions in sidebar order.
    pub fn sessions(&self) -> &[SessionEntry] {
        &self.0.sessions
    }

    /// Layout.
    pub fn layout(&self) -> Option<&Layout> {
        self.0.layout.as_ref()
    }

    /// Startup recipe.
    pub fn recipe(&self) -> Option<&Recipe> {
        self.0.recipe.as_ref()
    }

    /// Root for a new session when none is chosen (`FR-PROJ-003`: the first root).
    pub fn default_root(&self) -> &Path {
        self.0.roots.first().map_or_else(|| Path::new(""), PathBuf::as_path)
    }

    /// The editable parts, to change the project and build it again with `Project::try_from`.
    pub fn into_parts(self) -> ProjectParts {
        self.0
    }

    /// Checks the user's limits from the settings (`[projects.limits]`, Q-30), applied when the
    /// user edits a project; files are only held to the hard caps.
    ///
    /// # Errors
    /// [`ProjectError::BadName`], [`ProjectError::BadRoots`] or [`ProjectError::TooManySessions`].
    pub fn check_limits(&self, limits: &ProjectLimits) -> Result<(), ProjectError> {
        let max_name = usize::from(limits.max_name_chars.get());
        let names = std::iter::once(self.0.name.as_str())
            .chain(self.0.sessions.iter().filter_map(|s| s.name.as_deref()));
        if names.into_iter().any(|name| name.chars().count() > max_name) {
            return Err(ProjectError::BadName { max: max_name });
        }
        let max_roots = usize::from(limits.max_roots.get());
        if self.0.roots.len() > max_roots {
            return Err(ProjectError::BadRoots { max: max_roots });
        }
        let max_sessions = usize::from(limits.max_sessions.get());
        if self.0.sessions.len() > max_sessions {
            return Err(ProjectError::TooManySessions { max: max_sessions });
        }
        let max_recipe = usize::from(limits.max_recipe_sessions.get());
        match &self.0.recipe {
            Some(recipe) if recipe.sessions.len() > max_recipe => {
                Err(ProjectError::TooManySessions { max: max_recipe })
            }
            _ => Ok(()),
        }
    }
}

impl TryFrom<ProjectParts> for Project {
    type Error = ProjectError;

    fn try_from(parts: ProjectParts) -> Result<Self, Self::Error> {
        check_roots(&parts.roots)?;
        check_name(&parts.name)?;
        if let Some(icon) = &parts.icon {
            check_icon(icon)?;
        }
        let keys = check_sessions(&parts.sessions, parts.roots.len())?;
        if let Some(layout) = &parts.layout {
            layout.validate(&keys)?;
        }
        if let Some(recipe) = &parts.recipe {
            recipe.validate(parts.roots.len())?;
        }
        Ok(Self(parts))
    }
}

impl From<Project> for ProjectParts {
    fn from(project: Project) -> Self {
        project.0
    }
}

/// A name for a directory: its last component, or the whole path for a root such as `/` or
/// `C:\` (`FR-PROJ-002`), cut to the hard name cap.
///
/// ```
/// use ferrum_project::suggest_name;
/// assert_eq!(suggest_name(std::path::Path::new("/work/helpdesk")), "helpdesk");
/// assert_eq!(suggest_name(std::path::Path::new("/")), "/");
/// ```
pub fn suggest_name(dir: &Path) -> String {
    let name = dir.file_name().map_or_else(|| dir.to_string_lossy(), |n| n.to_string_lossy());
    let cap = usize::from(MaxNameChars::MAX);
    name.trim().chars().filter(|c| !c.is_control()).take(cap).collect()
}

/// A session, recipe or project name: not blank, no control characters, within the hard cap.
pub(crate) fn check_name(name: &str) -> Result<(), ProjectError> {
    let max = usize::from(MaxNameChars::MAX);
    let ok = !name.trim().is_empty()
        && name.chars().count() <= max
        && !name.chars().any(char::is_control);
    if ok { Ok(()) } else { Err(ProjectError::BadName { max }) }
}

/// A profile name follows the name rules.
pub(crate) fn check_profile(profile: Option<&str>) -> Result<(), ProjectError> {
    profile.map_or(Ok(()), |p| check_name(p).map_err(|_| ProjectError::BadProfile))
}

pub(crate) fn check_root_index(index: u16, roots: usize) -> Result<(), ProjectError> {
    if usize::from(index) < roots { Ok(()) } else { Err(ProjectError::UnknownRoot(index)) }
}

fn check_icon(icon: &str) -> Result<(), ProjectError> {
    let one_glyph = icon.graphemes(true).count() == 1 && !icon.chars().any(char::is_control);
    if one_glyph && icon.len() <= MAX_ICON_BYTES {
        Ok(())
    } else {
        Err(ProjectError::BadIcon { max: MAX_ICON_BYTES })
    }
}

pub(crate) fn check_roots(roots: &[PathBuf]) -> Result<(), ProjectError> {
    let max = usize::from(MaxRoots::MAX);
    let bad = ProjectError::BadRoots { max };
    let well_formed =
        roots.iter().all(|root| root.is_absolute() && is_clean_text(&root.to_string_lossy()));
    if roots.is_empty() || roots.len() > max || !well_formed {
        return Err(bad);
    }
    let distinct: HashSet<&PathBuf> = roots.iter().collect();
    if distinct.len() == roots.len() { Ok(()) } else { Err(bad) }
}

fn check_sessions(
    sessions: &[SessionEntry],
    roots: usize,
) -> Result<HashSet<SessionKey>, ProjectError> {
    let max = usize::from(MaxSessions::MAX);
    if sessions.len() > max {
        return Err(ProjectError::TooManySessions { max });
    }
    let mut keys = HashSet::with_capacity(sessions.len());
    for session in sessions {
        if !keys.insert(session.key) {
            return Err(ProjectError::DuplicateSession(session.key));
        }
        check_root_index(session.root, roots)?;
        if let Some(name) = &session.name {
            check_name(name)?;
        }
        check_profile(session.profile.as_deref())?;
        if let Some(cwd) = &session.cwd
            && (!cwd.is_absolute() || !is_clean_text(&cwd.to_string_lossy()))
        {
            return Err(ProjectError::BadCwd);
        }
    }
    Ok(keys)
}
