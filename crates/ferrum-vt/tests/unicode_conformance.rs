//! Conformance with the official Unicode data files (`docs/08` §4, FR-TERM-003), committed
//! unmodified in `fixtures/unicode/` (Unicode 17.0.0, the version of the `unicode-segmentation`
//! tables; attribution in `fixtures/vt/THIRD_PARTY.md`):
//!
//! - `GraphemeBreakTest.txt` (UAX #29): every line is the cluster split the text must have, on
//!   the segmentation functions and, for text without control characters, on a terminal.
//! - `emoji-sequences.txt` and `emoji-zwj-sequences.txt` (UTS #51): every recommended sequence is
//!   one cluster that takes two columns.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use std::fs;
use std::path::{Path, PathBuf};

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{CellFlags, NullSink, Size, Terminal, char_width, cluster_width, clusters};

const COLS: u16 = 64;
const EMOJI_COLUMNS: usize = 2;
/// Marks "break here" in `GraphemeBreakTest.txt`.
const BREAK: &str = "÷";
const NO_BREAK: &str = "×";

fn data(name: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/unicode");
    fs::read_to_string(path.join(name)).expect("Unicode data file")
}

/// The scalars in a run of hex code points.
fn scalars(codes: &str) -> String {
    codes
        .split_whitespace()
        .map(|code| {
            char::from_u32(u32::from_str_radix(code, 16).expect("hex code point"))
                .expect("a scalar value")
        })
        .collect()
}

/// `(text, clusters)` of every test line.
fn grapheme_cases() -> Vec<(String, Vec<String>)> {
    let mut cases = Vec::new();
    for line in data("GraphemeBreakTest.txt").lines() {
        let body = line.split('#').next().unwrap_or_default().trim();
        if body.is_empty() {
            continue;
        }
        let mut parts: Vec<String> = Vec::new();
        let mut text = String::new();
        for token in body.split_whitespace() {
            if token == BREAK || token == NO_BREAK {
                if token == BREAK && !parts.last().is_some_and(|_| text.is_empty()) {
                    parts.push(String::new());
                }
                continue;
            }
            let scalar = scalars(token);
            text.push_str(&scalar);
            if let Some(last) = parts.last_mut() {
                last.push_str(&scalar);
            }
        }
        parts.retain(|part| !part.is_empty());
        cases.push((text, parts));
    }
    cases
}

#[test]
fn the_grapheme_break_test_file_is_fully_loaded() {
    // Unicode 17.0.0 ships 766 test lines; a short parse means the loader is wrong.
    assert!(grapheme_cases().len() > 700);
}

#[test]
fn clusters_follow_every_line_of_grapheme_break_test() {
    for (text, expected) in grapheme_cases() {
        let got: Vec<&str> = clusters(&text).collect();
        assert_eq!(got, expected, "{text:?}");
    }
}

/// The cluster texts the first row of `term` holds (spacers merged into their head cell).
fn first_row_clusters(term: &Terminal<NullSink>) -> Vec<String> {
    let grid = term.grid();
    let Some(row) = grid.row(0) else { return Vec::new() };
    row.cells()
        .iter()
        .filter(|cell| !cell.flags().contains(CellFlags::SPACER))
        .map(|cell| {
            let mut buffer = [0; 4];
            grid.cell_text(cell, &mut buffer).to_owned()
        })
        .collect()
}

fn terminal() -> Terminal<NullSink> {
    Terminal::new(Size::new(COLS, 2), Scrollback::new(ScrollbackConfig::default()), NullSink)
}

#[test]
fn a_terminal_prints_the_printable_lines_of_grapheme_break_test_as_those_clusters() {
    let mut failures = Vec::new();
    for (text, expected) in grapheme_cases() {
        // Controls (C0, DEL, C1) are executed, not printed; the cases that start with a zero
        // width cluster have no cell to join and are covered by the fixtures
        // `unicode/zero_width__malformed_at_line_start`.
        let printable = text.chars().all(|c| !c.is_control() && !c.is_whitespace() || c == ' ');
        // A scalar of zero width never takes a cell: it joins the cell before it, even where
        // UAX #29 starts a cluster (a Hangul vowel after a consonant of another script).
        let starts_zero =
            |cluster: &String| cluster.chars().next().is_some_and(|c| char_width(c) == 0);
        if !printable || expected.iter().any(starts_zero) {
            continue;
        }
        let mut term = terminal();
        term.parse(text.as_bytes());
        let mut got = first_row_clusters(&term);
        got.truncate(expected.len());
        if got != expected {
            failures.push(format!("{text:?}: expected {expected:?}, got {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{} lines differ:\n{}", failures.len(), failures.join("\n"));
}

/// The sequences of an emoji data file (`<code points> ; <type> ; ...`), ranges expanded.
fn emoji_sequences(name: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in data(name).lines() {
        let body = line.split('#').next().unwrap_or_default();
        let Some(codes) = body.split(';').next().map(str::trim).filter(|c| !c.is_empty()) else {
            continue;
        };
        match codes.split_once("..") {
            Some((first, last)) => {
                let hex = |digits| u32::from_str_radix(digits, 16).expect("range bound");
                let range = hex(first)..=hex(last);
                found.extend(range.filter_map(char::from_u32).map(String::from));
            }
            None => found.push(scalars(codes)),
        }
    }
    found
}

fn check_emoji(name: &str) {
    let sequences = emoji_sequences(name);
    assert!(sequences.len() > 100, "{name} is not loaded");
    for text in sequences {
        let got: Vec<&str> = clusters(&text).collect();
        assert_eq!(got, [text.as_str()], "{text:?} is one cluster");
        assert_eq!(cluster_width(&text), EMOJI_COLUMNS, "{text:?} is two columns");
        let mut term = terminal();
        term.parse(text.as_bytes());
        assert_eq!(first_row_clusters(&term).first(), Some(&text), "{text:?} in one cell");
        assert_eq!(term.cursor().x, EMOJI_COLUMNS, "cursor after {text:?}");
    }
}

#[test]
fn every_recommended_emoji_sequence_is_one_double_width_cluster() {
    check_emoji("emoji-sequences.txt");
}

#[test]
fn every_recommended_emoji_zwj_sequence_is_one_double_width_cluster() {
    check_emoji("emoji-zwj-sequences.txt");
}
