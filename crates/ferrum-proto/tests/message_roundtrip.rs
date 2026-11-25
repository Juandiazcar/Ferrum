//! Round-trip property of the messages (`docs/08` §2: `proto` round-trip): any message of either
//! direction, with random fields, encodes into a frame and decodes back to the same value.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_proto::{
    Attrs, Axis, Bytes, CellRef, CellWidth, ClientKind, ClientMessage, Color, Cursor, CursorShape,
    DaemonMessage, Damage, Hello, Layout, LayoutNode, List, Modes, NONCE_LEN, Nonce, PerMille,
    Percent, Point, ProgressState, ProjectChange, ProjectEvent, ProjectId, ProjectInfo,
    ProtocolVersion, Refusal, RefusalReason, Rgb, RowBlock, RowRegion, SearchOptions, SelectAction,
    SelectionKind, SessionEvent, SessionId, SessionInfo, SessionKind, SessionStatus, Snapshot,
    Style, TerminalModes, Text, Uuid, Viewport, decode_message, encode_frame, split_frame,
};
use proptest::prelude::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

fn text() -> impl Strategy<Value = Text> {
    "\\PC{0,12}".prop_map(Text::new)
}

fn bytes() -> impl Strategy<Value = Bytes> {
    prop::collection::vec(any::<u8>(), 0..16).prop_map(Bytes::new)
}

fn session() -> impl Strategy<Value = SessionId> {
    any::<u64>().prop_map(SessionId)
}

fn project() -> impl Strategy<Value = ProjectId> {
    any::<u128>().prop_map(|id| ProjectId(Uuid::from_u128(id)))
}

fn version() -> impl Strategy<Value = ProtocolVersion> {
    (any::<u16>(), any::<u16>()).prop_map(|(major, minor)| ProtocolVersion { major, minor })
}

fn viewport() -> impl Strategy<Value = Viewport> {
    (any::<u16>(), any::<u16>()).prop_map(|(cols, rows)| Viewport { cols, rows })
}

fn rgb() -> impl Strategy<Value = Rgb> {
    any::<[u8; 3]>().prop_map(|[r, g, b]| Rgb { r, g, b })
}

fn texts() -> impl Strategy<Value = List<Text>> {
    prop::collection::vec(text(), 0..4).prop_map(|items| List::new(items).unwrap())
}

fn layout() -> impl Strategy<Value = Layout> {
    let node = prop_oneof![
        session().prop_map(LayoutNode::Pane),
        (prop_oneof![Just(Axis::Horizontal), Just(Axis::Vertical)], 1_u16..1_000).prop_map(
            |(axis, ratio)| LayoutNode::Split { axis, ratio: PerMille::try_from(ratio).unwrap() }
        ),
    ];
    prop::collection::vec(node, 0..6).prop_map(|nodes| Layout(List::new(nodes).unwrap()))
}

fn cursor() -> impl Strategy<Value = Cursor> {
    let shape =
        prop_oneof![Just(CursorShape::Block), Just(CursorShape::Underline), Just(CursorShape::Bar)];
    (any::<u16>(), any::<u16>(), shape, any::<bool>(), any::<bool>()).prop_map(
        |(col, row, shape, is_blinking, is_visible)| Cursor {
            col,
            row,
            shape,
            is_blinking,
            is_visible,
        },
    )
}

fn modes() -> impl Strategy<Value = TerminalModes> {
    (any::<u32>(), any::<u8>())
        .prop_map(|(modes, kitty_keyboard)| TerminalModes { modes: Modes(modes), kitty_keyboard })
}

/// A block of up to three rows with random text, colors and links.
fn rows() -> impl Strategy<Value = RowBlock> {
    let cell = ("[ -~]", any::<u8>(), prop::option::of("[a-z]{1,4}"));
    prop::collection::vec(prop::collection::vec(cell, 0..6), 0..3).prop_map(|rows| {
        let mut block = RowBlock::new();
        for (y, row) in (0_u16..).zip(&rows) {
            let cells = row.iter().map(|(text, color, link)| {
                let style = Style {
                    attrs: Attrs::BOLD,
                    fg: Color::indexed(*color),
                    link: link.as_deref(),
                    ..Style::default()
                };
                CellRef { text, width: CellWidth::Single, style }
            });
            block.push_row(y, 8, cells).unwrap();
        }
        block
    })
}

