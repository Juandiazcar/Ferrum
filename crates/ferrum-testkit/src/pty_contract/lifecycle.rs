//! Spawn, environment, exit, kill and close: PT-01 to PT-12 and PT-22.

use std::ffi::OsString;
use std::time::{Duration, Instant};

use ferrum_pty::{EnvOp, ExitStatus, PtyError};

use super::{Capture, DEFAULT_COLS, DEFAULT_ROWS, Harness, contains, size};

const SECOND: Duration = Duration::from_secs(1);
/// PT-03: wide enough that the reported path never wraps.
const WIDE_COLS: u16 = 1000;
/// PT-22: a 1 MiB environment value.
const HUGE_ENV_BYTES: usize = 1024 * 1024;
/// PT-22: a 32 KiB argument.
const LONG_ARG_BYTES: usize = 32 * 1024;
/// Generous bound for "the helper starts and prints": far above the spike's slowest (2.3 s).
const START: Duration = Duration::from_secs(10);

/// PT-01: a typed line is echoed and the command output arrives within 5 s.
pub fn spawn_echo_roundtrip(h: &Harness<'_>) {
    let mut session = h.spawn(&["repl"]);
    let mut out = Capture::start(session.reader);
    session.writer.write_all(b"hello-ferrum\r").unwrap();
    assert!(out.wait_for(b"OUT:hello-ferrum", 5 * SECOND), "no command output");
    let echoed = out.data.windows(12).filter(|window| window == b"hello-ferrum").count();
    assert!(echoed >= 2, "the typed line was not echoed");
    session.child.close().unwrap();
}

/// PT-02: missing or empty program, NUL in an argument, and an invalid cwd are errors, with the
/// same variant on every OS.
pub fn spawn_missing_program_is_error(h: &Harness<'_>) {
    let mut missing = h.spec(&[]);
    missing.program = "ferrum-no-such-program-pt02".into();
    let mut empty = h.spec(&[]);
    empty.program = OsString::new();
    let mut nul = h.spec(&["repl"]);
    nul.args.push("bad\0arg".into());
    let mut cwd = h.spec(&["repl"]);
    cwd.cwd = h.scratch.join("pt02-does-not-exist");
    let variant = |spec| match h.backend.spawn(&spec) {
        Ok(_) => "spawned",
        Err(PtyError::ProgramNotFound) => "ProgramNotFound",
        Err(PtyError::InvalidSpec(_)) => "InvalidSpec",
        Err(PtyError::InvalidCwd) => "InvalidCwd",
        Err(_) => "other error",
    };
    assert_eq!(variant(missing), "ProgramNotFound");
    assert_eq!(variant(empty), "InvalidSpec");
    assert_eq!(variant(nul), "InvalidSpec");
    assert_eq!(variant(cwd), "InvalidCwd");
}

/// PT-03: the child runs in the requested cwd (a name with a space and non-ASCII characters).
pub fn spawn_applies_cwd(h: &Harness<'_>) {
    let dir = h.scratch.join("pt03 dir é");
    std::fs::create_dir_all(&dir).unwrap();
    let mut spec = h.spec(&["cwd"]);
    spec.cwd.clone_from(&dir);
    spec.size = size(WIDE_COLS, DEFAULT_ROWS);
    let session = h.start(&spec);
    let mut out = Capture::start(session.reader);
    assert!(out.wait_until(START, |output| line_after(output, b"CWD:").is_some()), "no cwd line");
    let reported = line_after(&out.data, b"CWD:").unwrap();
    let reported = std::fs::canonicalize(String::from_utf8_lossy(&reported).as_ref()).unwrap();
    assert_eq!(reported, std::fs::canonicalize(&dir).unwrap());
    session.child.close().unwrap();
}

/// The text after `prefix` up to the end of its line, once the line is complete.
fn line_after(output: &[u8], prefix: &[u8]) -> Option<Vec<u8>> {
    let start = output.windows(prefix.len()).position(|window| window == prefix)? + prefix.len();
    let rest = &output[start..];
    let end = rest.iter().position(|&byte| byte == b'\r' || byte == b'\n')?;
    Some(rest[..end].to_vec())
}

