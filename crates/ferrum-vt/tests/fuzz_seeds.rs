//! Runs the `parse` fuzz body on the committed corpus and on random bytes, so a regression is
//! caught by `cargo test` on every platform and not only by the nightly fuzz job.

#[path = "support/fuzz_body.rs"]
mod fuzz_body;

use std::fs;
use std::path::Path;

use proptest::prelude::*;

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

proptest! {
    #[test]
    fn random_bytes_run_clean(input in prop::collection::vec(any::<u8>(), 0..2_000)) {
        fuzz_body::run(&input);
    }
}