fn project_change() -> impl Strategy<Value = ProjectChange> {
    prop_oneof![
        text().prop_map(ProjectChange::Rename),
        rgb().prop_map(ProjectChange::Recolor),
        prop::option::of(text()).prop_map(ProjectChange::SetIcon),
    ]
}

fn select_action() -> impl Strategy<Value = SelectAction> {
    let point = (any::<i32>(), any::<u16>()).prop_map(|(line, col)| Point { line, col });
    let kind = prop_oneof![
        Just(SelectionKind::Simple),
        Just(SelectionKind::Word),
        Just(SelectionKind::Line),
        Just(SelectionKind::Block),
    ];
    prop_oneof![
        (point.clone(), kind).prop_map(|(point, kind)| SelectAction::Start { point, kind }),
        point.prop_map(SelectAction::Extend),
        Just(SelectAction::Clear),
    ]
}

fn hello() -> impl Strategy<Value = ClientMessage> {
    (version(), any::<[u8; NONCE_LEN]>()).prop_map(|(protocol_version, nonce)| {
        ClientMessage::Hello(Hello {
            protocol_version,
            client_kind: ClientKind::Ui,
            nonce: Nonce(nonce),
        })
    })
}

fn session_requests() -> impl Strategy<Value = ClientMessage> {
    use ClientMessage as M;
    let search = (any::<bool>(), any::<bool>())
        .prop_map(|(is_case_sensitive, is_regex)| SearchOptions { is_case_sensitive, is_regex });
    prop_oneof![
        (session(), viewport(), any::<u16>()).prop_map(|(session_id, viewport, want)| {
            M::Attach { session_id, viewport, want_scrollback_rows: want }
        }),
        session().prop_map(|session_id| M::Detach { session_id }),
        (session(), bytes()).prop_map(|(session_id, bytes)| M::Input { session_id, bytes }),
        (session(), viewport())
            .prop_map(|(session_id, viewport)| M::Resize { session_id, viewport }),
        (session(), any::<u32>()).prop_map(|(session_id, offset)| M::Scroll { session_id, offset }),
        (session(), select_action())
            .prop_map(|(session_id, action)| M::Select { session_id, action }),
        (session(), text(), search).prop_map(|(session_id, query, options)| M::Search {
            session_id,
            query,
            options
        }),
        session().prop_map(|session_id| M::CloseSession { session_id }),
        session().prop_map(|session_id| M::Restart { session_id }),
        (session(), text()).prop_map(|(session_id, name)| M::RenameSession { session_id, name }),
    ]
}

fn project_requests() -> impl Strategy<Value = ClientMessage> {
    use ClientMessage as M;
    prop_oneof![
        (project(), any::<u16>()).prop_map(|(project_id, root)| M::CreateSession {
            project_id,
            kind: SessionKind::Terminal,
            root
        }),
        (text(), texts()).prop_map(|(name, roots)| M::CreateProject { name, roots }),
        (project(), project_change())
            .prop_map(|(project_id, change)| M::UpdateProject { project_id, change }),
        (session(), project(), any::<u32>()).prop_map(|(session_id, project_id, index)| {
            M::MoveSession { session_id, project_id, index }
        }),
        (project(), any::<u32>())
            .prop_map(|(project_id, index)| M::ReorderProject { project_id, index }),
        (project(), layout()).prop_map(|(project_id, layout)| M::SaveLayout { project_id, layout }),
        bytes().prop_map(M::ReservedAgent),
        bytes().prop_map(M::ReservedUsage),
        bytes().prop_map(M::ReservedVault),
    ]
}

fn client_message() -> impl Strategy<Value = ClientMessage> {
    prop_oneof![hello(), session_requests(), project_requests()]
}

fn session_event() -> impl Strategy<Value = SessionEvent> {
    let status = prop_oneof![
        Just(SessionStatus::Running),
        prop::option::of(any::<i32>()).prop_map(|code| SessionStatus::Exited { code }),
        Just(SessionStatus::Killed),
        "[a-z ]{0,16}".prop_map(|error| SessionStatus::Failed { error: Text::new(error) }),
    ];
    let info = (project(), prop::option::of(text()), text(), status).prop_map(
        |(project_id, name, title, status)| SessionInfo {
            project_id,
            kind: SessionKind::Terminal,
            name,
            title,
            status,
        },
    );
    let state = prop_oneof![
        Just(ProgressState::Remove),
        Just(ProgressState::Normal),
        Just(ProgressState::Error),
        Just(ProgressState::Indeterminate),
        Just(ProgressState::Warning),
    ];
    prop_oneof![
        info.prop_map(SessionEvent::Created),
        Just(SessionEvent::Closed),
        text().prop_map(SessionEvent::Renamed),
        text().prop_map(SessionEvent::Title),
        Just(SessionEvent::Bell),
        text().prop_map(SessionEvent::CwdChanged),
        prop::option::of(any::<i32>()).prop_map(|code| SessionEvent::Exited { code }),
        Just(SessionEvent::Killed),
        Just(SessionEvent::Activity),
        (state, 0_u8..=100).prop_map(|(state, value)| SessionEvent::Progress {
            state,
            value: Percent::try_from(value).unwrap()
        }),
        text().prop_map(|error| SessionEvent::Failed { error }),
        Just(SessionEvent::SpillDisabled),
    ]
}

