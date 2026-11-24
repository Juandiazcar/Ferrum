//! The daemon's event loop (`docs/02` §5): one I/O thread polls the IPC endpoint, every
//! client, every PTY and the exit watcher with `mio`, and does all session work inline except
//! what goes to the worker. It sleeps without a timeout when nothing is pending (NFR-PERF-06: no
//! timer fires while idle); the only timers are the next damage frame of a visible session, a
//! handshake deadline and the idle exit (FR-SESS-003). When the loop is about to sleep with
//! nothing pending, freed memory goes back to the OS (`docs/03` §3 rule 7), at most once per
//! [`MEMORY_RELEASE_INTERVAL`].

mod backlog;
mod clients;
mod frames;
mod lend;
mod lifecycle;
mod mutations;
mod output;
mod requests;
mod state;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};

use ferrum_platform::ipc::Listener;
use ferrum_platform::memory::MemoryRelease;
use ferrum_proto::{DaemonMessage, SessionId, Text};
use ferrum_pty::{ExitWatch, PtyBackend};
use ferrum_scrollback::SpillStore;
use mio::{Events, Interest, Poll, Token, Waker};

use crate::bus::{EventBus, EventSubscriber};
use crate::client::Client;
use crate::clock::Clock;
use crate::config::DaemonConfig;
use crate::error::DaemonError;
use crate::frame::FrameEncoder;
use crate::projects::Projects;
use crate::session::{READ_CHUNK_BYTES, Session};
use crate::worker::Worker;
use backlog::Backlog;

/// Token of the IPC endpoint.
const LISTENER: Token = Token(0);
/// Token of the waker (worker results, Windows PTY and exit notifications).
const WAKER: Token = Token(1);
/// Token of the exit watcher.
const EXITS: Token = Token(2);
/// Tokens of sessions carry this bit and their id; ids never reach it.
const SESSION_BIT: usize = 1 << 61;
/// Tokens of clients carry this bit and their connection number.
const CLIENT_BIT: usize = 1 << 62;
/// Events taken from the poller per wake-up; more stay ready for the next call.
const EVENTS_PER_POLL: usize = 256;
/// Most clients at once. One UI per window plus a few tools fit; the cap bounds the client
/// buffers (`client.rs`) against a local process that opens connections in a loop (`docs/10` §2).
const MAX_CLIENTS: usize = 16;
/// Least time between two returns of freed memory to the OS: a call walks the allocator's heaps
/// (about a millisecond for a large heap), so typing, which makes the loop idle between keys,
/// does not pay it at every key.
const MEMORY_RELEASE_INTERVAL: Duration = Duration::from_secs(30);

/// Creates the spill store of a session (`docs/03` §3 rule 11), owner-only in the run directory.
pub trait SpillFactory: Send {
    /// The store for session `session`; `None` keeps that session's history in RAM only.
    fn create(&self, session: u64) -> Option<Box<dyn SpillStore>>;
}

/// What the composition root injects (ADR 0023: real implementations only in `ferrumd`'s
/// `main`; tests may inject their own).
pub struct Ports {
    /// Starts session children.
    pub pty: Box<dyn PtyBackend>,
    /// Learns about child exits.
    pub exits: Box<dyn ExitWatch>,
    /// The IPC endpoint (`ferrum_platform::ipc::listen`).
    pub listener: Box<dyn Listener>,
    /// Spill stores of the sessions.
    pub spill: Box<dyn SpillFactory>,
    /// Gives freed memory back to the OS.
    pub memory: Box<dyn MemoryRelease>,
    /// Time.
    pub clock: Box<dyn Clock>,
}

/// The daemon: sessions, projects and clients, driven by [`Daemon::run`].
pub struct Daemon {
    poll: Poll,
    waker: Arc<Waker>,
    ports: Ports,
    config: DaemonConfig,
    sessions: BTreeMap<SessionId, Session>,
    clients: HashMap<u64, Client>,
    next_client: u64,
    next_session: u64,
    projects: Projects,
    /// `FileSetAside` for every file set aside at start, told to every client.
    set_aside: Vec<DaemonMessage>,
    worker: Worker,
    backlog: Backlog,
    bus: EventBus,
    read_buf: Vec<u8>,
    encoder: FrameEncoder,
    /// Reused lists of sessions and clients to visit in a turn (no allocation per turn).
    scratch_ids: Vec<SessionId>,
    scratch_clients: Vec<u64>,
    /// Bytes queued for all clients together (`client.rs`).
    outbox_total: Arc<AtomicUsize>,
    /// When the daemon became idle (no session, no client).
    idle_since: Option<Instant>,
    /// When freed memory was last given back, and whether work happened since.
    released_at: Option<Instant>,
    accept_ready: bool,
    exits_ready: bool,
}

