//! Plain data that crosses the [`PtyBackend`](crate::PtyBackend) boundary: what to spawn, the
//! terminal size, and how a child ended.

use std::ffi::OsString;
use std::num::NonZeroU16;
use std::path::PathBuf;

/// Terminal size in character cells. Both dimensions are at least one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PtySize {
    /// Number of columns.
    pub cols: NonZeroU16,
    /// Number of rows.
    pub rows: NonZeroU16,
}

/// One change applied over the environment the child inherits from the spawning process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvOp {
    /// Set the variable to this value.
    Set(OsString),
    /// Remove the variable, even when the spawning process has it.
    Remove,
}

/// Everything needed to start a child in a new pseudo terminal.
///
/// There is no default working directory: `cwd` is always explicit (ADR 0026, W8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnSpec {
    /// Program to run: a path, or a bare name searched in the child's `PATH`.
    pub program: OsString,
    /// Arguments, without the program name.
    pub args: Vec<OsString>,
    /// Working directory of the child; must be an existing directory.
    pub cwd: PathBuf,
    /// Environment changes applied in order over the inherited environment.
    pub env: Vec<(OsString, EnvOp)>,
    /// Initial terminal size.
    pub size: PtySize,
}

/// How a child ended (FR-SESS-006: exited with a code, or killed), the same on every OS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExitStatus {
    /// The child exited on its own with this code (on Unix `0..=255`; on Windows the full 32-bit
    /// value, 259 included).
    Exited(u32),
    /// The child was ended: by [`PtyChild::kill`](crate::PtyChild::kill),
    /// [`PtyChild::close`](crate::PtyChild::close), or (Unix) any signal.
    Killed,
}

/// How a caller learns that [`PtyReader::read`](crate::PtyReader::read) has data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Readiness {
    /// Nothing to poll or be notified by: `read` blocks. Only the in-memory fake of
    /// `ferrum-testkit` reports it; the OS backends never do.
    Blocking,
    /// A descriptor owned by the reader that a poller (`mio`) can watch, level-triggered: when it
    /// is readable, one `read` does not block. It is valid exactly as long as the reader exists.
    #[cfg(unix)]
    Fd(std::os::fd::RawFd),
    /// The backend's single I/O thread calls the function given to
    /// [`PtyReader::set_notify`](crate::PtyReader::set_notify) whenever output is ready; read it
    /// with [`PtyReader::try_read`](crate::PtyReader::try_read). No thread per session.
    Notify,
}

/// What a caller can wait on to learn that the child exited, besides polling
/// [`PtyChild::try_wait`](crate::PtyChild::try_wait).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExitSource {
    /// Nothing to wait on: poll `try_wait` (fake backend).
    Polling,
    /// Process id of the child (Unix: `SIGCHLD` or a pidfd tells when to call `try_wait`).
    Pid(u32),
    /// Process handle that is signaled when the child exits (Windows). Valid while the
    /// [`PtyChild`](crate::PtyChild) or one of its killers exists.
    #[cfg(windows)]
    Handle(std::os::windows::io::RawHandle),
}

impl SpawnSpec {
    /// Checks what every backend needs before touching the OS: a non-empty program, no NUL byte
    /// in the program, arguments or environment, variable names that are non-empty and contain no
    /// `=`, and a working directory that is an existing directory.
    ///
    /// # Errors
    /// [`PtyError::InvalidSpec`](crate::PtyError::InvalidSpec) or
    /// [`PtyError::InvalidCwd`](crate::PtyError::InvalidCwd).
    pub fn validate(&self) -> Result<(), crate::PtyError> {
        use crate::PtyError::{InvalidCwd, InvalidSpec};
        if self.program.is_empty() {
            return Err(InvalidSpec("empty program"));
        }
        if has_nul(&self.program) || self.args.iter().any(|arg| has_nul(arg)) {
            return Err(InvalidSpec("NUL byte in the program or an argument"));
        }
        for (name, op) in &self.env {
            if name.is_empty() || has_nul(name) || name.as_encoded_bytes().contains(&b'=') {
                return Err(InvalidSpec("invalid environment variable name"));
            }
            if matches!(op, EnvOp::Set(value) if has_nul(value)) {
                return Err(InvalidSpec("NUL byte in an environment value"));
            }
        }
        if !self.cwd.is_dir() {
            return Err(InvalidCwd);
        }
        Ok(())
    }
}

fn has_nul(text: &std::ffi::OsStr) -> bool {
    text.as_encoded_bytes().contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PtyError;

    fn spec() -> SpawnSpec {
        let one = NonZeroU16::MIN;
        SpawnSpec {
            program: "sh".into(),
            args: vec!["-c".into()],
            cwd: std::env::temp_dir(),
            env: vec![("A".into(), EnvOp::Set("1".into())), ("B".into(), EnvOp::Remove)],
            size: PtySize { cols: one, rows: one },
        }
    }

    #[test]
    fn accepts_a_well_formed_spec() {
        assert!(spec().validate().is_ok());
    }

    #[test]
    fn rejects_what_no_os_can_spawn() {
        let mut cases = [spec(), spec(), spec(), spec(), spec(), spec()];
        cases[0].program = OsString::new();
        cases[1].args.push("a\0b".into());
        cases[2].env.push(("A=B".into(), EnvOp::Remove));
        cases[3].env.push((OsString::new(), EnvOp::Remove));
        cases[4].env.push(("C".into(), EnvOp::Set("x\0".into())));
        cases[5].cwd = std::env::temp_dir().join("ferrum-pty-no-such-directory");
        for case in &cases[..5] {
            assert!(matches!(case.validate(), Err(PtyError::InvalidSpec(_))));
        }
        assert!(matches!(cases[5].validate(), Err(PtyError::InvalidCwd)));
    }
}
