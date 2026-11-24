//! Golden bytes (`docs/05` §7, `docs/12` WP-03), committed as `name hex` lines:
//! - `fixtures/proto/client.hex`, `daemon.hex`: one frame per message variant;
//! - `fixtures/proto/values.hex`: every variant of every enum nested in a message, and a row of
//!   each underline style, as bare `postcard` values.
//!
//! Each value must encode to exactly the committed bytes and the committed bytes must decode to
//! the same value, so a change of the wire format (a reordered variant, a changed field) fails
//! here and needs a protocol version bump. On a mismatch the test prints the file it expected,
//! to review and commit deliberately.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use std::fmt::Write as _;

use ferrum_proto::{
    Attrs, Axis, Bytes, CellRef, CellWidth, ClientKind, ClientMessage, Cursor, CursorShape,
    DaemonMessage, Damage, Hello, Layout, LayoutNode, List, Modes, Nonce, PROTOCOL_VERSION,
    PerMille, Percent, Point, ProgressState, ProjectChange, ProjectEvent, ProjectId, ProjectInfo,
    Refusal, RefusalReason, Rgb, RowBlock, RowRegion, SearchOptions, SelectAction, SelectionKind,
    SessionEvent, SessionId, SessionInfo, SessionKind, SessionStatus, Snapshot, Style,
    TerminalModes, Text, Underline, Uuid, Viewport, decode_message, encode_frame, split_frame,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[path = "support/sample.rs"]
mod sample;

const SESSION: SessionId = SessionId(7);
const PROJECT: ProjectId = ProjectId(Uuid::from_u128(0x0123_4567_89AB_CDEF_0123_4567_89AB_CDEF));
const VIEWPORT: Viewport = Viewport { cols: 200, rows: 55 };
const CURSOR: Cursor =
    Cursor { col: 4, row: 2, shape: CursorShape::Bar, is_blinking: true, is_visible: true };

fn text(value: &str) -> Text {
    Text::new(value)
}

fn layout() -> Layout {
    let nodes = vec![
        LayoutNode::Split { axis: Axis::Horizontal, ratio: PerMille::HALF },
        LayoutNode::Pane(SessionId(1)),
        LayoutNode::Pane(SessionId(2)),
    ];
    Layout(List::new(nodes).unwrap())
}

fn sample_block() -> RowBlock {
    let mut block = RowBlock::new();
    block.push_row(0, sample::COLS, sample::sample_cells()).unwrap();
    block.push_row(1, sample::COLS, []).unwrap();
    block
}

fn project() -> ProjectInfo {
    ProjectInfo {
        project_id: PROJECT,
        name: text("Shop"),
        color: Rgb { r: 0xE8, g: 0x59, b: 0x0C },
        icon: Some(text("λ")),
        roots: List::new(vec![text("/src/front"), text("/src/back")]).unwrap(),
        sessions: List::new(vec![SessionId(1), SessionId(2)]).unwrap(),
        layout: layout(),
    }
}

fn client_messages() -> Vec<(&'static str, ClientMessage)> {
    [client_view_messages(), client_session_messages(), client_project_messages()].concat()
}

/// The handshake and what the UI asks about the session it shows.
fn client_view_messages() -> Vec<(&'static str, ClientMessage)> {
    use ClientMessage as M;
    let hello = Hello {
        protocol_version: PROTOCOL_VERSION,
        client_kind: ClientKind::Ui,
        nonce: Nonce([0xA5; 32]),
    };
    let point = Point { line: -3, col: 9 };
    let start = SelectAction::Start { point, kind: SelectionKind::Word };
    let options = SearchOptions { is_case_sensitive: false, is_regex: true };
    let attach = M::Attach { session_id: SESSION, viewport: VIEWPORT, want_scrollback_rows: 100 };
    vec![
        ("hello", M::Hello(hello)),
        ("attach", attach),
        ("detach", M::Detach { session_id: SESSION }),
        ("input", M::Input { session_id: SESSION, bytes: Bytes::new(*b"ls\r") }),
        ("resize", M::Resize { session_id: SESSION, viewport: Viewport { cols: 80, rows: 24 } }),
        ("scroll", M::Scroll { session_id: SESSION, offset: 120 }),
        ("select_start", M::Select { session_id: SESSION, action: start }),
        ("select_extend", M::Select { session_id: SESSION, action: SelectAction::Extend(point) }),
        ("select_clear", M::Select { session_id: SESSION, action: SelectAction::Clear }),
        ("search", M::Search { session_id: SESSION, query: text("err.r"), options }),
    ]
}

/// Session mutations.
fn client_session_messages() -> Vec<(&'static str, ClientMessage)> {
    use ClientMessage as M;
    let create = M::CreateSession { project_id: PROJECT, kind: SessionKind::Terminal, root: 1 };
    vec![
        ("create_session", create),
        ("close_session", M::CloseSession { session_id: SESSION }),
        ("restart", M::Restart { session_id: SESSION }),
        ("rename_session", M::RenameSession { session_id: SESSION, name: text("build") }),
        ("move_session", M::MoveSession { session_id: SESSION, project_id: PROJECT, index: 2 }),
    ]
}

/// Project mutations and the reserved Phase 2 variants.
fn client_project_messages() -> Vec<(&'static str, ClientMessage)> {
    use ClientMessage as M;
    let roots = List::new(vec![text("/src/front"), text("/src/back")]).unwrap();
    let update = |change| M::UpdateProject { project_id: PROJECT, change };
    vec![
        ("create_project", M::CreateProject { name: text("Shop"), roots }),
        ("rename_project", update(ProjectChange::Rename(text("Store")))),
        ("recolor_project", update(ProjectChange::Recolor(Rgb { r: 0xE8, g: 0x59, b: 0x0C }))),
        ("set_icon", update(ProjectChange::SetIcon(Some(text("λ"))))),
        ("reorder_project", M::ReorderProject { project_id: PROJECT, index: 0 }),
        ("save_layout", M::SaveLayout { project_id: PROJECT, layout: layout() }),
        ("reserved_agent", M::ReservedAgent(Bytes::new([1]))),
        ("reserved_usage", M::ReservedUsage(Bytes::new([2]))),
        ("reserved_vault", M::ReservedVault(Bytes::new([3]))),
    ]
}

fn daemon_messages() -> Vec<(&'static str, DaemonMessage)> {
    [daemon_screen_messages(), daemon_session_events(), daemon_project_messages()].concat()
}

/// The handshake answers and the screen of a session.
fn daemon_screen_messages() -> Vec<(&'static str, DaemonMessage)> {
    use DaemonMessage as M;
    let modes = TerminalModes {
        modes: Modes(Modes::BRACKETED_PASTE.0 | Modes::MOUSE_SGR.0),
        kitty_keyboard: 1,
    };
    let snapshot = Snapshot {
        session_id: SESSION,
        viewport: VIEWPORT,
        cursor: CURSOR,
        modes,
        title: text("vim"),
        rows: sample_block(),
        has_more: false,
        history_rows: 1,
    };
    let rows = sample_block();
    let damage = Damage {
        session_id: SESSION,
        rows: sample_block(),
        cursor: CURSOR,
        scroll_delta: -2,
        modes: Some(modes),
        has_more: true,
    };
    let refused = |reason| M::Refused(Refusal { daemon_version: PROTOCOL_VERSION, reason });
    vec![
        ("welcome", M::Welcome { protocol_version: PROTOCOL_VERSION }),
        ("refused", refused(RefusalReason::VersionMismatch)),
        ("refused_nonce", refused(RefusalReason::BadNonce)),
        ("synced", M::Synced),
        ("snapshot", M::Snapshot(snapshot)),
        ("rows_history", M::Rows { session_id: SESSION, region: RowRegion::History, rows }),
        ("damage", M::Damage(damage)),
    ]
}

/// Session events.
fn daemon_session_events() -> Vec<(&'static str, DaemonMessage)> {
    let info = SessionInfo {
        project_id: PROJECT,
        kind: SessionKind::Terminal,
        name: Some(text("build")),
        title: text("cargo"),
        status: SessionStatus::Running,
    };
    let percent = Percent::try_from(40).unwrap();
    let progress = SessionEvent::Progress { state: ProgressState::Normal, value: percent };
    let events = [
        ("session_created", SessionEvent::Created(info)),
        ("session_closed", SessionEvent::Closed),
        ("session_renamed", SessionEvent::Renamed(text("build"))),
        ("title", SessionEvent::Title(text("build"))),
        ("bell", SessionEvent::Bell),
        ("cwd", SessionEvent::CwdChanged(text("file://host/src"))),
        ("exited", SessionEvent::Exited { code: Some(1) }),
        ("exited_unknown", SessionEvent::Exited { code: None }),
        ("killed", SessionEvent::Killed),
        ("activity", SessionEvent::Activity),
        ("progress", progress),
    ];
    let message =
        |(name, event)| (name, DaemonMessage::SessionEvent { session_id: SESSION, event });
    events.into_iter().map(message).collect()
}

/// Project and layout events and the reserved Phase 2 variants.
fn daemon_project_messages() -> Vec<(&'static str, DaemonMessage)> {
    use DaemonMessage as M;
    let updated =
        ProjectEvent::Updated { project_id: PROJECT, change: ProjectChange::SetIcon(None) };
    let reordered = ProjectEvent::Reordered { project_id: PROJECT, index: 1 };
    let moved = ProjectEvent::SessionMoved { session_id: SESSION, project_id: PROJECT, index: 0 };
    vec![
        ("project_created", M::ProjectEvent(ProjectEvent::Created(project()))),
        ("project_updated", M::ProjectEvent(updated)),
        ("project_reordered", M::ProjectEvent(reordered)),
        ("session_moved", M::ProjectEvent(moved)),
        ("layout", M::LayoutEvent { project_id: PROJECT, layout: layout() }),
        ("reserved_agent", M::ReservedAgent(Bytes::new([1]))),
        ("reserved_usage", M::ReservedUsage(Bytes::new([2]))),
        ("reserved_vault", M::ReservedVault(Bytes::new([3]))),
    ]
}

/// Whether committed bytes decode to the value they were made from.
type DecodeCheck = Box<dyn Fn(&[u8]) -> bool>;

/// A committed line: the name, the bytes of the value, and a check that bytes decode back to it.
struct Golden {
    name: String,
    bytes: Vec<u8>,
    decodes_back: DecodeCheck,
}

/// How a value is committed.
#[derive(Clone, Copy, PartialEq)]
enum Form {
    /// As a whole frame (messages).
    Frame,
    /// As a bare `postcard` value (values nested in messages).
    Bare,
}

fn golden<T>(name: &str, value: T, form: Form) -> Golden
where
    T: Serialize + DeserializeOwned + PartialEq + 'static,
{
    let bytes = if form == Form::Frame {
        let mut frame = Vec::new();
        encode_frame(&value, &mut frame).unwrap();
        frame
    } else {
        postcard::to_stdvec(&value).unwrap()
    };
    let decodes_back = Box::new(move |bytes: &[u8]| {
        let payload = if form == Form::Frame {
            let frame = split_frame(bytes).unwrap().unwrap();
            assert_eq!(frame.consumed, bytes.len());
            frame.payload
        } else {
            bytes
        };
        decode_message::<T>(payload).unwrap() == value
    });
    Golden { name: name.to_owned(), bytes, decodes_back }
}

/// A one-cell row underlined with `underline`.
fn underlined(underline: Underline) -> RowBlock {
    let style = Style { attrs: Attrs::NONE.with_underline(underline), ..Style::default() };
    let mut block = RowBlock::new();
    block.push_row(0, 1, [CellRef { text: "u", width: CellWidth::Single, style }]).unwrap();
    block
}

/// One bare golden entry per `(name, value)`, each name prefixed with `group`.
fn bare<T>(group: &str, entries: Vec<(&str, T)>) -> Vec<Golden>
where
    T: Serialize + DeserializeOwned + PartialEq + 'static,
{
    entries
        .into_iter()
        .map(|(name, value)| golden(&format!("{group}_{name}"), value, Form::Bare))
        .collect()
}

fn values() -> Vec<Golden> {
    [view_values(), lifecycle_values(), kind_values(), underline_values()]
        .into_iter()
        .flatten()
        .collect()
}

/// Enums of the cursor, selections and layouts.
fn view_values() -> Vec<Golden> {
    let shapes = vec![
        ("block", CursorShape::Block),
        ("underline", CursorShape::Underline),
        ("bar", CursorShape::Bar),
    ];
    let kinds = vec![
        ("simple", SelectionKind::Simple),
        ("word", SelectionKind::Word),
        ("line", SelectionKind::Line),
        ("block", SelectionKind::Block),
    ];
    let axes = vec![("horizontal", Axis::Horizontal), ("vertical", Axis::Vertical)];
    [bare("cursor_shape", shapes), bare("selection_kind", kinds), bare("axis", axes)]
        .into_iter()
        .flatten()
        .collect()
}

/// Enums of a session's lifecycle and of row messages.
fn lifecycle_values() -> Vec<Golden> {
    let states = vec![
        ("remove", ProgressState::Remove),
        ("normal", ProgressState::Normal),
        ("error", ProgressState::Error),
        ("indeterminate", ProgressState::Indeterminate),
        ("warning", ProgressState::Warning),
    ];
    let statuses = vec![
        ("running", SessionStatus::Running),
        ("exited", SessionStatus::Exited { code: Some(-1) }),
        ("exited_unknown", SessionStatus::Exited { code: None }),
        ("killed", SessionStatus::Killed),
    ];
    let regions = vec![("screen", RowRegion::Screen), ("history", RowRegion::History)];
    [bare("progress_state", states), bare("session_status", statuses), bare("row_region", regions)]
        .into_iter()
        .flatten()
        .collect()
}

/// Single-variant enums and a layout node.
fn kind_values() -> Vec<Golden> {
    vec![
        golden("session_kind_terminal", SessionKind::Terminal, Form::Bare),
        golden("client_kind_ui", ClientKind::Ui, Form::Bare),
        golden("layout_node_pane", LayoutNode::Pane(SessionId(9)), Form::Bare),
    ]
}

/// A row of each underline style.
fn underline_values() -> Vec<Golden> {
    let styles = vec![
        ("none", Underline::None),
        ("single", Underline::Single),
        ("double", Underline::Double),
        ("curly", Underline::Curly),
        ("dotted", Underline::Dotted),
        ("dashed", Underline::Dashed),
    ];
    let rows = styles.into_iter().map(|(name, underline)| (name, underlined(underline))).collect();
    bare("row_underline", rows)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        write!(out, "{byte:02x}").unwrap();
        out
    })
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