/// PT-04: set variables (one non-ASCII) arrive; removed `TERM` and `PATH` are absent although the
/// parent may have `TERM` and always has `PATH` (the case-insensitive `Path` on Windows).
pub fn spawn_env_set_and_remove(h: &Harness<'_>) {
    assert!(std::env::var_os("PATH").is_some(), "the test process must have PATH");
    let names = ["COLORTERM", "TERM_PROGRAM", "FERRUM_ID", "FERRUM_UNICODE", "TERM", "PATH"];
    let mut spec = h.spec(&[&["env"], &names[..]].concat());
    let set = |name: &str, value: &str| (OsString::from(name), EnvOp::Set(value.into()));
    spec.env = vec![
        set("COLORTERM", "truecolor"),
        set("TERM_PROGRAM", "Ferrum"),
        set("FERRUM_ID", "pt04"),
        set("FERRUM_UNICODE", "héllo-日本"),
        ("TERM".into(), EnvOp::Remove),
        ("PATH".into(), EnvOp::Remove),
    ];
    let session = h.start(&spec);
    let mut out = Capture::start(session.reader);
    assert!(out.wait_for(b"END", START), "no env report");
    for expected in [
        "COLORTERM=truecolor",
        "TERM_PROGRAM=Ferrum",
        "FERRUM_ID=pt04",
        "FERRUM_UNICODE=héllo-日本",
        "TERM!",
        "PATH!",
    ] {
        assert!(contains(&out.data, expected.as_bytes()), "missing {expected}");
    }
    session.child.close().unwrap();
}

fn exit_status_of(h: &Harness<'_>, code: &str) -> ExitStatus {
    let mut session = h.spawn(&["exit", code]);
    let mut out = Capture::start(session.reader);
    let status = out.wait_exit(session.child.as_mut(), START).expect("the child must exit");
    session.child.close().unwrap();
    status
}

/// PT-05: exit codes 0, 3 and 255 are reported exactly.
pub fn exit_code_zero_nonzero_and_255(h: &Harness<'_>) {
    for code in [0, 3, 255] {
        assert_eq!(exit_status_of(h, &code.to_string()), ExitStatus::Exited(code));
    }
}

/// PT-06: a child exiting with 259 is reported as exited (Windows: 259; Unix: `259 & 255`).
pub fn exit_code_259_is_reported(h: &Harness<'_>) {
    // Unix keeps the low 8 bits: 259 % 256 = 3.
    let expected = if cfg!(windows) { 259 } else { 259 % 256 };
    assert_eq!(exit_status_of(h, "259"), ExitStatus::Exited(expected));
}

/// PT-07: the exit is observable within 1 s while the reader is still open.
pub fn exit_detected_without_reader_eof(h: &Harness<'_>) {
    let mut session = h.spawn(&["exit", "0"]);
    let mut out = Capture::start(session.reader);
    assert!(out.wait_exit(session.child.as_mut(), SECOND).is_some(), "exit not seen within 1 s");
    session.child.close().unwrap();
}

/// PT-08: after `close`, `read` returns `Ok(0)` within 2 s.
pub fn reader_ends_after_close(h: &Harness<'_>) {
    let session = h.spawn(&["sleep"]);
    let mut out = Capture::start(session.reader);
    session.child.close().unwrap();
    assert_eq!(out.wait_end(2 * SECOND), Some(Ok(())), "the reader did not end within 2 s");
}

/// PT-09: 30 runs of "print a line and exit": the line always arrives, with the reader drained
/// before `close` and with `close` called right after the exit.
pub fn final_output_survives_fast_exit(h: &Harness<'_>) {
    for _ in 0..30 {
        let session = h.spawn(&["lines", "1"]);
        let mut out = Capture::start(session.reader);
        assert!(out.wait_for(b"END", START), "line lost (drained before close)");
        session.child.close().unwrap();

        let mut session = h.spawn(&["lines", "1"]);
        let mut out = Capture::start(session.reader);
        out.wait_exit(session.child.as_mut(), START).expect("the child must exit");
        session.child.close().unwrap();
        assert!(out.wait_end(START).is_some(), "the reader did not end after close");
        assert!(contains(&out.data, b"END"), "line lost (close right after exit)");
    }
}

