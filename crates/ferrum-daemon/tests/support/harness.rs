//! Integration harness: a real `ferrumd` process in private directories, a real client over the
//! IPC endpoint, and the test executable itself as the session child (`helper.rs`), so nothing
//! depends on an installed shell (`docs/08` §7). Every wait has a deadline and an explicit
//! failure message; nothing sleeps to synchronize.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    dead_code,
    unreachable_pub,
    reason = "test support: a failure is a failed test; each test file uses a subset"
)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ferrum_config::{Profile, Settings};
use ferrum_proto::{
    ClientKind, ClientMessage, DaemonMessage, Hello, List, Nonce, PROTOCOL_VERSION, ProjectId,
    RowBlock, RowRegion, SessionId, Text, Viewport, decode_message, encode_frame, split_frame,
};
use ferrum_testkit::temp::TempDir;

/// Deadline of every wait.
pub const DEADLINE: Duration = Duration::from_secs(30);
/// Name of the libtest function that runs the helper child.
pub const HELPER_TEST: &str = "helper_child";
/// Environment variable that turns the test executable into the helper child.
pub const HELPER_VAR: &str = "FERRUM_DAEMON_TEST_HELPER";
/// The idle-exit grace of the test daemons (`performance.daemon_exit_grace_secs`).
pub const GRACE_SECS: u16 = 1;
/// Messages the reader thread keeps for the test; it waits when the test is behind.
const MESSAGE_QUEUE: usize = 4_096;

/// Private directories of one daemon.
pub struct Home {
    pub root: TempDir,
}

impl Home {
    pub fn new(label: &str) -> Self {
        let root = TempDir::new(label).unwrap();
        for dir in ["config", "data", "state"] {
            std::fs::create_dir(root.join(dir)).unwrap();
        }
        ferrum_platform::prepare_private_dir(&root.join("run")).unwrap();
        let home = Self { root };
        home.write_settings(|_| {});
        home
    }

    pub fn dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn run_dir(&self) -> PathBuf {
        self.dir("run")
    }

    /// Settings whose default profile is the helper child; `edit` changes them further.
    pub fn write_settings(&self, edit: impl FnOnce(&mut Settings)) {
        let mut settings = Settings::default();
        let exe = std::env::current_exe().unwrap();
        let mut profile = Profile::new("helper", exe.to_string_lossy());
        profile.args = ["--exact", HELPER_TEST, "--nocapture", "--ignored", "--test-threads", "1"]
            .map(String::from)
            .to_vec();
        profile.env.insert(HELPER_VAR.into(), "1".into());
        profile.shell_integration = false;
        settings.shells.profiles = vec![profile];
        settings.shells.default_profile = Some("helper".into());
        settings.performance.daemon_exit_grace_secs =
            ferrum_config::DaemonExitGraceSecs::new(GRACE_SECS).unwrap();
        edit(&mut settings);
        settings.save(&self.dir("config").join(ferrum_config::SETTINGS_FILE)).unwrap();
    }

    /// Starts `ferrumd` on these directories; it is killed when the guard drops, so a failed
    /// test leaves no daemon (nor its sessions) running.
    pub fn start_daemon(&self) -> Guard {
        Guard(self.spawn_daemon())
    }

    fn spawn_daemon(&self) -> Child {
        // `FERRUM_TEST_FERRUMD` runs the tests against another build (`xtask` uses the release one).
        let daemon = std::env::var_os("FERRUM_TEST_FERRUMD")
            .map_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ferrumd")), PathBuf::from);
        Command::new(daemon)
            .env(ferrum_platform::CONFIG_DIR_VAR, self.dir("config"))
            .env(ferrum_platform::DATA_DIR_VAR, self.dir("data"))
            .env(ferrum_platform::STATE_DIR_VAR, self.dir("state"))
            .env(ferrum_platform::RUNTIME_DIR_VAR, self.run_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    /// Connects and completes the handshake, retrying while the daemon starts.
    pub fn connect(&self) -> Client {
        Self::connect_to(&self.run_dir())
    }

    /// Connects to the daemon of `run_dir`.
    pub fn connect_to(run_dir: &Path) -> Client {
        let start = Instant::now();
        loop {
            if let Ok(nonce) = ferrum_platform::instance::read_nonce(run_dir)
                && let Ok(stream) = ferrum_platform::ipc::connect(run_dir)
            {
                return Client::handshake(stream, nonce);
            }
            assert!(start.elapsed() < DEADLINE, "the daemon never accepted a connection");
            std::thread::yield_now();
        }
    }
}

/// A child process killed when dropped.
pub struct Guard(pub Child);

impl std::ops::Deref for Guard {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

impl std::ops::DerefMut for Guard {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _killed = self.0.kill();
            let _reaped = self.0.wait();
        }
    }
}

/// Waits for a child process to end, up to `deadline`; `None` when it is still running.
pub fn wait_exit_status(child: &mut Child, deadline: Duration) -> Option<ExitStatus> {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::yield_now();
    }
    None
}

