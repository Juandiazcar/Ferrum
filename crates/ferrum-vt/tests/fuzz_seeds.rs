//! Runs the `parse` fuzz body on the committed corpus, on random bytes and on random control
//! sequences, so a regression is caught by `cargo test` on every platform and not only by the
//! nightly fuzz job. Random bytes rarely form a valid CSI; the structured strategy does.

#[path = "support/fuzz_body.rs"]
mod fuzz_body;

use std::fs;
use std::path::Path;

use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

const HEADER_LEN: usize = 5;

#[test]
fn every_corpus_file_runs_clean() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/corpus/parse");
    let mut ran = 0;
    for entry in fs::read_dir(corpus).expect("corpus directory").flatten() {
        fuzz_body::run(&fs::read(entry.path()).expect("corpus file"));
        ran += 1;
    }
    assert!(ran > 0, "the corpus is empty");
}

/// `ESC [ params intermediate final`, with parameters up to and beyond `u16::MAX`.
fn csi() -> impl Strategy<Value = Vec<u8>> {
    let params = prop::collection::vec(prop::option::of(0u32..70_000), 0..5).prop_map(|list| {
        list.iter()
            .map(|p| p.map_or_else(String::new, |n| n.to_string()))
            .collect::<Vec<_>>()
            .join(";")
    });
    let marker = prop::sample::select(vec!["", "?", ">", "<", "=", "!", "$", " ", "\"", "?$"]);
    (params, marker, 0x40u8..0x7f).prop_map(|(params, marker, last)| {
        let mut bytes = b"\x1b[".to_vec();
        bytes.extend_from_slice(params.as_bytes());
        bytes.extend_from_slice(marker.as_bytes());
        bytes.push(last);
        bytes
    })
}

fn piece() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        5 => csi(),
        3 => "[ -~]{1,40}".prop_map(String::into_bytes),
        1 => prop::sample::select(vec!["\r\n", "\n", "\t", "\u{8}", "\x1bM", "\x1bD", "\x1bE", "\x1b7", "\x1b8", "\x1bc", "\x1b#8", "漢字", "e\u{301}", "👨\u{200d}👩"])
            .prop_map(|s: &str| s.as_bytes().to_vec()),
        1 => prop::collection::vec(any::<u8>(), 1..6),
    ]
}

/// Failing seeds are committed under `proptest-regressions/` (`docs/08` §2).
fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "proptest-regressions/fuzz_seeds.txt",
        ))),
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn random_bytes_run_clean(input in prop::collection::vec(any::<u8>(), 0..2_000)) {
        fuzz_body::run(&input);
    }

    #[test]
    fn random_control_sequences_run_clean(
        header in prop::collection::vec(any::<u8>(), HEADER_LEN),
        pieces in prop::collection::vec(piece(), 0..60),
    ) {
        let mut input = header;
        input.extend(pieces.concat());
        fuzz_body::run(&input);
    }
}
