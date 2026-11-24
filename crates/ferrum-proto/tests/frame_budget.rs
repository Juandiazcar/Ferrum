//! A hostile screen always reaches the UI (`docs/02` §4, `docs/10` §3 item 11): rows are packed
//! into blocks that never outgrow a frame, a worst-case row fits an empty block, and a worst-case
//! screen goes out as several `Damage` frames, or as a `Snapshot` with the longest title plus
//! `Rows`, each at most `MAX_FRAME_LEN` bytes.
//!
//! The worst case uses the limits `ferrum-vt` enforces (`crates/ferrum-vt/src/limits.rs`; this
//! crate may not depend on it, `docs/04` §2): `MAX_COLS` 1 024, `MAX_ROWS` 512,
//! `MAX_CLUSTER_BYTES` 64, `MAX_URI_BYTES` 4 096, `MAX_TITLE_BYTES` 1 024 and
//! `MAX_LINK_TABLE_BYTES` 256 KiB, so at most
//! 64 distinct URIs of the longest length. Every cell is a separate run (its link differs from
//! its neighbors') holding the longest cluster.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_proto::{
    CellRef, CellWidth, Color, Cursor, CursorShape, DaemonMessage, Damage, MAX_FRAME_LEN,
    ProtoError, RowBlock, SessionId, Snapshot, Style, TerminalModes, Text, Viewport,
    decode_message, encode_frame, split_frame,
};

const COLS: u16 = 1_024;
const ROWS: u16 = 512;
const URI_BYTES: usize = 4_096;
const LINK_TABLE_BYTES: usize = 256 * 1_024;
const TITLE_BYTES: usize = 1_024;

fn uris() -> Vec<String> {
    (0..LINK_TABLE_BYTES / URI_BYTES).map(|n| format!("{n:0>URI_BYTES$}")).collect()
}

/// `e` and 31 combining acute accents: 63 bytes, one column.
fn cluster() -> String {
    std::iter::once('e').chain(std::iter::repeat_n('\u{301}', 31)).collect()
}

fn worst_row<'a>(uris: &'a [String], text: &'a str) -> impl Iterator<Item = CellRef<'a>> {
    uris.iter().cycle().take(usize::from(COLS)).map(move |link| {
        let style = Style { fg: Color::rgb(1, 2, 3), link: Some(link), ..Style::default() };
        CellRef { text, width: CellWidth::Single, style }
    })
}

fn damage(rows: RowBlock, has_more: bool) -> DaemonMessage {
    let cursor =
        Cursor { col: 0, row: 0, shape: CursorShape::Block, is_blinking: false, is_visible: true };
    let session_id = SessionId(1);
    DaemonMessage::Damage(Damage {
        session_id,
        rows,
        cursor,
        scroll_delta: 0,
        modes: None,
        has_more,
    })
}

/// Encodes `block` as a `Damage` frame, checks its size and returns how many rows it decodes to.
fn send(block: RowBlock) -> usize {
    let mut frame = Vec::new();
    encode_frame(&damage(block, true), &mut frame).unwrap();
    assert!(frame.len() <= MAX_FRAME_LEN, "frame of {} bytes", frame.len());
    let payload = split_frame(&frame).unwrap().unwrap().payload;
    let DaemonMessage::Damage(damage) = decode_message(payload).unwrap() else { unreachable!() };
    damage.rows.rows().map(Result::unwrap).count()
}

#[test]
fn a_worst_case_screen_goes_out_in_frames_under_the_cap() {
    let (uris, text) = (uris(), cluster());
    let mut block = RowBlock::new();
    let (mut sent, mut frames) = (0, 0);
    for y in 0..ROWS {
        match block.push_row(y, COLS, worst_row(&uris, &text)) {
            Ok(()) => {}
            Err(ProtoError::BlockFull) => {
                sent += send(std::mem::take(&mut block));
                frames += 1;
                block.push_row(y, COLS, worst_row(&uris, &text)).unwrap();
            }
            Err(error) => panic!("{error}"),
        }
    }
    sent += send(block);
    assert_eq!(sent, usize::from(ROWS));
    assert!(frames >= 1, "the worst case needs more than one frame");
}

#[test]
fn a_worst_case_snapshot_with_the_longest_title_fits_a_frame() {
    let (uris, text) = (uris(), cluster());
    let mut rows = RowBlock::new();
    let mut y = 0;
    while rows.push_row(y, COLS, worst_row(&uris, &text)).is_ok() {
        y += 1;
    }
    assert!(y < ROWS, "the block is full before the screen ends, so has_more is set");
    let cursor =
        Cursor { col: 0, row: 0, shape: CursorShape::Block, is_blinking: false, is_visible: true };
    let snapshot = Snapshot {
        session_id: SessionId(1),
        viewport: Viewport { cols: COLS, rows: ROWS },
        cursor,
        modes: TerminalModes::default(),
        title: Text::new("t".repeat(TITLE_BYTES)),
        rows,
        has_more: true,
        history_rows: u16::MAX,
    };
    let mut frame = Vec::new();
    encode_frame(&DaemonMessage::Snapshot(snapshot), &mut frame).unwrap();
    assert!(frame.len() <= MAX_FRAME_LEN, "frame of {} bytes", frame.len());
}
