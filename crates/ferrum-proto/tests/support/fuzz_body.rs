//! The body of the `decode` fuzz target, shared with `tests/fuzz_seeds.rs`: the input is read as
//! a stream of frames, as one message payload of each direction and as one encoded row (with an
//! empty link table). Nothing may panic; a message that decodes must encode again and decode to
//! the same value.

use ferrum_proto::{
    ClientMessage, DaemonMessage, LinkTable, RowBlock, decode_message, decode_row, encode_frame,
    split_frame,
};

/// Decodes `$bytes` as a `$kind`; when that works, the value must survive an encode and a decode
/// unchanged. Evaluates to the decoded message. (A macro, so the fuzz crate needs no `serde`.)
macro_rules! round_trip {
    ($kind:ty, $bytes:expr) => {
        decode_message::<$kind>($bytes).ok().inspect(|message| {
            let mut frame = Vec::new();
            assert!(encode_frame(message, &mut frame).is_ok(), "a decoded message encodes");
            let again = split_frame(&frame)
                .ok()
                .flatten()
                .and_then(|frame| decode_message::<$kind>(frame.payload).ok());
            assert_eq!(again.as_ref(), Some(message), "a decoded message round-trips");
        })
    };
}

/// Runs every decoder on `input`.
pub(crate) fn run(input: &[u8]) {
    let mut rest = input;
    while let Ok(Some(frame)) = split_frame(rest) {
        payload(frame.payload);
        rest = rest.get(frame.consumed..).unwrap_or_default();
    }
    payload(input);
    if let Ok(cells) = decode_row(input, &LinkTable::new()) {
        cells.for_each(drop);
    }
}

fn payload(bytes: &[u8]) {
    let _client = round_trip!(ClientMessage, bytes);
    match round_trip!(DaemonMessage, bytes) {
        Some(DaemonMessage::Snapshot(snapshot)) => read_rows(&snapshot.rows),
        Some(DaemonMessage::Rows { rows, .. }) => read_rows(&rows),
        Some(DaemonMessage::Damage(damage)) => read_rows(&damage.rows),
        _ => {}
    }
}

fn read_rows(block: &RowBlock) {
    for (_, cells) in block.rows().flatten() {
        cells.for_each(drop);
    }
}
