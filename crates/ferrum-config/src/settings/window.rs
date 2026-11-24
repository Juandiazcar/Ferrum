//! Settings → Window and Projects (`docs/09` §8, `FR-UI-012`, `FR-UI-018`).
//!
//! Every default and bound below is either fixed by the spec (cited) or chosen and recorded in
//! `docs/14` Q-30 with the reason given here; all of them are editable in the settings UI.

use serde::{Deserialize, Serialize};

use crate::value::bounded;

/// Titlebar style (`FR-UI-012`, `docs/09` §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Titlebar {
    /// Client-drawn titlebar (default, `docs/00` §5).
    #[default]
    Custom,
    /// The platform's titlebar.
    Native,
}

/// Sidebar state when a window opens. Default expanded: projects and sessions are visible on
/// first launch (`docs/09` §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SidebarState {
    /// Full sidebar.
    #[default]
    Expanded,
    /// Icon rail.
    Collapsed,
}

/// Whether daemon sessions are kept when the last window closes (`docs/09` §8: ask / always /
/// never). Default ask: either answer can lose work the user expected, so the user decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum KeepSessions {
    /// Ask every time.
    #[default]
    Ask,
    /// Always keep them running.
    Always,
    /// Close them with the window.
    Never,
}

/// Where a project opens when another one is already shown (multi-window behavior,
/// `FR-UI-018`: a window shows one project at a time). Default current window: one window is the
/// `docs/09` §1 layout and costs no second surface (`docs/03` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OpenProjectIn {
    /// Replace the project shown in the focused window.
    #[default]
    CurrentWindow,
    /// Open a new window for it.
    NewWindow,
}

/// Status bar items that can be hidden (`FR-UI-005`; all shown by default, as `docs/09` §1
/// draws them).
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent on/off setting shown as a toggle in the UI (docs/09 §8)"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct StatusBar {
    /// Project name.
    pub project: bool,
    /// Git branch.
    pub branch: bool,
    /// Active session cwd.
    pub cwd: bool,
    /// Session count.
    pub session_count: bool,
    /// Ferrum's own live memory.
    pub memory: bool,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self { project: true, branch: true, cwd: true, session_count: true, memory: true }
    }
}

/// Window section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Window {
    /// Titlebar style.
    pub titlebar: Titlebar,
    /// Sidebar state at window open.
    pub sidebar: SidebarState,
    /// Visible status bar items.
    pub status_bar: StatusBar,
    /// Multi-window behavior.
    pub open_project_in: OpenProjectIn,
    /// Close behavior.
    pub keep_sessions_on_close: KeepSessions,
}

/// Root a new session starts in when none is chosen ("default roots behavior", `docs/09` §8).
/// Default first root: `FR-PROJ-003`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NewSessionRoot {
    /// The project's first root (`FR-PROJ-003`).
    #[default]
    FirstRoot,
    /// The root of the focused session.
    FocusedSessionRoot,
}

bounded!(
    /// How many recently opened projects the UI remembers (`docs/09` §8 "recent projects").
    /// Default 10: one screen of a menu without scrolling. Bound 50: the quick switch
    /// (`FR-PROJ-006`) covers longer histories.
    RecentProjects(u16),
    min = 0,
    max = 50,
    default = 10
);

/// Projects section.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Projects {
    /// Restore each project's last layout when it opens (`docs/09` §8, `FR-PROJ-005`: layouts
    /// persist; restoring is the point of persisting them, so it is on by default).
    pub restore_last_layout: bool,
    /// Default root of new sessions.
    pub new_session_root: NewSessionRoot,
    /// Recent projects remembered.
    pub recent_projects: RecentProjects,
}

impl Default for Projects {
    fn default() -> Self {
        Self {
            restore_last_layout: true,
            new_session_root: NewSessionRoot::default(),
            recent_projects: RecentProjects::default(),
        }
    }
}
