//! The settings model of `settings.toml` (`FR-UI-007`, `FR-DATA-001`, `docs/09` §8).
//!
//! Every section and field has a default, so a partial file (or none) is valid; unknown keys are
//! ignored (forward compatibility, `docs/02` §8). Keybindings and themes live in their own files
//! (`keybindings.toml`, `themes/*.toml`), modeled in [`crate::Keybindings`] and [`crate::Theme`].
//!
//! Values the spec does not fix are user settings whose default and bounds are explained on each
//! type (maintainer decision Q-30, `docs/14` §2).

mod appearance;
mod performance;
mod shells;
mod terminal;
mod v0;
mod window;

use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

pub use appearance::{
    Appearance, CursorStyle, DEFAULT_FONT_FAMILY, DEFAULT_THEME, Density, FontSize, LetterSpacing,
    LineHeight, PaddingX, PaddingY, SidebarWidth, UiScale,
};
pub use performance::{
    Channel, GlyphCacheMib, ImageCacheMib, MaxFps, Performance, SpillDiskCapMib,
    SpillThresholdLines, Updates,
};
pub use shells::{Profile, Shells};
pub use terminal::{
    Bell, COLUMN_PLACEHOLDER, DEFAULT_URL_SCHEMES, DEFAULT_WORD_CHARACTERS, EDITOR_PLACEHOLDER,
    EditorCommand, FILE_PLACEHOLDER, LINE_PLACEHOLDER, OptionAsAlt, Osc52Policy, ScrollSpeed,
    ScrollbackLines, Terminal, TitleReporting,
};
pub use window::{
    KeepSessions, NewSessionRoot, OpenProjectIn, Projects, RecentProjects, SidebarState, StatusBar,
    Titlebar, Window,
};

use crate::error::ConfigError;
use crate::schema::Schema;
use crate::store::{self, Loaded};

/// File name of the settings file inside the per-user config directory.
pub const SETTINGS_FILE: &str = "settings.toml";

/// Versions of `settings.toml`. Version 0 is the synthetic pre-versioned flat layout described in
/// `fixtures/config/README.md`; version 1 is the sectioned layout of [`Settings`].
pub const SETTINGS_SCHEMA: Schema =
    Schema { name: "settings", current: 1, migrations: &[v0::to_v1] };

/// All user settings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Settings {
    /// Appearance.
    pub appearance: Appearance,
    /// Terminal behavior.
    pub terminal: Terminal,
    /// Shells and profiles.
    pub shells: Shells,
    /// Projects behavior.
    pub projects: Projects,
    /// Window behavior.
    pub window: Window,
    /// Performance and memory.
    pub performance: Performance,
    /// Updates.
    pub updates: Updates,
}

impl Settings {
    /// Loads `path`, migrating old versions in memory and setting an unusable file aside
    /// ([`store::load_or_recover`]).
    ///
    /// # Errors
    /// See [`store::load_or_recover`].
    pub fn load(path: &Path, now: SystemTime) -> Result<Loaded<Self>, ConfigError> {
        store::load_or_recover(path, &SETTINGS_SCHEMA, now)
    }

    /// Writes the settings atomically to `path` at the current schema version.
    ///
    /// # Errors
    /// See [`store::save`].
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        store::save(path, self, &SETTINGS_SCHEMA)
    }
}
