//! OSC behavior that needs the owner's side of the API (replies, getters, the palette).

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{
    ANSI_COLORS, Color, Event, EventSink, MAX_CLIPBOARD_BYTES, Palette, Rgb, Role, Size, Terminal,
};

#[derive(Default)]
struct Log {
    replies: Vec<Vec<u8>>,
    stores: usize,
}

impl EventSink for Log {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Reply(bytes) => self.replies.push(bytes.to_vec()),
            Event::ClipboardStore { .. } => self.stores += 1,
            _ => {}
        }
    }
}

fn terminal() -> Terminal<Log> {
    let config = ScrollbackConfig::default();
    Terminal::new(Size::new(20, 4), Scrollback::new(config), Log::default())
}

#[test]
fn title_and_cwd_are_kept_for_snapshots() {
    let mut term = terminal();
    term.parse(b"\x1b]2;build\x07\x1b]7;file://host/tmp\x07");
    assert_eq!((term.title(), term.cwd()), ("build", "file://host/tmp"));
    term.parse(b"\x1b]2;bad\x1b]2;next\x07");
    assert_eq!(term.title(), "next");
}

#[test]
fn an_approved_clipboard_read_is_answered_with_base64() {
    let mut term = terminal();
    term.answer_clipboard_request("c", b"hello");
    assert_eq!(term.sink().replies, vec![b"\x1b]52;c;aGVsbG8=\x1b\\".to_vec()]);
}

#[test]
fn an_oversized_clipboard_store_is_dropped() {
    let mut term = terminal();
    let mut payload = b"\x1b]52;c;".to_vec();
    payload.resize(payload.len() + MAX_CLIPBOARD_BYTES + 1, b'A');
    payload.push(0x07);
    term.parse(&payload);
    assert_eq!(term.sink().stores, 0);
}

#[test]
fn queries_never_answer_with_program_supplied_text() {
    let mut term = terminal();
    term.parse(b"\x1b]4;1;?\x07\x1b]10;?\x07\x1b[c\x1b[5n\x1b[6n\x1b[?2004$p");
    for reply in &term.sink().replies {
        assert!(reply.starts_with(b"\x1b"), "{}", reply.escape_ascii());
        assert!(reply.len() < 64);
    }
}

#[test]
fn a_theme_palette_answers_queries_and_resolves_colors() {
    let mut term = terminal();
    let ansi = [Rgb::new(1, 2, 3); ANSI_COLORS];
    term.set_palette(Palette::with_theme(
        ansi,
        Rgb::new(9, 9, 9),
        Rgb::new(8, 8, 8),
        Rgb::new(7, 7, 7),
    ));
    term.parse(b"\x1b]4;5;?\x07\x1b]11;?\x07");
    assert_eq!(term.sink().replies[0], b"\x1b]4;5;rgb:0101/0202/0303\x1b\\");
    assert_eq!(term.sink().replies[1], b"\x1b]11;rgb:0808/0808/0808\x1b\\");
    assert_eq!(term.palette().resolve(Color::DEFAULT, Role::Foreground), Rgb::new(9, 9, 9));
}

#[test]
fn a_clipboard_store_of_exactly_one_megabyte_is_accepted_and_one_more_byte_is_not() {
    let mut term = terminal();
    for (extra, stores) in [(0, 1), (1, 1)] {
        let mut payload = b"\x1b]52;c;".to_vec();
        payload.resize(payload.len() + MAX_CLIPBOARD_BYTES + extra, b'A');
        payload.push(0x07);
        term.parse(&payload);
        assert_eq!(term.sink().stores, stores);
    }
}

#[test]
fn one_osc_4_answers_at_most_the_named_number_of_color_queries() {
    let mut term = terminal();
    let mut payload = b"\x1b]4".to_vec();
    for _ in 0..200_000 {
        payload.extend_from_slice(b";1;?");
    }
    payload.push(0x07);
    term.parse(&payload);
    assert_eq!(term.sink().replies.len(), ferrum_vt::MAX_COLOR_QUERIES);
}
