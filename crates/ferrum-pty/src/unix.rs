//! Unix and macOS backend (ADR 0026, decision 2): `portable-pty` opens the pseudo terminal; the
//! child is started by `std::process::Command` with the environment given by the spawn and
//! nothing else (`docs/10` §3 item 10), found by the one lookup rule of every backend
//! ([`program`](crate::program)), as a session leader with the terminal as its controlling TTY.
//! Every descriptor of this process is close-on-exec, so the child inherits only the slave.
//!
//! Behavior matches the `ConPTY` backend: the output ends at `close`, never at the child's exit
//! ([`terminal`]); `kill` sends `SIGKILL` at once; `close` also kills every process attached to
//! the terminal ([`processes`]); a killed child reports [`ExitStatus::Killed`].

mod processes;
mod sys;
mod terminal;

use std::ffi::OsString;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::program::{self, Search};
use crate::{
    EnvOp, ExitSource, ExitStatus, PtyBackend, PtyChild, PtyError, PtyKiller, PtyReader, PtyResize,
    PtySession, PtySize, PtyWriter, Readiness, SpawnSpec,
};
use processes::Tty;
use terminal::{Terminal, Wait};

/// The Unix and macOS [`PtyBackend`].
#[derive(Clone, Copy, Debug, Default)]
pub struct UnixBackend;

impl PtyBackend for UnixBackend {
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtySession, PtyError> {
        spec.validate()?;
        let env = environment(spec);
        let search = Search::new(lookup(&env, "PATH"), None);
        let program = program::resolve(&spec.program, &spec.cwd, &search)?;
        let (master, slave, tty) = open_terminal(spec.size)?;
        let child = spawn_child(spec, &program, &env, &slave)?;
        let terminal = Arc::new(Terminal::new(master, slave)?);
        let pid = child.id();
        let process = Arc::new(Mutex::new(Process { child, killed: false }));
        Ok(PtySession {
            reader: Box::new(UnixReader(Arc::clone(&terminal))),
            writer: Box::new(UnixWriter(Arc::clone(&terminal))),
            resize: Box::new(UnixResize {
                terminal: Arc::clone(&terminal),
                size: Mutex::new(spec.size),
            }),
            child: Box::new(UnixChild { process, terminal, tty, pid }),
        })
    }
}

/// The inherited environment with the spawn's changes applied in order (names match exactly).
fn environment(spec: &SpawnSpec) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = std::env::vars_os().collect();
    for (name, op) in &spec.env {
        env.retain(|(existing, _)| existing != name);
        if let EnvOp::Set(value) = op {
            env.push((name.clone(), value.clone()));
        }
    }
    env
}

fn lookup(env: &[(OsString, OsString)], name: &str) -> Option<OsString> {
    env.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone())
}

fn io_error(op: &'static str) -> impl Fn(std::io::Error) -> PtyError {
    move |source| PtyError::io(op, source)
}

/// Opens a terminal: an owned master, the parent's slave descriptor and the terminal's identity.
fn open_terminal(
    size: PtySize,
) -> Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd, Tty), PtyError> {
    let open = io_error("open the pseudo terminal");
    let pty_size = portable_pty::PtySize {
        rows: size.rows.get(),
        cols: size.cols.get(),
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = portable_pty::native_pty_system()
        .openpty(pty_size)
        .map_err(|error| open(std::io::Error::other(error.to_string())))?;
    let master = sys::duplicate_master(pair.master.as_ref()).map_err(&open)?;
    let path: PathBuf = pair
        .master
        .tty_name()
        .ok_or_else(|| open(std::io::Error::other("the terminal has no name")))?;
    let flags = rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NOCTTY | rustix::fs::OFlags::CLOEXEC;
    let slave = rustix::fs::open(&path, flags, rustix::fs::Mode::empty())
        .map_err(|errno| open(errno.into()))?;
    let tty = Tty::new(&slave).map_err(&open)?;
    // `portable-pty`'s own descriptors close here; this process keeps its duplicates.
    drop(pair);
    Ok((master, slave, tty))
}

fn spawn_child(
    spec: &SpawnSpec,
    program: &std::path::Path,
    env: &[(OsString, OsString)],
    slave: &std::os::fd::OwnedFd,
) -> Result<std::process::Child, PtyError> {
    let stdio = || slave.try_clone().map(std::process::Stdio::from).map_err(io_error("spawn"));
    let mut command = std::process::Command::new(program);
    command
        .arg0(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .env_clear()
        .envs(env.iter().map(|(name, value)| (name.as_os_str(), value.as_os_str())))
        .stdin(stdio()?)
        .stdout(stdio()?)
        .stderr(stdio()?);
    sys::controlling_terminal(&mut command);
    command.spawn().map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied => {
            PtyError::ProgramNotFound
        }
        _ => PtyError::io("spawn", error),
    })
}

