//! The spawn request of a session: its shell profile (FR-SESS-004), the session environment
//! (FR-SESS-007), its working directory (the last known one, else its project root: FR-PROJ-003,
//! FR-SESS-008) and shell integration (FR-SESS-005).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use ferrum_config::Profile;
use ferrum_pty::{EnvOp, PtySize, SpawnSpec, session_env, shell_integration};

use crate::config::DaemonConfig;

/// Everything about one spawn that is not in the settings.
pub(crate) struct Launch<'a> {
    /// Runtime id of the session, for `FERRUM_SESSION_ID`.
    pub(crate) session: u64,
    /// The project, for `FERRUM_PROJECT_ID`.
    pub(crate) project: String,
    /// The profile asked for, `None` for the default one.
    pub(crate) profile: Option<&'a str>,
    /// Where the child starts.
    pub(crate) cwd: PathBuf,
    /// Initial terminal size.
    pub(crate) size: PtySize,
}

/// The profile named `name`, else the settings' default profile, else the first profile;
/// `None` when no profile is configured.
fn find_profile<'a>(config: &'a DaemonConfig, name: Option<&str>) -> Option<&'a Profile> {
    let shells = &config.settings.shells;
    let by_name = |wanted: &str| shells.profiles.iter().find(|profile| profile.name == wanted);
    name.and_then(by_name)
        .or_else(|| shells.default_profile.as_deref().and_then(by_name))
        .or_else(|| shells.profiles.first())
}

/// The spawn request for `launch`.
pub(crate) fn spawn_spec(config: &DaemonConfig, launch: &Launch<'_>) -> SpawnSpec {
    let profile = find_profile(config, launch.profile);
    let mut env = session_env(&launch.session.to_string(), &launch.project);
    let (program, args) =
        match profile {
            Some(profile) => {
                env.extend(profile.env.iter().map(|(name, value)| {
                    (OsString::from(name), EnvOp::Set(OsString::from(value)))
                }));
                let args = profile.args.iter().map(OsString::from).collect();
                (OsString::from(&profile.command), args)
            }
            None => (config.default_shell.clone(), Vec::new()),
        };
    let cwd = profile_cwd(profile, &launch.cwd);
    let mut spec = SpawnSpec { program, args, cwd, env, size: launch.size };
    if profile.is_none_or(|profile| profile.shell_integration) {
        // `None` means the shell has no integration script or got other arguments: unchanged.
        let _shell = shell_integration::inject(&mut spec, &config.shell_integration_dir());
    }
    spec
}

/// A profile's own directory wins over the session's only when the session has none that exists.
fn profile_cwd(profile: Option<&Profile>, session_cwd: &Path) -> PathBuf {
    if session_cwd.is_dir() {
        return session_cwd.to_path_buf();
    }
    profile.and_then(|profile| profile.cwd.clone()).unwrap_or_else(|| session_cwd.to_path_buf())
}
