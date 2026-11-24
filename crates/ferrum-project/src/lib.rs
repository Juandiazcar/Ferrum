//! `ferrum-project`: project model, `.git/HEAD` reader, `cwd` tracking logic and startup recipes.
//!
//! # Model
//!
//! A [`Project`] is a named, colored group of 1..N root directories with its sessions in sidebar
//! order ([`SessionEntry`]), a [`Layout`] (a split tree whose leaves are sessions) and an optional
//! startup [`Recipe`]. The daemon owns projects and persists each one as
//! `<data_dir>/ferrum/projects/<id>.toml` with `ferrum_config::store` and [`PROJECT_SCHEMA`]. A
//! `Project` is valid by construction: files and edits go through `TryFrom<ProjectParts>`, and
//! the user's size limits from the settings apply through [`Project::check_limits`].
//!
//! [`read_head`] shows the branch of a root without a git library: it finds the nearest `.git`
//! (directory, or a `gitdir:` file for linked worktrees and submodules) and parses `HEAD`, on demand
//! only (`FR-PROJ-004`).
//!
//! Allowed internal dependencies: ferrum-config (types only: `Rgb`, `Schema`, `ProjectLimits`
//! and its bounded limit types).
//! Must never depend on: ferrum-daemon, ferrum-ui, OS-specific code.

#![forbid(unsafe_code)]

mod error;
mod git;
mod layout;
mod project;
mod recipe;

pub use error::{GitError, ProjectError};
pub use git::{GitHead, Head, MAX_GIT_FILE_BYTES, read_head};
pub use layout::{Axis, Layout, MAX_LAYOUT_DEPTH, RATIO_SCALE};
pub use project::{
    DEFAULT_COLOR, MAX_ICON_BYTES, PROJECT_SCHEMA, PROJECTS_DIR, Project, ProjectId, ProjectParts,
    SessionEntry, SessionKey, suggest_name,
};
pub use recipe::{Arrangement, MAX_TEXT_BYTES, Recipe, RecipeSession, SessionLaunch};
