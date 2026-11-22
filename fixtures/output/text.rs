//! Text of the fixture lines: log-like content from a small vocabulary, or random printable ASCII.

use std::fmt::Write as _;

use super::rng::Rng;

const WORDS: &[&str] = &[
    "error",
    "warning",
    "build",
    "target",
    "release",
    "debug",
    "cargo",
    "check",
    "test",
    "passed",
    "failed",
    "running",
    "finished",
    "compiling",
    "linking",
    "module",
    "session",
    "scrollback",
    "chunk",
    "buffer",
    "thread",
    "daemon",
    "socket",
    "resize",
    "reflow",
    "damage",
    "frame",
    "glyph",
    "cache",
    "style",
    "config",
    "profile",
    "request",
    "response",
    "timeout",
    "retry",
    "connect",
    "closed",
    "opened",
    "loaded",
    "parsed",
    "encoded",
    "sealed",
    "spilled",
    "evicted",
    "unused",
    "variable",
    "function",
    "struct",
    "trait",
    "borrow",
    "lifetime",
    "expected",
    "found",
    "value",
    "index",
    "offset",
    "length",
    "total",
    "average",
    "median",
    "latency",
    "throughput",
    "memory",
    "stack",
    "heap",
];

/// Non-ASCII scalars: accented Latin (1 column), CJK and emoji (2 columns).
const NON_ASCII: &[char] = &['é', 'ü', 'ñ', 'ø', '漢', '字', '日', '本', '語', '😀', '🚀', '✨'];

/// Terminal columns a scalar occupies in the fixtures (only the scalars generated here matter).
pub(crate) fn char_width(c: char) -> usize {
    let wide = matches!(u32::from(c), 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0x1F300..=0x1FAFF);
    if wide { 2 } else { 1 }
}

fn word(rng: &mut Rng) -> &'static str {
    rng.pick(WORDS).copied().unwrap_or("x")
}

/// One of six log-like line shapes, padded with vocabulary words to `target` columns.
pub(crate) fn mixed_line(rng: &mut Rng, target: usize) -> String {
    let mut line = String::new();
    let (a, b, c) = (rng.below(100), rng.below(10_000), rng.below(256));
    // write! to a String cannot fail.
    match rng.below(6) {
        0 => {
            write!(line, "2026-11-22T12:{a:02}:{c:02}Z INFO  ferrum::{}: {}", word(rng), word(rng))
        }
        1 => write!(
            line,
            "warning: unused variable `{}` --> src/{}.rs:{a}:{c}",
            word(rng),
            word(rng)
        ),
        2 => {
            write!(line, "{{\"id\": {b}, \"name\": \"{}\", \"ok\": true, \"count\": {c}", word(rng))
        }
        3 => write!(line, "-rw-r--r--  1 user staff {b:>6} Nov 22 12:{a:02} {}.rs", word(rng)),
        4 => write!(line, "* {b:04x}{c:02x} (HEAD -> main) {}", word(rng)),
        _ => write!(line, "{b:08x}  {c:02x} {a:02x} {c:02x} {a:02x} de ad be ef"),
    }
    .ok();
    while line.len() < target {
        line.push(' ');
        line.push_str(word(rng));
    }
    line.truncate(target);
    line
}

/// Uniformly random printable ASCII: the lower bound for any compressor.
pub(crate) fn entropy_line(rng: &mut Rng, target: usize) -> String {
    (0..target).map(|_| char::from(u8::try_from(rng.between(0x20, 0x7E)).unwrap_or(b' '))).collect()
}

/// Replaces one to four scalars of `line` with non-ASCII ones.
pub(crate) fn inject_non_ascii(rng: &mut Rng, line: &str) -> String {
    let mut chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    for _ in 0..rng.between(1, 4) {
        let at = rng.below(chars.len());
        if let (Some(slot), Some(&replacement)) = (chars.get_mut(at), rng.pick(NON_ASCII)) {
            *slot = replacement;
        }
    }
    chars.into_iter().collect()
}
