//! FR-SESS-005 with real shells: the injected bootstrap loads the user's startup file, reports the
//! working directory (OSC 7) and marks prompts and exit statuses (OSC 133). The shells run with a
//! private `HOME` under the target directory, never the real user's files.
//!
//! bash runs on Linux and macOS, zsh on macOS (installed there by default), Windows `PowerShell`
//! on Windows; fish is covered by `docs/manual-verification.md` (MV-05-3).

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ferrum_pty::{EnvOp, PtyError, PtyReader, PtySession, Readiness, SpawnSpec, shell_integration};
use ferrum_testkit::pty_contract::{Capture, NATIVE_STARTUP, size};

/// Cold shell start on a CI runner (Windows `PowerShell` took 2.3 s in spike S4).
const START: Duration = Duration::from_secs(30);

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shell-integration").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(dir.join("home").join("sub é")).unwrap();
    dir
}

/// Spawns `program` with integration from `dir/bootstrap` and `HOME` (and `USERPROFILE`) at
/// `dir/home`, after writing `rc.1` as the user's startup file `rc.0`.
#[cfg(unix)]
fn start(program: &str, dir: &Path, rc: (&str, &str)) -> (PtySession, Capture) {
    start_with(program, dir, rc, None)
}

/// [`start`] with one more environment variable.
fn start_with(
    program: &str,
    dir: &Path,
    rc: (&str, &str),
    extra: Option<(&str, &str)>,
) -> (PtySession, Capture) {
    let home = dir.join("home");
    std::fs::write(home.join(rc.0), rc.1).unwrap();
    let bootstrap = dir.join("bootstrap");
    shell_integration::write_bootstrap_files(&bootstrap).unwrap();
    let set = |name: &str, value: &Path| (OsString::from(name), EnvOp::Set(value.into()));
    let mut spec = SpawnSpec {
        program: program.into(),
        args: Vec::new(),
        cwd: home.clone(),
        env: vec![set("HOME", &home), set("USERPROFILE", &home), ("ZDOTDIR".into(), EnvOp::Remove)],
        size: size(120, 30),
    };
    if let Some((name, value)) = extra {
        spec.env.push((name.into(), EnvOp::Set(value.into())));
    }
    assert!(shell_integration::inject(&mut spec, &bootstrap).is_some());
    let mut session = ferrum_pty::native_backend().spawn(&spec).unwrap();
    // The test is the session layer: it answers the startup query of ConPTY (W1).
    ferrum_testkit::pty_contract::answer_startup(&mut session, NATIVE_STARTUP);
    let out = Capture::start(std::mem::replace(&mut session.reader, Box::new(Done)));
    (session, out)
}

/// Placeholder left in the session once its reader moved to the capture thread.
struct Done;

impl PtyReader for Done {
    fn readiness(&self) -> Readiness {
        Readiness::Blocking
    }

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, PtyError> {
        Ok(0)
    }

    fn try_read(&mut self, _buf: &mut [u8]) -> Result<Option<usize>, PtyError> {
        Ok(Some(0))
    }
}

/// Types `line`, then waits for `expected` in the output that follows.
fn run(session: &mut PtySession, out: &mut Capture, line: &str, expected: &[u8]) {
    let mark = out.data.len();
    session.writer.write_all(format!("{line}\r").as_bytes()).unwrap();
    let found = out.wait_for_after(mark, expected, START);
    let tail = String::from_utf8_lossy(out.data.get(mark..).unwrap_or_default()).into_owned();
    assert!(
        found,
        "{line:?} did not produce {:?}; got {tail:?}",
        String::from_utf8_lossy(expected)
    );
}

/// The checks shared by the Unix shells: the user's file was loaded, prompts are marked, changing
/// directory reports the percent-encoded path, and a failing command reports its status.
#[cfg(unix)]
fn check_shell(session: &mut PtySession, out: &mut Capture) {
    assert!(out.wait_for(b"\x1b]133;A", START), "no prompt mark");
    assert!(out.wait_for(b"\x1b]133;B", START), "no input mark");
    assert!(out.wait_for(b"\x1b]7;file://", START), "no working directory report");
    run(session, out, "echo rc-$FERRUM_TEST_RC", b"rc-loaded");
    run(session, out, "cd 'sub \u{e9}'", b"sub%20%C3%A9");
    run(session, out, "false", b"\x1b]133;D;1");
}

#[cfg(unix)]
#[test]
fn bash_reports_cwd_and_marks_prompts() {
    let dir = scratch("bash");
    let (mut session, mut out) = start("bash", &dir, (".bashrc", "FERRUM_TEST_RC=loaded\n"));
    check_shell(&mut session, &mut out);
    session.child.close().unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn zsh_reports_cwd_marks_prompts_and_restores_zdotdir() {
    let dir = scratch("zsh");
    let (mut session, mut out) = start("zsh", &dir, (".zshrc", "FERRUM_TEST_RC=loaded\n"));
    check_shell(&mut session, &mut out);
    let output_marked = ferrum_testkit::pty_contract::contains(&out.data, b"\x1b]133;C");
    assert!(output_marked, "no output mark");
    run(&mut session, &mut out, "echo zdotdir-${ZDOTDIR:-unset}", b"zdotdir-unset");
    // History stays in the user's directory (macOS may pick a per-session file under it), never
    // in the bootstrap directory.
    let expected = format!("histfile-{}/", dir.join("home").display());
    run(&mut session, &mut out, "echo histfile-$HISTFILE", expected.as_bytes());
    session.child.close().unwrap();
}

#[cfg(windows)]
#[test]
fn powershell_reports_cwd_and_marks_prompts() {
    let dir = scratch("powershell");
    // The default execution policy of desktop Windows: no script file may run. The integration
    // is passed inline, so it must work anyway.
    let restricted = ("PSExecutionPolicyPreference", "Restricted");
    let (mut session, mut out) =
        start_with("powershell", &dir, ("unused.txt", ""), Some(restricted));
    assert!(out.wait_for(b"\x1b]133;A", START), "no prompt mark");
    assert!(out.wait_for(b"\x1b]7;file:///", START), "no working directory report");
    run(&mut session, &mut out, "cd 'sub \u{e9}'", b"sub%20%C3%A9");
    run(&mut session, &mut out, "cmd /c exit 3", b"\x1b]133;D;3");
    session.child.close().unwrap();
}

#[cfg(unix)]
#[test]
fn bootstrap_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("private").join("bootstrap");
    std::fs::create_dir_all(dir.join("zsh")).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    // A file left by an earlier run with looser permissions is tightened too.
    std::fs::write(dir.join("ferrum.bash"), b"old").unwrap();
    std::fs::set_permissions(dir.join("ferrum.bash"), std::fs::Permissions::from_mode(0o644))
        .unwrap();
    shell_integration::write_bootstrap_files(&dir).unwrap();
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("zsh")), 0o700);
    for file in ["ferrum.bash", "ferrum.fish", "zsh/.zshenv"] {
        assert_eq!(mode(&dir.join(file)), 0o600, "{file}");
    }
}
