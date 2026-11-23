//! The front end must not care how the byte stream is cut into reads: the daemon reads the PTY
//! in arbitrary chunks (`docs/02` §5), and the fast paths in `input.rs` keep state between calls.

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{Event, EventSink, MAX_OSC_BYTES, Size, Terminal};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const COLS: u16 = 12;
const ROWS: u16 = 5;
const HISTORY_LINES: usize = 200;
const SPILL_LINES: usize = 1_024;

#[derive(Default)]
struct Log(Vec<String>);

impl EventSink for Log {
    fn event(&mut self, event: Event<'_>) {
        self.0.push(format!("{event:?}"));
    }
}

fn terminal() -> Terminal<Log> {
    let config = ScrollbackConfig::new(HISTORY_LINES, SPILL_LINES).unwrap_or_default();
    Terminal::new(Size::new(COLS, ROWS), Scrollback::new(config), Log::default())
}

fn state(term: &Terminal<Log>) -> String {
    format!("{}{:?}", term.dump(), term.sink().0)
}

/// Pieces that exercise every path of the front end.
fn piece() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        4 => "[ -~]{1,30}".prop_map(String::into_bytes),
        2 => prop::sample::select(vec![
            "\r\n", "\n", "\r", "\t", "\u{8}", "\u{7}", "\u{e}", "\u{f}", "\u{18}", "\u{1a}", "\u{7f}",
        ]).prop_map(|s: &str| s.as_bytes().to_vec()),
        3 => prop::sample::select(vec![
            "\x1b[1;31m", "\x1b[0m", "\x1b[2J", "\x1b[3;4H", "\x1b[K", "\x1b[2L", "\x1b[3P", "\x1b[?25l",
            "\x1b[?1049h", "\x1b[?1049l", "\x1b[2;4r", "\x1b[r", "\x1b7", "\x1b8", "\x1bM", "\x1bD",
            "\x1b(0", "\x1b(B", "\x1b[38:2::1:2:3m", "\x1b[4:3m", "\x1b[?7l", "\x1b[?7h", "\x1b[4h",
            "\x1b[4l", "\x1b[999999999999;1H", "\x1b[", "\x1b", "\x1b]", "\x1b[?", "\x1b[1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17;18;19;20;21;22;23;24;25;26;27;28;29;30;31;32;33;34m",
        ]).prop_map(|s: &str| s.as_bytes().to_vec()),
        2 => prop::sample::select(vec![
            "\x1b]0;title\x07", "\x1b]2;t\x1b\\", "\x1b]1;i\x07", "\x1b]2;cut", "\x1b]x;y\x07",
        ]).prop_map(|s: &str| s.as_bytes().to_vec()),
        2 => prop::sample::select(vec![
            "\x1bP+q544e\x1b\\", "\x1bP$qm\x1b\\", "\x1bP$qr", "\x1bP+q", "\x1bPq#0;2~~", "\x1b_Gf=1;AAAA",
            "\x1b^pm", "\x1bXsos", "\u{9c}", "\x1b[1?h", "\x1b[1;2$~",
        ]).prop_map(|s: &str| s.as_bytes().to_vec()),
        2 => prop::sample::select(vec!["é", "漢字", "e\u{301}", "👨‍👩‍👧", "🇺🇸", "\u{2764}\u{fe0f}", "ｱ"])
            .prop_map(|s: &str| s.as_bytes().to_vec()),
        1 => prop::collection::vec(any::<u8>(), 1..8),
    ]
}

fn stream() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(piece(), 1..40).prop_map(|pieces| pieces.concat())
}

/// Failing seeds are committed under `proptest-regressions/` (`docs/08` §2); the case count is
/// proptest's default (256, or `PROPTEST_CASES`).
fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "proptest-regressions/chunking.txt",
        ))),
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn the_state_does_not_depend_on_how_the_stream_is_chunked(
        bytes in stream(),
        cuts in prop::collection::vec(any::<prop::sample::Index>(), 0..12),
    ) {
        let mut whole = terminal();
        whole.parse(&bytes);

        let mut points: Vec<usize> = cuts.iter().map(|cut| cut.index(bytes.len() + 1)).collect();
        points.sort_unstable();
        let mut pieces = terminal();
        let mut from = 0;
        for point in points.into_iter().chain([bytes.len()]) {
            pieces.parse(bytes.get(from..point).unwrap_or_default());
            from = point;
        }
        prop_assert_eq!(state(&whole), state(&pieces));

        let mut single = terminal();
        for byte in &bytes {
            single.parse(std::slice::from_ref(byte));
        }
        prop_assert_eq!(state(&whole), state(&single));
    }

    #[test]
    fn arbitrary_bytes_never_panic_and_keep_the_cursor_inside_the_grid(bytes in prop::collection::vec(any::<u8>(), 0..400)) {
        let mut term = terminal();
        term.parse(&bytes);
        prop_assert!(term.cursor().x < usize::from(COLS));
        prop_assert!(term.cursor().y < usize::from(ROWS));
    }
}

