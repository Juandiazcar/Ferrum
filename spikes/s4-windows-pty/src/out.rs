//! Output helper: all harness output goes through `std::io::Write` on a locked stdout (docs/05 §8).

use std::fmt::Arguments;
use std::io::Write as _;

/// Writes one line to stdout.
pub(crate) fn write_line(args: Arguments<'_>) {
    let mut out = std::io::stdout().lock();
    // A closed stdout (for example `| head`) leaves nothing useful to do in a throwaway harness.
    writeln!(out, "{args}").ok();
}

/// `say!("...", args)`: formatted line on stdout.
macro_rules! say {
    ($($arg:tt)*) => { $crate::out::write_line(format_args!($($arg)*)) };
}
pub(crate) use say;
