//! PTY output and input: read and parse with per-session fairness, then act on what the terminal
//! reported (replies to the PTY, session events, history chunks to seal, a damage frame or an
//! `Activity`); write waiting input when the PTY has room again.

use ferrum_project::SessionEntry;
use ferrum_proto::{Percent, ProgressState, SessionEvent, SessionId, Text};
use ferrum_vt::ProgressState as VtProgress;

use super::Daemon;
use crate::cwd;
use crate::session::sink::Changes;
use crate::session::writer::InputRefused;
use crate::session::{Pump, guarded};
use crate::worker::Job;

/// What a session failed in parsing reports (`SessionEvent::Failed`).
pub(super) const FAILED_IN_PARSER: &str = "internal error while parsing the program's output";

/// What one session's output produced, taken from its terminal.
struct Produced {
    changes: Changes,
    title: Option<Text>,
    uri: Option<String>,
}

impl Daemon {
    /// One fairness quantum for every session with output waiting (`docs/02` §5).
    pub(super) fn pump_sessions(&mut self) {
        let mut ready = std::mem::take(&mut self.scratch_ids);
        ready.clear();
        for session in self.sessions.values_mut() {
            session.take_notifications();
            if session.readable && session.term.is_some() && session.pty.is_some() {
                ready.push(session.id);
            }
        }
        for &id in &ready {
            let Some(session) = self.sessions.get_mut(&id) else { continue };
            match session.pump(&mut self.read_buf) {
                Pump::Drained | Pump::More => self.after_output(id),
                Pump::Ended => {
                    self.after_output(id);
                    if let Some(mut session) = self.sessions.remove(&id) {
                        self.drop_pty(&mut session);
                        self.sessions.insert(id, session);
                    }
                }
                Pump::Panicked => self.fail_session(id, FAILED_IN_PARSER),
            }
        }
        self.scratch_ids = ready;
    }

    /// Writes waiting input of every session whose PTY has room again.
    pub(super) fn flush_inputs(&mut self) {
        let mut ids = std::mem::take(&mut self.scratch_ids);
        ids.clear();
        ids.extend(self.sessions.values().filter(|session| session.writable).map(|s| s.id));
        for &id in &ids {
            let Some(session) = self.sessions.get_mut(&id) else { continue };
            session.writable = false;
            let Some(input) = session.pty.as_mut().and_then(|pty| pty.input.as_mut()) else {
                continue;
            };
            if input.flush() == Err(InputRefused::Closed) {
                tracing::debug!(session = id.0, "input to a closed session was dropped");
            }
            self.watch_input(id);
        }
        self.scratch_ids = ids;
    }

    /// Acts on what parsing the last output produced.
    fn after_output(&mut self, id: SessionId) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let (Some(term), Some(pty)) = (session.term.as_deref_mut(), session.pty.as_mut()) else {
            return;
        };
        let input = pty.input.as_mut();
        let taken = guarded(|| {
            if let Some(input) = input {
                // A refused reply only means the child is not reading; it would not read it either.
                let _sent = input.send(term.sink().replies());
            }
            term.sink_mut().clear_replies();
            let changes = term.sink_mut().take();
            let title = changes.title.then(|| Text::new(term.title()));
            let uri = changes.cwd.then(|| term.cwd().to_owned());
            let mut chunks = Vec::new();
            while let Some(chunk) = term.take_pending_chunk() {
                chunks.push(chunk);
            }
            (Produced { changes, title, uri }, chunks)
        });
        let Some((produced, chunks)) = taken else {
            return self.fail_session(id, FAILED_IN_PARSER);
        };
        for chunk in chunks {
            self.submit(Job::Seal { session: id, chunk });
        }
        self.watch_input(id);
        self.report_changes(id, produced);
        self.after_damage(id);
    }

    fn report_changes(&mut self, id: SessionId, produced: Produced) {
        let Produced { changes, title, uri } = produced;
        if let Some(title) = title {
            self.publish(id, SessionEvent::Title(title));
        }
        if let Some(uri) = uri {
            self.remember_cwd(id, &uri);
            self.publish(id, SessionEvent::CwdChanged(Text::new(uri)));
        }
        if changes.bell {
            self.publish(id, SessionEvent::Bell);
        }
        if let Some((state, value)) = changes.progress {
            let value = Percent::try_from(value.min(100)).unwrap_or_default();
            self.publish(id, SessionEvent::Progress { state: progress(state), value });
        }
        if changes.spill_disabled {
            // docs/02 §10: spill disabled for this session, oldest lines dropped, user notified
            // once (the scrollback reports it once).
            let _span = tracing::warn_span!("spill", session = id.0).entered();
            tracing::warn!("scrollback spill disabled; old lines are dropped");
            self.publish(id, SessionEvent::SpillDisabled);
        }
    }

    /// Schedules a frame for a visible session, or reports activity of a hidden one.
    pub(super) fn after_damage(&mut self, id: SessionId) {
        self.end_sync_hold(id);
        let is_visible = self.clients.values().any(|client| client.views.contains_key(&id));
        if is_visible {
            // Even without row damage the cursor or the modes may have changed.
            self.schedule_frame(id);
            return;
        }
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let Some(term) = session.term.as_deref_mut() else { return };
        if term.damage().is_clean() {
            return;
        }
        term.take_damage();
        if !std::mem::replace(&mut session.activity_sent, true) {
            self.publish(id, SessionEvent::Activity);
        }
    }

    /// Asks for a frame of session `id` at its next frame boundary (`docs/02` §4: coalesced per
    /// frame interval, at most one frame per interval): at once when the last one went out at
    /// least an interval ago, else when the interval ends. Input never shortens the interval.
    pub(super) fn schedule_frame(&mut self, id: SessionId) {
        let now = self.ports.clock.now();
        let interval = self.config.frame_interval();
        let Some(session) = self.sessions.get_mut(&id) else { return };
        if session.frame_due.is_some() {
            return;
        }
        let next = session.last_frame.map_or(now, |last| last + interval);
        session.frame_due = Some(next.max(now));
    }

    /// Remembers a session's working directory in its project (FR-SESS-008).
    fn remember_cwd(&mut self, id: SessionId, uri: &str) {
        let Some(path) = cwd::from_uri(uri) else { return };
        let Some(project) = self.projects.owner_of(id.0) else { return };
        if self.projects.session_cwd(project, id.0) == Some(path.as_path()) {
            return; // shells report it at every prompt: write only changes
        }
        let changed = self.projects.update::<ferrum_project::ProjectError>(project, |parts| {
            if let Some(entry) =
                parts.sessions.iter_mut().find(|entry: &&mut SessionEntry| entry.key.0 == id.0)
            {
                entry.cwd = Some(path);
            }
            Ok(())
        });
        match changed {
            Ok(Some(write)) => self.submit(Job::Write(write)),
            Ok(None) => {}
            Err(_) => tracing::warn!(session = id.0, "a working directory was not remembered"),
        }
    }
}

fn progress(state: VtProgress) -> ProgressState {
    match state {
        VtProgress::Normal => ProgressState::Normal,
        VtProgress::Error => ProgressState::Error,
        VtProgress::Indeterminate => ProgressState::Indeterminate,
        VtProgress::Warning => ProgressState::Warning,
        _ => ProgressState::Remove,
    }
}
