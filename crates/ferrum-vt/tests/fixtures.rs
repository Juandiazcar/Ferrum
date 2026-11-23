//! Fixture runner (`docs/08` §4, §6): every `fixtures/vt/<topic>/<case>.in` is fed to a fresh
//! terminal and the resulting state (`Terminal::dump`, plus the events the sink saw) is compared
//! with the insta snapshot next to it.
//!
//! Layout and naming (checked by the meta tests below):
//! - `<sequence>__<variant>.in`: raw bytes; `<sequence>__<variant>.md`: hex annotation;
//!   `<sequence>__<variant>.snap`: the snapshot; optional `<sequence>__<variant>.size`: `COLSxROWS`
//!   (default 20x6).
//! - every sequence listed in `fixtures/vt/implemented.txt` has at least one positive variant and
//!   one variant starting with `malformed`.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{Event, EventSink, Size, Terminal};

const DEFAULT_COLS: u16 = 20;
const DEFAULT_ROWS: u16 = 6;
const HISTORY_LINES: usize = 100;
const SPILL_THRESHOLD_LINES: usize = 1_024;

#[derive(Default)]
struct Recorder(Vec<String>);

impl EventSink for Recorder {
    fn event(&mut self, event: Event<'_>) {
        let text = match event {
            Event::Reply(bytes) => format!("Reply {}", bytes.escape_ascii()),
            Event::ClipboardStore { selection, bytes } => {
                format!("ClipboardStore {selection} {}", bytes.escape_ascii())
            }
            other => format!("{other:?}"),
        };
        self.0.push(text);
    }
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vt")
}

fn cases() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(topics) = fs::read_dir(fixtures_root()) else { return found };
    for topic in topics.flatten().filter(|entry| entry.path().is_dir()) {
        for file in fs::read_dir(topic.path()).expect("topic directory").flatten() {
            if file.path().extension().is_some_and(|ext| ext == "in") {
                found.push(file.path());
            }
        }
    }
    found.sort();
    found
}

fn size_of_case(case: &Path) -> Size {
    let Ok(text) = fs::read_to_string(case.with_extension("size")) else {
        return Size::new(DEFAULT_COLS, DEFAULT_ROWS);
    };
    let (cols, rows) = text.trim().split_once('x').expect("size is COLSxROWS");
    Size::new(cols.parse().expect("columns"), rows.parse().expect("rows"))
}

fn render(case: &Path) -> String {
    let input = fs::read(case).expect("fixture input");
    let config = ScrollbackConfig::new(HISTORY_LINES, SPILL_THRESHOLD_LINES).expect("config");
    let mut terminal =
        Terminal::new(size_of_case(case), Scrollback::new(config), Recorder::default());
    terminal.parse(&input);
    let mut out = terminal.dump();
    let events = &terminal.sink().0;
    if !events.is_empty() {
        let _ = writeln!(out, "events");
        for event in events {
            let _ = writeln!(out, "  {event}");
        }
    }
    out
}

fn check(case: &Path) {
    let name = case.file_stem().and_then(|stem| stem.to_str()).expect("file name").to_owned();
    let dir = case.parent().expect("topic directory").to_path_buf();
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(dir);
    settings.set_prepend_module_to_snapshot(false);
    settings.set_omit_expression(true);
    settings.bind(|| insta::assert_snapshot!(name, render(case)));
}

#[test]
fn every_fixture_matches_its_snapshot() {
    let cases = cases();
    assert!(!cases.is_empty(), "no fixtures found under fixtures/vt");
    let failures: Vec<String> = cases
        .iter()
        .filter(|case| std::panic::catch_unwind(|| check(case)).is_err())
        .map(|case| case.display().to_string())
        .collect();
    assert!(failures.is_empty(), "{} fixtures differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_fixture_has_a_hex_annotation() {
    for case in cases() {
        let notes = fs::read_to_string(case.with_extension("md")).unwrap_or_default();
        assert!(notes.contains("| hex |"), "{} has no hex annotation table", case.display());
    }
}

/// `<topic>/<sequence>` names of the implemented sequences (`fixtures/vt/implemented.txt`).
fn implemented_sequences() -> BTreeSet<String> {
    let text = fs::read_to_string(fixtures_root().join("implemented.txt")).expect("list");
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_implemented_sequence_has_a_positive_and_a_malformed_case() {
    let mut variants: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for case in cases() {
        let stem = case.file_stem().and_then(|stem| stem.to_str()).expect("file name");
        let topic = case.parent().and_then(|dir| dir.file_name()).and_then(|n| n.to_str());
        let (sequence, variant) = stem.split_once("__").expect("name is <sequence>__<variant>");
        variants
            .entry(format!("{}/{sequence}", topic.unwrap_or_default()))
            .or_default()
            .insert(variant.to_owned());
    }
    let implemented = implemented_sequences();
    for sequence in &implemented {
        let names = variants.get(sequence).unwrap_or_else(|| panic!("{sequence}: no fixture"));
        assert!(
            names.iter().any(|name| name.starts_with("malformed")),
            "{sequence}: no malformed case"
        );
        assert!(
            names.iter().any(|name| !name.starts_with("malformed")),
            "{sequence}: no positive case"
        );
    }
    for sequence in variants.keys() {
        assert!(implemented.contains(sequence), "{sequence}: fixture for an unlisted sequence");
    }
}
