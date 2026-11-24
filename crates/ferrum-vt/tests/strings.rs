//! Escape strings (DCS, APC, PM, SOS): replies only for completed strings, and a string over the
//! cap is cancelled and dropped up to its terminator instead of swallowing the output for ever
//! (`docs/10` §3 item 9; `tests/watchdog.rs` has the byte-exact cases).

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{Event, EventSink, MAX_SEQUENCE_BYTES, Size, Terminal};

#[derive(Default)]
struct Log {
    replies: Vec<Vec<u8>>,
}

impl EventSink for Log {
    fn event(&mut self, event: Event<'_>) {
        if let Event::Reply(bytes) = event {
            self.replies.push(bytes.to_vec());
        }
    }
}

fn terminal() -> Terminal<Log> {
    Terminal::new(Size::new(20, 4), Scrollback::new(ScrollbackConfig::default()), Log::default())
}

fn row_text(term: &Terminal<Log>, y: usize) -> String {
    let row = term.grid().row(y).map(|row| row.cells().to_vec()).unwrap_or_default();
    row.iter().map(ferrum_vt::Cell::scalar).collect::<String>().trim_end().to_owned()
}

#[test]
fn xtgettcap_is_answered_only_when_the_string_is_completed() {
    let mut term = terminal();
    term.parse(b"\x1bP+q544e\x18");
    term.parse(b"\x1bP+q544e\x1a");
    assert_eq!(term.sink().replies.len(), 0);
    term.parse(b"\x1bP+q544e\x1b\\");
    assert_eq!(term.sink().replies, vec![b"\x1bP0+r\x1b\\".to_vec()]);
}

#[test]
fn decrqss_is_answered_only_when_the_string_is_completed() {
    let mut term = terminal();
    term.parse(b"\x1bP$qm\x18\x1bP$qm\x1b\\");
    assert_eq!(term.sink().replies.len(), 1);
}

#[test]
fn a_terminator_split_across_reads_still_completes_the_string() {
    let mut term = terminal();
    term.parse(b"\x1bP+q544e\x1b");
    term.parse(b"\\ok");
    assert_eq!(term.sink().replies.len(), 1);
    assert_eq!(row_text(&term, 0), "ok");
}

#[test]
fn a_string_over_the_cap_is_dropped_and_output_after_its_terminator_is_shown() {
    for introducer in [&b"\x1bP"[..], b"\x1b_", b"\x1b^", b"\x1bX"] {
        let mut term = terminal();
        term.parse(introducer);
        term.parse(&vec![b'x'; MAX_SEQUENCE_BYTES + 16]);
        term.parse(b"\r\nstill payload\x1b\\after");
        assert_eq!(row_text(&term, 0), "after");
        assert_eq!(term.sink().replies.len(), 0);
    }
}

#[test]
fn a_csi_over_the_cap_is_dropped_up_to_its_final_byte() {
    let mut term = terminal();
    term.parse(b"\x1b[");
    term.parse(&vec![b'1'; MAX_SEQUENCE_BYTES + 16]);
    term.parse(b";2mafter");
    assert_eq!(row_text(&term, 0), "after");
}
