//! Settings → Performance & memory and Updates (`docs/09` §8, `docs/03`).
//!
//! Every default and bound below is either fixed by the spec (cited) or chosen and recorded in
//! `docs/14` Q-30 with the reason given here; all of them are editable in the settings UI.

use serde::{Deserialize, Serialize};

use crate::value::bounded;

bounded!(
    /// Scrollback lines kept in RAM before cold history spills to disk (`FR-TERM-011`: default
    /// 50 000). Bound 1 024 to 1 000 000: one hot chunk of 1 024 lines (`docs/02` §3.2) is the
    /// smallest unit that can spill; 1 000 000 is the scrollback maximum, i.e. never spill.
    SpillThresholdLines(u32),
    min = 1_024,
    max = 1_000_000,
    default = 50_000
);
bounded!(
    /// Per-session disk cap of spilled scrollback in MiB (`docs/10` §3 item 11: default 256 MB).
    /// Bounds: 16 MiB holds a few hot-chunk spills; 4 096 MiB keeps ten sessions under 40 GiB of
    /// disk, beyond which a single program could fill a laptop disk.
    SpillDiskCapMib(u32),
    min = 16,
    max = 4_096,
    default = 256
);
bounded!(
    /// Glyph cache cap in MiB (`docs/02` §6: default 3 MB). Bounds: 1 MiB still holds every
    /// ASCII glyph of a few sizes (S3 measured 10 KB for 94 glyphs); 64 MiB exceeds the whole
    /// 26 MB UI allowance (`docs/03` §2), so more can never be within budget.
    GlyphCacheMib(u16),
    min = 1,
    max = 64,
    default = 3
);
bounded!(
    /// Image cache cap in MiB (`docs/03` §2: +3 MB cap). Bounds: as [`GlyphCacheMib`].
    ImageCacheMib(u16),
    min = 1,
    max = 64,
    default = 3
);

bounded!(
    /// Seconds the daemon waits with no session and no client before it exits (`FR-SESS-003`:
    /// default 10 s). Bounds: 1 s still lets a UI that started the daemon connect to it; 3 600 s
    /// (an hour) is the longest an unused daemon may stay resident.
    DaemonExitGraceSecs(u16),
    min = 1,
    max = 3_600,
    default = 10
);

/// Frame rate cap (`docs/02` §6: default 60 Hz).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MaxFps {
    /// 30 frames per second.
    #[serde(rename = "30")]
    Fps30,
    /// 60 frames per second (default).
    #[default]
    #[serde(rename = "60")]
    Fps60,
    /// 120 frames per second.
    #[serde(rename = "120")]
    Fps120,
    /// The display refresh rate.
    #[serde(rename = "auto")]
    Auto,
}

/// Performance & memory section.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Performance {
    /// Frame rate cap.
    pub max_fps: MaxFps,
    /// Spill threshold.
    pub spill_threshold_lines: SpillThresholdLines,
    /// Spill disk cap per session.
    pub spill_disk_cap_mib: SpillDiskCapMib,
    /// Glyph cache cap.
    pub glyph_cache_mib: GlyphCacheMib,
    /// Image cache cap.
    pub image_cache_mib: ImageCacheMib,
    /// Idle exit grace period of the daemon.
    pub daemon_exit_grace_secs: DaemonExitGraceSecs,
}

/// Update channel (`FR-DIST-002`). Default stable: pre-releases are opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Channel {
    /// Stable releases.
    #[default]
    Stable,
    /// Pre-releases.
    Beta,
}

/// Updates section (`FR-DIST-002`: user-disableable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Updates {
    /// Check for updates at most once per 24 h. Default on: `FR-DIST-002` describes the check as
    /// a feature the user can disable, i.e. enabled unless turned off.
    pub check_automatically: bool,
    /// Release channel.
    pub channel: Channel,
}

impl Default for Updates {
    fn default() -> Self {
        Self { check_automatically: true, channel: Channel::default() }
    }
}
