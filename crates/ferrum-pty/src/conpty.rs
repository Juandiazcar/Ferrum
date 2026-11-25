//! Windows backend: `ConPTY` called directly through `windows-sys` (ADR 0026, decision 1).
//!
//! The pseudoconsole is created with `PSEUDOCONSOLE_INHERIT_CURSOR`, the configuration measured by
//! spike S4 (`docs/11` §4): its first output is the cursor query `ESC[6n`, which withholds
//! everything else until the session layer answers it (W1). Its input and output are named
//! pipes whose ends in this process are overlapped and served, with every other session's, by the
//! backend's single I/O thread ([`driver`], maintainer decision Q-31; [`input`]): readers and
//! writers report [`Readiness::Notify`](crate::Readiness) and no thread runs per session. The child is started with
//! `CreateProcessW`, attached to the pseudoconsole, inheriting no handle, with an explicit
//! environment block and working directory (W8), and found by the one lookup rule of every
//! backend ([`program`](crate::program)); a batch file runs through the system `cmd.exe` with
//! its arguments escaped to reach it literally. Exit is read from the process handle, never from
//! the reader, so code 259 is an exit like any other (W2); `kill` is `TerminateProcess`; `close`
//! terminates the child and closes the pseudoconsole, which ends every attached process and lets
//! the reader reach the end of the output (W3). A write never blocks the I/O thread (W4).
//!
//! Every `unsafe` call lives in [`sys`].

mod driver;
mod input;
mod sys;

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use crate::program::{self, Search};
use crate::windows_command::{self, Variable};
use crate::{
    EnvOp, ExitSource, ExitStatus, PtyBackend, PtyChild, PtyError, PtyKiller, PtyResize,
    PtySession, PtySize, SpawnSpec,
};
use driver::Driver;
use input::ConptyWriter;
use sys::{Launch, Process, PseudoConsole, Terminated};

type SharedConsole = Arc<Mutex<Option<PseudoConsole>>>;

/// The Windows [`PtyBackend`] (`ConPTY`). Its I/O thread starts with the first session and
/// stops when the backend and every session are gone.
#[derive(Default)]
pub struct ConptyBackend {
    driver: OnceLock<Arc<Driver>>,
}

impl ConptyBackend {
    fn driver(&self) -> Result<Arc<Driver>, PtyError> {
        if let Some(driver) = self.driver.get() {
            return Ok(Arc::clone(driver));
        }
        let driver =
            Driver::start().map_err(|error| PtyError::io("start the I/O thread", error))?;
        // A concurrent first spawn may have won the race; its driver is used and ours stops.
        Ok(Arc::clone(self.driver.get_or_init(|| driver)))
    }
}

impl PtyBackend for ConptyBackend {
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtySession, PtyError> {
        spec.validate()?;
        let variables = environment(spec);
        let (application, mut command_line) = command(spec, &variables)?;
        let environment = windows_command::environment_block(variables);
        let (input, child_input) =
            sys::input_pipe().map_err(|error| PtyError::io("pipe", error))?;
        let (output, child_output) =
            sys::output_pipe().map_err(|error| PtyError::io("pipe", error))?;
        let driver = self.driver()?;
        let reader =
            driver.reader(output).map_err(|error| PtyError::io("register the output", error))?;
        let console = PseudoConsole::new(spec.size, &child_input, &child_output)
            .map_err(|error| PtyError::io("create the pseudoconsole", error))?;
        // The console holds its own duplicates; with ours closed, its exit ends the output.
        drop((child_input, child_output));
        let mut launch = Launch {
            application: &nul_terminated(application.as_os_str()),
            command_line: &mut command_line,
            environment: &environment,
            cwd: &nul_terminated(spec.cwd.as_os_str()),
        };
        let process = sys::spawn(&console, &mut launch)
            .map_err(|error| PtyError::io("create the process", error))?;
        let closed = Arc::new(AtomicBool::new(false));
        let writer = ConptyWriter::new(&driver, input, Arc::clone(&closed))
            .map_err(|error| PtyError::io("register the input", error))?;
        let process = Arc::new(Child { process, killed: AtomicBool::new(false) });
        let console: SharedConsole = Arc::new(Mutex::new(Some(console)));
        Ok(PtySession {
            reader: Box::new(reader),
            writer: Box::new(writer),
            resize: Box::new(ConptyResize {
                console: Arc::clone(&console),
                size: Mutex::new(spec.size),
            }),
            child: Box::new(ConptyChild { process, console, closed }),
        })
    }
}

