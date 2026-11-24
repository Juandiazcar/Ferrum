//! Startup recipes: the sessions a project opens with (`FR-PROJ-007`, `docs/09` §3).

use std::path::{Component, Path, PathBuf};

use ferrum_config::MaxRecipeSessions;
use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::layout::{Axis, Layout};
use crate::project::{
    SessionKey, check_name, check_profile, check_root_index, check_roots, suggest_name,
};

/// Longest startup command, recipe sub-directory or stored path in bytes. Anti-hostile cap: it is
/// Linux `PATH_MAX` (4 096), longer than any path a session can have, and a typed startup command
/// longer than a full path is not a startup command.
pub const MAX_TEXT_BYTES: usize = 4_096;

/// How the recipe's sessions are arranged in the project view (the two presets of `docs/09` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrangement {
    /// Only the first session is shown; the others wait in the sidebar ("One terminal").
    #[default]
    Single,
    /// All sessions side by side with equal widths ("Front + back").
    SideBySide,
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
    /// Command typed into the shell once it starts (one line).
    #[serde(default)]
    pub command: Option<String>,
}

impl RecipeSession {
    /// A session named `name` in root `root`, default profile, no command.
    pub fn new(name: impl Into<String>, root: u16) -> Self {
        Self { name: name.into(), profile: None, root, subdir: None, command: None }
    }
}

/// A startup recipe. Checked as part of its project ([`Recipe::validate`]).
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
    /// Absolute start directory (an absolute root joined with a relative sub-directory).
    pub cwd: PathBuf,
    /// Command to type once the shell starts.
    pub command: Option<String>,
}

impl Recipe {
    /// Preset "One terminal" (`docs/09` §3): one session in the first root, named after it.
    pub fn one_terminal(first_root: &Path) -> Self {
        Self {
            sessions: vec![RecipeSession::new(suggest_name(first_root), 0)],
            arrangement: Arrangement::Single,
        }
    }

    /// Preset "Front + back" (`docs/09` §3): two sessions side by side, in the first and second
    /// root (both in the first when there is only one), each named after its root. `None` when
    /// `roots` is empty.
    pub fn front_and_back(roots: &[PathBuf]) -> Option<Self> {
        let front = roots.first()?;
        let (back_index, back) = roots.get(1).map_or((0, front), |root| (1, root));
        Some(Self {
            sessions: vec![
                RecipeSession::new(suggest_name(front), 0),
                RecipeSession::new(suggest_name(back), back_index),
            ],
            arrangement: Arrangement::SideBySide,
        })
    }

    /// Checks names, profiles, root indices, sub-directories, commands and the session count
    /// (hard cap `MaxRecipeSessions::MAX`; the user's own limit is in `Project::check_limits`).
    ///
    /// # Errors
    /// The first [`ProjectError`] found.
    pub fn validate(&self, root_count: usize) -> Result<(), ProjectError> {
        let max = usize::from(MaxRecipeSessions::MAX);
        if self.sessions.len() > max {
            return Err(ProjectError::TooManySessions { max });
        }
        for session in &self.sessions {
            check_name(&session.name)?;
            check_profile(session.profile.as_deref())?;
            check_root_index(session.root, root_count)?;
            if let Some(subdir) = &session.subdir {
                check_subdir(subdir)?;
            }
            if let Some(command) = &session.command
                && (command.trim().is_empty() || !is_clean_text(command))
            {
                return Err(ProjectError::BadCommand);
            }
        }
        Ok(())
    }

    /// The sessions to spawn with their directories resolved against `roots`, which are checked
    /// like a project's roots (absolute, distinct), so every [`SessionLaunch::cwd`] is absolute.
    ///
    /// # Errors
    /// [`ProjectError::BadRoots`] for unusable roots, or another [`ProjectError`] if the recipe is
    /// not valid for them.
    pub fn launches(&self, roots: &[PathBuf]) -> Result<Vec<SessionLaunch>, ProjectError> {
        check_roots(roots)?;
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
        }
    }
}

/// Text that is at most [`MAX_TEXT_BYTES`] long and has no control characters.
pub(crate) fn is_clean_text(text: &str) -> bool {
    text.len() <= MAX_TEXT_BYTES && !text.chars().any(char::is_control)
}

