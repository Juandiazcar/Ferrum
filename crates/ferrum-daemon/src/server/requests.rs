//! Requests about what a client shows and types: attach, detach, input, resize, scroll and
//! search (`docs/02` §4). Mutations of sessions and projects are in `mutations`, held to the
//! client's rate limit (`docs/10` §2).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ferrum_proto::{ClientMessage, DaemonMessage, List, SearchOptions, SessionId, Text, Viewport};
use ferrum_vt::Size;

use super::Daemon;
use super::state::MINOR_WITH_FAILED;
use crate::client::Phase;
use crate::search::{Found, SearchInput};
use crate::session::writer::InputRefused;
use crate::view::View;
use crate::worker::{Job, Query, SearchJob};

/// What a session failed while its screen was copied for a search reports.
const FAILED_IN_SEARCH: &str = "internal error while preparing a search";

impl Daemon {
    /// Handles one request of an accepted client.
    pub(super) fn handle_request(&mut self, client: u64, message: ClientMessage) {
        match message {
            ClientMessage::Attach { session_id, viewport, want_scrollback_rows } => {
                self.attach(client, session_id, viewport, want_scrollback_rows);
            }
            ClientMessage::Detach { session_id } => {
                if let Some(c) = self.clients.get_mut(&client) {
                    c.views.remove(&session_id);
                }
            }
            ClientMessage::Input { session_id, bytes } => self.input(session_id, bytes.as_slice()),
            ClientMessage::Resize { session_id, viewport } => self.resize(session_id, viewport),
            ClientMessage::Scroll { session_id, offset } => self.scroll(client, session_id, offset),
            ClientMessage::Search { session_id, query, options } => {
                self.search(client, session_id, &query, options);
            }
            // Selection belongs to WP-11 (selection and clipboard, `docs/12`): the request is
            // accepted and has no effect until then.
            ClientMessage::Select { .. } => {}
            ClientMessage::Hello(_) => {
                tracing::warn!("a client sent a second hello; it is disconnected");
                if let Some(c) = self.clients.get_mut(&client) {
                    c.phase = Phase::Closing;
                }
            }
            other => self.limited_mutation(client, other),
        }
    }

    /// A mutation, unless the client exceeded its rate or the worker's backlog is full.
    fn limited_mutation(&mut self, client: u64, message: ClientMessage) {
        let now = self.ports.clock.now();
        let Some(c) = self.clients.get_mut(&client) else { return };
        if !c.allow_mutation(now) || self.is_backlog_full() {
            tracing::warn!("a client sent requests too fast; one was dropped");
            return;
        }
        self.handle_mutation(message);
    }

    fn attach(&mut self, client: u64, id: SessionId, viewport: Viewport, history: u16) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        session.activity_sent = false;
        let Some(c) = self.clients.get_mut(&client) else { return };
        c.views.insert(id, View::attached(history));
        self.resize(id, viewport);
        self.schedule_frame(id);
    }

    /// Writes input to the session's child; its echo is paced like any output.
    fn input(&mut self, id: SessionId, bytes: &[u8]) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let Some(input) = session.pty.as_mut().and_then(|p| p.input.as_mut()) else {
            return; // the session exited: input goes nowhere
        };
        if input.send(bytes) == Err(InputRefused::Full) {
            tracing::warn!(session = id.0, "input dropped: the program is not reading it");
        }
        self.watch_input(id);
    }

    /// Records a new size; it is applied when the terminal is here (`dispatch_resizes`). A
    /// reflow still running for an older size stops at its next chunk boundary.
    fn resize(&mut self, id: SessionId, viewport: Viewport) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let size = Size::new(viewport.cols, viewport.rows);
        let current = session.term.as_deref().map(ferrum_vt::Terminal::size);
        if current != Some(size) || session.pending_size.is_some() {
            session.pending_size = Some(size);
            session.reflow_cancel.store(true, Ordering::Release);
        }
    }

    fn scroll(&mut self, client: u64, id: SessionId, offset: u32) {
        let Some(view) = self.clients.get_mut(&client).and_then(|c| c.views.get_mut(&id)) else {
            return;
        };
        let rows =
            self.sessions.get(&id).and_then(|s| s.term.as_deref()).map_or(0, |t| t.grid().lines());
        let rows = u32::try_from(rows).unwrap_or(u32::MAX);
        view.history = Some(offset.saturating_sub(rows)..offset);
        self.schedule_frame(id);
    }

    /// Starts a search on the worker over the session's history as it is now; the session keeps
    /// running (`docs/02` §5). A newer search cancels the one running.
    fn search(&mut self, client: u64, id: SessionId, query: &Text, options: SearchOptions) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        session.search_cancel.store(true, Ordering::Release);
        let query = Query { text: query.as_str().to_owned(), options };
        session.pending_search = Some((client, query));
        self.start_search(id);
    }

    /// Starts the pending search of session `id` once its terminal is here (not lent to a
    /// reflow).
    pub(super) fn start_search(&mut self, id: SessionId) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let Some(term) = session.term.as_deref() else { return };
        let Some((client, query)) = session.pending_search.take() else { return };
        let cancel = Arc::new(AtomicBool::new(false));
        session.search_cancel = Arc::clone(&cancel);
        let Some(input) = crate::session::guarded(|| SearchInput::of(term)) else {
            return self.fail_session(id, FAILED_IN_SEARCH);
        };
        self.submit(Job::Search(Box::new(SearchJob { session: id, client, input, query, cancel })));
    }

    /// Sends a search result to the client that asked (protocol 1.1 and later).
    pub(super) fn send_search_results(&mut self, client: u64, id: SessionId, found: Found) {
        let Some(c) = self.clients.get_mut(&client) else { return };
        if c.version().is_none_or(|version| version.minor < MINOR_WITH_FAILED) {
            return;
        }
        let matches = List::new(found.matches).unwrap_or_default();
        c.send(&DaemonMessage::SearchResults {
            session_id: id,
            matches,
            is_truncated: found.is_truncated,
        });
    }
}