/// Waits for a child process to end, up to `deadline`.
pub fn wait_exit(child: &mut Child, deadline: Duration) -> bool {
    wait_exit_status(child, deadline).is_some()
}

/// Waits for a child process that must end by itself, and asserts it succeeded.
pub fn wait_success(child: &mut Child, what: &str) {
    let status = wait_exit_status(child, DEADLINE).unwrap_or_else(|| panic!("{what} never ended"));
    assert!(status.success(), "{what} failed: {status}");
}

/// Stops a daemon (or any child) and reaps it.
pub fn kill(child: &mut Child) {
    child.kill().unwrap(); // SIGKILL on Unix, TerminateProcess on Windows
    assert!(wait_exit(child, DEADLINE), "a killed process did not end");
}

/// The text a client shows of one session: its screen rows and the history rows it received.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Mirror {
    pub screen: HashMap<u16, String>,
    pub history: HashMap<u16, String>,
    pub rows: u16,
    /// History rows the last snapshot announced.
    pub expected_history: u16,
}

impl Mirror {
    /// Whether every announced history row arrived.
    pub fn is_complete(&self) -> bool {
        self.history.len() >= usize::from(self.expected_history)
    }
}

impl Mirror {
    /// Every row, oldest history first, then the screen.
    pub fn text(&self) -> String {
        let mut lines = Vec::new();
        let mut history: Vec<_> = self.history.iter().collect();
        history.sort_by_key(|(y, _)| std::cmp::Reverse(**y));
        lines.extend(history.into_iter().map(|(_, text)| text.trim_end().to_owned()));
        for y in 0..self.rows {
            lines.push(self.screen.get(&y).map_or("", |t| t.trim_end()).to_owned());
        }
        lines.join("\n")
    }

    /// Moves the screen rows as `Damage.scroll_delta` says (positive: up); rows the scroll
    /// brings in are blank until the damage fills them.
    fn scroll(&mut self, delta: i32) {
        if delta == 0 {
            return;
        }
        let old = std::mem::take(&mut self.screen);
        for (y, text) in old {
            let moved = i32::from(y) - delta;
            if let Ok(moved) = u16::try_from(moved)
                && moved < self.rows
            {
                self.screen.insert(moved, text);
            }
        }
    }

    fn apply(target: &mut HashMap<u16, String>, block: &RowBlock) {
        for row in block.rows() {
            let (y, cells) = row.unwrap();
            let mut text = String::new();
            for cell in cells {
                let cell = cell.unwrap();
                while text.chars().count() < usize::from(cell.col) {
                    text.push(' ');
                }
                text.push_str(cell.text);
            }
            target.insert(y, text);
        }
    }
}

/// Reads frames from the daemon and hands the messages to the test, until either side ends.
fn read_messages(reader: &mut dyn Read, tx: &std::sync::mpsc::SyncSender<DaemonMessage>) {
    let mut buf = Vec::new();
    let mut chunk = vec![0; 65_536];
    while let Ok(read) = reader.read(&mut chunk) {
        if read == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..read]);
        while let Ok(Some(frame)) = split_frame(&buf) {
            let used = frame.consumed;
            let message: DaemonMessage = decode_message(frame.payload).unwrap();
            buf.drain(..used);
            if tx.send(message).is_err() {
                return;
            }
        }
    }
}

