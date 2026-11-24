//! Fuzz target for `ferrum_proto` decoding (`docs/08` §2: `proto::decode`; NFR-REL-01: the daemon
//! never panics on any byte stream): frames, both message directions and the rows they carry.
//! The body is shared with `tests/fuzz_seeds.rs`.

#![no_main]

#[path = "../../tests/support/fuzz_body.rs"]
mod fuzz_body;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| fuzz_body::run(input));
