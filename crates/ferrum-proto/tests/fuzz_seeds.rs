//! Runs the `decode` fuzz body on the committed corpus and on random bytes, so a regression is
//! caught by `cargo test` on every platform and not only by the nightly fuzz job.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "support/fuzz_body.rs"]
mod fuzz_body;

use std::fs;
use std::path::Path;

use proptest::prelude::*;

#[test]
fn every_corpus_file_runs_clean() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/corpus/decode");
    let mut ran = 0;
    for entry in fs::read_dir(corpus).expect("corpus directory").flatten() {
        fuzz_body::run(&fs::read(entry.path()).expect("corpus file"));
        ran += 1;
    }
    assert!(ran > 0, "the corpus is empty");
}

proptest! {
    #[test]
    fn random_bytes_run_clean(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        fuzz_body::run(&bytes);
    }
}
