//! Deterministic, seeded terminal-output generator matching the reference scenario of docs/03 §1:
//! 35 % colored lines (3 to 5 SGR runs, mean 4), ~70 visible columns on average, 5 % lines with
//! non-ASCII (CJK, emoji, accented Latin), 2 % very long lines (300 to 400 columns).
//!
//! Kept in the spike (not `fixtures/output/`): it is throwaway and not yet the reusable product
//! fixture generator that WP-01/WP-08 will write.

use std::fmt::Write as _;

use unicode_width::UnicodeWidthChar;

/// splitmix64: tiny deterministic PRNG, no dependency.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (modulo bias is irrelevant for fixture mixing).
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n.max(1) as u64)).unwrap_or(0)
    }

    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        items.get(self.below(items.len()))
    }
}

/// Text source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Profile {
    /// Synthetic log-like lines from a small vocabulary (optimistic for compression).
    Mixed,
    /// Real prose from files on disk, ASCII-sanitized (the spec markdown by default).
    Corpus,
    /// Uniformly random printable ASCII (pessimistic bound for compression).
    Entropy,
}

/// Counters describing what was actually generated (never assumed).
#[derive(Default, Clone)]
pub(crate) struct Stats {
    pub(crate) lines: usize,
    pub(crate) colored: usize,
    pub(crate) non_ascii: usize,
    pub(crate) long: usize,
    pub(crate) cols: usize,
    pub(crate) bytes: usize,
    pub(crate) sgr_runs: usize,
}

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
    "src",
    "lib",
    "main",
    "mod",
    "use",
    "fn",
    "impl",
    "struct",
    "enum",
    "trait",
    "async",
    "await",
    "result",
    "option",
    "value",
    "buffer",
    "session",
    "daemon",
    "client",
    "socket",
    "frame",
    "render",
    "parse",
    "scroll",
    "chunk",
    "index",
    "offset",
    "style",
    "color",
    "theme",
    "config",
    "default",
    "update",
    "commit",
    "branch",
    "merge",
    "rebase",
    "origin",
    "remote",
    "fetch",
    "push",
    "pull",
    "status",
    "clean",
    "modified",
    "deleted",
    "renamed",
    "untracked",
    "staged",
    "diff",
    "patch",
    "hunk",
    "line",
    "column",
    "token",
    "symbol",
    "module",
    "crate",
    "version",
    "feature",
    "dependency",
    "lock",
    "request",
    "response",
    "timeout",
    "retry",
    "connect",
    "closed",
    "opened",
    "bytes",
    "read",
    "write",
    "flush",
    "queue",
    "thread",
    "worker",
    "task",
    "event",
    "signal",
    "memory",
    "alloc",
    "free",
    "heap",
    "stack",
    "cache",
    "hit",
    "miss",
    "latency",
    "throughput",
    "the",
    "of",
    "and",
    "to",
    "in",
    "is",
    "for",
    "with",
    "on",
    "at",
    "from",
    "by",
    "not",
    "found",
    "invalid",
];
const CRATES: &[&str] =
    &["serde", "tokio", "vte", "lz4_flex", "memchr", "bitflags", "smallvec", "anyhow", "regex"];
const CJK: &[char] = &[
    '\u{4F60}', '\u{597D}', '\u{4E16}', '\u{754C}', '\u{7EC8}', '\u{7AEF}', '\u{6D4B}', '\u{8BD5}',
    '\u{6587}', '\u{4EF6}', '\u{9519}', '\u{8BEF}', '\u{65E5}', '\u{672C}', '\u{8A9E}', '\u{30C6}',
    '\u{30B9}', '\u{30C8}', '\u{D55C}', '\u{AE00}',
];
const EMOJI: &[char] = &[
    '\u{1F680}',
    '\u{2705}',
    '\u{274C}',
    '\u{1F525}',
    '\u{1F41B}',
    '\u{1F4E6}',
    '\u{26A1}',
    '\u{1F389}',
];
const SGR: &[&str] = &[
    "31",
    "32",
    "33",
    "34",
    "35",
    "36",
    "1;31",
    "1;32",
    "1;33",
    "1;34",
    "90",
    "91",
    "92",
    "38;5;208",
    "38;5;39",
    "38;5;245",
    "38;5;141",
    "38;2;255;128;0",
    "38;2;80;200;120",
    "4",
    "7",
    "1;4;36",
    "37;41",
    "30;43",
    "38;5;250;48;5;236",
];

