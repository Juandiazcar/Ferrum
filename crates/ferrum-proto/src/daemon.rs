//! Messages from the daemon to the UI (`docs/02` §4).
//!
//! Variants are appended, never reordered or removed, within a major protocol version (the golden
//! vectors in `fixtures/proto/` pin their positions). `Welcome` and `Refused` stay variants 0 and
//! 1 in every major (see the handshake module). User content is [`Text`] or a [`RowBlock`], whose
//! `Debug` prints only sizes.
//!
//! # Sequences
//!
//! - **Connect** (`FR-SESS-002`): after `Welcome` the daemon sends its whole state, one message
//!   per item so that no message nests lists inside lists: a `ProjectEvent::Created` for every
//!   project in sidebar order (each with all of its fields, its ordered sessions and its layout),
//!   then a `SessionEvent` with `SessionEvent::Created` for every session, then `Synced`.
//! - **Attach**: a `Snapshot`, the full screen for the viewport (`docs/02` §4): the metadata and
//!   the screen rows that fit its block. Screen rows that did not fit (`has_more`) and up to
//!   `want_scrollback_rows` history rows (`history_rows`) follow in `Rows` messages.
//! - **Damage**: one `Damage` per frame interval; when its rows do not fit one frame
//!   ([`RowBlock::push_row`] returns `BlockFull`), the daemon sends several, all but the last
//!   with `has_more` set and only the first with a `scroll_delta`; the UI paints after the last.

use serde::{Deserialize, Serialize};

use crate::block::RowBlock;
use crate::client::ProjectChange;
use crate::handshake::Refusal;
use crate::list::List;
use crate::redact::{Bytes, Text};
use crate::types::{
    Cursor, Layout, Percent, Point, ProgressState, ProjectId, Rgb, RowRegion, SessionId,
    SessionKind, SessionStatus, TerminalModes, Viewport,
};
use crate::version::ProtocolVersion;

/// A message from the daemon to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DaemonMessage {
    /// The `Hello` was accepted; both sides use `protocol_version` from now on. Variant 0 in every
    /// major version.
    Welcome {
        /// The negotiated version.
        protocol_version: ProtocolVersion,
    },
    /// The `Hello` was refused; the daemon closes the connection. Variant 1 in every major
    /// version.
    Refused(Refusal),
    /// The daemon has sent its whole state after `Welcome` (see the module docs).
    Synced,
    /// The full screen of a session for a viewport, the answer to `Attach`.
    Snapshot(Snapshot),
    /// Rows of a snapshot that did not fit the `Snapshot`, and its history rows.
    Rows {
        /// The session.
        session_id: SessionId,
        /// Screen or history rows.
        region: RowRegion,
        /// The rows.
        rows: RowBlock,
    },
    /// What changed on a visible session since the last frame, coalesced.
    Damage(Damage),
    /// Something happened to a session; sent for every session, visible or not.
    SessionEvent {
        /// The session.
        session_id: SessionId,
        /// What happened.
        event: SessionEvent,
    },
    /// A project changed.
    ProjectEvent(ProjectEvent),
    /// A project's layout changed.
    LayoutEvent {
        /// The project.
        project_id: ProjectId,
        /// The new layout.
        layout: Layout,
    },
    /// Reserved for Phase 2 agent messages (`docs/02` §9); a v1 peer never sends it.
    ReservedAgent(Bytes),
    /// Reserved for Phase 2 usage messages.
    ReservedUsage(Bytes),
    /// Reserved for Phase 2 vault messages.
    ReservedVault(Bytes),
    /// The matches of a `Search` (protocol 1.1, `FR-TERM-015`), in order from the oldest line;
    /// a match spans soft-wrapped rows when the text does.
    SearchResults {
        /// The session.
        session_id: SessionId,
        /// The matches, at most [`MAX_LIST_ITEMS`](crate::MAX_LIST_ITEMS).
        matches: List<SearchMatch>,
        /// More matches exist than the list holds.
        is_truncated: bool,
    },
    /// A configuration or project file could not be used and was set aside; defaults or the
    /// remaining projects were loaded (protocol 1.1, `docs/02` §8 and §10: the UI notifies with
    /// the path).
    FileSetAside {
        /// The file that could not be used.
        path: Text,
        /// Where it was moved.
        backup: Text,
    },
}