/// Locks without panicking: a poisoned lock only means another thread panicked while holding it,
/// and the guarded process stays valid.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

struct UnixReader(Arc<Terminal>);

impl PtyReader for UnixReader {
    fn readiness(&self) -> Readiness {
        // The descriptor lives in the shared terminal, which this reader keeps alive.
        Readiness::Fd(self.0.raw_master())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        // With `Wait::Forever` the terminal always returns a count.
        Ok(self.0.read(buf, Wait::Forever)?.unwrap_or(0))
    }

    fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, PtyError> {
        self.0.read(buf, Wait::Never)
    }
}

struct UnixWriter(Arc<Terminal>);

impl PtyWriter for UnixWriter {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        self.0.write_all(bytes)
    }
}

struct UnixResize {
    terminal: Arc<Terminal>,
    size: Mutex<PtySize>,
}

impl PtyResize for UnixResize {
    fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        if self.terminal.is_closed() {
            return Err(PtyError::Closed);
        }
        let winsize = rustix::termios::Winsize {
            ws_row: size.rows.get(),
            ws_col: size.cols.get(),
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        rustix::termios::tcsetwinsize(self.terminal.master_fd(), winsize)
            .map_err(|errno| PtyError::io("resize", errno.into()))?;
        *lock(&self.size) = size;
        Ok(())
    }

    fn size(&self) -> PtySize {
        *lock(&self.size)
    }
}

/// The child and whether this crate ended it.
struct Process {
    child: std::process::Child,
    killed: bool,
}

type SharedProcess = Arc<Mutex<Process>>;

struct UnixChild {
    process: SharedProcess,
    terminal: Arc<Terminal>,
    tty: Tty,
    pid: u32,
}

impl PtyChild for UnixChild {
    fn pid(&self) -> Option<u32> {
        Some(self.pid)
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, PtyError> {
        let mut process = lock(&self.process);
        let status = process.child.try_wait().map_err(io_error("wait"))?;
        Ok(status.map(|status| match status.code() {
            Some(code) if !process.killed && status.signal().is_none() => {
                ExitStatus::Exited(code.cast_unsigned())
            }
            _ => ExitStatus::Killed,
        }))
    }

    fn exit_source(&self) -> ExitSource {
        ExitSource::Pid(self.pid)
    }

    fn kill(&mut self) -> Result<(), PtyError> {
        kill(&self.process)
    }

    fn killer(&self) -> Box<dyn PtyKiller> {
        Box::new(UnixKiller(Arc::clone(&self.process)))
    }

    fn close(self: Box<Self>) -> Result<(), PtyError> {
        // Listed before the child dies (its exit detaches the terminal from the session), and
        // while the parent holds its slave descriptor, so no other session can reuse the terminal.
        let attached = self.terminal.with_slave(|_| self.tty.attached_processes());
        let killed = kill(&self.process);
        processes::kill_all(&attached.unwrap_or_default());
        self.terminal.close();
        killed
    }
}

struct UnixKiller(SharedProcess);

impl PtyKiller for UnixKiller {
    fn kill(&self) -> Result<(), PtyError> {
        kill(&self.0)
    }
}

/// `SIGKILL` to the child unless it already exited. The lock is held only for these two system
/// calls, so a reaped (and possibly reused) pid is never signalled.
fn kill(process: &Mutex<Process>) -> Result<(), PtyError> {
    let mut process = lock(process);
    if process.child.try_wait().map_err(io_error("wait"))?.is_some() {
        return Ok(());
    }
    process.child.kill().map_err(io_error("kill"))?;
    process.killed = true;
    Ok(())
}