/// The application `CreateProcessW` starts and its command line: the program itself, or the
/// system `cmd.exe` running a batch file.
fn command(spec: &SpawnSpec, variables: &[Variable]) -> Result<(PathBuf, Vec<u16>), PtyError> {
    let var = |name| windows_command::lookup(variables, name).map(OsString::from_wide);
    let search = Search::new(var("PATH"), var("PATHEXT").as_deref());
    let resolved = program::resolve(&spec.program, &spec.cwd, &search)?;
    let args: Vec<Vec<u16>> = spec.args.iter().map(|arg| wide(arg)).collect();
    if program::is_batch(&resolved) {
        let system =
            sys::system_directory().map_err(|error| PtyError::io("find cmd.exe", error))?;
        let line = windows_command::batch_command_line(&wide(resolved.as_os_str()), &args)?;
        return Ok((system.join("cmd.exe"), line));
    }
    Ok((resolved, windows_command::command_line(&wide(&spec.program), &args)?))
}

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().collect()
}

fn nul_terminated(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(Some(0)).collect()
}

/// The inherited environment with the spawn's changes applied.
fn environment(spec: &SpawnSpec) -> Vec<Variable> {
    let inherited = std::env::vars_os().map(|(name, value)| (wide(&name), wide(&value))).collect();
    let changes = spec.env.iter().map(|(name, op)| {
        let value = match op {
            EnvOp::Set(value) => Some(wide(value)),
            EnvOp::Remove => None,
        };
        (wide(name), value)
    });
    windows_command::apply_changes(inherited, changes)
}

/// Locks without panicking: a poisoned lock only means another thread panicked while holding it,
/// and the guarded handle stays valid.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

struct ConptyResize {
    console: SharedConsole,
    size: Mutex<PtySize>,
}

impl PtyResize for ConptyResize {
    fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        let console = lock(&self.console);
        let console = console.as_ref().ok_or(PtyError::Closed)?;
        console.resize(size).map_err(|error| PtyError::io("resize the pseudoconsole", error))?;
        *lock(&self.size) = size;
        Ok(())
    }

    fn size(&self) -> PtySize {
        *lock(&self.size)
    }
}

/// The process and whether this crate ended it ([`ExitStatus::Killed`]).
struct Child {
    process: Process,
    killed: AtomicBool,
}

impl Child {
    fn kill(&self) -> Result<(), PtyError> {
        match self.process.terminate().map_err(|error| PtyError::io("kill", error))? {
            Terminated::Now => self.killed.store(true, Ordering::Release),
            Terminated::AlreadyEnding => {}
        }
        Ok(())
    }
}

struct ConptyChild {
    process: Arc<Child>,
    console: SharedConsole,
    closed: Arc<AtomicBool>,
}

impl PtyChild for ConptyChild {
    fn pid(&self) -> Option<u32> {
        Some(self.process.process.pid())
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, PtyError> {
        let code = self.process.process.exit_code().map_err(|error| PtyError::io("wait", error))?;
        let killed = self.process.killed.load(Ordering::Acquire);
        Ok(code.map(|code| if killed { ExitStatus::Killed } else { ExitStatus::Exited(code) }))
    }

    fn exit_source(&self) -> ExitSource {
        ExitSource::Handle(self.process.process.raw_handle())
    }

    fn kill(&mut self) -> Result<(), PtyError> {
        self.process.kill()
    }

    fn killer(&self) -> Box<dyn PtyKiller> {
        Box::new(ConptyKiller(Arc::clone(&self.process)))
    }

    fn close(self: Box<Self>) -> Result<(), PtyError> {
        // Ending the child first does not depend on how far its startup got; closing the
        // pseudoconsole then ends every other attached process, descendants included (W3).
        let killed = self.process.kill();
        self.closed.store(true, Ordering::Release);
        drop(lock(&self.console).take());
        killed
    }
}

struct ConptyKiller(Arc<Child>);

impl PtyKiller for ConptyKiller {
    fn kill(&self) -> Result<(), PtyError> {
        self.0.kill()
    }
}
