//! The sequence watchdog (`docs/10` §3 item 9): a sequence or string over the cap is cancelled at
//! the exact byte, nothing of its payload reaches the screen, the bytes up to its terminator are
//! discarded without being stored, and nothing else is ever cancelled by it.

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{Event, EventSink, MAX_SEQUENCE_BYTES, Size, Terminal};

#[derive(Default)]
struct Log(Vec<String>);

impl EventSink for Log {
    fn event(&mut self, event: Event<'_>) {
        self.0.push(match event {
            Event::Reply(bytes) => format!("Reply {}", bytes.escape_ascii()),
            other => format!("{other:?}"),
        });
    }
}

fn terminal() -> Terminal<Log> {
    Terminal::new(Size::new(20, 4), Scrollback::new(ScrollbackConfig::default()), Log::default())
}

fn screen(term: &Terminal<Log>) -> String {
    let mut text = String::new();
    for y in 0..4 {
        let row = term.grid().row(y).map(|row| row.cells().to_vec()).unwrap_or_default();
        text.extend(row.iter().map(ferrum_vt::Cell::scalar));
        text.push('\n');
    }
    text
}

/// `DCS + q` (XTGETTCAP) whose bytes from the ESC to the end of the payload are exactly `total`.
fn xtgettcap(total: usize) -> Vec<u8> {
    let mut bytes = b"\x1bP+q".to_vec();
    bytes.resize(total, b'6');
    bytes
}

#[test]
fn text_after_a_sequence_that_ended_without_a_dispatch_is_never_cancelled() {
    for ended in [&b"\x1b[1?h"[..], b"\x1b[\x18", b"\x1b[\x1a", b"\x1b[1;2$~"] {
        let mut whole = terminal();
        let mut input = ended.to_vec();
        input.resize(input.len() + MAX_SEQUENCE_BYTES, b'a');
        input.extend_from_slice("é".as_bytes());
        whole.parse(&input);
        // A read that ends inside the UTF-8 character must not change anything either.
        let mut split = terminal();
        let cut = input.len() - 1;
        split.parse(input.get(..cut).unwrap_or_default());
        split.parse(input.get(cut..).unwrap_or_default());
        for term in [&whole, &split] {
            let text = screen(term);
            assert!(!text.contains('\u{fffd}'), "{ended:?}: {text}");
            assert!(text.contains('é'), "{ended:?}: {text}");
        }
    }
}

#[test]
fn the_cap_is_checked_at_the_exact_byte_in_one_read() {
    let mut fits = xtgettcap(MAX_SEQUENCE_BYTES);
    fits.extend_from_slice(b"\x1b\\ok");
    let mut term = terminal();
    term.parse(&fits);
    assert_eq!(term.sink().0, vec![String::from("Reply \\x1bP0+r\\x1b\\\\")]);
    assert!(screen(&term).starts_with("ok"));

    let mut over = xtgettcap(MAX_SEQUENCE_BYTES + 1);
    over.extend_from_slice(b"\x1b\\ok");
    let mut term = terminal();
    term.parse(&over);
    assert!(term.sink().0.is_empty(), "a string over the cap is never answered");
    assert!(screen(&term).starts_with("ok"), "{}", screen(&term));
}

#[test]
fn nothing_of_a_capped_string_reaches_the_screen() {
    for introducer in [&b"\x1bPq"[..], b"\x1b_G", b"\x1b^", b"\x1bX", b"\x1b]52;c;", b"\x1b]1337;"]
    {
        let mut input = introducer.to_vec();
        input.resize(2 * MAX_SEQUENCE_BYTES + 3, b'Q');
        input.extend_from_slice(b"\r\nmore payload\x1b\\after");
        for reads in [1, 7, 4_096] {
            let mut term = terminal();
            for piece in input.chunks(input.len().div_ceil(reads)) {
                term.parse(piece);
            }
            let text = screen(&term);
            assert!(!text.contains('Q') && !text.contains("more"), "{introducer:?}: {text}");
            assert!(text.contains("after"), "{introducer:?}/{reads}: {text}");
            assert!(term.sink().0.is_empty(), "{introducer:?}: {:?}", term.sink().0);
        }
    }
}

#[test]
fn an_osc_does_not_inherit_the_byte_count_of_an_earlier_sequence() {
    let mut term = terminal();
    term.parse(b"\x1b[1?h");
    term.parse(&vec![b'a'; MAX_SEQUENCE_BYTES - 6]);
    term.parse(b"\x1b]0;t\x07");
    term.parse(b"\x1b[");
    term.parse(b"2;3HX");
    assert_eq!(term.cursor().y, 1);
    assert_eq!(term.cursor().x, 3);
    assert!(!screen(&term).contains("2;3H"), "{}", screen(&term));
}
