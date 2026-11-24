//! Jobs that did not fit the worker's channel, kept in order for the next turn (`docs/05` §5:
//! bounded queues). The backlog stays bounded: seals, reflows and closes are bounded by the
//! sessions (at most two pending chunks each, one reflow, one close), a newer search of a session
//! replaces its older one, a newer write of a file replaces the older one where it stands (it
//! carries every change before it, so an announcement queued after the older write still follows
//! a write that holds its change), and mutations (announcements) are refused while the backlog
//! holds [`MAX_BACKLOG_JOBS`] jobs (`requests.rs`).

use std::collections::VecDeque;

use super::Daemon;
use crate::worker::Job;

/// Jobs above which mutations are refused: far more than a burst of a person's actions, and a
/// few hundred small jobs at most (a write is one project file of a few KiB).
const MAX_BACKLOG_JOBS: usize = 256;

/// Jobs waiting for room in the worker's channel.
#[derive(Default)]
pub(super) struct Backlog {
    jobs: VecDeque<Job>,
}

impl Backlog {
    pub(super) fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub(super) fn pop(&mut self) -> Option<Job> {
        self.jobs.pop_front()
    }

    /// Keeps `job`, merging it with a job it supersedes.
    fn push(&mut self, job: Job) {
        match job {
            Job::Write(write) => {
                let older = self.jobs.iter_mut().find_map(|queued| match queued {
                    Job::Write(queued) if queued.path == write.path => Some(queued),
                    _ => None,
                });
                match older {
                    Some(older) => older.text = write.text,
                    None => self.jobs.push_back(Job::Write(write)),
                }
            }
            Job::Search(search) => {
                let session = search.session;
                self.jobs
                    .retain(|queued| !matches!(queued, Job::Search(s) if s.session == session));
                self.jobs.push_back(Job::Search(search));
            }
            other => self.jobs.push_back(other),
        }
    }
}

impl Daemon {
    /// Sends a job to the worker, or keeps it, in order, for the next turn.
    pub(super) fn submit(&mut self, job: Job) {
        if !self.backlog.is_empty() {
            return self.backlog.push(job);
        }
        if let Err(job) = self.worker.try_send(job) {
            self.backlog.push(job);
        }
    }

    /// Whether mutations must wait (`MAX_BACKLOG_JOBS`).
    pub(super) fn is_backlog_full(&self) -> bool {
        self.backlog.jobs.len() >= MAX_BACKLOG_JOBS
    }

    pub(super) fn send_backlog(&mut self) {
        while let Some(job) = self.backlog.pop() {
            if let Err(job) = self.worker.try_send(job) {
                self.backlog.jobs.push_front(job);
                break;
            }
        }
    }
}