/// The generator. All scratch buffers are reused, so steady-state generation does not allocate.
pub(crate) struct Generator {
    rng: Rng,
    profile: Profile,
    corpus: Vec<String>,
    corpus_pos: usize,
    text: String,
    width: usize,
    /// What has been generated so far.
    pub(crate) stats: Stats,
}

impl Generator {
    pub(crate) fn new(seed: u64, profile: Profile, corpus: Vec<String>) -> Self {
        let mut rng = Rng::new(seed);
        let corpus_pos = rng.below(corpus.len().max(1));
        Self {
            rng,
            profile,
            corpus,
            corpus_pos,
            text: String::with_capacity(1024),
            width: 0,
            stats: Stats::default(),
        }
    }

    /// Appends one logical line (CRLF terminated, with SGR escapes if colored) to `out`.
    pub(crate) fn next_line(&mut self, out: &mut Vec<u8>) {
        let long = self.rng.below(1000) < 20;
        let non_ascii = self.rng.below(1000) < 50;
        let colored = self.rng.below(1000) < 350;
        let target = if long { self.rng.range(300, 400) } else { self.rng.range(10, 118) };
        self.text.clear();
        self.width = 0;
        match self.profile {
            Profile::Mixed => self.fill_mixed(target, non_ascii),
            Profile::Corpus => self.fill_corpus(target, non_ascii),
            Profile::Entropy => self.fill_entropy(target, non_ascii),
        }
        if non_ascii && self.text.is_ascii() {
            // The glyph did not fit or was not drawn: swap the last column for an accented letter.
            if let Some(c) = self.text.pop() {
                self.width -= c.width().unwrap_or(0);
            }
            self.push_char('\u{E9}', usize::MAX);
        }
        let is_ascii = self.text.is_ascii();
        let before = out.len();
        if colored {
            self.emit_colored(out);
        } else {
            out.extend_from_slice(self.text.as_bytes());
        }
        out.extend_from_slice(b"\r\n");
        let s = &mut self.stats;
        s.lines += 1;
        s.colored += usize::from(colored);
        s.non_ascii += usize::from(!is_ascii);
        s.long += usize::from(long);
        s.cols += self.width;
        s.bytes += out.len() - before;
    }

    fn push_char(&mut self, c: char, limit: usize) -> bool {
        let w = c.width().unwrap_or(0);
        if self.width + w > limit {
            return false;
        }
        self.text.push(c);
        self.width += w;
        true
    }

    fn push_str(&mut self, s: &str, limit: usize) {
        for c in s.chars() {
            if !self.push_char(c, limit) {
                return;
            }
        }
    }

    fn push_word(&mut self, limit: usize, non_ascii_kind: Option<usize>) {
        let Some(&word) = self.rng.pick(WORDS) else { return };
        match non_ascii_kind {
            Some(0) => {
                for _ in 0..self.rng.range(2, 4) {
                    if let Some(&c) = self.rng.pick(CJK) {
                        self.push_char(c, limit);
                    }
                }
            }
            Some(1) => {
                if let Some(&c) = self.rng.pick(EMOJI) {
                    self.push_char(c, limit);
                }
            }
            Some(_) => {
                for c in word.chars() {
                    let c = match c {
                        'e' => '\u{E9}',
                        'u' => '\u{FC}',
                        'n' => '\u{F1}',
                        o => o,
                    };
                    self.push_char(c, limit);
                }
            }
            None => self.push_str(word, limit),
        }
        self.push_char(' ', limit);
    }

