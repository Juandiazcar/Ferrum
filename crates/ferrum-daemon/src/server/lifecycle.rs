//! Session lifecycle (FR-SESS-006/008): start, respawn after a restart of the daemon, exit, close,
//! restart, and failure after an isolated panic.

use std::num::NonZeroU16;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use ferrum_project::ProjectId as ProjectKey;
use ferrum_proto::{ProjectId, SessionEvent, SessionId, SessionStatus, Text};
use ferrum_pty::{PtySize, shell_integration};
use ferrum_scrollback::Scrollback;
use ferrum_vt::{Size, Terminal};
use mio::Interest;

use super::{Daemon, EXITS, SESSION_BIT, session_token};
use crate::encode::Term;
use crate::error::DaemonError;
use crate::os_source::{self, Slot};
use crate::session::sink::Sink;
use crate::session::spawn::{Launch, spawn_spec};
use crate::session::{Parts, Pty, Session, SessionKind, failed_status};
use crate::worker::Job;

/// Where and how a session's child starts.
pub(super) struct Start {
    pub(super) id: SessionId,
    pub(super) project: ProjectId,
    pub(super) kind: SessionKind,
    pub(super) profile: Option<String>,
    pub(super) cwd: PathBuf,
}

impl Daemon {
    /// Registers the exit watcher with the loop.
    pub(super) fn watch_exits(&mut self) -> Result<(), DaemonError> {
        let readiness = self.ports.exits.readiness();
        let waker = Arc::clone(&self.waker);
        let notify = Box::new(move || {
            let _woken = waker.wake(); // a lost wake-up is caught by the next event
        });
        let slot = Slot { token: EXITS, interest: Interest::READABLE };
        let installed = os_source::register(self.poll.registry(), readiness, slot, notify)
            .map_err(DaemonError::poll("register the exit watcher"))?;
        if let Some(notify) = installed {
            // Windows: every notification also means "look at the exit watcher".
            self.ports.exits.set_notify(notify);
            self.exits_ready = true;
        }
        Ok(())
    }

    /// Writes the shell integration scripts.
    pub(super) fn prepare_runtime(&self) {
        if let Err(error) =
            shell_integration::write_bootstrap_files(&self.config.shell_integration_dir())
        {
            tracing::warn!(%error, "the shell integration scripts could not be written");
        }
    }

    /// Respawns every session of every project (FR-SESS-008: the processes are new, the layout
    /// and the working directories are the remembered ones).
    pub(super) fn restore_sessions(&mut self) {
        let mut starts = Vec::new();
        for project in self.projects.ordered() {
            for entry in project.sessions() {
                if usize::try_from(entry.key.0).is_ok_and(|key| key < SESSION_BIT) {
                    let roots = project.roots();
                    let root = roots
                        .get(usize::from(entry.root))
                        .map_or_else(|| project.default_root(), PathBuf::as_path);
                    starts.push(Start {
                        id: SessionId(entry.key.0),
                        project: ProjectId(project.id().0),
                        kind: SessionKind::Terminal,
                        profile: entry.profile.clone(),
                        cwd: entry
                            .cwd
                            .clone()
                            .filter(|cwd| cwd.is_dir())
                            .unwrap_or_else(|| root.to_path_buf()),
                    });
                }
            }
        }
        for start in starts {
            self.next_session = self.next_session.max(start.id.0.saturating_add(1));
            let mut session = self.start_session(&start);
            session.announced = true; // no client is connected yet: the state lists it
            self.sessions.insert(start.id, session);
        }
    }

    /// A session with a running child, or, when the spawn fails, an exited one.
    pub(super) fn start_session(&mut self, start: &Start) -> Session {
        let _span = tracing::info_span!("session_start", session = start.id.0).entered();
        let size = Size::default();
        let term = self.new_term(start.id, size);
        let pty = self.spawn_pty(start, size);
        let parts = Parts { id: start.id, project: start.project, kind: start.kind, term };
        let mut session = Session::new(parts, pty);
        if session.pty.is_some() && !self.connect_pty(&mut session) {
            self.release_pty(&mut session);
            session.pty = None;
            session.status = SessionStatus::Exited { code: None };
        }
        session
    }

    fn new_term(&self, id: SessionId, size: Size) -> Box<Term> {
        let config = self.config.scrollback();
        let scrollback = match self.ports.spill.create(id.0) {
            Some(spill) => Scrollback::with_spill(config, spill),
            None => Scrollback::new(config),
        };
        Box::new(Terminal::new(size, scrollback, Sink::default()))
    }

    fn spawn_pty(&self, start: &Start, size: Size) -> Option<Pty> {
        let one = NonZeroU16::MIN;
        let to_cells =
            |value: usize| u16::try_from(value).ok().and_then(NonZeroU16::new).unwrap_or(one);
        let launch = Launch {
            session: start.id.0,
            project: ProjectKey(start.project.0).to_string(),
            profile: start.profile.as_deref(),
            cwd: start.cwd.clone(),
            size: PtySize { cols: to_cells(size.cols()), rows: to_cells(size.rows()) },
        };
        let spec = spawn_spec(&self.config, &launch);
        match self.ports.pty.spawn(&spec) {
            Ok(session) => Some(Pty::new(session)),
            Err(error) => {
                tracing::warn!(%error, "a session could not start");
                None
            }
        }
    }

