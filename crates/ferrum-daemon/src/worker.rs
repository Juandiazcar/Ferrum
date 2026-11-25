//! The worker thread (`docs/02` §5): CPU-heavy and blocking work off the I/O thread, through
//! bounded channels.
//!
//! - **Seal**: LZ4 compression of a full history chunk (maintainer decisions Q-21/Q-26); the
//!   pending chunk is an immutable handle, the sealed blob goes back to the session.
//! - **Reflow**: the session's terminal is lent (moved) to the worker and given back; the reflow
//!   stops at a chunk boundary when a newer size cancels it (`docs/05` §5) or when its time budget
//!   is spent, so the session's output is read before it resumes (`docs/02` §3.2).
//! - **Search**: compiles the query and searches a history snapshot and a screen copy, while the
//!   session runs on.
//! - **Close**: ends a session's processes (`PtyChild::close` can take a moment on old `ConPTY`
//!   builds).
//! - **Write**: atomic writes of project files (`docs/02` §8).
//!
//! Reflow and search run under `catch_unwind`, so a bug fails that session only. Results come
//! back on a bounded channel and wake the I/O thread through its `mio::Waker`. The worker may
//! block on that channel; the I/O thread never blocks on the worker (it uses `try_send` and keeps
//! what did not fit), so they cannot deadlock.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ferrum_proto::{DaemonMessage, SearchOptions, SessionId};
use ferrum_pty::PtyChild;
use ferrum_scrollback::{ChunkSealer, PendingChunk, ScrollbackError, SealedBlob};
use ferrum_vt::{Reflowed, Size};

use crate::encode::Term;
use crate::projects::Write;
use crate::search::{Found, Matcher, SearchInput, search};

/// Jobs waiting in the channel. The worker drains it continuously; what does not fit waits in the
/// I/O thread's backlog, whose size is bounded by the sessions and the mutation cap
/// (`server.rs`), so this only sets how often the I/O thread retries.
const JOB_QUEUE: usize = 64;
/// Results waiting for the I/O thread; the worker waits when it is full.
const RESULT_QUEUE: usize = 64;
/// Stack of the worker: LZ4 and reflow keep their buffers on the heap (`docs/03` §3 rule 9).
const STACK_BYTES: usize = 512 * 1_024;

/// A query as a client sent it; the worker compiles it (`Matcher::new`), never the I/O thread.
pub(crate) struct Query {
    pub(crate) text: String,
    pub(crate) options: SearchOptions,
}

/// A search for the worker.
pub(crate) struct SearchJob {
    pub(crate) session: SessionId,
    pub(crate) client: u64,
    pub(crate) input: SearchInput,
    pub(crate) query: Query,
    pub(crate) cancel: Arc<AtomicBool>,
}

/// What a reflow does: a new size, or the rest of a reflow that yielded at the current size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReflowTarget {
    Resize(Size),
    Resume,
}

/// A reflow for the worker: it stops at a chunk boundary when `cancel` is set (a newer size) or
/// when `budget` is spent (`docs/02` §3.2: a time budget and yield), and gives the terminal
/// back either way.
pub(crate) struct ReflowJob {
    pub(crate) session: SessionId,
    pub(crate) term: Box<Term>,
    pub(crate) target: ReflowTarget,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) budget: Duration,
}

/// Work for the worker.
pub(crate) enum Job {
    Seal {
        session: SessionId,
        chunk: PendingChunk,
    },
    Reflow(Box<ReflowJob>),
    Search(Box<SearchJob>),
    Close(Box<dyn PtyChild>),
    Write(Write),
    /// A message to broadcast once every job queued before it is done (an echo of a mutation
    /// goes out only after its project file is on disk).
    Announce(Box<DaemonMessage>),
}

/// A finished job the I/O thread must act on.
pub(crate) enum Done {
    Sealed {
        session: SessionId,
        blob: Result<SealedBlob, ScrollbackError>,
    },
    /// A lent terminal comes back; `None` when the reflow panicked.
    Returned {
        session: SessionId,
        term: Option<Box<Term>>,
        reflowed: Reflowed,
    },
    /// A search result; `None` when the search panicked.
    Searched {
        session: SessionId,
        client: u64,
        found: Option<Found>,
        cancel: Arc<AtomicBool>,
    },
    Announced(Box<DaemonMessage>),
}

/// The I/O thread's end of the worker.
pub(crate) struct Worker {
    jobs: Option<SyncSender<Job>>,
    pub(crate) done: Receiver<Done>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// Starts the worker; `wake` is called after every result.
    pub(crate) fn start(wake: Box<dyn Fn() + Send>) -> std::io::Result<Self> {
        let (jobs, inbox) = sync_channel(JOB_QUEUE);
        let (outbox, done) = sync_channel(RESULT_QUEUE);
        let thread = std::thread::Builder::new()
            .name("daemon-worker".into())
            .stack_size(STACK_BYTES)
            .spawn(move || run(&inbox, &outbox, &*wake))?;
        Ok(Self { jobs: Some(jobs), done, thread: Some(thread) })
    }

    /// Queues `job` without blocking; gives it back when the queue is full.
    pub(crate) fn try_send(&self, job: Job) -> Result<(), Job> {
        let Some(jobs) = &self.jobs else { return Err(job) };
        jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(job) | TrySendError::Disconnected(job) => job,
        })
    }

    /// Finishes the queued jobs (saves, closes) and stops the thread.
    pub(crate) fn stop(&mut self) {
        self.jobs = None;
        // Results are no longer wanted; dropping them lets a worker waiting on the channel go on.
        while self.done.try_recv().is_ok() {}
        if let Some(thread) = self.thread.take() {
            let _joined = thread.join(); // a panic was already isolated per job
        }
    }
}

