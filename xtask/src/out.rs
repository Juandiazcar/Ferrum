//! The only place in xtask that prints (docs/05 §8: no prints except xtask output helpers).

#![allow(clippy::print_stdout, clippy::print_stderr)]

/// Progress and results, on stdout.
pub(crate) fn info(message: &str) {
    println!("{message}");
}

/// Problems and failures, on stderr.
pub(crate) fn error(message: &str) {
    eprintln!("{message}");
}
