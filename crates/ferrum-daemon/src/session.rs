//! One session: its terminal state, its PTY and its lifecycle (FR-SESS-006).
//!
//! The I/O thread owns every session (`docs/02` §5). Output is read in chunks of at most
//! [`READ_CHUNK_BYTES`] into one buffer shared by all sessions and parsed at once, at most
//! [`FAIRNESS_QUANTUM_BYTES`] and [`FAIRNESS_READS_PER_TURN`] reads per session per loop turn, so
//! a flooding session cannot starve the others (FR-SESS-009, NFR-PERF-04). Parsing runs under
//! `catch_unwind`: a bug in one session's terminal marks that session failed and leaves every
//! other session running (`docs/02` §10, NFR-REL-01). The terminal is lent to the worker only for
//! a large reflow; meanwhile its output is not read, which is backpressure on that PTY only.

pub(crate) mod sink;
pub(crate) mod spawn;
pub(crate) mod writer;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ferrum_proto::{ProjectId, SessionEvent, SessionId, SessionStatus, Text};
use ferrum_pty::{ExitStatus, PtyChild, PtyReader, PtyResize, PtySession};
use ferrum_vt::Size;

use crate::encode::Term;
use writer::Input;

/// Largest read from a PTY (`docs/02` §5: chunks of at most 64 KiB).
pub(crate) const READ_CHUNK_BYTES: usize = 64 * 1_024;
/// Most output parsed for one session per loop turn (`docs/02` §5: "e.g. ≤ 256 KiB per session
/// per loop turn"): one full read. Every other session and every client is served between two
/// quanta of a flood, so their latency is bounded by parsing 64 KiB (NFR-PERF-04).
pub(crate) const FAIRNESS_QUANTUM_BYTES: usize = READ_CHUNK_BYTES;
/// Most reads per session per loop turn. A Unix PTY hands a flood over in chunks of about 1 KiB
/// (ADR 0026 item 4), so the byte quantum alone would allow some 64 reads, each with its own
/// system calls, before the other sessions are served; 8 keeps a turn short (NFR-PERF-04) while a
/// large chunk still fills the byte quantum in one read.
pub(crate) const FAIRNESS_READS_PER_TURN: usize = 8;

/// Most events held before a session's `Created` went out: one per kind of `SessionEvent`
/// (twelve kinds today), so a hold never has to drop one.
pub(crate) const MAX_HELD_EVENTS: usize = 16;

/// What runs in a session (`docs/02` §9: `Terminal` now, agent sessions in Phase 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum SessionKind {
    /// A shell or program in a PTY.
    Terminal,
}

impl SessionKind {
    /// The daemon's kind of a requested kind; `None` for a kind this daemon cannot run.
    pub(crate) fn from_proto(kind: ferrum_proto::SessionKind) -> Option<Self> {
        match kind {
            ferrum_proto::SessionKind::Terminal => Some(Self::Terminal),
            #[allow(unreachable_patterns, reason = "the protocol enum is non_exhaustive")]
            _ => None,
        }
    }

    /// The protocol form.
    pub(crate) fn to_proto(self) -> ferrum_proto::SessionKind {
        match self {
            Self::Terminal => ferrum_proto::SessionKind::Terminal,
        }
    }
}

/// The PTY side of a running (or exited, not yet drained) session.
pub(crate) struct Pty {
    pub(crate) reader: Box<dyn PtyReader>,
    pub(crate) resize: Box<dyn PtyResize>,
    /// `None` once handed to the worker to be closed.
    pub(crate) child: Option<Box<dyn PtyChild>>,
    /// `None` once the child exited.
    pub(crate) input: Option<Input>,
    /// The loop waits for room in the PTY (input is waiting).
    pub(crate) watching_room: bool,
}

impl Pty {
    /// Takes the parts of a spawned session.
    pub(crate) fn new(session: PtySession) -> Self {
        Self {
            reader: session.reader,
            resize: session.resize,
            child: Some(session.child),
            input: Some(Input::new(session.writer)),
            watching_room: false,
        }
    }
}

