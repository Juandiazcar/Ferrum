//! Fuzz target for `ferrum_vt::Terminal::parse` and what hangs off it: reflow, selection and the
//! text dumps (NFR-REL-01: the daemon never panics on any byte stream; `docs/08` §2). The body is
//! shared with `tests/fuzz_seeds.rs`.

#![no_main]

#[path = "../../tests/support/fuzz_body.rs"]
mod fuzz_body;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| fuzz_body::run(data));