/// A connected, accepted client.
pub struct Client {
    writer: Box<dyn Write + Send>,
    messages: Receiver<DaemonMessage>,
    _reader: JoinHandle<()>,
    /// Every title event with its arrival time (`SessionEvent::Title`).
    pub titles: Vec<(SessionId, String, Instant)>,
    /// Frames (`Snapshot`, `Rows`, `Damage`) received per session.
    pub frames: HashMap<SessionId, usize>,
    pub mirrors: HashMap<SessionId, Mirror>,
    /// The state the daemon sent after `Welcome`, up to `Synced`.
    pub state: Vec<DaemonMessage>,
}

impl Client {
    fn handshake(stream: ferrum_platform::ipc::ClientStream, nonce: [u8; 32]) -> Self {
        let (tx, messages) = sync_channel(MESSAGE_QUEUE);
        let mut reader = stream.reader;
        let thread = std::thread::Builder::new()
            .name("test-client-reader".into())
            .spawn(move || read_messages(&mut *reader, &tx));
        let mut client = Self {
            writer: stream.writer,
            messages,
            _reader: thread.unwrap(),
            titles: Vec::new(),
            frames: HashMap::new(),
            mirrors: HashMap::new(),
            state: Vec::new(),
        };
        let hello = Hello {
            protocol_version: PROTOCOL_VERSION,
            client_kind: ClientKind::Ui,
            nonce: Nonce(nonce),
        };
        client.send(&ClientMessage::Hello(hello));
        client.wait_for("welcome", |m| matches!(m, DaemonMessage::Welcome { .. }));
        let mut state = Vec::new();
        client.wait_for("synced", |m| {
            state.push(m.clone());
            matches!(m, DaemonMessage::Synced)
        });
        client.state = state;
        client
    }

    pub fn send(&mut self, message: &ClientMessage) {
        let mut frame = Vec::new();
        encode_frame(message, &mut frame).unwrap();
        self.writer.write_all(&frame).unwrap();
        self.writer.flush().unwrap();
    }

    /// Handles messages until `found` matches one (returned) or the deadline passes.
    pub fn wait_for(
        &mut self,
        what: &str,
        mut found: impl FnMut(&DaemonMessage) -> bool,
    ) -> DaemonMessage {
        let start = Instant::now();
        loop {
            let left = DEADLINE.checked_sub(start.elapsed()).unwrap_or_default();
            match self.messages.recv_timeout(left) {
                Ok(message) => {
                    self.track(&message);
                    if found(&message) {
                        return message;
                    }
                }
                Err(RecvTimeoutError::Timeout) => panic!("timed out waiting for {what}"),
                Err(RecvTimeoutError::Disconnected) => panic!("disconnected waiting for {what}"),
            }
        }
    }

    /// Waits until the mirror of `session` satisfies `done`.
    pub fn wait_text(&mut self, session: SessionId, what: &str, done: impl Fn(&str) -> bool) {
        self.wait_text_within(DEADLINE, session, what, done);
    }

    /// Like [`wait_text`](Self::wait_text), with its own deadline (a long output).
    pub fn wait_text_within(
        &mut self,
        deadline: Duration,
        session: SessionId,
        what: &str,
        done: impl Fn(&str) -> bool,
    ) {
        let start = Instant::now();
        while !self.mirrors.get(&session).is_some_and(|m| done(&m.text())) {
            let left = deadline.checked_sub(start.elapsed()).unwrap_or_default();
            let message = self
                .messages
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
            self.track(&message);
        }
    }

