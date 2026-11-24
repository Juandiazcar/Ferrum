//! Mutations of sessions and projects (`docs/02` §4: authoritative in the daemon, persisted, and
//! echoed to every client as events). A change the project model refuses (limits, a bad layout)
//! changes nothing and is logged.

use std::collections::HashSet;
use std::path::PathBuf;

use ferrum_project::{Project, ProjectError, ProjectId as ProjectKey, SessionEntry, SessionKey};
use ferrum_proto::{
    ClientMessage, DaemonMessage, Layout, ProjectChange, ProjectEvent, ProjectId, SessionEvent,
    SessionId, SessionStatus, Text,
};

use super::Daemon;
use super::lifecycle::Start;
use super::state::project_info;
use crate::layout;
use crate::session::SessionKind;
use crate::worker::Job;

fn key(id: ProjectId) -> ProjectKey {
    ProjectKey(id.0)
}

impl Daemon {
    /// Handles a mutation request.
    pub(super) fn handle_mutation(&mut self, message: ClientMessage) {
        match message {
            ClientMessage::CreateSession { project_id, kind, root } => {
                match SessionKind::from_proto(kind) {
                    Some(kind) => self.create_session(project_id, kind, root),
                    None => {
                        tracing::warn!("a session of a kind this daemon cannot run was refused");
                    }
                }
            }
            ClientMessage::CloseSession { session_id } => self.close_session(session_id),
            ClientMessage::Restart { session_id } => self.restart_session(session_id),
            ClientMessage::CreateProject { name, roots } => {
                let roots =
                    roots.into_vec().into_iter().map(|root| PathBuf::from(root.into_string()));
                self.create_project(&name, roots.collect());
            }
            ClientMessage::UpdateProject { project_id, change } => {
                self.update_project(project_id, change);
            }
            ClientMessage::RenameSession { session_id, name } => {
                self.rename_session(session_id, name);
            }
            ClientMessage::MoveSession { session_id, project_id, index } => {
                self.move_session(session_id, project_id, index);
            }
            ClientMessage::ReorderProject { project_id, index } => {
                self.reorder_project(project_id, index);
            }
            ClientMessage::SaveLayout { project_id, layout } => {
                self.save_layout(project_id, &layout);
            }
            // Reserved for Phase 2 (`docs/02` §9); a v1 client never sends them.
            _ => tracing::warn!("a client sent a message this daemon does not handle"),
        }
    }

    fn reorder_project(&mut self, project_id: ProjectId, index: u32) {
        let at = usize::try_from(index).unwrap_or(usize::MAX);
        if let Some(write) = self.projects.reorder(key(project_id), at) {
            self.submit(Job::Write(write));
            let event = ProjectEvent::Reordered { project_id, index };
            self.announce(DaemonMessage::ProjectEvent(event));
        }
    }

