//! PT-25: the deterministic fake passes PT-01, 05, 07, 08, 10, 15 and 22.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use std::path::PathBuf;

use ferrum_testkit::fake_pty::FakeBackend;
use ferrum_testkit::pty_contract::{self as pt, Harness};

fn harness(name: &str) -> Harness<'static> {
    let scratch = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pty-contract-fake").join(name);
    std::fs::create_dir_all(&scratch).unwrap();
    Harness { backend: &FakeBackend, helper: "fake".into(), scratch, startup: pt::Startup::Plain }
}

#[test]
fn fake_backend_parity() {
    let h = harness("parity");
    pt::spawn_echo_roundtrip(&h);
    pt::exit_code_zero_nonzero_and_255(&h);
    pt::exit_detected_without_reader_eof(&h);
    pt::reader_ends_after_close(&h);
    pt::kill_is_truthful_and_idempotent(&h);
    pt::resize_updates_child_size(&h);
    pt::no_panic_on_hostile_input(&h);
}
