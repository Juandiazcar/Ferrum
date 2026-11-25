//! The messages that carry a session's rows (`docs/02` §4): a `Snapshot` with the screen rows
//! that fit, `Rows` for the rest and for history, and `Damage` for the rows a view missed.
//!
//! A block is sent once it holds 128 KiB (or when the next row does not fit a
//! frame), which keeps every message small enough for the client buffers (`client.rs`). Its
//! buffer comes back after the message is encoded, so a frame allocates nothing once the buffers
//! have grown (`docs/03` §3 rule 8, `tests/alloc_frame.rs`).

use std::hash::{BuildHasher, Hash, Hasher, RandomState};

use ferrum_proto::{
    Cursor, DaemonMessage, Damage, ProtoError, RowBlock, RowRegion, SessionId, Snapshot,
    TerminalModes, Text, Viewport,
};
use ferrum_scrollback::LineCursor;
use ferrum_vt::{CellFlags, EventSink, Terminal};

use crate::encode::{self, HistoryRow, RowScratch, clamp_u16};
use crate::view::{View, best_shift, predicted};

/// Bytes of rows after which a block is sent: a full 200×55 screen of plain text is about
/// 20 KiB, so damage of a whole screen is one message.
const BLOCK_FLUSH_BYTES: usize = 128 * 1_024;
/// Bytes of the longest UTF-8 scalar.
const UTF8_MAX_BYTES: usize = 4;

/// Where the messages of a frame go (a client's send).
pub type Outbox<'a> = &'a mut dyn FnMut(&DaemonMessage);

/// Reusable encoding buffers, shared by every view.
#[derive(Default)]
pub struct FrameEncoder {
    block: RowBlock,
    scratch: RowScratch,
    cursor: LineCursor,
    hashes: Vec<u64>,
    hasher: RandomState,
    /// The first message of the current snapshot or frame went out.
    opened: bool,
}

/// The fixed part of the `Damage` messages of one frame.
#[derive(Clone, Copy)]
struct DamageHead {
    id: SessionId,
    cursor: Cursor,
    modes: Option<TerminalModes>,
    scroll_delta: i32,
}

/// Which message a block goes out as.
enum Out<'a, S: EventSink> {
    Snapshot { term: &'a Terminal<S>, id: SessionId, history: u16 },
    Rows { id: SessionId, region: RowRegion },
    Damage(DamageHead),
}

/// One row to append to a block.
type PushRow<'a> =
    &'a mut dyn FnMut(&mut RowBlock, &mut RowScratch, &mut LineCursor) -> Result<(), ProtoError>;

impl FrameEncoder {
    /// Sends the block as `out` (`has_more` when rows follow) and takes its buffer back. Only
    /// the first message of a snapshot is a `Snapshot`, only the first `Damage` of a frame
    /// carries the scroll.
    fn emit<S: EventSink>(&mut self, out: &Out<'_, S>, has_more: bool, send: Outbox<'_>) {
        let rows = std::mem::take(&mut self.block);
        let first = !std::mem::replace(&mut self.opened, true);
        let message = match *out {
            Out::Snapshot { term, id, history } if first => {
                DaemonMessage::Snapshot(snapshot_header(term, (id, history), rows, has_more))
            }
            Out::Snapshot { id, .. } => {
                DaemonMessage::Rows { session_id: id, region: RowRegion::Screen, rows }
            }
            Out::Rows { id, region } => DaemonMessage::Rows { session_id: id, region, rows },
            Out::Damage(head) => DaemonMessage::Damage(Damage {
                session_id: head.id,
                rows,
                cursor: head.cursor,
                scroll_delta: if first { head.scroll_delta } else { 0 },
                modes: head.modes,
                has_more,
            }),
        };
        send(&message);
        self.block = take_block(message);
    }

    /// Appends one row, sending the block first when it is full or the row does not fit.
    fn push<S: EventSink>(&mut self, out: &Out<'_, S>, push: PushRow<'_>, send: Outbox<'_>) {
        if self.block.len_bytes() >= BLOCK_FLUSH_BYTES {
            self.emit(out, true, send);
        }
        let mut result = push(&mut self.block, &mut self.scratch, &mut self.cursor);
        if matches!(result, Err(ProtoError::BlockFull)) && !self.block.is_empty() {
            self.emit(out, true, send);
            result = push(&mut self.block, &mut self.scratch, &mut self.cursor);
        }
        if let Err(error) = result {
            tracing::warn!(%error, "a row could not be encoded");
        }
    }

    /// The snapshot of `term` announcing up to `history` history rows, which `view` then owes
    /// (sent by [`history_rows`](Self::history_rows) a step at a time); `view` holds what the
    /// client has.
    pub fn snapshot<S: EventSink>(
        &mut self,
        term: &Terminal<S>,
        (id, history): (SessionId, u16),
        view: &mut View,
        send: Outbox<'_>,
    ) {
        let history = history.min(clamp_u16(term.scrollback().len()));
        let out = Out::Snapshot { term, id, history };
        self.block.clear();
        self.opened = false;
        for y in 0..term.grid().lines() {
            let mut row = |b: &mut RowBlock, s: &mut RowScratch, _: &mut LineCursor| {
                encode::push_screen_row(b, term, y, s)
            };
            self.push(&out, &mut row, send);
        }
        self.emit(&out, false, send);
        view.history = (history > 0).then_some(0..u32::from(history));
        view.dirty.clear();
        view.cursor = Some(encode::cursor(term));
        view.modes = Some(encode::modes(term));
        self.hash_rows(term, view, &mut |_| true);
        std::mem::swap(&mut view.hashes, &mut self.hashes);
    }