    /// Applies `change` to a project and persists it; false when refused.
    fn change_project(
        &mut self,
        project: ProjectId,
        change: impl FnOnce(&mut ferrum_project::ProjectParts) -> Result<(), ProjectError>,
    ) -> bool {
        let limits = self.config.settings.projects.limits;
        let checked = |parts: &mut ferrum_project::ProjectParts| {
            change(parts)?;
            Project::try_from(parts.clone())?.check_limits(&limits)
        };
        match self.projects.update(key(project), checked) {
            Ok(Some(write)) => {
                self.submit(Job::Write(write));
                true
            }
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(%error, "a project change was refused");
                false
            }
        }
    }

    fn create_session(&mut self, project_id: ProjectId, kind: SessionKind, root: u16) {
        let Some(project) = self.projects.get(key(project_id)) else { return };
        let Some(cwd) = project.roots().get(usize::from(root)).cloned() else { return };
        let id = SessionId(self.next_session);
        let added = self.change_project(project_id, |parts| {
            parts.sessions.push(SessionEntry::new(SessionKey(id.0), root));
            Ok(())
        });
        if !added {
            return;
        }
        self.next_session = self.next_session.saturating_add(1);
        let start = Start { id, project: project_id, kind, profile: None, cwd };
        let session = self.start_session(&start);
        self.sessions.insert(id, session);
        if let Some(info) = self.session_info(id) {
            self.announce(DaemonMessage::SessionEvent {
                session_id: id,
                event: SessionEvent::Created(info),
            });
        }
    }

    fn close_session(&mut self, id: SessionId) {
        let Some(mut session) = self.sessions.remove(&id) else { return };
        self.drop_pty(&mut session);
        for client in self.clients.values_mut() {
            client.views.remove(&id);
        }
        let project = session.project;
        drop(session); // the terminal and its spill file go now
        self.forget_session(project, id);
        self.announce(DaemonMessage::SessionEvent { session_id: id, event: SessionEvent::Closed });
    }

    /// Removes a session from its project's list and layout.
    fn forget_session(&mut self, project: ProjectId, id: SessionId) {
        let removed = self.change_project(project, |parts| {
            parts.sessions.retain(|entry| entry.key.0 != id.0);
            parts.layout =
                parts.layout.take().and_then(|tree| layout::without(tree, SessionKey(id.0)));
            Ok(())
        });
        if removed {
            self.broadcast_layout(project);
        }
    }

    fn broadcast_layout(&mut self, project_id: ProjectId) {
        let Some(project) = self.projects.get(key(project_id)) else { return };
        let layout = layout::to_proto(project.layout());
        self.announce(DaemonMessage::LayoutEvent { project_id, layout });
    }

    fn restart_session(&mut self, id: SessionId) {
        let Some(session) = self.sessions.get(&id) else { return };
        if session.status == SessionStatus::Running {
            return;
        }
        let project_id = session.project;
        let Some(project) = self.projects.get(key(project_id)) else { return };
        let Some(entry) = project.sessions().iter().find(|entry| entry.key.0 == id.0) else {
            return;
        };
        let root = project.roots().get(usize::from(entry.root)).cloned();
        let cwd = entry
            .cwd
            .clone()
            .filter(|cwd| cwd.is_dir())
            .or(root)
            .unwrap_or_else(|| project.default_root().to_path_buf());
        let kind = session.kind;
        let start = Start { id, project: project_id, kind, profile: entry.profile.clone(), cwd };
        let Some(mut old) = self.sessions.remove(&id) else { return };
        self.drop_pty(&mut old);
        let mut fresh = self.start_session(&start);
        // The screen and history stay: the new shell continues below the old output.
        if !old.has_failed() {
            fresh.term = old.term.take().or(fresh.term);
        }
        self.sessions.insert(id, fresh);
        if let Some(info) = self.session_info(id) {
            self.announce(DaemonMessage::SessionEvent {
                session_id: id,
                event: SessionEvent::Created(info),
            });
        }
    }

    fn create_project(&mut self, name: &Text, roots: Vec<PathBuf>) {
        let built = Project::new(ProjectKey::new_random(), roots).and_then(|project| {
            let mut parts = project.into_parts();
            if !name.as_str().is_empty() {
                name.as_str().clone_into(&mut parts.name);
            }
            let project = Project::try_from(parts)?;
            project.check_limits(&self.config.settings.projects.limits)?;
            Ok(project)
        });
        match built {
            Ok(project) => {
                let info = project_info(&project);
                for write in self.projects.insert(project) {
                    self.submit(Job::Write(write));
                }
                self.announce(DaemonMessage::ProjectEvent(ProjectEvent::Created(info)));
            }
            Err(error) => tracing::warn!(%error, "a project was not created"),
        }
    }

    fn update_project(&mut self, project_id: ProjectId, change: ProjectChange) {
        let applied = self.change_project(project_id, |parts| {
            match &change {
                ProjectChange::Rename(name) => name.as_str().clone_into(&mut parts.name),
                ProjectChange::Recolor(rgb) => {
                    parts.color = ferrum_config::Rgb { r: rgb.r, g: rgb.g, b: rgb.b }
                }
                ProjectChange::SetIcon(icon) => {
                    parts.icon = icon.as_ref().map(|icon| icon.as_str().to_owned());
                }
                _ => {}
            }
            Ok(())
        });
        if applied {
            self.announce(DaemonMessage::ProjectEvent(ProjectEvent::Updated {
                project_id,
                change,
            }));
        }
    }

    fn rename_session(&mut self, id: SessionId, name: Text) {
        let Some(project) = self.sessions.get(&id).map(|session| session.project) else { return };
        let new_name = (!name.as_str().is_empty()).then(|| name.as_str().to_owned());
        let applied = self.change_project(project, |parts| {
            if let Some(entry) = parts.sessions.iter_mut().find(|entry| entry.key.0 == id.0) {
                entry.name = new_name;
            }
            Ok(())
        });
        if applied {
            self.announce(DaemonMessage::SessionEvent {
                session_id: id,
                event: SessionEvent::Renamed(name),
            });
        }
    }

    fn move_session(&mut self, id: SessionId, to: ProjectId, index: u32) {
        let Some(from) = self.sessions.get(&id).map(|session| session.project) else { return };
        let Some(entry) = self
            .projects
            .get(key(from))
            .and_then(|p| p.sessions().iter().find(|e| e.key.0 == id.0).cloned())
        else {
            return;
        };
        let index = usize::try_from(index).unwrap_or(usize::MAX);
        let inserted = self.change_project(to, |parts| {
            parts.sessions.retain(|known| known.key.0 != id.0);
            let at = index.min(parts.sessions.len());
            parts.sessions.insert(at, entry);
            Ok(())
        });
        if !inserted {
            return;
        }
        if from != to {
            self.forget_session(from, id);
        }
        if let Some(session) = self.sessions.get_mut(&id) {
            session.project = to;
        }
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        self.announce(DaemonMessage::ProjectEvent(ProjectEvent::SessionMoved {
            session_id: id,
            project_id: to,
            index,
        }));
    }

    fn save_layout(&mut self, project_id: ProjectId, wire: &Layout) {
        let Some(project) = self.projects.get(key(project_id)) else { return };
        let known: HashSet<SessionKey> = project.sessions().iter().map(|entry| entry.key).collect();
        let Ok(tree) = layout::from_proto(wire, &known) else {
            tracing::warn!("a layout that is not a valid tree was refused");
            return;
        };
        if self.change_project(project_id, |parts| {
            parts.layout = tree;
            Ok(())
        }) {
            self.announce(DaemonMessage::LayoutEvent { project_id, layout: wire.clone() });
        }
    }
}
