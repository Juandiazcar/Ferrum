//! Damage frames (`docs/02` §4): at most one per frame interval and session, from the session's
//! accumulated damage to every view that is not congested. Pacing is per session, so a busy
//! session never delays the first frame of another one (NFR-PERF-04). Synchronized output (mode
//! 2026, FR-TERM-007) holds a session's damage back until the program ends the update, or at
//! most [`SYNC_HOLD`]; the frame goes at the next frame boundary once it ends.

use std::time::Duration;

use ferrum_proto::{DaemonMessage, SessionId};
use ferrum_vt::Mode;

use super::Daemon;
use crate::client::Client;
use crate::encode::Term;
use crate::frame::FrameEncoder;
use crate::view::View;

/// Longest time synchronized output holds damage back; a program that never ends its update
/// cannot freeze its view (the hold xterm-compatible terminals use is of this order).
const SYNC_HOLD: Duration = Duration::from_millis(150);
/// History rows sent between two checks of the client's buffer: about 50 KiB of rows at 200
/// columns, so a client is never more than one step past congestion.
const HISTORY_STEP_ROWS: u32 = 256;
/// What a session failed while its screen was encoded reports (`SessionEvent::Failed`).
const FAILED_IN_FRAME: &str = "internal error while encoding the screen";

impl Daemon {
    /// Sends the frames that are due.
    pub(super) fn send_frames(&mut self) {
        let now = self.ports.clock.now();
        let mut due = std::mem::take(&mut self.scratch_ids);
        due.clear();
        for session in self.sessions.values_mut() {
            if session.frame_due.is_some_and(|at| at <= now) {
                session.frame_due = None;
                due.push(session.id);
            }
        }
        for &id in &due {
            if self.collect_damage(id) {
                if let Some(session) = self.sessions.get_mut(&id) {
                    session.last_frame = Some(now);
                }
                self.send_views(id);
            }
        }
        self.scratch_ids = due;
    }

    /// Ends a synchronized-output hold as soon as the program ends its update: the frame goes at
    /// the next frame boundary instead of at the end of the hold (FR-TERM-007).
    pub(super) fn end_sync_hold(&mut self, id: SessionId) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let Some(term) = session.term.as_deref() else { return };
        if session.sync_since.is_none() || term.modes().contains(Mode::SYNC_UPDATE) {
            return;
        }
        session.sync_since = None;
        session.frame_due = None;
        self.schedule_frame(id);
    }

    /// Moves a session's damage into its views; false while the terminal is lent (it gets a
    /// frame when it comes back) or held by synchronized output.
    fn collect_damage(&mut self, id: SessionId) -> bool {
        let now = self.ports.clock.now();
        let Some(session) = self.sessions.get_mut(&id) else { return false };
        let Some(term) = session.term.as_deref_mut() else { return false };
        if term.modes().contains(Mode::SYNC_UPDATE) {
            let since = *session.sync_since.get_or_insert(now);
            if now.duration_since(since) < SYNC_HOLD {
                session.frame_due = Some(since + SYNC_HOLD);
                return false;
            }
        } else {
            session.sync_since = None;
        }
        let lines = term.grid().lines();
        for client in self.clients.values_mut() {
            if let Some(view) = client.views.get_mut(&id) {
                view.dirty.merge(term.damage(), lines);
            }
        }
        term.take_damage();
        true
    }

    /// Sends each view of session `id` what it owes: a snapshot, history rows, or damage.
    fn send_views(&mut self, id: SessionId) {
        let Some(term) = self.sessions.get(&id).and_then(|s| s.term.as_deref()) else { return };
        let encoder = &mut self.encoder;
        let mut failed = false;
        for client in self.clients.values_mut() {
            if client.version().is_none() {
                continue;
            }
            if client.is_congested() {
                // The views keep their damage; the client gets a frame once it drained.
                client.skipped_frames = true;
                continue;
            }
            let Some(mut view) = client.views.remove(&id) else { continue };
            let outcome = crate::session::guarded(|| {
                let mut send = |message: &DaemonMessage| client.send(message);
                if let Some(history) = view.snapshot.take() {
                    encoder.snapshot(term, (id, history), &mut view, &mut send);
                }
                encoder.damage(term, id, &mut view, &mut send);
                send_history(client, (encoder, term), id, &mut view);
            });
            // A panic while reading the terminal fails the session (`docs/02` §10); the view
            // gets a fresh snapshot of the blank failed screen.
            if outcome.is_none() {
                failed = true;
                view = View::attached(0);
            }
            client.views.insert(id, view);
        }
        if failed {
            self.fail_session(id, FAILED_IN_FRAME);
        }
    }
}

/// Sends the history rows `view` owes, [`HISTORY_STEP_ROWS`] at a time, until they are all sent
/// or the client is congested; then the rest waits for the client to drain (`docs/02` §4).
fn send_history(
    client: &mut Client,
    (encoder, term): (&mut FrameEncoder, &Term),
    id: SessionId,
    view: &mut View,
) {
    while let Some(range) = view.history.take() {
        if range.is_empty() {
            return;
        }
        if client.is_congested() {
            view.history = Some(range);
            client.skipped_frames = true;
            return;
        }
        let step = range.start..range.end.min(range.start.saturating_add(HISTORY_STEP_ROWS));
        view.history = Some(step.end..range.end);
        encoder.history_rows(term, id, step, &mut |message| client.send(message));
    }
}
