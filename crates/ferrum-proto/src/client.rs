//! Messages from the UI to the daemon (`docs/02` §4).
//!
//! Variants are appended, never reordered or removed, within a major protocol version: `postcard`
//! encodes a variant by its position (the golden vectors in `fixtures/proto/` pin it). `Hello`
//! stays variant 0 in every major (see the handshake module). User content is [`Text`] or
//! [`Bytes`], whose `Debug` prints only sizes.

use serde::{Deserialize, Serialize};

use crate::handshake::Hello;
use crate::list::List;
use crate::redact::{Bytes, Text};
use crate::types::{
    Layout, Point, ProjectId, Rgb, SearchOptions, SelectionKind, SessionId, SessionKind, Viewport,
};

/// A message from the UI to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ClientMessage {
    /// The first frame of every connection. Variant 0 in every major version.
    Hello(Hello),
    /// Show a session; the daemon replies with a `Snapshot` and then sends `Damage`.
    Attach {
        /// The session.
        session_id: SessionId,
        /// The size the UI shows it at.
        viewport: Viewport,
        /// History rows wanted above the screen (the daemon sends at most what it has).
        want_scrollback_rows: u16,
    },
    /// Stop showing a session (it keeps running and keeps sending `SessionEvent`s).
    Detach {
        /// The session.
        session_id: SessionId,
    },
    /// Bytes for the session's PTY, already encoded per the session's modes.
    Input {
        /// The session.
        session_id: SessionId,
        /// The bytes.
        bytes: Bytes,
    },
    /// The session's pane changed size.
    Resize {
        /// The session.
        session_id: SessionId,
        /// The new size.
        viewport: Viewport,
    },
    /// Show the screen `offset` lines up into the scrollback (0 is the live screen).
    Scroll {
        /// The session.
        session_id: SessionId,
        /// Lines above the live screen.
        offset: u32,
    },
    /// Change the selection.
    Select {
        /// The session.
        session_id: SessionId,
        /// What to do.
        action: SelectAction,
    },
    /// Search the session's scrollback; the search runs in the daemon (`FR-TERM-015`).
    Search {
        /// The session.
        session_id: SessionId,
        /// The text or regular expression.
        query: Text,
        /// Options.
        options: SearchOptions,
    },
    /// Start a session in a project.
    CreateSession {
        /// The project.
        project_id: ProjectId,
        /// What runs in it.
        kind: SessionKind,
        /// Index of the project root it starts in (`FR-PROJ-003`).
        root: u16,
    },
    /// Close a session and end its process.
    CloseSession {
        /// The session.
        session_id: SessionId,
    },
    /// Start an exited session again (`FR-SESS-006`).
    Restart {
        /// The session.
        session_id: SessionId,
    },
    /// Create a project from its root directories (`FR-PROJ-002`).
    CreateProject {
        /// Name.
        name: Text,
        /// Root directories.
        roots: List<Text>,
    },
    /// Change a project.
    UpdateProject {
        /// The project.
        project_id: ProjectId,
        /// The change.
        change: ProjectChange,
    },
    /// Rename a session (`FR-UI-002`).
    RenameSession {
        /// The session.
        session_id: SessionId,
        /// The new name.
        name: Text,
    },
    /// Move a session to `index` in a project's session list, possibly another project.
    MoveSession {
        /// The session.
        session_id: SessionId,
        /// The destination project.
        project_id: ProjectId,
        /// Position in that project's sessions.
        index: u32,
    },
    /// Move a project to `index` in the sidebar.
    ReorderProject {
        /// The project.
        project_id: ProjectId,
        /// New position.
        index: u32,
    },
    /// Store a project's layout.
    SaveLayout {
        /// The project.
        project_id: ProjectId,
        /// The layout.
        layout: Layout,
    },
    /// Reserved for Phase 2 agent messages (`docs/02` §9); a v1 peer never sends it.
    ReservedAgent(Bytes),
    /// Reserved for Phase 2 usage messages.
    ReservedUsage(Bytes),
    /// Reserved for Phase 2 vault messages.
    ReservedVault(Bytes),
}

/// A selection change (`FR-TERM-014`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SelectAction {
    /// Start a selection at a point.
    Start {
        /// Where.
        point: Point,
        /// How it grows.
        kind: SelectionKind,
    },
    /// Move the selection's end to a point.
    Extend(Point),
    /// Remove the selection.
    Clear,
}

/// A change to a project (`FR-PROJ-001`, `FR-UI-002`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProjectChange {
    /// New name.
    Rename(Text),
    /// New color.
    Recolor(Rgb),
    /// New icon glyph, or none.
    SetIcon(Option<Text>),
}
