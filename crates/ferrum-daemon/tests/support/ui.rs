//! A UI process for the tests: the test executable started with [`UI_VAR`] connects to the
//! daemon, does one script and prints its results, so "closing the UI" and "`kill -9` of the
//! UI" (AC-SESS-01/02) are real process exits, with every handle closed by the OS.
//!
//! | Script | Does | Prints |
//! |---|---|---|
//! | `setup <n>` | a project with `n` sessions, each fed `lines <300 + 100 i>` | `SESSION <id> <hash> <lines>` per session, then `SETUP-DONE` |
//! | `setup-wait <n>` | the same, then waits forever (to be killed) | the same |
//! | `check <id>...` | checks the state after `Welcome` lists each session, attaches it and types `ping` | `LISTED <id>`, `SESSION <id> <hash> <lines>` and `ALIVE <id>` per session, then `CHECK-DONE` |
//! | `close <id>...` | closes each session | nothing |

#![allow(clippy::unwrap_used, clippy::panic, dead_code, unreachable_pub, reason = "test support")]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::thread::JoinHandle;
use std::time::Instant;

use ferrum_proto::{ClientMessage, DaemonMessage, SessionEvent, SessionId};

use super::harness::{Client, DEADLINE, Guard, HELPER_TEST, Home};

/// Environment variable carrying `<run dir>\n<data dir>\n<script>` to a UI process.
pub const UI_VAR: &str = "FERRUM_DAEMON_TEST_UI";
/// History rows asked for at attach: more than the sessions produce.
pub const HISTORY: u16 = 2_000;

/// Lines of a UI process's output kept for the test.
const LINE_QUEUE: usize = 1_024;

/// What a UI process reported about one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub id: u64,
    pub hash: u64,
    pub lines: usize,
}

/// What a `check` reported.
#[derive(Debug, Default)]
pub struct Report {
    pub seen: Vec<Seen>,
    pub alive: Vec<u64>,
    pub listed: Vec<u64>,
}

/// The full text of `session` as a freshly attached client sees it.
pub fn attached_text(client: &mut Client, session: SessionId, marker: &str) -> String {
    client.mirrors.remove(&session);
    client.attach(session, HISTORY);
    client.wait_text(session, marker, |text| text.contains(marker));
    client.pump_until("the history rows", |c| c.mirrors[&session].is_complete());
    let text = client.mirrors[&session].text();
    client.send(&ClientMessage::Detach { session_id: session });
    text
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn report(out: &mut impl Write, session: SessionId, text: &str) {
    writeln!(out, "SESSION {} {} {}", session.0, fnv(text), text.lines().count()).unwrap();
}

/// Runs the UI script when this process is a test UI; returns otherwise.
pub fn run_if_ui() {
    let Some(value) = std::env::var_os(UI_VAR) else { return };
    let value = value.to_string_lossy().into_owned();
    let mut parts = value.split('\n');
    let run = PathBuf::from(parts.next().unwrap());
    let data_dir = PathBuf::from(parts.next().unwrap());
    let words: Vec<&str> = parts.next().unwrap().split(' ').collect();
    let mut client = Home::connect_to(&run);
    let mut out = std::io::stdout().lock();
    // libtest printed `test helper_child ... ` without a newline: start on a line of our own.
    writeln!(out).unwrap();
    match words.as_slice() {
        ["setup", count] => setup(&mut client, &data_dir, count.parse().unwrap(), &mut out),
        ["setup-wait", count] => {
            setup(&mut client, &data_dir, count.parse().unwrap(), &mut out);
            out.flush().unwrap();
            loop {
                std::thread::park(); // until killed
            }
        }
        ["check", ids @ ..] => check(&mut client, ids, &mut out),
        ["close", ids @ ..] => close(&mut client, ids),
        ["layout"] => super::ui_layout::layout(&mut client, &data_dir, &mut out),
        ["inspect"] => super::ui_layout::inspect(&mut client, &data_dir, &mut out),
        _ => panic!("unknown UI script"),
    }
    out.flush().unwrap();
    std::process::exit(0);
}

fn setup(client: &mut Client, data_dir: &Path, count: usize, out: &mut impl Write) {
    let project = client.create_project(data_dir);
    for index in 0..count {
        let session = client.create_session(project);
        client.attach(session, HISTORY);
        client.wait_text(session, "READY", |text| text.contains("READY"));
        let lines = 300 + index * 100;
        client.type_line(session, &format!("lines {lines}"));
        let marker = format!("DONE{lines}");
        client.wait_text(session, &marker, |text| text.contains(&marker));
        // A round trip after the output: ConPTY renders the console at its own pace, so the snapshot
        // is taken only once everything printed before `sync` has arrived.
        client.type_line(session, "sync");
        client.wait_text(session, "sync", |text| text.contains("OUT:sync"));
        let text = attached_text(client, session, &marker);
        report(out, session, &text);
    }
    writeln!(out, "SETUP-DONE").unwrap();
}

fn check(client: &mut Client, ids: &[&str], out: &mut impl Write) {
    for id in ids {
        let session = SessionId(id.parse().unwrap());
        let listed = client.state.iter().any(|m| {
            matches!(m, DaemonMessage::SessionEvent { session_id, event: SessionEvent::Created(_) } if *session_id == session)
        });
        if listed {
            writeln!(out, "LISTED {}", session.0).unwrap();
        }
        let text = attached_text(client, session, "DONE");
        report(out, session, &text);
        client.attach(session, 0);
        client.type_line(session, "ping");
        client.wait_text(session, "pong", |text| text.contains("OUT:ping"));
        writeln!(out, "ALIVE {}", session.0).unwrap();
    }
    writeln!(out, "CHECK-DONE").unwrap();
}

fn close(client: &mut Client, ids: &[&str]) {
    for id in ids {
        let session = SessionId(id.parse().unwrap());
        client.send(&ClientMessage::CloseSession { session_id: session });
        client.wait_for("closed", |m| {
            matches!(m, DaemonMessage::SessionEvent { session_id, event: SessionEvent::Closed } if *session_id == session)
        });
    }
}

/// A UI process and the named thread that reads its output.
pub struct Ui {
    child: Guard,
    lines: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}

impl std::ops::Deref for Ui {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child.0
    }
}

