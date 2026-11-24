//! A deterministic, in-memory [`PtyBackend`] for tests (`docs/05` §2: the fake runs the same
//! contract suite as the real backends; PT-25).
//!
//! The fake ignores `program` and interprets the first argument with the same meaning as the
//! `ferrum-pty-helper` subcommands it supports: `repl [n]`, `exit <code>`, `size` and `sleep`. There
//! is no thread and no timing: written bytes are echoed and answered before `write_all` returns, as
//! a terminal in canonical mode with echo would, and `kill`/`close` take effect immediately.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use ferrum_pty::{
    ExitSource, ExitStatus, PtyBackend, PtyChild, PtyError, PtyKiller, PtyReader, PtyResize,
    PtySession, PtySize, PtyWriter, Readiness, SpawnSpec,
};

/// The deterministic fake backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct FakeBackend;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Program {
    Repl,
    Size,
    Sleep,
}

struct State {
    program: Program,
    output: VecDeque<u8>,
    line: Vec<u8>,
    size: PtySize,
    exit: Option<ExitStatus>,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    readable: Condvar,
}

type Handle = Arc<Shared>;

impl PtyBackend for FakeBackend {
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtySession, PtyError> {
        spec.validate()?;
        let words: Vec<String> = spec.args.iter().map(|arg| arg.to_string_lossy().into()).collect();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let (program, exit, lines) = match words.as_slice() {
            ["repl"] => (Program::Repl, None, 0),
            ["repl", count] => (Program::Repl, None, parse(count)?),
            ["exit", code] => (Program::Sleep, Some(ExitStatus::Exited(parse(code)?)), 0),
            ["size"] => (Program::Size, None, 0),
            ["sleep"] => (Program::Sleep, None, 0),
            _ => return Err(PtyError::ProgramNotFound),
        };
        let mut state = State {
            program,
            output: VecDeque::new(),
            line: Vec::new(),
            size: spec.size,
            exit,
            closed: false,
        };
        state.start(lines);
        let shared: Handle =
            Arc::new(Shared { state: Mutex::new(state), readable: Condvar::new() });
        Ok(PtySession {
            reader: Box::new(FakeReader(Arc::clone(&shared))),
            writer: Box::new(FakeWriter(Arc::clone(&shared))),
            resize: Box::new(FakeResize(Arc::clone(&shared))),
            child: Box::new(FakeChild(shared)),
        })
    }
}

fn parse(text: &str) -> Result<u32, PtyError> {
    text.parse().map_err(|_| PtyError::InvalidSpec("fake: not a number"))
}

impl State {
    fn start(&mut self, lines: u32) {
        if self.program == Program::Repl {
            for index in 0..lines {
                self.say(&format!("LINE{index}"));
            }
            self.say("END");
        }
        if self.program == Program::Size {
            self.report_size();
        }
    }

    fn say(&mut self, line: &str) {
        self.output.extend(line.as_bytes());
        self.output.extend(b"\r\n");
    }

    fn report_size(&mut self) {
        let text = format!("SIZE {} {}", self.size.rows, self.size.cols);
        self.say(&text);
    }

    fn input(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\r' || byte == b'\n' {
                self.output.extend(b"\r\n");
                let line = String::from_utf8_lossy(&self.line).into_owned();
                self.line.clear();
                match self.program {
                    Program::Repl => self.say(&format!("OUT:{line}")),
                    Program::Size => self.report_size(),
                    Program::Sleep => {}
                }
            } else {
                self.output.push_back(byte);
                self.line.push(byte);
            }
        }
    }

    /// Copies pending output into `buf`: `Some(0)` once closed and drained, `None` if nothing yet.
    fn take_output(&mut self, buf: &mut [u8]) -> Option<usize> {
        if !self.output.is_empty() {
            let count = buf.len().min(self.output.len());
            for (slot, byte) in buf.iter_mut().zip(self.output.drain(..count)) {
                *slot = byte;
            }
            return Some(count);
        }
        self.closed.then_some(0)
    }

    fn kill(&mut self) {
        if self.exit.is_none() {
            self.exit = Some(ExitStatus::Killed);
        }
    }
}

fn lock(shared: &Shared) -> MutexGuard<'_, State> {
    shared.state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `action` on the live state, or fails with [`PtyError::Closed`].
fn with_state<T>(shared: &Shared, action: impl FnOnce(&mut State) -> T) -> Result<T, PtyError> {
    let mut state = lock(shared);
    if state.closed {
        return Err(PtyError::Closed);
    }
    Ok(action(&mut state))
}

struct FakeReader(Handle);

impl PtyReader for FakeReader {
    fn readiness(&self) -> Readiness {
        Readiness::Blocking
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        let mut state = lock(&self.0);
        loop {
            if let Some(read) = state.take_output(buf) {
                return Ok(read);
            }
            state = self.0.readable.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, PtyError> {
        Ok(lock(&self.0).take_output(buf))
    }
}

struct FakeWriter(Handle);

impl PtyWriter for FakeWriter {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        with_state(&self.0, |state| {
            if state.exit.is_none() {
                state.input(bytes);
            }
        })?;
        self.0.readable.notify_all();
        Ok(())
    }
}

struct FakeResize(Handle);

impl PtyResize for FakeResize {
    fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        with_state(&self.0, |state| state.size = size)
    }

    fn size(&self) -> PtySize {
        lock(&self.0).size
    }
}

struct FakeChild(Handle);

impl PtyChild for FakeChild {
    fn pid(&self) -> Option<u32> {
        None
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, PtyError> {
        Ok(lock(&self.0).exit)
    }

    fn exit_source(&self) -> ExitSource {
        ExitSource::Polling
    }

    fn kill(&mut self) -> Result<(), PtyError> {
        FakeKiller(Arc::clone(&self.0)).kill()
    }

    fn killer(&self) -> Box<dyn PtyKiller> {
        Box::new(FakeKiller(Arc::clone(&self.0)))
    }

    fn close(self: Box<Self>) -> Result<(), PtyError> {
        {
            let mut state = lock(&self.0);
            state.kill();
            state.closed = true;
        }
        self.0.readable.notify_all();
        Ok(())
    }
}

struct FakeKiller(Handle);

impl PtyKiller for FakeKiller {
    fn kill(&self) -> Result<(), PtyError> {
        lock(&self.0).kill();
        Ok(())
    }
}
