//! Settings → Import / Export (`docs/09` §8, `FR-DATA-002`, `FR-DATA-003`).
//!
//! The caps of the `.ferrum` archive. They apply to export and import alike, so an archive this
//! machine writes can be imported on another with the same settings. Defaults and bounds are
//! recorded in `docs/14` Q-30.

use serde::{Deserialize, Serialize};

use crate::value::bounded;

bounded!(
    /// Most files in one archive. Default 10 000: settings, keybindings and projects are a few
    /// dozen files, themes a few hundred, scrollback one file per session; 10 000 leaves two
    /// orders of magnitude of room while bounding the work an import does. Bounds: 100 covers a
    /// minimal configuration with its themes; 100 000 is beyond any real configuration.
    ArchiveMaxEntries(u32),
    min = 100,
    max = 100_000,
    default = 10_000
);
bounded!(
    /// Largest sum of file sizes in one archive, in MiB. Default 5 184 = 20 sessions (the
    /// `NFR-PERF-04` scenario) × 256 MiB per-session spill cap (`docs/10` §3 item 11) + 64 MiB for
    /// configuration, so an export with scrollback of that scenario fits. Bounds: 64 MiB holds
    /// any configuration without scrollback; 65 536 MiB (64 GiB) is 256 sessions at the maximal
    /// default cap.
    ArchiveMaxTotalMib(u32),
    min = 64,
    max = 65_536,
    default = 5_184
);

/// Import / Export section.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct ImportExport {
    /// Most files in one archive.
    pub max_entries: ArchiveMaxEntries,
    /// Largest total size in MiB.
    pub max_total_mib: ArchiveMaxTotalMib,
}
