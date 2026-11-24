//! `ferrumd`: the Ferrum daemon executable, the composition root of the daemon side.
//!
//! It resolves the per-user directories, takes the single-instance lock (a second daemon of the
//! same user exits at once: the first one serves), writes the handshake nonce, opens the IPC
//! endpoint, loads the settings, and wires the real PTY backend, exit watcher, endpoint, spill
//! files and memory release into [`ferrum_daemon::Daemon`] (ADR 0023: this file is the only place
//! that names them). Logs go to the state directory, never to the terminal (`docs/05` §8), and
//! carry no path, content or environment value (`docs/10` §4): errors are logged by their fixed
//! text and kind only. It takes no arguments; the `FERRUM_*_DIR` variables move its directories
//! (`docs/11` §1).

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::Context as _;
use ferrum_config::{SETTINGS_FILE, Settings};
use ferrum_daemon::logging::RotatingFile;
use ferrum_daemon::{Daemon, DaemonConfig, Ports, SpillFactory, SystemClock};
use ferrum_platform::instance::{self, InstanceLock, LockError};
use ferrum_platform::memory::SystemMemory;
use ferrum_platform::{Dirs, ipc};
use ferrum_scrollback::{FileSpill, SpillStore};

/// Prefix and extension of spill files in the run directory.
const SPILL_PREFIX: &str = "spill-";
const SPILL_EXTENSION: &str = "bin";

/// Allocator per ADR 0016: `mimalloc` on macOS, the system allocator elsewhere.
#[cfg(target_os = "macos")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // `Display` of the outermost context only: a fixed text, never a source's path.
            tracing::error!(%error, "the daemon stopped");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let dirs = Dirs::resolve().context("the per-user directories are unknown")?;
    dirs.prepare_runtime().context("the run directory is not usable")?;
    init_logging(&dirs);
    let _lock = match InstanceLock::acquire(&dirs.runtime) {
        Ok(lock) => lock,
        // Another daemon of this user is running and serves the clients.
        Err(LockError::Held) => return Ok(()),
        Err(error) => return Err(error).context("the instance lock failed"),
    };
    let nonce = instance::write_nonce(&dirs.runtime).context("the nonce could not be written")?;
    let listener = ipc::listen(&dirs.runtime).context("the IPC endpoint could not be opened")?;
    let config = DaemonConfig {
        data_dir: dirs.data.clone(),
        runtime_dir: dirs.runtime.clone(),
        nonce,
        default_shell: ferrum_platform::default_shell(),
        settings: load_settings(&dirs),
    };
    let spill = RunDirSpill { dir: dirs.runtime.clone(), cap: config.spill_cap_bytes() };
    spill.remove_leftovers();
    let ports = Ports {
        pty: ferrum_pty::native_backend(),
        exits: ferrum_pty::native_exit_watch().context("the exit watcher could not start")?,
        listener,
        spill: Box::new(spill),
        memory: Box::new(SystemMemory),
        clock: Box::new(SystemClock),
    };
    let result = Daemon::new(config, ports).context("the daemon could not start")?.run();
    remove_run_files(&dirs);
    result.context("the event loop failed")
}

/// Spill files in the owner-only run directory (`docs/03` §3 rule 11, `docs/10` §3 item 11).
struct RunDirSpill {
    dir: PathBuf,
    cap: u64,
}

impl RunDirSpill {
    fn path(&self, session: u64) -> PathBuf {
        self.dir.join(format!("{SPILL_PREFIX}{session}.{SPILL_EXTENSION}"))
    }

    /// Removes spill files a crashed daemon left behind (this daemon holds the instance lock, so
    /// they are nobody's).
    fn remove_leftovers(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return };
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            let is_spill = path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
                name.starts_with(SPILL_PREFIX) && name.ends_with(SPILL_EXTENSION)
            });
            if is_spill {
                let _removed = std::fs::remove_file(&path); // a leftover only costs disk space
            }
        }
    }
}

impl SpillFactory for RunDirSpill {
    fn create(&self, session: u64) -> Option<Box<dyn SpillStore>> {
        Some(Box::new(FileSpill::new(self.path(session), self.cap)))
    }
}

/// The settings file, or the defaults when it is missing or unusable (it is set aside).
fn load_settings(dirs: &Dirs) -> Settings {
    match Settings::load(&dirs.config.join(SETTINGS_FILE), SystemTime::now()) {
        Ok(loaded) => loaded.value,
        Err(error) => {
            tracing::error!(kind = error.kind(), "the settings could not be read; defaults apply");
            Settings::default()
        }
    }
}

/// Logs at `warn` and above to the rotating file; a panic is logged with its location only.
fn init_logging(dirs: &Dirs) {
    let Ok(file) = RotatingFile::open(&dirs.state) else { return };
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .finish();
    // Only fails if a subscriber is already set, which nothing else does.
    let _set = tracing::subscriber::set_global_default(subscriber);
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().map(|at| format!("{}:{}", at.file(), at.line()));
        tracing::error!(location, "panic (isolated per session where possible)");
    }));
}

/// The socket and nonce of this daemon; the lock file stays (the OS releases the lock).
fn remove_run_files(dirs: &Dirs) {
    for name in [ipc::SOCKET_FILE, instance::NONCE_FILE] {
        let _removed = std::fs::remove_file(dirs.runtime.join(name)); // absent on Windows: fine
    }
}