fn run(inbox: &Receiver<Job>, outbox: &SyncSender<Done>, wake: &dyn Fn()) {
    let mut sealer = ChunkSealer::new();
    for job in inbox {
        if let Some(done) = perform(job, &mut sealer) {
            if outbox.send(done).is_err() {
                return; // the I/O thread stopped
            }
            wake();
        }
    }
}

fn perform(job: Job, sealer: &mut ChunkSealer) -> Option<Done> {
    match job {
        Job::Seal { session, chunk } => Some(Done::Sealed { session, blob: sealer.seal(chunk) }),
        Job::Reflow(job) => Some(reflow(*job)),
        Job::Search(job) => {
            let SearchJob { session, client, input, query, cancel } = *job;
            let _span = tracing::debug_span!("search", session = session.0).entered();
            // An invalid or too large expression matches nothing.
            let found = catch_unwind(AssertUnwindSafe(|| {
                Matcher::new(&query.text, query.options)
                    .map(|matcher| search(&input, &matcher, &cancel))
                    .unwrap_or_default()
            }));
            if found.is_err() {
                tracing::error!(session = session.0, "a search panicked");
            }
            Some(Done::Searched { session, client, found: found.ok(), cancel })
        }
        Job::Announce(message) => Some(Done::Announced(message)),
        Job::Close(child) => {
            close(child);
            None
        }
        Job::Write(Write { path, text }) => {
            if let Err(error) = ferrum_config::store::write_atomic(&path, text.as_bytes()) {
                tracing::error!(kind = error.kind(), "a project file could not be saved");
            }
            None
        }
    }
}

/// Reflows a lent terminal until it is done, `cancel` is set or the budget is spent.
fn reflow(job: ReflowJob) -> Done {
    let ReflowJob { session, mut term, target, cancel, budget } = job;
    let _span = tracing::debug_span!("reflow", session = session.0).entered();
    let started = Instant::now();
    let mut stop = || cancel.load(Ordering::Acquire) || started.elapsed() >= budget;
    let reflowed = catch_unwind(AssertUnwindSafe(|| match target {
        ReflowTarget::Resize(size) => term.resize_until(size, &mut stop).map(|(_, done)| done),
        ReflowTarget::Resume => term.reflow_until(&mut stop),
    }));
    match reflowed {
        Ok(Ok(reflowed)) => Done::Returned { session, term: Some(term), reflowed },
        Ok(Err(error)) => {
            tracing::warn!(%error, "part of a session's history was lost in a reflow");
            Done::Returned { session, term: Some(term), reflowed: Reflowed::Complete }
        }
        Err(_) => {
            tracing::error!(session = session.0, "a reflow panicked; the session fails");
            Done::Returned { session, term: None, reflowed: Reflowed::Complete }
        }
    }
}

/// Ends a session's processes.
pub(crate) fn close(child: Box<dyn PtyChild>) {
    if let Err(error) = child.close() {
        tracing::warn!(%error, "a session could not be closed cleanly");
    }
}

#[cfg(test)]
mod tests {
    use ferrum_scrollback::{Scrollback, ScrollbackConfig};
    use ferrum_vt::Terminal;

    use super::*;
    use crate::session::sink::Sink;

    /// A terminal with enough history that a reflow passes several chunk boundaries.
    fn long_history() -> Box<Term> {
        let config = ScrollbackConfig::new(10_000, 50_000).unwrap_or_default();
        let mut term = Terminal::new(Size::new(80, 24), Scrollback::new(config), Sink::default());
        for index in 0..5_000 {
            term.parse(format!("{index:05} {}\r\n", "word ".repeat(12)).as_bytes());
        }
        Box::new(term)
    }

    fn job(target: ReflowTarget, cancel: bool, budget: Duration) -> ReflowJob {
        let cancel = Arc::new(AtomicBool::new(cancel));
        ReflowJob { session: SessionId(1), term: long_history(), target, cancel, budget }
    }

    fn outcome(done: Done) -> (Box<Term>, Reflowed) {
        let Done::Returned { term: Some(term), reflowed, .. } = done else {
            unreachable!("a reflow gives the terminal back");
        };
        (term, reflowed)
    }

    #[test]
    fn a_newer_size_stops_a_reflow_and_the_terminal_comes_back() {
        let resize = ReflowTarget::Resize(Size::new(40, 24));
        let (term, reflowed) = outcome(reflow(job(resize, true, Duration::MAX)));
        assert_eq!(reflowed, Reflowed::Interrupted, "the reflow stopped at the first chunk");
        assert_eq!(term.size(), Size::new(40, 24), "the screen has the size asked for");
    }

    #[test]
    fn a_spent_budget_yields_and_resuming_finishes_the_reflow() {
        let resize = ReflowTarget::Resize(Size::new(40, 24));
        let (term, reflowed) = outcome(reflow(job(resize, false, Duration::ZERO)));
        assert_eq!(reflowed, Reflowed::Interrupted, "a spent budget gives the terminal back");
        let cancel = Arc::new(AtomicBool::new(false));
        let target = ReflowTarget::Resume;
        let resumed =
            ReflowJob { session: SessionId(1), term, target, cancel, budget: Duration::MAX };
        let (_, reflowed) = outcome(reflow(resumed));
        assert_eq!(reflowed, Reflowed::Complete);
    }
}
