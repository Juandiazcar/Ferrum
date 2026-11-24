//! Startup recipes: the sessions a project opens with (`FR-PROJ-007`, `docs/09` §3).

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::layout::{Axis, Layout};
use crate::project::{SessionKey, check_name, check_root_index};

/// Most sessions one recipe may open.
pub const MAX_RECIPE_SESSIONS: usize = 32;

/// How the recipe's sessions are arranged in the project view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrangement {
    /// Only the first session is shown; the others wait in the sidebar.
    #[default]
    Single,
    /// All sessions side by side with equal widths.
    SideBySide,
    /// All sessions top to bottom with equal heights.
    Stacked,
}

/// One session of a recipe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RecipeSession {
    /// Session name.
    pub name: String,
    /// Shell profile name; `None` uses the default profile.
    #[serde(default)]
    pub profile: Option<String>,
    /// Index of the project root the session starts in.
    #[serde(default)]
    pub root: u16,
    /// Directory below the root to start in.
    #[serde(default)]
    pub subdir: Option<PathBuf>,
    /// Command typed into the shell once it starts.
    #[serde(default)]
    pub command: Option<String>,
}

impl RecipeSession {
    /// A session named `name` in root `root`, default profile, no command.
    pub fn new(name: impl Into<String>, root: u16) -> Self {
        Self { name: name.into(), profile: None, root, subdir: None, command: None }
    }
}

/// A startup recipe.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Recipe {
    /// Sessions in sidebar order.
    pub sessions: Vec<RecipeSession>,
    /// Arrangement in the project view.
    #[serde(default)]
    pub arrangement: Arrangement,
}

/// A session to spawn, resolved against the project's roots.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SessionLaunch {
    /// Session name.
    pub name: String,
    /// Shell profile name.
    pub profile: Option<String>,
    /// Absolute start directory (root joined with the sub-directory).
    pub cwd: PathBuf,
    /// Command to type once the shell starts.
    pub command: Option<String>,
}

impl Recipe {
    /// Preset "One terminal" (`docs/09` §3): one session in the first root.
    pub fn one_terminal() -> Self {
        Self { sessions: vec![RecipeSession::new("terminal", 0)], arrangement: Arrangement::Single }
    }

    /// Preset "Front + back" (`docs/09` §3): two sessions side by side, in the first and second
    /// root when the project has two or more roots, else both in the first.
    pub fn front_and_back(root_count: usize) -> Self {
        let back_root = u16::from(root_count > 1);
        Self {
            sessions: vec![RecipeSession::new("front", 0), RecipeSession::new("back", back_root)],
            arrangement: Arrangement::SideBySide,
        }
    }

    /// Checks names, root indices, sub-directories and the session count.
    ///
    /// # Errors
    /// The first [`ProjectError`] found.
    pub fn validate(&self, root_count: usize) -> Result<(), ProjectError> {
        if self.sessions.len() > MAX_RECIPE_SESSIONS {
            return Err(ProjectError::TooManySessions { max: MAX_RECIPE_SESSIONS });
        }
        for session in &self.sessions {
            check_name(&session.name)?;
            check_root_index(session.root, root_count)?;
            if let Some(subdir) = &session.subdir {
                check_subdir(subdir)?;
            }
        }
        Ok(())
    }

    /// The sessions to spawn with their directories resolved against `roots`.
    ///
    /// # Errors
    /// [`ProjectError`] if the recipe is not valid for `roots`.
    pub fn launches(&self, roots: &[PathBuf]) -> Result<Vec<SessionLaunch>, ProjectError> {
        self.validate(roots.len())?;
        let launch = |session: &RecipeSession| {
            let root = roots
                .get(usize::from(session.root))
                .ok_or(ProjectError::UnknownRoot(session.root))?;
            let cwd = session.subdir.as_ref().map_or_else(|| root.clone(), |sub| root.join(sub));
            Ok(SessionLaunch {
                name: session.name.clone(),
                profile: session.profile.clone(),
                cwd,
                command: session.command.clone(),
            })
        };
        self.sessions.iter().map(launch).collect()
    }

    /// The layout for the spawned sessions `keys` (in recipe order), per [`Recipe::arrangement`].
    pub fn layout(&self, keys: &[SessionKey]) -> Option<Layout> {
        match self.arrangement {
            Arrangement::Single => keys.first().map(|&session| Layout::Pane { session }),
            Arrangement::SideBySide => Layout::equal(keys, Axis::Horizontal),
            Arrangement::Stacked => Layout::equal(keys, Axis::Vertical),
        }
    }
}

/// A sub-directory must be relative and only go down (`..`, roots and prefixes are refused).
fn check_subdir(subdir: &Path) -> Result<(), ProjectError> {
    let only_down =
        subdir.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if only_down && !subdir.as_os_str().is_empty() {
        Ok(())
    } else {
        Err(ProjectError::BadRecipeDir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid_for_their_root_count() {
        assert_eq!(Recipe::one_terminal().validate(1), Ok(()));
        assert_eq!(Recipe::front_and_back(1).validate(1), Ok(()));
        let two = Recipe::front_and_back(2);
        assert_eq!(two.sessions.get(1).map(|s| s.root), Some(1));
        assert_eq!(two.validate(1), Err(ProjectError::UnknownRoot(1)));
    }

    #[test]
    fn launches_resolve_roots_and_subdirectories() {
        let roots = [PathBuf::from("/w/front"), PathBuf::from("/w/back")];
        let mut recipe = Recipe::front_and_back(2);
        if let Some(back) = recipe.sessions.get_mut(1) {
            back.subdir = Some(PathBuf::from("api"));
            back.command = Some("cargo run".to_owned());
        }
        let launches = recipe.launches(&roots).unwrap();
        assert_eq!(
            launches.iter().map(|l| l.cwd.clone()).collect::<Vec<_>>(),
            [PathBuf::from("/w/front"), PathBuf::from("/w/back/api")]
        );
        assert_eq!(launches.get(1).and_then(|l| l.command.as_deref()), Some("cargo run"));
    }

    #[test]
    fn escaping_subdirectories_and_bad_names_are_rejected() {
        for bad in ["../x", "a/../../x", "", "/abs"] {
            let mut recipe = Recipe::one_terminal();
            if let Some(s) = recipe.sessions.first_mut() {
                s.subdir = Some(PathBuf::from(bad));
            }
            assert_eq!(recipe.validate(1), Err(ProjectError::BadRecipeDir), "{bad}");
        }
        let unnamed = Recipe { sessions: vec![RecipeSession::new(" ", 0)], ..Recipe::default() };
        assert!(matches!(unnamed.validate(1), Err(ProjectError::BadName { .. })));
        let many = Recipe {
            sessions: vec![RecipeSession::new("s", 0); MAX_RECIPE_SESSIONS + 1],
            ..Recipe::default()
        };
        assert!(matches!(many.validate(1), Err(ProjectError::TooManySessions { .. })));
    }

    #[test]
    fn arrangement_decides_the_layout() {
        let keys = [SessionKey(1), SessionKey(2)];
        assert_eq!(
            Recipe::one_terminal().layout(&keys),
            Some(Layout::Pane { session: SessionKey(1) })
        );
        let side = Recipe::front_and_back(2).layout(&keys);
        assert!(matches!(side, Some(Layout::Split { axis: Axis::Horizontal, ratio: 500, .. })));
        assert_eq!(Recipe::one_terminal().layout(&[]), None);
    }
}