/// What one turn of reading did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pump {
    /// Nothing more to read now.
    Drained,
    /// The quantum was used up; more output may be waiting.
    More,
    /// The output ended (the session was closed).
    Ended,
    /// Parsing panicked; the session must be failed.
    Panicked,
}

/// Flags set from the backend's threads (`Readiness::Notify`, Windows).
#[derive(Debug, Default)]
pub(crate) struct Notified {
    /// Output is waiting.
    pub(crate) output: AtomicBool,
    /// The PTY takes input again.
    pub(crate) input: AtomicBool,
}

/// One session.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent markers of one session, each set and read in one place"
)]
pub(crate) struct Session {
    pub(crate) id: SessionId,
    pub(crate) project: ProjectId,
    pub(crate) kind: SessionKind,
    /// `None` while lent to the worker.
    pub(crate) term: Option<Box<Term>>,
    pub(crate) pty: Option<Pty>,
    pub(crate) status: SessionStatus,
    /// Clients were told about the session (`Created` went out): its other events wait for it.
    pub(crate) announced: bool,
    /// Its `Created` is on the bus, not yet delivered.
    pub(crate) created_queued: bool,
    /// Events that happened before `Created` went out, delivered right after it; one of each kind
    /// (the latest wins), so at most [`MAX_HELD_EVENTS`].
    pub(crate) held: Vec<SessionEvent>,
    /// Output may be waiting (a readiness event or a notification arrived, or the last turn
    /// stopped at the quantum).
    pub(crate) readable: bool,
    /// The PTY reported room for input.
    pub(crate) writable: bool,
    pub(crate) notified: Arc<Notified>,
    /// An `Activity` event was sent since the session was last attached.
    pub(crate) activity_sent: bool,
    /// A resize waiting for the terminal (lent to the worker) or not applied yet.
    pub(crate) pending_size: Option<Size>,
    /// Stops the reflow running on the worker when a newer size arrives.
    pub(crate) reflow_cancel: Arc<AtomicBool>,
    /// The time budget of the reflow lent to the worker.
    pub(crate) reflow_budget: Duration,
    /// A reflow yielded at its time budget and continues with this (doubled) one.
    pub(crate) reflow_resume: Option<Duration>,
    /// When this session's next damage frame is due, and when its last one was sent: damage is
    /// paced per session, so a busy session never delays another one's first frame.
    pub(crate) frame_due: Option<Instant>,
    pub(crate) last_frame: Option<Instant>,
    /// Since when synchronized output (mode 2026) holds the damage back.
    pub(crate) sync_since: Option<Instant>,
    /// A search waiting for the terminal (lent to a reflow): the client that asked, the matcher.
    pub(crate) pending_search: Option<(u64, crate::worker::Query)>,
    /// Cancels the search running on the worker when a newer one is asked for.
    pub(crate) search_cancel: Arc<AtomicBool>,
    /// Chunks sealed while the terminal was lent, installed when it comes back.
    pub(crate) sealed_waiting: Vec<ferrum_scrollback::SealedBlob>,
}

/// What a new session is made of.
pub(crate) struct Parts {
    pub(crate) id: SessionId,
    pub(crate) project: ProjectId,
    pub(crate) kind: SessionKind,
    pub(crate) term: Box<Term>,
}

impl Session {
    /// A session around a spawned PTY, or, without one, a session whose process could not be
    /// started (shown as exited, ready to restart).
    pub(crate) fn new(parts: Parts, pty: Option<Pty>) -> Self {
        let status = if pty.is_some() {
            SessionStatus::Running
        } else {
            SessionStatus::Exited { code: None }
        };
        Self {
            id: parts.id,
            project: parts.project,
            kind: parts.kind,
            term: Some(parts.term),
            pty,
            status,
            announced: false,
            created_queued: false,
            held: Vec::new(),
            readable: true,
            writable: false,
            notified: Arc::new(Notified::default()),
            activity_sent: false,
            pending_size: None,
            reflow_cancel: Arc::new(AtomicBool::new(false)),
            reflow_budget: Duration::ZERO,
            reflow_resume: None,
            pending_search: None,
            sync_since: None,
            frame_due: None,
            last_frame: None,
            search_cancel: Arc::new(AtomicBool::new(false)),
            sealed_waiting: Vec::new(),
        }
    }