    /// Handles messages until `deadline`.
    pub fn drain_until(&mut self, deadline: Instant) {
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.messages.recv_timeout(left) {
                Ok(message) => self.track(&message),
                Err(RecvTimeoutError::Timeout) => return,
                Err(RecvTimeoutError::Disconnected) => panic!("the daemon disconnected"),
            }
        }
    }

    /// Handles one more message.
    pub fn pump_one(&mut self) {
        let message = self.messages.recv_timeout(DEADLINE).expect("no message arrived");
        self.track(&message);
    }

    /// The next message (tracked), waiting at most `left`; `None` when none arrived.
    pub fn next_message(&mut self, left: Duration) -> Option<DaemonMessage> {
        let message = self.messages.recv_timeout(left).ok()?;
        self.track(&message);
        Some(message)
    }

    /// Handles messages until `done` holds, within one overall deadline.
    pub fn pump_until(&mut self, what: &str, mut done: impl FnMut(&Self) -> bool) {
        let start = Instant::now();
        while !done(self) {
            let left = DEADLINE.checked_sub(start.elapsed()).unwrap_or_default();
            let message = self
                .messages
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
            self.track(&message);
        }
    }

    fn track(&mut self, message: &DaemonMessage) {
        self.count(message);
        match message {
            DaemonMessage::Snapshot(snapshot) => {
                let mirror = self.mirrors.entry(snapshot.session_id).or_default();
                mirror.screen.clear();
                mirror.history.clear();
                mirror.rows = snapshot.viewport.rows;
                mirror.expected_history = snapshot.history_rows;
                Mirror::apply(&mut mirror.screen, &snapshot.rows);
            }
            DaemonMessage::Rows { session_id, region, rows } => {
                let mirror = self.mirrors.entry(*session_id).or_default();
                let target = if *region == RowRegion::History {
                    &mut mirror.history
                } else {
                    &mut mirror.screen
                };
                Mirror::apply(target, rows);
            }
            DaemonMessage::Damage(damage) => {
                let mirror = self.mirrors.entry(damage.session_id).or_default();
                mirror.scroll(damage.scroll_delta);
                Mirror::apply(&mut mirror.screen, &damage.rows);
            }
            _ => {}
        }
    }

    /// Counts frames per session and records titles.
    fn count(&mut self, message: &DaemonMessage) {
        let framed = match message {
            DaemonMessage::Snapshot(snapshot) => Some(snapshot.session_id),
            DaemonMessage::Rows { session_id, .. } => Some(*session_id),
            DaemonMessage::Damage(damage) => Some(damage.session_id),
            _ => None,
        };
        if let Some(session) = framed {
            *self.frames.entry(session).or_default() += 1;
        }
        if let DaemonMessage::SessionEvent {
            session_id,
            event: ferrum_proto::SessionEvent::Title(title),
        } = message
        {
            self.titles.push((*session_id, title.as_str().to_owned(), Instant::now()));
        }
    }

    /// Creates a project over `root` and returns its id.
    pub fn create_project(&mut self, root: &Path) -> ProjectId {
        let roots = List::new(vec![Text::new(root.to_string_lossy())]).unwrap();
        self.send(&ClientMessage::CreateProject { name: Text::new("test"), roots });
        let created = self.wait_for("project created", |m| {
            matches!(m, DaemonMessage::ProjectEvent(ferrum_proto::ProjectEvent::Created(_)))
        });
        let DaemonMessage::ProjectEvent(ferrum_proto::ProjectEvent::Created(info)) = created else {
            unreachable!()
        };
        info.project_id
    }

    /// Creates a session in `project` and returns its id.
    pub fn create_session(&mut self, project: ProjectId) -> SessionId {
        self.send(&ClientMessage::CreateSession {
            project_id: project,
            kind: ferrum_proto::SessionKind::Terminal,
            root: 0,
        });
        let created = self.wait_for("session created", |m| {
            matches!(
                m,
                DaemonMessage::SessionEvent { event: ferrum_proto::SessionEvent::Created(_), .. }
            )
        });
        let DaemonMessage::SessionEvent { session_id, .. } = created else { unreachable!() };
        session_id
    }

    pub fn attach(&mut self, session: SessionId, history: u16) {
        self.attach_at(session, history, Viewport { cols: 80, rows: 24 });
    }

    pub fn attach_at(&mut self, session: SessionId, history: u16, viewport: Viewport) {
        self.send(&ClientMessage::Attach {
            session_id: session,
            viewport,
            want_scrollback_rows: history,
        });
    }

    pub fn type_line(&mut self, session: SessionId, line: &str) {
        let bytes = ferrum_proto::Bytes::new(format!("{line}\r").into_bytes());
        self.send(&ClientMessage::Input { session_id: session, bytes });
    }
}
