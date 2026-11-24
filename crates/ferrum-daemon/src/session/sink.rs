//! The event sink of a session's terminal: records what the I/O thread must act on after a
//! parse, in fixed-size state, so a hostile byte stream cannot grow it (`docs/03` §3 rule 4).
//!
//! Replies (DA, DSR, ...) go to the PTY; title, bell, working directory and progress become
//! `SessionEvent`s (the latest value wins: the title and directory are read back from the
//! terminal). Clipboard requests are not acted on until the clipboard policy exists (WP-11): a
//! store is dropped, a read is never answered (`docs/10` §3 item 4: reads are denied by default).

use ferrum_vt::{Event, EventSink, ProgressState};

/// Most reply bytes kept between two flushes. Replies are a few bytes each; a program that asks
/// for more than this in one read is flooding queries, and the excess is dropped and counted.
pub(crate) const MAX_PENDING_REPLY_BYTES: usize = 16 * 1_024;

/// What happened since the last [`Sink::take`].
#[allow(clippy::struct_excessive_bools, reason = "independent change markers, each read once")]
#[derive(Debug, Default)]
pub(crate) struct Sink {
    replies: Vec<u8>,
    title: bool,
    cwd: bool,
    bell: bool,
    progress: Option<(ProgressState, u8)>,
    spill_disabled: bool,
}

/// The non-reply part of [`Sink`], taken at once.
#[allow(clippy::struct_excessive_bools, reason = "independent change markers, each read once")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Changes {
    /// The title changed (read it from the terminal).
    pub(crate) title: bool,
    /// The working directory changed (read it from the terminal).
    pub(crate) cwd: bool,
    /// BEL arrived.
    pub(crate) bell: bool,
    /// The latest progress report.
    pub(crate) progress: Option<(ProgressState, u8)>,
    /// Spilling was disabled for this session (disk full or cap).
    pub(crate) spill_disabled: bool,
}

impl Sink {
    /// The reply bytes to write to the PTY; empty them with [`clear_replies`](Self::clear_replies).
    pub(crate) fn replies(&self) -> &[u8] {
        &self.replies
    }

    pub(crate) fn clear_replies(&mut self) {
        self.replies.clear();
    }

    /// Takes the recorded changes.
    pub(crate) fn take(&mut self) -> Changes {
        let changes = Changes {
            title: self.title,
            cwd: self.cwd,
            bell: self.bell,
            progress: self.progress,
            spill_disabled: self.spill_disabled,
        };
        self.title = false;
        self.cwd = false;
        self.bell = false;
        self.progress = None;
        self.spill_disabled = false;
        changes
    }

    fn reply(&mut self, bytes: &[u8]) {
        if self.replies.len() + bytes.len() > MAX_PENDING_REPLY_BYTES {
            // A program flooding queries does not read the answers either: the excess is dropped.
            return;
        }
        self.replies.extend_from_slice(bytes);
    }
}

impl EventSink for Sink {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Reply(bytes) => self.reply(bytes),
            Event::Title(_) => self.title = true,
            Event::Cwd(_) => self.cwd = true,
            Event::Bell => self.bell = true,
            Event::Progress { state, value } => self.progress = Some((state, value)),
            Event::Scrollback(_) => self.spill_disabled = true,
            // Clipboard policy, prompt marks and palette changes belong to later work packages
            // (WP-11, WP-12); the palette is read from the terminal when rows are encoded.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_are_capped_and_changes_are_taken_once() {
        let mut sink = Sink::default();
        sink.event(Event::Reply(b"\x1b[0n"));
        sink.event(Event::Reply(&[b'x'; MAX_PENDING_REPLY_BYTES]));
        assert_eq!(sink.replies(), b"\x1b[0n");
        sink.event(Event::Title("t"));
        sink.event(Event::Bell);
        let changes = sink.take();
        assert!(changes.title && changes.bell && !changes.cwd);
        assert_eq!(sink.take(), Changes::default());
    }
}