impl std::ops::DerefMut for Ui {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child.0
    }
}

impl Drop for Ui {
    /// Ends the process if it still runs, then joins the reader: it ends with the process's
    /// output, which the test waits for with a deadline.
    fn drop(&mut self) {
        if matches!(self.child.0.try_wait(), Ok(None)) {
            let _killed = self.child.0.kill();
            let _reaped = self.child.0.wait();
        }
        if std::thread::panicking() {
            return; // the test already failed; a second panic would abort the run
        }
        let start = Instant::now();
        loop {
            let left = DEADLINE.checked_sub(start.elapsed()).unwrap_or_default();
            match self.lines.recv_timeout(left) {
                Ok(_) => {}
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => panic!("the UI output reader never ended"),
            }
        }
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
    }
}

/// Starts a UI process running `script` against `home`.
pub fn start(home: &Home, script: &str) -> Ui {
    let value = format!("{}\n{}\n{script}", home.run_dir().display(), home.dir("data").display());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", HELPER_TEST, "--nocapture", "--ignored", "--test-threads", "1"])
        .env(UI_VAR, value)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, lines) = sync_channel(LINE_QUEUE);
    let reader = std::thread::Builder::new().name("test-ui-stdout".into()).spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { return };
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    Ui { child: Guard(child), lines, reader: Some(reader.unwrap()) }
}

/// Every line a UI process printed, up to `until`, within the deadline.
pub fn read_lines(ui: &mut Ui, until: &str) -> Vec<String> {
    let start = Instant::now();
    let mut read = Vec::new();
    loop {
        let left = DEADLINE.checked_sub(start.elapsed()).unwrap_or_default();
        let line = ui.lines.recv_timeout(left).unwrap_or_else(|_| {
            panic!("the UI process did not print {until} in time; it printed {read:?}")
        });
        let is_end = line == until;
        read.push(line);
        if is_end {
            return read;
        }
    }
}

/// Reads a UI process's report up to the line `until`.
pub fn read_report(ui: &mut Ui, until: &str) -> Report {
    let mut report = Report::default();
    for line in read_lines(ui, until) {
        let words: Vec<&str> = line.split(' ').collect();
        match words.as_slice() {
            ["SESSION", id, hash, lines] => report.seen.push(Seen {
                id: id.parse().unwrap(),
                hash: hash.parse().unwrap(),
                lines: lines.parse().unwrap(),
            }),
            ["ALIVE", id] => report.alive.push(id.parse().unwrap()),
            ["LISTED", id] => report.listed.push(id.parse().unwrap()),
            _ => {}
        }
    }
    report
}
