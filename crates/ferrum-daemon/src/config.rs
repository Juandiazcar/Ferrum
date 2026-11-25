//! What the composition root hands the daemon: directories, the handshake nonce, the default
//! shell and the user's settings, all as plain values (ADR 0023), plus the derived limits.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use ferrum_config::{MaxFps, Settings};
use ferrum_proto::NONCE_LEN;
use ferrum_scrollback::ScrollbackConfig;

/// Frame interval of damage at the 60 Hz default (`docs/02` §6), also used for `auto`: the daemon
/// has no display, the UI paces its own frames.
const FRAME_60_HZ: Duration = Duration::from_micros(16_667);
/// Frame interval at 30 Hz.
const FRAME_30_HZ: Duration = Duration::from_micros(33_333);
/// Frame interval at 120 Hz.
const FRAME_120_HZ: Duration = Duration::from_micros(8_333);
/// Bytes in a MiB (the spill cap setting is in MiB).
const MIB: u64 = 1_024 * 1_024;

/// Everything the daemon needs besides its ports.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// `data_dir/ferrum`: projects and layouts.
    pub data_dir: PathBuf,
    /// `runtime_dir/ferrum`: spill files and shell integration scripts (owner-only).
    pub runtime_dir: PathBuf,
    /// The nonce clients must present (`docs/10` §2), as written to the nonce file.
    pub nonce: [u8; NONCE_LEN],
    /// The shell used when no profile is configured.
    pub default_shell: OsString,
    /// The user's settings.
    pub settings: Settings,
}

impl DaemonConfig {
    /// Interval at which damage of a visible session is sent (`docs/02` §4: coalesced per frame).
    pub fn frame_interval(&self) -> Duration {
        match self.settings.performance.max_fps {
            MaxFps::Fps30 => FRAME_30_HZ,
            MaxFps::Fps120 => FRAME_120_HZ,
            _ => FRAME_60_HZ,
        }
    }

    /// How long the daemon waits with no session and no client before it exits (FR-SESS-003).
    pub fn exit_grace(&self) -> Duration {
        Duration::from_secs(u64::from(self.settings.performance.daemon_exit_grace_secs.get()))
    }

    /// Scrollback length and spill threshold of new sessions (FR-TERM-011). The settings are
    /// range-checked when loaded, so the fallback to the defaults is never taken in practice.
    pub fn scrollback(&self) -> ScrollbackConfig {
        let lines = self.settings.terminal.scrollback_lines.get();
        let threshold = self.settings.performance.spill_threshold_lines.get();
        let to_usize = |value: u32| usize::try_from(value).unwrap_or(usize::MAX);
        ScrollbackConfig::new(to_usize(lines), to_usize(threshold)).unwrap_or_default()
    }

    /// Disk cap of one session's spill file in bytes (`docs/10` §3 item 11).
    pub fn spill_cap_bytes(&self) -> u64 {
        u64::from(self.settings.performance.spill_disk_cap_mib.get()).saturating_mul(MIB)
    }

    /// Directory of the shell integration scripts.
    pub fn shell_integration_dir(&self) -> PathBuf {
        self.runtime_dir.join("shell-integration")
    }
}