impl Daemon {
    /// Sets the daemon up: the poller, the worker, the projects from disk, and the sessions of
    /// every project respawned in their last working directory (FR-SESS-008).
    ///
    /// # Errors
    /// [`DaemonError`] when the poller, the worker or the project directory fails.
    pub fn new(config: DaemonConfig, mut ports: Ports) -> Result<Self, DaemonError> {
        let (poll, waker) = poller(&mut ports.listener)?;
        let wake = Arc::clone(&waker);
        // A failed wake-up only delays the result to the next event; nothing is lost.
        let worker = Worker::start(Box::new(move || {
            let _woken = wake.wake();
        }))
        .map_err(DaemonError::Worker)?;
        let projects = Projects::load(&config.data_dir).map_err(DaemonError::Projects)?;
        let set_aside = set_aside_messages(&projects);
        let mut daemon = Self {
            poll,
            waker,
            ports,
            config,
            sessions: BTreeMap::new(),
            clients: HashMap::new(),
            next_client: 0,
            next_session: 1,
            projects,
            set_aside,
            worker,
            backlog: Backlog::default(),
            bus: EventBus::default(),
            read_buf: vec![0; READ_CHUNK_BYTES],
            encoder: FrameEncoder::default(),
            scratch_ids: Vec::new(),
            scratch_clients: Vec::new(),
            outbox_total: Arc::new(AtomicUsize::new(0)),
            idle_since: None,
            released_at: None,
            accept_ready: true,
            exits_ready: false,
        };
        daemon.watch_exits()?;
        daemon.prepare_runtime();
        daemon.restore_sessions();
        Ok(daemon)
    }

    /// Adds a subscriber to the internal event bus (`docs/02` §9).
    pub fn subscribe(&mut self, subscriber: Box<dyn EventSubscriber>) {
        self.bus.subscribe(subscriber);
    }

