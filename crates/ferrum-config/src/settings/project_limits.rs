//! Settings → Projects → limits (`FR-PROJ-001`, maintainer decision Q-30).
//!
//! The user's limits on a project's size. `ferrum-project` applies them when the user edits a
//! project (`Project::check_limits`); the upper bound of each range is also the hard cap checked
//! on every project file, which is untrusted input. Defaults and bounds are recorded in
//! `docs/14` Q-30.

use serde::{Deserialize, Serialize};

use crate::value::bounded;

bounded!(
    /// Most root directories of one project ("1..N", `FR-PROJ-001`). Default 16: room for a
    /// front end, a back end and several shared libraries or monorepo packages while the root
    /// list of the project dialog stays one screen. Bound 64: the dialog list scrolls past that.
    MaxRoots(u16),
    min = 1,
    max = 64,
    default = 16
);
bounded!(
    /// Longest project, session or recipe-session name in characters. Default 128: longer than
    /// any name the 160–420 px sidebar can show (`docs/09` §1) and than common directory names.
    /// Bounds: 16 fits a directory-derived name; 1 024 bounds what a file may carry.
    MaxNameChars(u16),
    min = 16,
    max = 1_024,
    default = 128
);
bounded!(
    /// Most sessions of one project. Default 256: ten times the 20+ session scenario of
    /// `NFR-PERF-04`. Bound 1 024: at about 1 MB per session (`docs/03` §2) more cannot fit the
    /// memory budget anyway.
    MaxSessions(u16),
    min = 1,
    max = 1_024,
    default = 256
);
bounded!(
    /// Most sessions a startup recipe opens at once (`FR-PROJ-007`). Default 32: every pane of a
    /// densely split screen; opening more at once is not a startup layout. Bound 256: one quarter
    /// of the session cap.
    MaxRecipeSessions(u16),
    min = 1,
    max = 256,
    default = 32
);

/// Limits on a project's size.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct ProjectLimits {
    /// Most roots.
    pub max_roots: MaxRoots,
    /// Longest name.
    pub max_name_chars: MaxNameChars,
    /// Most sessions.
    pub max_sessions: MaxSessions,
    /// Most recipe sessions.
    pub max_recipe_sessions: MaxRecipeSessions,
}
