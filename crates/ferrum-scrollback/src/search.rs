//! Search over decoded lines (`FR-TERM-015` hooks): an iterator the daemon search worker drives,
//! plus a literal matcher with a case toggle. Regex is a [`LineMatcher`] the daemon implements;
//! this crate has no regex engine dependency.
//!
//! Matching is per row. A match that spans a soft wrap (row flag
//! [`LineFlags::WRAPPED`](crate::LineFlags::WRAPPED)) is not found here; the daemon joins wrapped
//! rows when it needs that (WP-06).

use std::ops::Range;

use crate::error::ScrollbackError;
use crate::scrollback::{LineCursor, Scrollback};

/// Finds matches inside the text of one line.
pub trait LineMatcher {
    /// Appends the byte ranges of all non-overlapping matches in `text` to `out`, in order.
    /// Ranges must lie on `char` boundaries.
    fn find_matches(&self, text: &str, out: &mut Vec<Range<usize>>);
}

/// Case handling of a [`LiteralMatcher`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CaseMode {
    /// Exact characters.
    Sensitive,
    /// Per-character simple lowercase folding (a char whose lowercase is several chars is left as is).
    Insensitive,
}

/// Plain substring search.
#[derive(Debug, Clone)]
pub struct LiteralMatcher {
    needle: String,
    folded: Vec<char>,
    case: CaseMode,
}

impl LiteralMatcher {
    /// A matcher for `needle`. An empty needle matches nothing.
    pub fn new(needle: &str, case: CaseMode) -> Self {
        Self { needle: needle.to_owned(), folded: needle.chars().map(fold).collect(), case }
    }
}

fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ => c,
    }
}

impl LineMatcher for LiteralMatcher {
    fn find_matches(&self, text: &str, out: &mut Vec<Range<usize>>) {
        if self.needle.is_empty() {
            return;
        }
        match self.case {
            CaseMode::Sensitive => {
                out.extend(
                    text.match_indices(self.needle.as_str())
                        .map(|(at, found)| at..at + found.len()),
                );
            }
            CaseMode::Insensitive => self.find_folded(text, out),
        }
    }
}

impl LiteralMatcher {
    fn find_folded(&self, text: &str, out: &mut Vec<Range<usize>>) {
        let mut resume = 0;
        for (start, _) in text.char_indices() {
            if start < resume {
                continue;
            }
            if let Some(end) = self.matches_at(text, start) {
                out.push(start..end);
                resume = end;
            }
        }
    }

    /// End offset of a folded match starting at `start`, if any.
    fn matches_at(&self, text: &str, start: usize) -> Option<usize> {
        let rest = text.get(start..)?;
        let mut wanted = self.folded.iter();
        let mut end = start;
        for (offset, c) in rest.char_indices() {
            match wanted.next() {
                Some(&expected) if fold(c) == expected => end = start + offset + c.len_utf8(),
                Some(_) => return None,
                None => break,
            }
        }
        wanted.next().is_none().then_some(end)
    }
}

/// Which way a search walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Direction {
    /// Towards newer lines.
    Forward,
    /// Towards older lines.
    Backward,
}

/// One matching line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMatch {
    /// Line index (0 = oldest retained at the time of the search).
    pub line: usize,
    /// Byte ranges of the matches inside the line text.
    pub ranges: Vec<Range<usize>>,
}

/// Iterator over the lines matching a [`LineMatcher`]; yields an error once and then stops.
/// Borrows the scrollback, so it sees a stable snapshot (the daemon runs it on an immutable
/// handle, `docs/02` §5).
#[derive(Debug)]
pub struct Search<'a, M> {
    store: &'a Scrollback,
    matcher: M,
    direction: Direction,
    next: Option<usize>,
    cursor: LineCursor,
    ranges: Vec<Range<usize>>,
}

impl Scrollback {
    /// Starts a search at line `start` (inclusive) walking in `direction`. A `start` past the end
    /// is clamped to the last line for [`Direction::Backward`] and yields nothing for forward.
    pub fn search<M: LineMatcher>(
        &self,
        matcher: M,
        direction: Direction,
        start: usize,
    ) -> Search<'_, M> {
        let first = match direction {
            Direction::Forward => (start < self.len()).then_some(start),
            Direction::Backward => self.len().checked_sub(1).map(|last| start.min(last)),
        };
        Search {
            store: self,
            matcher,
            direction,
            next: first,
            cursor: LineCursor::new(),
            ranges: Vec::new(),
        }
    }
}

impl<M: LineMatcher> Iterator for Search<'_, M> {
    type Item = Result<LineMatch, ScrollbackError>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(index) = self.next {
            self.next = self.step(index);
            match self.store.read_line(index, &mut self.cursor) {
                Err(error) => {
                    self.next = None;
                    return Some(Err(error));
                }
                Ok(None) => return None,
                Ok(Some(line)) => {
                    self.ranges.clear();
                    self.matcher.find_matches(&line.text, &mut self.ranges);
                    if !self.ranges.is_empty() {
                        return Some(Ok(LineMatch { line: index, ranges: self.ranges.clone() }));
                    }
                }
            }
        }
        None
    }
}

impl<M> Search<'_, M> {
    fn step(&self, index: usize) -> Option<usize> {
        match self.direction {
            Direction::Forward => index.checked_add(1).filter(|&next| next < self.store.len()),
            Direction::Backward => index.checked_sub(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(matcher: &LiteralMatcher, text: &str) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        matcher.find_matches(text, &mut out);
        out
    }

    #[test]
    fn sensitive_and_insensitive_literals() {
        let sensitive = LiteralMatcher::new("ab", CaseMode::Sensitive);
        assert_eq!(find(&sensitive, "ab AB ab"), [0..2, 6..8]);
        let insensitive = LiteralMatcher::new("ab", CaseMode::Insensitive);
        assert_eq!(find(&insensitive, "ab AB ab"), [0..2, 3..5, 6..8]);
        assert_eq!(find(&insensitive, "aab"), vec![1..3]);
    }

    #[test]
    fn unicode_ranges_are_byte_ranges_on_boundaries() {
        let matcher = LiteralMatcher::new("ÉCOLE", CaseMode::Insensitive);
        assert_eq!(find(&matcher, "la école"), vec![3..9]);
        assert_eq!(find(&LiteralMatcher::new("", CaseMode::Sensitive), "x"), vec![]);
    }
}
