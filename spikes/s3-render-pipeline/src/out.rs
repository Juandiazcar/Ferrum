//! Line output. The harness prints `RESULT ...` lines that CI logs and the ADR quote verbatim.
//! `clippy::print_stdout` is denied, so lines are written explicitly.

use std::io::Write;

/// Write one line to stdout (line-buffered, so a sampler on the other end of a pipe sees it now).
pub(crate) fn emit(line: &str) {
    // A closed pipe means the reader is gone and there is nobody left to tell.
    writeln!(std::io::stdout().lock(), "{line}").ok();
}

/// `out!("RESULT x={}", 1)`: format and emit one line.
macro_rules! out {
    ($($arg:tt)*) => { $crate::out::emit(&format!($($arg)*)) };
}
