//! Settings → Performance & memory and Updates (`docs/09` §8, `docs/03`).

use serde::{Deserialize, Serialize};

use crate::value::bounded;

bounded!(
    /// Scrollback lines kept in RAM before cold history spills to disk (`FR-TERM-011`: default
    /// 50 000).
    SpillThresholdLines(u32),
    min = 1_024,
    max = 1_000_000,
    default = 50_000
);
bounded!(
    /// Glyph cache cap in MiB (`docs/02` §6: default 3 MB).
    GlyphCacheMib(u16),
    min = 1,
    max = 64,
    default = 3
);
bounded!(
    /// Image cache cap in MiB (`docs/03` §2: +3 MB cap).
    ImageCacheMib(u16),
    min = 1,
    max = 64,
    default = 3
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
    /// Glyph cache cap.
    pub glyph_cache_mib: GlyphCacheMib,
    /// Image cache cap.
    pub image_cache_mib: ImageCacheMib,
}

/// Update channel (`FR-DIST-002`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Channel {
    /// Stable releases (default).
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
    /// Check for updates at most once per 24 h.
    pub check_automatically: bool,
    /// Release channel.
    pub channel: Channel,
}

impl Default for Updates {
    fn default() -> Self {
        Self { check_automatically: true, channel: Channel::default() }
    }
}
