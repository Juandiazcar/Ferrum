//! Fuzz target for `ferrum_scrollback::decode_chunk` (NFR-REL-01: the daemon never panics on any
//! input; docs/08 §2). Arbitrary bytes stand for a corrupt sealed chunk or spill record.

#![no_main]

use ferrum_scrollback::{ChunkBuf, DecodedLine, decode_chunk};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut buf = ChunkBuf::new();
    if decode_chunk(data, &mut buf).is_ok() {
        // The contract: after Ok every line of the chunk decodes.
        let mut line = DecodedLine::new();
        for index in 0..buf.line_count() {
            assert!(matches!(buf.line(index, &mut line), Some(Ok(()))));
        }
    }
    // A failed decode must leave the buffer reusable for the next input.
    assert!(decode_chunk(data, &mut buf).is_ok() || buf.line_count() == 0);
});