    fn fill_mixed(&mut self, target: usize, non_ascii: bool) {
        let kind = self.rng.below(100);
        let mut head_buf = String::new();
        let n = self.rng.range(1, 999);
        let head: &str = if kind < 35 {
            let _ = write!(
                head_buf,
                "[2026-11-21T{:02}:{:02}:{:02}Z] INFO worker-{} ",
                self.rng.below(24),
                self.rng.below(60),
                self.rng.below(60),
                self.rng.below(8)
            );
            &head_buf
        } else if kind < 55 {
            let c = self.rng.pick(CRATES).copied().unwrap_or("serde");
            let _ =
                write!(head_buf, "   Compiling {c} v{}.{}.{} ", self.rng.below(3), n % 30, n % 17);
            &head_buf
        } else if kind < 65 {
            let _ = write!(head_buf, " --> src/vt/mod{}.rs:{}:{} ", n % 40, n, n % 80);
            &head_buf
        } else if kind < 75 {
            let _ = write!(head_buf, "{{\"id\": {n}, \"ok\": true, \"name\": \"item\", ");
            &head_buf
        } else if kind < 85 {
            let _ =
                write!(head_buf, "-rw-r--r-- 1 user staff {:>7} Nov 21 12:{:02} ", n * 37, n % 60);
            &head_buf
        } else if kind < 95 {
            let _ = write!(head_buf, "* {:07x} (origin/main) ", self.rng.next() & 0x0FFF_FFFF);
            &head_buf
        } else {
            let _ = write!(head_buf, "{:08x}  de ad be ef 00 11 22 33 ", n * 16);
            &head_buf
        };
        self.push_str(head, target);
        let kind_na = non_ascii.then(|| self.rng.below(3));
        let mut first = true;
        while self.width < target {
            let k =
                if first { kind_na } else { (self.rng.below(3) == 0).then_some(kind_na).flatten() };
            first = false;
            let before = self.width;
            self.push_word(target, k);
            if self.width == before {
                break;
            }
        }
    }

    fn fill_corpus(&mut self, target: usize, non_ascii: bool) {
        let kind_na = non_ascii.then(|| self.rng.below(3));
        let mut guard = 0;
        while self.width < target && guard < 64 && !self.corpus.is_empty() {
            guard += 1;
            let idx = self.corpus_pos % self.corpus.len();
            self.corpus_pos += 1;
            let line = self.corpus.get(idx).cloned().unwrap_or_default();
            self.push_str(&line, target);
            self.push_char(' ', target);
        }
        if non_ascii {
            self.push_word_replace_tail(target, kind_na);
        }
    }

    /// Guarantees at least one non-ASCII glyph: appends (or replaces the tail with) one.
    fn push_word_replace_tail(&mut self, limit: usize, kind: Option<usize>) {
        // Drop up to 6 trailing columns so the glyph fits within the target width.
        for _ in 0..6 {
            if self.width + 6 <= limit {
                break;
            }
            if let Some(c) = self.text.pop() {
                self.width -= c.width().unwrap_or(0);
            }
        }
        self.push_word(limit + 6, kind);
    }

    fn fill_entropy(&mut self, target: usize, non_ascii: bool) {
        let kind_na = non_ascii.then(|| self.rng.below(3));
        if kind_na == Some(0) {
            self.push_word(target, Some(0));
        } else if kind_na == Some(1) {
            self.push_word(target, Some(1));
        } else if kind_na == Some(2) {
            self.push_char('\u{E9}', target);
        }
        while self.width < target {
            let c = char::from(u8::try_from(self.rng.range(0x21, 0x7E)).unwrap_or(b'?'));
            if !self.push_char(c, target) {
                break;
            }
        }
    }

    fn emit_colored(&mut self, out: &mut Vec<u8>) {
        let runs = self.rng.range(3, 5);
        let chars = self.text.chars().count();
        let mut prev = usize::MAX;
        let mut text = std::mem::take(&mut self.text);
        let mut it = text.chars();
        for i in 0..runs {
            let take = if i + 1 == runs { chars } else { chars / runs };
            let mut idx = self.rng.below(SGR.len());
            if idx == prev {
                idx = (idx + 1) % SGR.len();
            }
            prev = idx;
            out.extend_from_slice(b"\x1b[0;"); // reset first, as printf-style tools do
            out.extend_from_slice(SGR.get(idx).copied().unwrap_or("0").as_bytes());
            out.push(b'm');
            let mut buf = [0u8; 4];
            for c in it.by_ref().take(take) {
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
        out.extend_from_slice(b"\x1b[0m");
        self.stats.sgr_runs += runs;
        text.clear();
        self.text = text;
    }
}

/// Reads every `*.md` under `dir` (sorted), ASCII-sanitizes lines and keeps non-empty ones.
pub(crate) fn load_corpus(dir: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let p = e?.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut lines = Vec::new();
    for f in files {
        for l in std::fs::read_to_string(f)?.lines() {
            let s: String = l
                .chars()
                .map(|c| if c.is_ascii() && !c.is_ascii_control() { c } else { ' ' })
                .collect();
            let s = s.trim();
            if !s.is_empty() {
                lines.push(s.to_owned());
            }
        }
    }
    Ok(lines)
}
