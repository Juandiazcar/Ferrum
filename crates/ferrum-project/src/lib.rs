//! `ferrum-project`: project model, `.git/HEAD` reader, `cwd` tracking logic and startup recipes.
//!
//! # Model
//!
//! A [`Project`] is a named, colored group of 1..N root directories with its sessions in sidebar
//! order ([`SessionEntry`]), a [`Layout`] (a split tree whose leaves are sessions) and an optional
//! startup [`Recipe`]. The daemon owns projects and persists each one as
//! `data_dir/projects/<id>.toml` with `ferrum_config::store` and [`PROJECT_SCHEMA`]; a loaded
//! project is untrusted input and goes through [`Project::validate`].
//!
//! Allowed internal dependencies: ferrum-config (types only: `Rgb`, `Schema`).
//! Must never depend on: ferrum-daemon, ferrum-ui, OS-specific code.

#![forbid(unsafe_code)]

mod error;
mod layout;
mod project;
mod recipe;

pub use error::ProjectError;
pub use layout::{Axis, Layout, MAX_LAYOUT_DEPTH, RATIO_SCALE};
pub use project::{
    DEFAULT_COLOR, FALLBACK_NAME, MAX_ICON_BYTES, MAX_NAME_CHARS, MAX_ROOTS, MAX_SESSIONS,
    PROJECT_SCHEMA, PROJECTS_DIR, Project, ProjectId, SessionEntry, SessionKey, suggest_name,
};
pub use recipe::{Arrangement, MAX_RECIPE_SESSIONS, Recipe, RecipeSession, SessionLaunch};