/// PT-10: `kill` is truthful and idempotent, directly and through a cloned killer; a killed child
/// reports [`ExitStatus::Killed`] on every OS.
pub fn kill_is_truthful_and_idempotent(h: &Harness<'_>) {
    let mut session = h.spawn(&["sleep"]);
    let mut out = Capture::start(session.reader);
    session.child.kill().unwrap();
    let status = out.wait_exit(session.child.as_mut(), SECOND);
    assert_eq!(status, Some(ExitStatus::Killed), "kill: no exit in 1 s");
    session.child.kill().unwrap();
    session.child.close().unwrap();

    let mut session = h.spawn(&["sleep"]);
    let mut out = Capture::start(session.reader);
    let killer = session.child.killer();
    killer.kill().unwrap();
    let status = out.wait_exit(session.child.as_mut(), SECOND);
    assert_eq!(status, Some(ExitStatus::Killed), "killer: no exit in 1 s");
    killer.kill().unwrap();
    session.child.kill().unwrap();
    session.child.close().unwrap();
    killer.kill().unwrap();
}

/// PT-11: a grandchild is gone within 2 s after `close` (its file lock is released), whether it
/// shares the child's process group or runs in a group of its own.
pub fn close_ends_descendants(h: &Harness<'_>) {
    for (command, lock) in [("spawn-lock", "pt11.lock"), ("spawn-lock-group", "pt11-group.lock")] {
        grandchild_ends_at_close(h, command, &h.scratch.join(lock));
    }
}

fn grandchild_ends_at_close(h: &Harness<'_>, command: &str, path: &std::path::Path) {
    let path = path.to_owned();
    // A fresh file: a lock left by a process of an earlier failed run must not interfere.
    let _absent = std::fs::remove_file(&path);
    let session = h.spawn(&[command, path.to_str().unwrap()]);
    let mut out = Capture::start(session.reader);
    assert!(out.wait_for(b"LOCKED", START), "the grandchild did not take the lock");
    session.child.close().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        // Blocks until the grandchild, the lock holder, is gone.
        let _ignored = tx.send(file.lock());
    });
    let released = rx.recv_timeout(2 * SECOND).expect(command);
    released.unwrap();
}

/// PT-12: `close` returns within 1 s while the child floods and nobody reads.
pub fn close_does_not_hang_with_unread_output(h: &Harness<'_>) {
    let mut session = h.spawn(&["flood", "0"]);
    let mut seen = Vec::new();
    let mut buf = vec![0; 4096];
    while !contains(&seen, b"F00000001") {
        let read = session.reader.read(&mut buf).unwrap();
        assert!(read > 0, "the flood ended early");
        seen.extend_from_slice(&buf[..read]);
    }
    // Nobody reads from here on, so the child blocks on a full pipe.
    let started = Instant::now();
    session.child.close().unwrap();
    assert!(started.elapsed() < SECOND, "close took {:?}", started.elapsed());
}

/// PT-22: hostile input gives errors, never a panic: a huge environment, a 32 KiB argument, I/O
/// after `close`, the largest size, and kills after `close`.
pub fn no_panic_on_hostile_input(h: &Harness<'_>) {
    let mut huge_env = h.spec(&["exit", "0"]);
    huge_env.env.push(("FERRUM_HUGE".into(), EnvOp::Set("a".repeat(HUGE_ENV_BYTES).into())));
    let mut long_arg = h.spec(&["exit", "0"]);
    long_arg.args.push("b".repeat(LONG_ARG_BYTES).into());
    for spec in [huge_env, long_arg] {
        if let Ok(session) = h.backend.spawn(&spec) {
            session.child.close().unwrap();
        }
    }
    let mut session = h.spawn(&["repl"]);
    let _largest = session.resize.resize(size(u16::MAX, u16::MAX));
    let killer = session.child.killer();
    session.child.close().unwrap();
    let after_close = session.resize.resize(size(DEFAULT_COLS, DEFAULT_ROWS));
    assert!(matches!(after_close, Err(PtyError::Closed)));
    let write = session.writer.write_all(b"after close\r");
    assert!(matches!(write, Err(PtyError::Closed)), "write after close must fail: {write:?}");
    let mut buf = [0; 64];
    let _read = session.reader.read(&mut buf);
    killer.kill().unwrap();
}
