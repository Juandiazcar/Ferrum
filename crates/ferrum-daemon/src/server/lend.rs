//! Resizes (reflow, FR-TERM-010) and what the worker gives back. A large history is reflowed on
//! the worker (`docs/02` §3.2: large reflows run on a worker, with a time budget and yield)
//! while the UI keeps painting the other sessions, a small one inline. The worker gives the
//! terminal back after [`REFLOW_BUDGET`]; the session's output waiting meanwhile is read, then
//! the reflow resumes with twice the budget, so it always ends. Only the latest pending size is
//! kept, and a newer size stops a running reflow at its next chunk boundary (`docs/05` §5), so a
//! burst of resizes costs about one reflow; the rows not reached keep their width until the next.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ferrum_proto::SessionId;
use ferrum_pty::PtySize;
use ferrum_scrollback::SealedBlob;
use ferrum_vt::{Reflowed, Size};

use super::Daemon;
use crate::encode::Term;
use crate::search::Found;
use crate::session::guarded;
use crate::worker::{Done, Job, ReflowJob, ReflowTarget};

/// History lines above which a reflow goes to the worker: below this a reflow takes well under a
/// frame interval (10 000 lines take at most 40 ms, `docs/03` §4, so 1 000 lines take ~4 ms).
const INLINE_REFLOW_LINES: usize = 1_000;
/// What a failed reflow or install reports (`SessionEvent::Failed`).
const FAILED_IN_REFLOW: &str = "internal error while reflowing the terminal";
/// What a failed install of a compressed history chunk reports.
const FAILED_IN_INSTALL: &str = "internal error while storing history";
/// Time a reflow runs on the worker before it gives the terminal back: one frame at 60 Hz, so a
/// session's output waits at most about a frame (`docs/02` §3.2).
const REFLOW_BUDGET: Duration = Duration::from_millis(16);

impl Daemon {
    /// Applies the pending size of every session whose terminal is here.
    pub(super) fn dispatch_resizes(&mut self) {
        let mut ids = std::mem::take(&mut self.scratch_ids);
        ids.clear();
        // A yielded reflow resumes once the frame showing the output read meanwhile went out.
        let waiting = self.sessions.values().filter(|s| {
            let resume = s.reflow_resume.is_some() && s.frame_due.is_none();
            s.term.is_some() && (s.pending_size.is_some() || resume)
        });
        ids.extend(waiting.map(|s| s.id));
        for &id in &ids {
            self.dispatch_resize(id);
        }
        self.scratch_ids = ids;
    }