/// A sub-directory must be relative and only go down (`..`, roots and prefixes are refused).
fn check_subdir(subdir: &Path) -> Result<(), ProjectError> {
    let only_down =
        subdir.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    let text = subdir.to_string_lossy();
    if only_down && !text.is_empty() && is_clean_text(&text) {
        Ok(())
    } else {
        Err(ProjectError::BadRecipeDir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> [PathBuf; 2] {
        let base = std::env::temp_dir();
        [base.join("front"), base.join("back")]
    }

    #[test]
    fn presets_are_named_after_their_roots() {
        let [front, back] = roots();
        let one = Recipe::one_terminal(&front);
        assert_eq!(one.sessions.first().map(|s| s.name.as_str()), Some("front"));
        assert_eq!(one.validate(1), Ok(()));
        let two = Recipe::front_and_back(&[front.clone(), back]).unwrap();
        let names: Vec<_> = two.sessions.iter().map(|s| (s.name.as_str(), s.root)).collect();
        assert_eq!(names, [("front", 0), ("back", 1)]);
        assert_eq!(two.validate(1), Err(ProjectError::UnknownRoot(1)));
        let single = Recipe::front_and_back(std::slice::from_ref(&front)).unwrap();
        assert_eq!(single.validate(1), Ok(()));
        assert_eq!(Recipe::front_and_back(&[]), None);
    }

    #[test]
    fn launches_resolve_roots_and_subdirectories() {
        let roots = roots();
        let mut recipe = Recipe::front_and_back(&roots).unwrap();
        if let Some(back) = recipe.sessions.get_mut(1) {
            back.subdir = Some(PathBuf::from("api"));
            back.command = Some("cargo run".to_owned());
        }
        let launches = recipe.launches(&roots).unwrap();
        let [front, back] = roots;
        let cwds: Vec<_> = launches.iter().map(|l| l.cwd.clone()).collect();
        assert_eq!(cwds, [front, back.join("api")]);
        assert_eq!(launches.get(1).and_then(|l| l.command.as_deref()), Some("cargo run"));
        assert!(launches.iter().all(|l| l.cwd.is_absolute()));
    }

    #[test]
    fn launches_refuse_relative_or_missing_roots() {
        let recipe = Recipe::one_terminal(Path::new("/w"));
        let relative = [PathBuf::from("relative/front")];
        assert!(matches!(recipe.launches(&relative), Err(ProjectError::BadRoots { .. })));
        assert!(matches!(recipe.launches(&[]), Err(ProjectError::BadRoots { .. })));
    }

    #[test]
    fn escaping_subdirectories_bad_names_profiles_and_commands_are_rejected() {
        let long = "d".repeat(MAX_TEXT_BYTES + 1);
        for bad in ["../x", "a/../../x", "", "/abs", "a\u{1}b", long.as_str()] {
            let mut recipe = Recipe::one_terminal(Path::new("/w"));
            if let Some(s) = recipe.sessions.first_mut() {
                s.subdir = Some(PathBuf::from(bad));
            }
            assert_eq!(recipe.validate(1), Err(ProjectError::BadRecipeDir), "{bad}");
        }
        let edit = |change: fn(&mut RecipeSession)| {
            let mut recipe = Recipe::one_terminal(Path::new("/w"));
            if let Some(s) = recipe.sessions.first_mut() {
                change(s);
            }
            recipe.validate(1)
        };
        assert!(matches!(edit(|s| s.name = " ".to_owned()), Err(ProjectError::BadName { .. })));
        assert_eq!(edit(|s| s.profile = Some(String::new())), Err(ProjectError::BadProfile));
        assert_eq!(
            edit(|s| s.command = Some("ls\nrm -rf x".to_owned())),
            Err(ProjectError::BadCommand)
        );
        assert_eq!(edit(|s| s.command = Some("  ".to_owned())), Err(ProjectError::BadCommand));
        let many = Recipe {
            sessions: vec![RecipeSession::new("s", 0); usize::from(MaxRecipeSessions::MAX) + 1],
            ..Recipe::default()
        };
        assert!(matches!(many.validate(1), Err(ProjectError::TooManySessions { .. })));
    }

    #[test]
    fn arrangement_decides_the_layout() {
        let keys = [SessionKey(1), SessionKey(2)];
        let one = Recipe::one_terminal(Path::new("/w"));
        assert_eq!(one.layout(&keys), Some(Layout::Pane { session: SessionKey(1) }));
        let side = Recipe::front_and_back(&roots()).unwrap().layout(&keys);
        assert!(matches!(side, Some(Layout::Split { axis: Axis::Horizontal, ratio: 500, .. })));
        assert_eq!(one.layout(&[]), None);
    }
}