fn check(file: &str, entries: &[Golden]) {
    let path = format!("{}/../../fixtures/proto/{file}", env!("CARGO_MANIFEST_DIR"));
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    let mut expected = String::new();
    for entry in entries {
        writeln!(expected, "{} {}", entry.name, hex(&entry.bytes)).unwrap();
    }
    assert!(committed == expected, "{path} differs; the current encoding is:\n{expected}");
    for (entry, line) in entries.iter().zip(committed.lines()) {
        let bytes = unhex(line.split_once(' ').unwrap().1);
        assert!((entry.decodes_back)(&bytes), "{} does not decode back", entry.name);
    }
}

#[test]
fn client_messages_match_the_golden_bytes() {
    let entries: Vec<_> = client_messages()
        .into_iter()
        .map(|(name, value)| golden(name, value, Form::Frame))
        .collect();
    check("client.hex", &entries);
}

#[test]
fn daemon_messages_match_the_golden_bytes() {
    let entries: Vec<_> = daemon_messages()
        .into_iter()
        .map(|(name, value)| golden(name, value, Form::Frame))
        .collect();
    check("daemon.hex", &entries);
}

#[test]
fn nested_values_match_the_golden_bytes() {
    check("values.hex", &values());
}

#[test]
fn golden_rows_decode_to_the_sample() {
    let block = sample_block();
    let rows: Vec<_> = block.rows().map(Result::unwrap).collect();
    assert_eq!(rows.len(), 2);
    let cells: Vec<_> = rows[0].1.clone().map(Result::unwrap).collect();
    assert_eq!(cells.len(), sample::sample_cells().count());
}