/// One match of a search: from `start` to `end` (exclusive column), in the line coordinates of
/// [`Point`] at the time of the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SearchMatch {
    /// First cell.
    pub start: Point,
    /// The cell after the last one, on the line of the last one.
    pub end: Point,
}

/// The full screen of a session for a viewport: its metadata and the screen rows that fit one
/// block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The session.
    pub session_id: SessionId,
    /// The size the rows are laid out at.
    pub viewport: Viewport,
    /// The cursor.
    pub cursor: Cursor,
    /// The modes.
    pub modes: TerminalModes,
    /// The window title.
    pub title: Text,
    /// Screen rows; `y` is the screen row. The rows that do not fit this block follow in `Rows`
    /// messages with `RowRegion::Screen`.
    pub rows: RowBlock,
    /// Screen rows follow in `Rows` messages because they did not fit `rows`.
    pub has_more: bool,
    /// History rows that follow in `Rows` messages with `RowRegion::History` (at most
    /// `want_scrollback_rows`).
    pub history_rows: u16,
}

/// What changed on a visible session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Damage {
    /// The session.
    pub session_id: SessionId,
    /// The changed screen rows; `y` is the screen row.
    pub rows: RowBlock,
    /// The cursor.
    pub cursor: Cursor,
    /// Lines the screen scrolled up (positive) or down (negative) before `rows` apply.
    pub scroll_delta: i32,
    /// The modes, when they changed.
    pub modes: Option<TerminalModes>,
    /// More `Damage` of the same frame follows.
    pub has_more: bool,
}

/// A session as the UI lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// The project it belongs to.
    pub project_id: ProjectId,
    /// What runs in it.
    pub kind: SessionKind,
    /// The name the user gave it, if any (`FR-UI-002`).
    pub name: Option<Text>,
    /// The window title.
    pub title: Text,
    /// Running, exited or killed (`FR-SESS-006`).
    pub status: SessionStatus,
}

/// Something that happened to a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SessionEvent {
    /// The session exists: created now, or listed after `Welcome`.
    Created(SessionInfo),
    /// The session was closed.
    Closed,
    /// The user renamed the session.
    Renamed(Text),
    /// New title.
    Title(Text),
    /// BEL.
    Bell,
    /// New working directory (`OSC 7`), as the URI the shell sent.
    CwdChanged(Text),
    /// The process exited by itself, with its exit code when the OS reported one.
    Exited {
        /// Exit code.
        code: Option<i32>,
    },
    /// The process was killed (`FR-SESS-006`).
    Killed,
    /// Output arrived while the session was not visible.
    Activity,
    /// Progress (`OSC 9;4`).
    Progress {
        /// State.
        state: ProgressState,
        /// Percentage.
        value: Percent,
    },
    /// The daemon stopped the session after an internal error in its terminal state (protocol
    /// 1.1, `docs/02` §10: the failure is isolated to this session; the process was ended).
    Failed {
        /// What failed, in words fixed by the daemon (never terminal content).
        error: Text,
    },
    /// Spilling history to disk stopped for this session (disk full or cap): the oldest lines
    /// are dropped from now on (protocol 1.1, `docs/02` §10: "user notified once").
    SpillDisabled,
}

/// A project with every field of `FR-PROJ-001`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// The project.
    pub project_id: ProjectId,
    /// Name.
    pub name: Text,
    /// Color.
    pub color: Rgb,
    /// Icon glyph.
    pub icon: Option<Text>,
    /// Root directories, 1 or more.
    pub roots: List<Text>,
    /// Its sessions, in order.
    pub sessions: List<SessionId>,
    /// Its layout.
    pub layout: Layout,
}

/// A change to a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProjectEvent {
    /// The project exists: created now, or listed after `Welcome`.
    Created(ProjectInfo),
    /// A project was changed.
    Updated {
        /// The project.
        project_id: ProjectId,
        /// The change.
        change: ProjectChange,
    },
    /// A project moved in the sidebar.
    Reordered {
        /// The project.
        project_id: ProjectId,
        /// New position.
        index: u32,
    },
    /// A session moved, possibly to another project.
    SessionMoved {
        /// The session.
        session_id: SessionId,
        /// The destination project.
        project_id: ProjectId,
        /// Position in that project's sessions.
        index: u32,
    },
}