    /// Registers the session's output, input room and exit with the loop; false when that fails.
    fn connect_pty(&mut self, session: &mut Session) -> bool {
        let Some(pty) = session.pty.as_mut() else { return false };
        let notified = Arc::clone(&session.notified);
        let waker = Arc::clone(&self.waker);
        let output = Box::new(move || {
            notified.output.store(true, Ordering::Release);
            let _woken = waker.wake(); // the flag stays set for the next turn
        });
        let slot = Slot { token: session_token(session.id), interest: Interest::READABLE };
        match os_source::register(self.poll.registry(), pty.reader.readiness(), slot, output) {
            Ok(Some(notify)) => pty.reader.set_notify(notify),
            Ok(None) => {}
            Err(error) => {
                tracing::error!(%error, "a session's output could not be registered");
                return false;
            }
        }
        if let Some(input) = pty.input.as_mut() {
            // A descriptor writer shares the reader's registration (`watch_input`).
            let notified = Arc::clone(&session.notified);
            let waker = Arc::clone(&self.waker);
            input.set_notify(Box::new(move || {
                notified.input.store(true, Ordering::Release);
                let _woken = waker.wake();
            }));
        }
        let watched = pty
            .child
            .as_ref()
            .map(|child| self.ports.exits.watch(session.id.0, child.exit_source()));
        if let Some(Err(error)) = watched {
            // Without the watcher the exit is still seen when the session is closed.
            tracing::warn!(%error, "a session's exit cannot be watched");
        }
        true
    }

    /// Makes the loop wait for room in a session's PTY while input is waiting, and only then:
    /// a descriptor is registered for writability (`docs/11` §5: non-blocking writes on the
    /// loop); a notifying writer reports by itself.
    pub(super) fn watch_input(&mut self, id: SessionId) {
        let Some(pty) = self.sessions.get_mut(&id).and_then(|s| s.pty.as_mut()) else { return };
        let waiting = pty.input.as_ref().is_some_and(crate::session::writer::Input::is_waiting);
        if waiting == pty.watching_room {
            return;
        }
        pty.watching_room = waiting;
        let interest =
            if waiting { Interest::READABLE | Interest::WRITABLE } else { Interest::READABLE };
        let slot = Slot { token: session_token(id), interest };
        os_source::reregister(self.poll.registry(), pty.reader.readiness(), slot);
    }

    /// Ends a session's processes on the worker; the reader stays until its output ends.
    pub(super) fn release_pty(&mut self, session: &mut Session) {
        let Some(pty) = session.pty.as_mut() else { return };
        self.ports.exits.unwatch(session.id.0);
        pty.input = None;
        if let Some(child) = pty.child.take() {
            self.submit(Job::Close(child));
        }
    }

    /// Drops a session's PTY after its output ended.
    pub(super) fn drop_pty(&mut self, session: &mut Session) {
        if let Some(pty) = session.pty.as_ref() {
            os_source::deregister(self.poll.registry(), pty.reader.readiness());
        }
        self.release_pty(session);
        session.pty = None;
        session.readable = false;
        session.writable = false;
    }

    /// Children that exited: their status is read and announced, and their terminals closed.
    pub(super) fn handle_exits(&mut self) {
        if !std::mem::take(&mut self.exits_ready) {
            return;
        }
        let mut keys = Vec::new();
        if let Err(error) = self.ports.exits.take_exited(&mut keys) {
            tracing::error!(%error, "the exit watcher failed");
        }
        for key in keys {
            self.child_exited(SessionId(key));
        }
    }

    fn child_exited(&mut self, id: SessionId) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let status =
            session.pty.as_mut().and_then(|pty| pty.child.as_mut()).map(|child| child.try_wait());
        match status {
            Some(Ok(Some(status))) => session.exited(status),
            Some(Err(error)) => {
                tracing::warn!(%error, "an exit status could not be read");
                session.status = SessionStatus::Exited { code: None };
            }
            // Not exited after all (or already closed): keep watching.
            _ => return,
        }
        let _span = tracing::info_span!("session_exit", session = id.0).entered();
        let event = match session.status {
            SessionStatus::Exited { code } => SessionEvent::Exited { code },
            _ => SessionEvent::Killed,
        };
        // The output written before the exit is still read: the reader ends after the close.
        let Some(mut session) = self.sessions.remove(&id) else { return };
        self.release_pty(&mut session);
        self.sessions.insert(id, session);
        self.publish(id, event);
    }

    /// Fails a session after a panic in its terminal: its processes end, it keeps a blank screen
    /// and stays listed, with `error`, until closed or restarted (`docs/02` §10).
    pub(super) fn fail_session(&mut self, id: SessionId, error: &'static str) {
        let _span = tracing::error_span!("session_failed", session = id.0).entered();
        tracing::error!(error, "a session's terminal failed; the session is stopped");
        let term = self.new_term(id, Size::default());
        let Some(mut session) = self.sessions.remove(&id) else { return };
        self.drop_pty(&mut session);
        session.term = Some(term);
        session.status = failed_status(error);
        self.sessions.insert(id, session);
        self.publish(id, SessionEvent::Failed { error: Text::new(error) });
    }
}
