//! Property tests: encode/decode round trip (including wide, CJK, emoji and combining text),
//! chunk sealing and random access (`docs/08` §2).

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/lines.rs"]
mod lines;

use ferrum_scrollback::{
    HOT_CHUNK_LINES, LineCursor, LineFlags, Scrollback, ScrollbackConfig, Style,
};
use lines::{GenLine, numbered, push, read};
use proptest::prelude::*;

const STYLE_CHOICES: u32 = 5;
/// RAM threshold above every `keep` the property uses, so the spill cap never interferes.
const RAM_LINES: usize = 4_096;

fn style() -> impl Strategy<Value = Style> {
    (0..STYLE_CHOICES, 0..2u32, 0..3u16).prop_map(|(fg, bg, attrs)| Style { fg, bg, attrs })
}

/// ASCII mostly, with CJK, emoji, ZWJ sequences, combining marks, wide and zero-width scalars.
fn scalar() -> impl Strategy<Value = char> {
    prop_oneof![
        8 => prop::char::range(' ', '~'),
        1 => Just(' '),
        1 => prop::sample::select(vec!['é', '\u{0301}', '漢', '字', 'ｱ', '😀', '\u{200D}', '👨', '\u{FE0F}', '\u{0}', '\t']),
        1 => any::<char>(),
    ]
}

fn flags() -> impl Strategy<Value = LineFlags> {
    (0u8..16).prop_map(|bits| LineFlags::from_bits(bits).unwrap_or(LineFlags::NONE))
}

fn gen_line() -> impl Strategy<Value = GenLine> {
    let cells = prop::collection::vec((scalar(), style()), 0..120);
    (flags(), cells).prop_map(|(flags, cells)| GenLine { flags, cells })
}

fn small_store() -> Scrollback {
    Scrollback::new(ScrollbackConfig::default())
}

proptest! {
    #[test]
    fn single_line_round_trips(line in gen_line(), cuts in prop::collection::vec(0usize..120, 0..6)) {
        let mut store = small_store();
        push(&mut store, &line, &cuts);
        let mut cursor = LineCursor::new();
        prop_assert_eq!(read(&store, 0, &mut cursor), Some(line.expected()));
        prop_assert_eq!(store.len(), 1);
    }

    #[test]
    fn lines_round_trip_through_sealed_chunks(
        batch in prop::collection::vec(gen_line(), 1..40),
        repeat in 1usize..80,
    ) {
        // `repeat` copies of the batch exceed 2 hot chunks, so reads cross seal boundaries.
        let mut store = small_store();
        let all: Vec<&GenLine> = (0..repeat).flat_map(|_| batch.iter()).collect();
        for line in &all {
            push(&mut store, line, &[]);
        }
        prop_assert_eq!(store.len(), all.len());
        let mut cursor = LineCursor::new();
        for (index, line) in all.iter().enumerate() {
            prop_assert_eq!(read(&store, index, &mut cursor), Some(line.expected()), "line {}", index);
        }
        prop_assert!(read(&store, all.len(), &mut cursor).is_none());
    }

    #[test]
    fn random_access_after_eviction_maps_to_the_newest_lines(
        total in 1usize..4_000,
        keep in 1usize..2_500,
        probes in prop::collection::vec(any::<prop::sample::Index>(), 1..30),
    ) {
        let config = ScrollbackConfig::new(keep, RAM_LINES).expect("config");
        let mut store = Scrollback::new(config);
        for n in 0..total {
            push(&mut store, &numbered(n), &[]);
        }
        let retained = total.min(keep);
        prop_assert_eq!(store.len(), retained);
        prop_assert_eq!(store.dropped_lines(), u64::try_from(total - retained).unwrap_or(0));
        let first = total - retained;
        let mut cursor = LineCursor::new();
        for probe in probes {
            let index = probe.index(retained);
            prop_assert_eq!(read(&store, index, &mut cursor), Some(numbered(first + index).expected()));
        }
    }
}

#[test]
fn sealing_happens_at_exactly_one_hot_chunk() {
    let mut store = small_store();
    for n in 0..HOT_CHUNK_LINES - 1 {
        push(&mut store, &numbered(n), &[]);
    }
    assert_eq!(store.memory_report().sealed_bytes, 0, "no chunk is sealed before it is full");
    push(&mut store, &numbered(HOT_CHUNK_LINES - 1), &[]);
    assert!(store.memory_report().sealed_bytes > 0, "the 1 024th line seals the chunk");
    let mut cursor = LineCursor::new();
    for n in 0..HOT_CHUNK_LINES {
        assert_eq!(read(&store, n, &mut cursor), Some(numbered(n).expected()));
    }
}

#[test]
fn zero_max_lines_keeps_nothing_and_never_fails() {
    let mut store = Scrollback::new(ScrollbackConfig::new(0, HOT_CHUNK_LINES).expect("config"));
    for n in 0..3_000 {
        push(&mut store, &numbered(n), &[]);
    }
    assert!(store.is_empty());
    assert_eq!(store.pushed_lines(), 3_000);
}

#[test]
fn a_line_over_the_limits_is_kept_clipped_and_counted() {
    let mut scrollback = Scrollback::new(ScrollbackConfig::default());
    scrollback
        .push_line(&ferrum_scrollback::LineRef {
            flags: LineFlags::NONE,
            text: "before",
            runs: &[],
        })
        .expect("push");
    let long = "é".repeat(ferrum_scrollback::MAX_LINE_TEXT_BYTES);
    let runs = [ferrum_scrollback::StyledRun {
        style: Style { fg: 1, bg: 0, attrs: 0 },
        len: long.chars().count(),
    }];
    let line = ferrum_scrollback::LineRef { flags: LineFlags::WRAPPED, text: &long, runs: &runs };
    assert!(scrollback.push_line(&line).is_err(), "the loss is reported");
    let mismatched =
        ferrum_scrollback::LineRef { flags: LineFlags::NONE, text: "abc", runs: &runs };
    assert!(scrollback.push_line(&mismatched).is_err(), "the loss is reported");
    assert_eq!((scrollback.len(), scrollback.pushed_lines()), (3, 3), "every line counts");
    let mut cursor = LineCursor::new();
    let clipped = scrollback.read_line(1, &mut cursor).expect("read").expect("line");
    assert_eq!(clipped.flags, LineFlags::WRAPPED);
    assert!(clipped.text.len() <= ferrum_scrollback::MAX_LINE_TEXT_BYTES);
    assert!(long.starts_with(clipped.text.as_str()) && !clipped.text.is_empty());
    let plain = scrollback.read_line(2, &mut cursor).expect("read").expect("line");
    assert_eq!(plain.text, "abc");
}