    /// Moves the notifications from the backend's threads into `readable` and `writable`.
    pub(crate) fn take_notifications(&mut self) {
        if self.notified.output.swap(false, Ordering::AcqRel) {
            self.readable = true;
        }
        if self.notified.input.swap(false, Ordering::AcqRel) {
            self.writable = true;
        }
    }

    /// Reads and parses output until nothing is ready or the quantum is used up.
    pub(crate) fn pump(&mut self, buf: &mut [u8]) -> Pump {
        let (Some(term), Some(pty)) = (self.term.as_deref_mut(), self.pty.as_mut()) else {
            return Pump::Drained;
        };
        let mut parsed = 0;
        for _ in 0..FAIRNESS_READS_PER_TURN {
            if parsed >= FAIRNESS_QUANTUM_BYTES {
                break;
            }
            let read = match pty.reader.try_read(buf) {
                Ok(Some(0)) => return Pump::Ended,
                Ok(Some(read)) => read,
                Ok(None) => {
                    self.readable = false;
                    return Pump::Drained;
                }
                Err(error) => {
                    tracing::warn!(session = self.id.0, %error, "PTY read failed");
                    return Pump::Ended;
                }
            };
            let chunk = buf.get(..read).unwrap_or_default();
            // The terminal is left as the panic found it; the session is failed and never parses
            // again, so the broken state cannot be observed.
            if catch_unwind(AssertUnwindSafe(|| term.parse(chunk))).is_err() {
                return Pump::Panicked;
            }
            parsed += read;
        }
        Pump::More
    }

    /// Records the exit status of the child.
    pub(crate) fn exited(&mut self, status: ExitStatus) {
        self.status = match status {
            // Windows codes such as 0xC000013A keep their bits, as the OS tools show them.
            ExitStatus::Exited(code) => SessionStatus::Exited { code: Some(code.cast_signed()) },
            _ => SessionStatus::Killed,
        };
    }

    /// Keeps `event` until the session's `Created` went out: a newer event of the same kind
    /// replaces the older one, so nothing is lost but superseded values and the list stays
    /// bounded.
    pub(crate) fn hold(&mut self, event: SessionEvent) {
        let kind = std::mem::discriminant(&event);
        if let Some(older) = self.held.iter_mut().find(|held| std::mem::discriminant(*held) == kind)
        {
            *older = event;
        } else if self.held.len() < MAX_HELD_EVENTS {
            self.held.push(event);
        }
    }

    /// Whether the session was failed after an internal error.
    pub(crate) fn has_failed(&self) -> bool {
        matches!(self.status, SessionStatus::Failed { .. })
    }
}

/// The status of a session failed after an internal error (fixed words, never content).
pub(crate) fn failed_status(error: &str) -> SessionStatus {
    SessionStatus::Failed { error: Text::new(error) }
}

/// Runs `work` on a session's state at the session boundary (`docs/02` §10): a panic is caught
/// and reported as `None`, after which the caller fails that session only.
pub(crate) fn guarded<R>(work: impl FnOnce() -> R) -> Option<R> {
    catch_unwind(AssertUnwindSafe(work)).ok()
}

#[cfg(test)]
mod tests {
    use ferrum_scrollback::{Scrollback, ScrollbackConfig};
    use ferrum_vt::Terminal;

    use super::*;

    #[test]
    fn held_events_keep_the_latest_of_each_kind_in_order() {
        let scrollback = Scrollback::new(ScrollbackConfig::default());
        let term = Box::new(Terminal::new(Size::default(), scrollback, sink::Sink::default()));
        let parts = Parts {
            id: SessionId(1),
            project: ProjectId(ferrum_project::ProjectId::new_random().0),
            kind: SessionKind::Terminal,
            term,
        };
        let mut session = Session::new(parts, None);
        session.hold(SessionEvent::Title(Text::new("a")));
        session.hold(SessionEvent::CwdChanged(Text::new("file:///x")));
        session.hold(SessionEvent::Title(Text::new("b")));
        let expected =
            [SessionEvent::Title(Text::new("b")), SessionEvent::CwdChanged(Text::new("file:///x"))];
        assert_eq!(session.held, expected);
    }
}
