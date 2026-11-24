//! Settings → Window and Projects (`docs/09` §8, `FR-UI-012`).

use serde::{Deserialize, Serialize};

/// Titlebar style (`FR-UI-012`, `docs/09` §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Titlebar {
    /// Client-drawn titlebar (default).
    #[default]
    Custom,
    /// The platform's titlebar.
    Native,
}

/// Sidebar state when a window opens.
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

/// Whether daemon sessions are kept when the last window closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum KeepSessions {
    /// Ask every time (default).
    #[default]
    Ask,
    /// Always keep them running.
    Always,
    /// Close them with the window.
    Never,
}

/// Status bar items that can be hidden (`FR-UI-005`).
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
    /// Close behavior.
    pub keep_sessions_on_close: KeepSessions,
}

/// Projects section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Projects {
    /// Restore each project's last layout when it opens (`FR-PROJ-005`).
    pub restore_last_layout: bool,
}

impl Default for Projects {
    fn default() -> Self {
        Self { restore_last_layout: true }
    }
}