fn project_event() -> impl Strategy<Value = ProjectEvent> {
    let sessions = prop::collection::vec(session(), 0..4).prop_map(|ids| List::new(ids).unwrap());
    let info = (project(), text(), rgb(), prop::option::of(text()), texts(), sessions, layout())
        .prop_map(|(project_id, name, color, icon, roots, sessions, layout)| ProjectInfo {
            project_id,
            name,
            color,
            icon,
            roots,
            sessions,
            layout,
        });
    prop_oneof![
        info.prop_map(ProjectEvent::Created),
        (project(), project_change())
            .prop_map(|(project_id, change)| ProjectEvent::Updated { project_id, change }),
        (project(), any::<u32>())
            .prop_map(|(project_id, index)| ProjectEvent::Reordered { project_id, index }),
        (session(), project(), any::<u32>()).prop_map(|(session_id, project_id, index)| {
            ProjectEvent::SessionMoved { session_id, project_id, index }
        }),
    ]
}

fn screen_messages() -> impl Strategy<Value = DaemonMessage> {
    use DaemonMessage as M;
    let metadata = (session(), viewport(), cursor(), modes(), text());
    let snapshot = (metadata, rows(), any::<bool>(), any::<u16>()).prop_map(
        |((session_id, viewport, cursor, modes, title), rows, has_more, history_rows)| {
            let snapshot = Snapshot {
                session_id,
                viewport,
                cursor,
                modes,
                title,
                rows,
                has_more,
                history_rows,
            };
            M::Snapshot(snapshot)
        },
    );
    let region = prop_oneof![Just(RowRegion::Screen), Just(RowRegion::History)];
    let damage =
        (session(), rows(), cursor(), any::<i32>(), prop::option::of(modes()), any::<bool>())
            .prop_map(|(session_id, rows, cursor, scroll_delta, modes, has_more)| {
                M::Damage(Damage { session_id, rows, cursor, scroll_delta, modes, has_more })
            });
    prop_oneof![
        snapshot,
        (session(), region, rows()).prop_map(|(session_id, region, rows)| M::Rows {
            session_id,
            region,
            rows
        }),
        damage,
    ]
}

fn daemon_message() -> impl Strategy<Value = DaemonMessage> {
    use DaemonMessage as M;
    let reason = prop_oneof![Just(RefusalReason::VersionMismatch), Just(RefusalReason::BadNonce)];
    prop_oneof![
        version().prop_map(|protocol_version| M::Welcome { protocol_version }),
        (version(), reason)
            .prop_map(|(daemon_version, reason)| M::Refused(Refusal { daemon_version, reason })),
        Just(M::Synced),
        screen_messages(),
        (session(), session_event())
            .prop_map(|(session_id, event)| M::SessionEvent { session_id, event }),
        project_event().prop_map(M::ProjectEvent),
        (project(), layout())
            .prop_map(|(project_id, layout)| M::LayoutEvent { project_id, layout }),
        bytes().prop_map(M::ReservedAgent),
        bytes().prop_map(M::ReservedUsage),
        bytes().prop_map(M::ReservedVault),
    ]
}

fn round_trip<T>(message: &T) -> T
where
    T: Serialize + DeserializeOwned,
{
    let mut frame = Vec::new();
    encode_frame(message, &mut frame).unwrap();
    let split = split_frame(&frame).unwrap().unwrap();
    assert_eq!(split.consumed, frame.len());
    decode_message(split.payload).unwrap()
}

proptest! {
    #[test]
    fn client_messages_round_trip(message in client_message()) {
        prop_assert_eq!(round_trip(&message), message);
    }

    #[test]
    fn daemon_messages_round_trip(message in daemon_message()) {
        prop_assert_eq!(round_trip(&message), message);
    }
}