    fn dispatch_resize(&mut self, id: SessionId) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let Some(size) = session.pending_size.take() else {
            return self.resume_reflow(id);
        };
        session.reflow_resume = None; // a new size rewraps everything
        let Some(term) = session.term.take() else { return };
        let _span = tracing::debug_span!("resize", session = id.0).entered();
        if let Some(pty) = &session.pty
            && let Some(pty_size) = pty_size(size)
            && let Err(error) = pty.resize.resize(pty_size)
        {
            tracing::warn!(%error, "a session's PTY could not be resized");
        }
        if term.scrollback().len() > INLINE_REFLOW_LINES {
            return self.lend_reflow(id, term, (ReflowTarget::Resize(size), REFLOW_BUDGET));
        }
        let mut term = term;
        let applied = guarded(|| term.resize(size));
        session.term = Some(term);
        match applied {
            Some(Err(error)) => tracing::warn!(%error, "part of a session's history was lost"),
            Some(Ok(_)) => {}
            None => return self.fail_session(id, FAILED_IN_REFLOW),
        }
        self.resized(id);
    }

    /// Continues a reflow that yielded, with its doubled budget.
    fn resume_reflow(&mut self, id: SessionId) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let (Some(budget), Some(term)) = (session.reflow_resume.take(), session.term.take()) else {
            return;
        };
        self.lend_reflow(id, term, (ReflowTarget::Resume, budget));
    }

    /// Lends the terminal of session `id` to the worker for a reflow.
    fn lend_reflow(
        &mut self,
        id: SessionId,
        term: Box<Term>,
        (target, budget): (ReflowTarget, Duration),
    ) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let cancel = Arc::new(AtomicBool::new(false));
        session.reflow_cancel = Arc::clone(&cancel);
        session.reflow_budget = budget;
        self.submit(Job::Reflow(Box::new(ReflowJob { session: id, term, target, cancel, budget })));
    }

    /// Every view of a resized session gets a fresh snapshot at the size applied.
    fn resized(&mut self, id: SessionId) {
        for client in self.clients.values_mut() {
            if let Some(view) = client.views.get_mut(&id) {
                view.snapshot.get_or_insert(0);
            }
        }
        self.schedule_frame(id);
    }

    /// Acts on everything the worker finished.
    pub(super) fn drain_worker(&mut self) {
        while let Ok(done) = self.worker.done.try_recv() {
            match done {
                Done::Sealed { session, blob } => self.sealed(session, blob),
                Done::Returned { session, term, reflowed } => {
                    self.returned(session, term, reflowed);
                }
                Done::Searched { session, client, found, cancel } => {
                    self.searched(session, client, found, &cancel);
                }
                Done::Announced(message) => self.announced(*message),
            }
        }
    }

    fn sealed(
        &mut self,
        id: SessionId,
        blob: Result<SealedBlob, ferrum_scrollback::ScrollbackError>,
    ) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let blob = match blob {
            Ok(blob) => blob,
            Err(error) => {
                tracing::error!(%error, "a history chunk could not be compressed");
                return;
            }
        };
        let Some(term) = session.term.as_deref_mut() else {
            session.sealed_waiting.push(blob);
            return;
        };
        match guarded(|| term.install_sealed_chunk(blob)) {
            Some(Ok(())) => {}
            Some(Err(error)) => tracing::error!(%error, "a sealed chunk did not fit its history"),
            None => self.fail_session(id, FAILED_IN_INSTALL),
        }
    }

    fn returned(&mut self, id: SessionId, term: Option<Box<Term>>, reflowed: Reflowed) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let Some(mut term) = term else {
            return self.fail_session(id, FAILED_IN_REFLOW);
        };
        let waiting = std::mem::take(&mut session.sealed_waiting);
        // A blob whose chunk the reflow rewrote is ignored by the history.
        let installed =
            guarded(|| waiting.into_iter().for_each(|b| drop(term.install_sealed_chunk(b))));
        session.term = Some(term);
        session.readable = true; // output waited while the terminal was away
        if installed.is_none() {
            return self.fail_session(id, FAILED_IN_INSTALL);
        }
        let newer_size = session.reflow_cancel.load(Ordering::Acquire);
        if reflowed == Reflowed::Interrupted && newer_size {
            // The newer size is pending and rewraps everything: no frame for this one.
            tracing::debug!(session = id.0, "a reflow gave way to a newer size");
            return self.start_search(id);
        }
        if reflowed == Reflowed::Interrupted {
            session.reflow_resume = Some(session.reflow_budget.saturating_mul(2));
        }
        self.resized(id);
        self.start_search(id);
    }

    fn searched(&mut self, id: SessionId, client: u64, found: Option<Found>, cancel: &AtomicBool) {
        if cancel.load(Ordering::Acquire) || !self.sessions.contains_key(&id) {
            return; // a newer search replaced it, or the session is gone
        }
        // A search that panicked found nothing; the session itself is untouched.
        self.send_search_results(client, id, found.unwrap_or_default());
    }
}

fn pty_size(size: Size) -> Option<PtySize> {
    let cols = u16::try_from(size.cols()).ok().and_then(std::num::NonZeroU16::new)?;
    let rows = u16::try_from(size.rows()).ok().and_then(std::num::NonZeroU16::new)?;
    Some(PtySize { cols, rows })
}
