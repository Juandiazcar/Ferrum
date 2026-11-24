//! Settings → Shells & profiles (`FR-SESS-004`, `FR-SESS-005`, `docs/09` §8).
//!
//! Only user-defined profiles are stored; auto-detected shells come from `ferrum-platform` at run
//! time and are not persisted.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A user-defined shell profile.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Profile {
    /// Display name; also the key other settings refer to.
    pub name: String,
    /// Program to run.
    pub command: String,
    /// Arguments, passed as an argv array (never through a shell string).
    pub args: Vec<String>,
    /// Extra environment variables.
    pub env: BTreeMap<String, String>,
    /// Start directory; `None` uses the project root.
    pub cwd: Option<PathBuf>,
    /// Glyph shown next to the profile name.
    pub icon: Option<String>,
    /// Inject shell integration (OSC 7 / OSC 133) for supported shells (`FR-SESS-005`: opt-out).
    pub shell_integration: bool,
}

impl Profile {
    /// A profile running `command` with no arguments and shell integration on.
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            shell_integration: true,
            ..Self::default()
        }
    }
}

/// Shells & profiles section.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Shells {
    /// Name of the default profile; `None` uses the detected login shell.
    pub default_profile: Option<String>,
    /// User-defined profiles.
    pub profiles: Vec<Profile>,
}
