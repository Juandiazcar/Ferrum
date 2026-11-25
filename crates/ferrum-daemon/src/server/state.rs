//! What clients are told: the whole state after `Welcome` (`ProjectEvent::Created` per project,
//! `SessionEvent::Created` per session, the files set aside at start, then `Synced`;
//! FR-SESS-002) and every change as it happens. Session events go through the [`EventBus`]
//! (`docs/02` §9) and reach the clients at the end of the turn; a session's events wait until
//! its `Created` went out. A message newer than a client's negotiated minor version is replaced
//! by its 1.0 form, or left out when it has none (`docs/02` §4: additive changes only).
//!
//! [`EventBus`]: crate::EventBus

use ferrum_project::Project;
use ferrum_proto::{
    DaemonMessage, List, ProjectEvent, ProjectId, ProjectInfo, ProtocolVersion, Rgb, SessionEvent,
    SessionId, SessionInfo, SessionStatus, Text,
};

use super::Daemon;
use crate::layout;

/// The minor version that added `Failed`, `SpillDisabled`, `FileSetAside` and `SearchResults`.
pub(super) const MINOR_WITH_FAILED: u16 = 1;

impl Daemon {
    /// Publishes a session event on the bus once clients know the session; before its
    /// `Created` went out the event is held (`Session::hold`) and published right after it.
    pub(super) fn publish(&mut self, session_id: SessionId, event: SessionEvent) {
        let Some(session) = self.sessions.get_mut(&session_id) else { return };
        if session.announced {
            self.bus.publish(session_id, event);
        } else {
            session.hold(event);
        }
    }

    /// Delivers the events of this turn to the bus's subscribers and to every accepted client.
    /// Once a session's `Created` is delivered, the state sent to new clients lists it.
    pub(super) fn deliver_events(&mut self) {
        let clients = &mut self.clients;
        let sessions = &mut self.sessions;
        self.bus.deliver(|session_id, event| {
            if let SessionEvent::Created(_) = event
                && let Some(session) = sessions.get_mut(&session_id)
            {
                session.created_queued = false;
            }
            for client in clients.values_mut() {
                let Some(version) = client.version() else { continue };
                if let Some(event) = downgrade_event(event, version) {
                    client.send(&DaemonMessage::SessionEvent { session_id, event });
                }
            }
        });
    }

    /// Sends a message to every accepted client, in the form its version understands.
    pub(super) fn broadcast(&mut self, message: &DaemonMessage) {
        for client in self.clients.values_mut() {
            let Some(version) = client.version() else { continue };
            if let Some(message) = downgrade(message, version) {
                client.send(&message);
            }
        }
    }

    /// Broadcasts `message` once the project writes queued before it are on disk, so a client
    /// never learns of a change a crash could still lose (`docs/02` §4: authoritative in D).
    pub(super) fn announce(&mut self, message: DaemonMessage) {
        self.submit(crate::worker::Job::Announce(Box::new(message)));
    }

    /// An announcement whose writes are on disk. Session events go on the bus; a session's
    /// `Created` is rebuilt now, so it carries the current title and status, and opens the
    /// session's other events.
    pub(super) fn announced(&mut self, message: DaemonMessage) {
        let DaemonMessage::SessionEvent { session_id, event } = message else {
            return self.broadcast(&message);
        };
        if !matches!(event, SessionEvent::Created(_)) {
            return self.bus.publish(session_id, event);
        }
        let Some(info) = self.session_info(session_id) else { return };
        let Some(session) = self.sessions.get_mut(&session_id) else { return };
        session.announced = true;
        // Until the bus delivers it, `send_state` leaves the session out (no second `Created`).
        session.created_queued = true;
        let held = std::mem::take(&mut session.held);
        self.bus.publish(session_id, SessionEvent::Created(info));
        for event in held {
            self.bus.publish(session_id, event);
        }
    }

    /// The listing of a session.
    pub(super) fn session_info(&self, id: SessionId) -> Option<SessionInfo> {
        let session = self.sessions.get(&id)?;
        let project = self.projects.get(ferrum_project::ProjectId(session.project.0));
        let entry = project.and_then(|p| p.sessions().iter().find(|entry| entry.key.0 == id.0));
        let title = session.term.as_deref().map_or("", |term| term.title());
        Some(SessionInfo {
            project_id: session.project,
            kind: session.kind.to_proto(),
            name: entry.and_then(|entry| entry.name.as_deref()).map(Text::new),
            title: Text::new(title),
            status: session.status.clone(),
        })
    }

    /// Tells a newly accepted client the whole state.
    pub(super) fn send_state(&mut self, client_id: u64) {
        let mut messages: Vec<DaemonMessage> = self
            .projects
            .ordered()
            .map(|project| {
                DaemonMessage::ProjectEvent(ProjectEvent::Created(project_info(project)))
            })
            .collect();
        for (&id, session) in &self.sessions {
            if session.announced
                && !session.created_queued
                && let Some(info) = self.session_info(id)
            {
                let event = SessionEvent::Created(info);
                messages.push(DaemonMessage::SessionEvent { session_id: id, event });
            }
        }
        messages.extend(self.set_aside.iter().cloned());
        messages.push(DaemonMessage::Synced);
        let Some(client) = self.clients.get_mut(&client_id) else { return };
        let Some(version) = client.version() else { return };
        for message in &messages {
            if let Some(message) = downgrade(message, version) {
                client.send(&message);
            }
        }
    }
}

/// The protocol form of a project.
pub(super) fn project_info(project: &Project) -> ProjectInfo {
    let roots = project.roots().iter().map(|root| Text::new(root.to_string_lossy())).collect();
    let sessions = project.sessions().iter().map(|entry| SessionId(entry.key.0)).collect();
    let color = project.color();
    // Projects are held to the limits (at most 64 roots and 1 024 sessions), below the list cap.
    ProjectInfo {
        project_id: ProjectId(project.id().0),
        name: Text::new(project.name()),
        color: Rgb { r: color.r, g: color.g, b: color.b },
        icon: project.icon().map(Text::new),
        roots: List::new(roots).unwrap_or_default(),
        sessions: List::new(sessions).unwrap_or_default(),
        layout: layout::to_proto(project.layout()),
    }
}

/// The form of `event` a client at `version` understands; `None` when it has none.
fn downgrade_event(event: &SessionEvent, version: ProtocolVersion) -> Option<SessionEvent> {
    if version.minor >= MINOR_WITH_FAILED {
        return Some(event.clone());
    }
    match event {
        SessionEvent::Failed { .. } => Some(SessionEvent::Killed),
        SessionEvent::SpillDisabled => None,
        SessionEvent::Created(info) if matches!(info.status, SessionStatus::Failed { .. }) => {
            let info = SessionInfo { status: SessionStatus::Killed, ..info.clone() };
            Some(SessionEvent::Created(info))
        }
        other => Some(other.clone()),
    }
}

fn downgrade(message: &DaemonMessage, version: ProtocolVersion) -> Option<DaemonMessage> {
    match message {
        DaemonMessage::SessionEvent { session_id, event } => {
            let event = downgrade_event(event, version)?;
            Some(DaemonMessage::SessionEvent { session_id: *session_id, event })
        }
        DaemonMessage::FileSetAside { .. } | DaemonMessage::SearchResults { .. }
            if version.minor < MINOR_WITH_FAILED =>
        {
            None
        }
        other => Some(other.clone()),
    }
}
