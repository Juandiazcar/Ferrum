//! User content never shows in `Debug` output (`docs/05` §8, `docs/10` §4): logging a message
//! logs ids and sizes, never keystrokes, queries, titles, paths, names or terminal rows.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_proto::{
    Bytes, CellRef, CellWidth, ClientMessage, Cursor, CursorShape, DaemonMessage, Damage, List,
    ProjectChange, ProjectId, RowBlock, SearchOptions, SessionEvent, SessionId, Style, Text, Uuid,
};

const SECRET: &str = "hunter2-secret";

fn assert_hidden(message: &impl std::fmt::Debug) {
    let text = format!("{message:?}");
    assert!(!text.contains(SECRET), "{text}");
}

#[test]
fn client_messages_hide_user_content() {
    let session_id = SessionId(1);
    let project_id = ProjectId(Uuid::nil());
    let secret = || Text::new(SECRET);
    assert_hidden(&ClientMessage::Input { session_id, bytes: Bytes::new(SECRET.as_bytes()) });
    let options = SearchOptions::default();
    assert_hidden(&ClientMessage::Search { session_id, query: secret(), options });
    let roots = List::new(vec![secret()]).unwrap();
    assert_hidden(&ClientMessage::CreateProject { name: secret(), roots });
    let change = ProjectChange::Rename(secret());
    assert_hidden(&ClientMessage::UpdateProject { project_id, change });
    assert_hidden(&ClientMessage::RenameSession { session_id, name: secret() });
    assert_hidden(&ClientMessage::ReservedVault(Bytes::new(SECRET.as_bytes())));
}

#[test]
fn daemon_messages_hide_user_content() {
    let session_id = SessionId(1);
    for event in [
        SessionEvent::Title(Text::new(SECRET)),
        SessionEvent::CwdChanged(Text::new(SECRET)),
        SessionEvent::Renamed(Text::new(SECRET)),
    ] {
        assert_hidden(&DaemonMessage::SessionEvent { session_id, event });
    }
    let style = Style { link: Some(SECRET), ..Style::default() };
    let mut rows = RowBlock::new();
    let cells = SECRET.split_inclusive(|_: char| true);
    let cells = cells.map(|text| CellRef { text, width: CellWidth::Single, style });
    rows.push_row(0, 80, cells).unwrap();
    let cursor =
        Cursor { col: 0, row: 0, shape: CursorShape::Block, is_blinking: false, is_visible: true };
    let damage = Damage { session_id, rows, cursor, scroll_delta: 0, modes: None, has_more: false };
    assert_hidden(&DaemonMessage::Damage(damage));
}