    /// Runs until the daemon has had no session and no client for the grace period
    /// (FR-SESS-003).
    ///
    /// # Errors
    /// [`DaemonError::Poll`] when the poller fails.
    pub fn run(mut self) -> Result<(), DaemonError> {
        let mut events = Events::with_capacity(EVENTS_PER_POLL);
        loop {
            let timeout = self.timeout();
            if timeout.is_none() {
                self.release_memory();
            }
            match self.poll.poll(&mut events, timeout) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(DaemonError::poll("wait for events")(error)),
            }
            for event in &events {
                self.on_event(event);
            }
            self.turn();
            if self.is_done() {
                break;
            }
        }
        self.shutdown();
        Ok(())
    }

    fn on_event(&mut self, event: &mio::event::Event) {
        match event.token() {
            LISTENER => self.accept_ready = true,
            EXITS => self.exits_ready = true,
            WAKER => {}
            Token(raw) if raw & SESSION_BIT != 0 => {
                let id = SessionId(token_id(raw, SESSION_BIT));
                if let Some(session) = self.sessions.get_mut(&id) {
                    session.readable = true;
                    session.writable |= event.is_writable();
                }
            }
            Token(raw) => self.on_client_event(token_id(raw, CLIENT_BIT), event),
        }
    }

    fn on_client_event(&mut self, id: u64, event: &mio::event::Event) {
        let Some(client) = self.clients.get_mut(&id) else { return };
        client.readable = true;
        if !(event.is_writable() && std::mem::take(&mut client.skipped_frames)) {
            return;
        }
        // A client that was congested can take frames again, at the sessions' next boundary.
        let mut ids = std::mem::take(&mut self.scratch_ids);
        ids.clear();
        ids.extend(client.views.keys().copied());
        for &id in &ids {
            self.schedule_frame(id);
        }
        self.scratch_ids = ids;
    }

    /// One pass over everything that may have work.
    fn turn(&mut self) {
        self.accept_clients();
        self.drain_worker();
        self.handle_exits();
        self.pump_sessions();
        self.serve_clients();
        self.flush_inputs();
        self.expire_handshakes();
        // Frames first: a terminal back from a reflow that yielded shows its new size and the
        // output read meanwhile before it is lent again.
        self.send_frames();
        self.dispatch_resizes();
        self.deliver_events();
        self.flush_clients();
        self.send_backlog();
        self.update_idle();
    }

    /// How long the poller may sleep: not at all while output or frames are pending, until the
    /// next frame, handshake or idle deadline otherwise, and forever when there is nothing to do.
    fn timeout(&self) -> Option<Duration> {
        let busy =
            self.sessions.values().any(|s| s.readable && s.term.is_some() && s.pty.is_some())
                || self.sessions.values().any(|s| s.writable)
                || self.clients.values().any(|client| client.readable)
                || !self.backlog.is_empty();
        if busy {
            return Some(Duration::ZERO);
        }
        let idle_end = self.idle_since.map(|since| since + self.config.exit_grace());
        let frames = self.sessions.values().filter_map(|session| session.frame_due);
        let deadline = frames.chain(idle_end).chain(self.next_handshake_deadline()).min()?;
        Some(deadline.saturating_duration_since(self.ports.clock.now()))
    }

    /// Gives freed memory back before a sleep with nothing pending, unless that was done less
    /// than [`MEMORY_RELEASE_INTERVAL`] ago (no timer: the next idle sleep does it).
    fn release_memory(&mut self) {
        let now = self.ports.clock.now();
        if self.released_at.is_some_and(|at| now < at + MEMORY_RELEASE_INTERVAL) {
            return;
        }
        self.ports.memory.release();
        self.released_at = Some(now);
    }

    fn update_idle(&mut self) {
        let is_idle = self.sessions.is_empty() && self.clients.is_empty();
        self.idle_since = match (is_idle, self.idle_since) {
            (true, None) => Some(self.ports.clock.now()),
            (true, since) => since,
            (false, _) => None,
        };
    }

    fn is_done(&self) -> bool {
        self.idle_since
            .is_some_and(|since| self.ports.clock.now() >= since + self.config.exit_grace())
    }

    fn shutdown(&mut self) {
        let mut backlog = std::mem::take(&mut self.backlog);
        while let Some(job) = backlog.pop() {
            self.submit_blocking(job);
        }
        self.worker.stop();
    }

    /// At shutdown the remaining jobs (project writes) must not be lost: wait for room.
    fn submit_blocking(&mut self, mut job: crate::worker::Job) {
        loop {
            match self.worker.try_send(job) {
                Ok(()) => return,
                Err(back) => {
                    job = back;
                    while self.worker.done.try_recv().is_ok() {}
                    std::thread::yield_now();
                }
            }
        }
    }
}

/// The poller, its waker and the endpoint registered with it.
fn poller(listener: &mut Box<dyn Listener>) -> Result<(Poll, Arc<Waker>), DaemonError> {
    let poll = Poll::new().map_err(DaemonError::poll("create the poller"))?;
    let waker =
        Waker::new(poll.registry(), WAKER).map_err(DaemonError::poll("create the waker"))?;
    poll.registry()
        .register(listener, LISTENER, Interest::READABLE | Interest::WRITABLE)
        .map_err(DaemonError::poll("register the endpoint"))?;
    Ok((poll, Arc::new(waker)))
}

/// `FileSetAside` for every project file set aside at start (`docs/02` §8 and §10).
fn set_aside_messages(projects: &Projects) -> Vec<DaemonMessage> {
    let text = |path: &std::path::Path| Text::new(path.to_string_lossy());
    let messages = projects.set_aside.iter();
    messages
        .map(|(path, backup)| DaemonMessage::FileSetAside {
            path: text(path),
            backup: text(backup),
        })
        .collect()
}

/// The id inside a session or client token.
fn token_id(raw: usize, bit: usize) -> u64 {
    u64::try_from(raw & !bit).unwrap_or(u64::MAX)
}

/// The token of a session.
fn session_token(id: SessionId) -> Token {
    Token(SESSION_BIT | usize::try_from(id.0).unwrap_or(0))
}

/// The token of a client connection.
fn client_token(id: u64) -> Token {
    Token(CLIENT_BIT | usize::try_from(id).unwrap_or(0))
}