#[test]
fn an_osc_over_the_cap_is_discarded_and_the_terminal_recovers() {
    let mut term = terminal();
    let mut payload = b"\x1b]2;".to_vec();
    payload.resize(payload.len() + MAX_OSC_BYTES + 10, b'x');
    payload.extend_from_slice(b"\x07ok");
    term.parse(&payload);
    assert!(
        !term.sink().0.iter().any(|event| event.starts_with("Title")),
        "an over-long OSC is dropped"
    );
    assert!(term.dump().contains("ok"), "the parser is back in the ground state");
}

#[test]
fn an_osc_over_the_cap_is_dropped_up_to_its_terminator_and_later_output_is_shown() {
    let mut term = terminal();
    term.parse(b"\x1b]2;");
    for _ in 0..4 {
        term.parse(&vec![b'y'; MAX_OSC_BYTES]);
    }
    term.parse(b"\r\nhidden\x07visible");
    assert!(term.sink().0.is_empty(), "the capped title is never dispatched");
    assert!(!term.dump().contains("yy") && !term.dump().contains("hidden"), "{}", term.dump());
    assert!(term.dump().contains("visible"), "output after the terminator is shown");
}

#[test]
fn a_capped_osc_ends_at_bel_can_sub_or_esc() {
    for (end, shown) in [(&b"\x07x"[..], "x"), (b"\x18x", "x"), (b"\x1ax", "x"), (b"\x1b[Cx", " x")]
    {
        let mut input = b"\x1b]2;".to_vec();
        input.resize(MAX_OSC_BYTES + 10, b'Q');
        input.extend_from_slice(end);
        let mut term = terminal();
        term.parse(&input);
        assert!(term.dump().contains(&format!("00 |{shown}")), "{end:?}: {}", term.dump());
        assert_eq!(term.sink().0, Vec::<String>::new());
    }
}

#[test]
fn an_esc_at_the_end_of_a_chunk_followed_by_a_bracket_in_the_next_starts_an_osc() {
    let mut term = terminal();
    term.parse(b"a\x1b");
    term.parse(b"]2;split\x07b");
    assert_eq!(term.sink().0, vec![String::from("Title(\"split\")")]);
    assert!(term.dump().contains("00 |ab"));
}

#[test]
fn an_empty_read_between_an_esc_and_its_bracket_changes_nothing() {
    let mut term = terminal();
    term.parse(b"a");
    term.parse(b"");
    term.parse(b"]2;splitb");
    assert_eq!(term.sink().0, vec![String::from("Title(\"split\")")]);
}

#[test]
fn a_dcs_string_ended_by_the_esc_of_an_osc_is_answered_however_the_bytes_are_cut() {
    let input = b"\x1bP+q544e\x1b]0;t\x07";
    let mut expected = None;
    for cut in 0..=input.len() {
        let mut term = terminal();
        term.parse(input.get(..cut).unwrap_or_default());
        term.parse(input.get(cut..).unwrap_or_default());
        let events = term.sink().0.clone();
        assert!(events.iter().any(|e| e.starts_with("Reply")), "cut {cut}: {events:?}");
        assert_eq!(*expected.get_or_insert_with(|| events.clone()), events, "cut {cut}");
    }
}

#[test]
fn the_plain_sgr_shortcut_gives_the_same_cells_as_the_generic_path() {
    let sequences = [
        "\x1b[m",
        "\x1b[0m",
        "\x1b[;m",
        "\x1b[1;m",
        "\x1b[31m",
        "\x1b[1;32;44m",
        "\x1b[38;5;208m",
        "\x1b[38;2;10;120;250m",
        "\x1b[48;5;300m",
        "\x1b[4m",
        "\x1b[22;24m",
        "\x1b[38;2;1;2m",
        "\x1b[0001m",
        "\x1b[12345m",
        "\x1b[1;2;3;4;5;6;7;8;9;1;2;3;4;5;6;7;8m",
        "\x1b[1;2;3;4;5;6;7;8;9;1;2;3;4;5;6;7m",
        "\x1b[1:2m",
        "\x1b[?1m",
        "\x1b[1 m",
    ];
    for sequence in sequences {
        let text = format!("a{sequence}b\x1b[0mc");
        let mut whole = terminal();
        whole.parse(text.as_bytes());
        let mut single = terminal();
        for byte in text.as_bytes() {
            single.parse(std::slice::from_ref(byte));
        }
        let cells = |term: &Terminal<Log>| format!("{:?}", term.grid().row(0));
        assert_eq!(cells(&whole), cells(&single), "{sequence:?}");
        assert_eq!(state(&whole), state(&single), "{sequence:?}");
    }
}