    /// History rows `range` (0 is the row just above the screen, counting up) as `Rows`.
    pub fn history_rows<S: EventSink>(
        &mut self,
        term: &Terminal<S>,
        id: SessionId,
        range: std::ops::Range<u32>,
        send: Outbox<'_>,
    ) {
        let out = Out::<S>::Rows { id, region: RowRegion::History };
        let len = term.scrollback().len();
        self.block.clear();
        for y in range {
            let Some(index) = usize::try_from(y).ok().and_then(|y| len.checked_sub(y + 1)) else {
                break;
            };
            let y = clamp_u16(usize::try_from(y).unwrap_or(usize::MAX));
            let mut row = |b: &mut RowBlock, s: &mut RowScratch, cursor: &mut LineCursor| {
                match term.scrollback().read_line(index, cursor) {
                    Ok(Some(line)) => encode::push_history_row(b, &HistoryRow { y, line }, term, s),
                    _ => Ok(()), // a line that cannot be read is left out
                }
            };
            self.push(&out, &mut row, send);
        }
        if !self.block.is_empty() {
            self.emit(&out, false, send);
        }
    }

    /// The rows `view` misses, as `Damage`; nothing when nothing changed. A scroll is sent as
    /// `scroll_delta` and the rows it brings are not sent again.
    pub fn damage<S: EventSink>(
        &mut self,
        term: &Terminal<S>,
        id: SessionId,
        view: &mut View,
        send: Outbox<'_>,
    ) {
        let cursor = encode::cursor(term);
        let modes = encode::modes(term);
        if view.dirty.is_clean() && view.cursor == Some(cursor) && view.modes == Some(modes) {
            return;
        }
        let known = view.hashes.len() == term.grid().lines();
        let seen: &View = view;
        self.hash_rows(term, seen, &mut |y| !known || seen.dirty.is_dirty(y));
        let scroll_delta =
            if known { best_shift(&view.hashes, &self.hashes, view.dirty.scrolled()) } else { 0 };
        let changed = (view.modes != Some(modes)).then_some(modes);
        let head = DamageHead { id, cursor, modes: changed, scroll_delta };
        self.damage_rows(term, head, &view.hashes, send);
        view.cursor = Some(cursor);
        view.modes = Some(modes);
        view.dirty.clear();
        std::mem::swap(&mut view.hashes, &mut self.hashes);
    }

    /// Sends the rows that differ from the client's rows `before` shifted by the scroll.
    fn damage_rows<S: EventSink>(
        &mut self,
        term: &Terminal<S>,
        head: DamageHead,
        before: &[u64],
        send: Outbox<'_>,
    ) {
        let known = before.len() == self.hashes.len();
        let out = Out::<S>::Damage(head);
        self.block.clear();
        self.opened = false;
        for y in 0..self.hashes.len() {
            if known && predicted(before, y, head.scroll_delta) == self.hashes.get(y) {
                continue;
            }
            let mut row = |b: &mut RowBlock, s: &mut RowScratch, _: &mut LineCursor| {
                encode::push_screen_row(b, term, y, s)
            };
            self.push(&out, &mut row, send);
        }
        self.emit(&out, false, send);
    }

    /// Fills `self.hashes` with the screen rows: rows `fresh` says are new are hashed, the others
    /// are the client's (`view.hashes`).
    fn hash_rows<S: EventSink>(
        &mut self,
        term: &Terminal<S>,
        view: &View,
        fresh: &mut dyn FnMut(usize) -> bool,
    ) {
        self.hashes.clear();
        for y in 0..term.grid().lines() {
            let hash = match view.hashes.get(y) {
                Some(&hash) if !fresh(y) => hash,
                _ => row_hash(&self.hasher, term, y),
            };
            self.hashes.push(hash);
        }
    }
}

/// The header of a snapshot around its first block.
fn snapshot_header<S: EventSink>(
    term: &Terminal<S>,
    (id, history): (SessionId, u16),
    rows: RowBlock,
    has_more: bool,
) -> Snapshot {
    let grid = term.grid();
    Snapshot {
        session_id: id,
        viewport: Viewport { cols: clamp_u16(grid.cols()), rows: clamp_u16(grid.lines()) },
        cursor: encode::cursor(term),
        modes: encode::modes(term),
        title: Text::new(term.title()),
        rows,
        has_more,
        history_rows: history,
    }
}

/// The row buffer of a sent message, emptied for reuse.
fn take_block(message: DaemonMessage) -> RowBlock {
    let mut block = match message {
        DaemonMessage::Snapshot(snapshot) => snapshot.rows,
        DaemonMessage::Rows { rows, .. } => rows,
        DaemonMessage::Damage(damage) => damage.rows,
        _ => RowBlock::default(),
    };
    block.clear();
    block
}

/// A hash of what screen row `y` shows: the text, style words and width of each cell. The keys
/// are random per process, so content cannot be crafted to collide.
fn row_hash<S: EventSink>(hasher: &RandomState, term: &Terminal<S>, y: usize) -> u64 {
    let grid = term.grid();
    let mut state = hasher.build_hasher();
    let mut utf8 = [0; UTF8_MAX_BYTES];
    if let Some(row) = grid.row(y) {
        row.is_wrapped().hash(&mut state);
        for cell in row.cells().iter().take(row.used_len()) {
            grid.cell_text(cell, &mut utf8).hash(&mut state);
            (cell.fg_word(), cell.bg_word(), cell.attr_word()).hash(&mut state);
            cell.flags().contains(CellFlags::WIDE).hash(&mut state);
        }
    }
    state.finish()
}
