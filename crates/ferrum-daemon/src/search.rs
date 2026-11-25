//! Search over a session's history and screen (FR-TERM-015), run on the worker over immutable
//! handles while the session keeps running (`docs/02` §5): a [`HistorySnapshot`] of the
//! scrollback and a copy of the screen rows, taken when the search starts.
//!
//! Rows soft-wrapped into each other (`LineFlags::WRAPPED`) are joined into one logical line
//! before matching, so a match that crosses a wrap is found; every match is mapped back to the
//! cells it covers. Rows are read one at a time through one decode cursor. A logical line is
//! matched in windows of at most 64 KiB (`docs/03` §3 rule 4: memory is bounded
//! whatever the input): consecutive windows overlap by 4 KiB, so only a match longer
//! than the overlap that crosses a window edge can be missed; each window also keeps a few bytes
//! of text before it, so `^`, `\b` and `\B` see the real neighbors at its start. A regular
//! expression is compiled on the worker with bounded program and DFA sizes. The search stops at
//! [`MAX_LIST_ITEMS`] matches and checks its cancel flag between rows, so a newer search
//! interrupts it (`docs/05` §5).

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use ferrum_proto::{MAX_LIST_ITEMS, Point, SearchMatch, SearchOptions};
use ferrum_scrollback::{
    CaseMode, HistorySnapshot, LineCursor, LineFlags, LineMatcher, LiteralMatcher,
};
use ferrum_vt::{CellFlags, EventSink, Terminal};

use crate::encode::clamp_u16;

/// Bytes of the longest UTF-8 scalar.
const UTF8_MAX_BYTES: usize = 4;
/// Most text of one logical line matched at once: 64 rows of the widest grid (1 024 columns) in
/// 4-byte scalars would be 256 KiB; a quarter of that keeps the search's memory far below one
/// session's allowance (`docs/03` §2) while normal lines (a few rows) fit whole.
const WINDOW_BYTES: usize = 64 * 1_024;
/// Text kept from the end of a full window at the start of the next: longer than any query
/// (`MAX_QUERY_BYTES` of the protocol's text cap is far larger, but a literal or regular
/// expression match across 4 KiB of one line is not a search the UI offers).
const OVERLAP_BYTES: usize = 4 * 1_024;
/// Text kept before a window's first byte for the assertions that look back (`\b`, `\B`,
/// `^`): one scalar is enough, four cover any cluster's first scalar.
const CONTEXT_BYTES: usize = 4 * UTF8_MAX_BYTES;
/// Largest compiled regular expression and lazy DFA cache: 1 MiB each keeps a search far within
/// the daemon's 2 MB reserve (`docs/03` §2); an expression that needs more is refused.
const REGEX_SIZE_LIMIT: usize = 1_024 * 1_024;
const REGEX_DFA_SIZE_LIMIT: usize = 1_024 * 1_024;

/// How a query matches text.
pub enum Matcher {
    /// A literal, with the case toggle.
    Literal(LiteralMatcher),
    /// A regular expression (Unicode case folding when case-insensitive).
    Regex(regex::Regex),
}

impl Matcher {
    /// The matcher for a query; `None` for an invalid or too large regular expression.
    pub fn new(query: &str, options: SearchOptions) -> Option<Self> {
        if !options.is_regex {
            let case =
                if options.is_case_sensitive { CaseMode::Sensitive } else { CaseMode::Insensitive };
            return Some(Self::Literal(LiteralMatcher::new(query, case)));
        }
        regex::RegexBuilder::new(query)
            .case_insensitive(!options.is_case_sensitive)
            .size_limit(REGEX_SIZE_LIMIT)
            .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
            .build()
            .ok()
            .map(Self::Regex)
    }
}

impl Matcher {
    /// The non-empty matches in `text` that start at or after byte `from`; the text before
    /// `from` is context for the assertions only.
    fn find_from(&self, text: &str, from: usize, out: &mut Vec<Range<usize>>) {
        match self {
            Self::Literal(literal) => {
                literal.find_matches(text, out);
                out.retain(|range| range.start >= from);
            }
            Self::Regex(regex) => {
                let mut at = from;
                while let Some(found) = regex.find_at(text, at) {
                    let end = found.end();
                    if found.is_empty() {
                        // An empty match moves on by one scalar.
                        let next = text.get(end..).and_then(|rest| rest.chars().next());
                        at = end + next.map_or(1, char::len_utf8);
                    } else {
                        out.push(found.range());
                        at = end;
                    }
                    if at > text.len() {
                        break;
                    }
                }
            }
        }
    }
}

/// The result of a search.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// The matches, oldest first, at most [`MAX_LIST_ITEMS`].
    pub matches: Vec<SearchMatch>,
    /// More matches exist than were returned.
    pub is_truncated: bool,
}

/// One screen row as text: its clusters back to back and the column of each.
#[derive(Debug, Default)]
struct ScreenRow {
    text: String,
    starts: Vec<(usize, u16)>,
    wrapped: bool,
}

/// What a search reads: the history at one moment and a copy of the screen.
pub struct SearchInput {
    history: HistorySnapshot,
    screen: Vec<ScreenRow>,
    cols: usize,
}

impl SearchInput {
    /// Takes the history handles and copies the screen of `term` (a few KiB).
    pub fn of<S: EventSink>(term: &Terminal<S>) -> Self {
        let grid = term.grid();
        let mut utf8 = [0; UTF8_MAX_BYTES];
        let screen = (0..grid.lines())
            .map(|y| {
                let mut row = ScreenRow::default();
                let Some(cells) = grid.row(y) else { return row };
                for (col, cell) in cells.cells().iter().take(cells.used_len()).enumerate() {
                    if !cell.flags().contains(CellFlags::SPACER) {
                        row.starts.push((row.text.len(), clamp_u16(col)));
                        row.text.push_str(grid.cell_text(cell, &mut utf8));
                    }
                }
                row.wrapped = cells.is_wrapped();
                row
            })
            .collect();
        Self { history: term.scrollback().snapshot(), screen, cols: grid.cols() }
    }
}

/// One logical line being assembled: its text and, per cluster, where it starts in the text and
/// which cell it is.
#[derive(Default)]
struct Logical {
    text: String,
    cells: Vec<(usize, Point)>,
    ranges: Vec<Range<usize>>,
    /// Bytes before this are context from the previous window, already searched.
    from: usize,
}

impl Logical {
    fn push(&mut self, cluster: &str, at: Point) {
        self.cells.push((self.text.len(), at));
        self.text.push_str(cluster);
    }

    /// The cell holding byte `offset`, and the cell after the one holding byte `end - 1`.
    fn span(&self, range: &Range<usize>) -> Option<SearchMatch> {
        let index = |offset: usize| self.cells.partition_point(|(start, _)| *start <= offset);
        let first = self.cells.get(index(range.start).checked_sub(1)?)?.1;
        let last = self.cells.get(index(range.end.checked_sub(1)?).checked_sub(1)?)?.1;
        Some(SearchMatch {
            start: first,
            end: Point { line: last.line, col: last.col.saturating_add(1) },
        })
    }

    /// Drops the text before cluster `keep` (a window moved on); the next search starts at
    /// byte `edge` of the old text.
    fn keep_from(&mut self, keep: usize, edge: usize) {
        let Some(&(cut, _)) = self.cells.get(keep) else {
            self.clear();
            return;
        };
        self.text.drain(..cut);
        self.cells.drain(..keep);
        for (start, _) in &mut self.cells {
            *start -= cut;
        }
        self.from = edge.saturating_sub(cut);
    }

    fn clear(&mut self) {
        self.text.clear();
        self.cells.clear();
        self.from = 0;
    }
}

/// The search in progress.
struct Search<'a> {
    matcher: &'a Matcher,
    logical: Logical,
    found: Found,
}

impl Search<'_> {
    /// Matches the whole logical line and starts the next one.
    fn finish(&mut self) {
        self.match_until(usize::MAX);
        self.logical.clear();
    }

    /// When the window is full: matches it, reporting the matches that start before the overlap,
    /// and keeps the overlap for the next window.
    fn slide(&mut self) {
        if self.logical.text.len() < WINDOW_BYTES {
            return;
        }
        let cells = &self.logical.cells;
        let cut = self.logical.text.len() - OVERLAP_BYTES;
        let overlap = cells.partition_point(|(start, _)| *start < cut);
        let edge = cells.get(overlap).map_or(usize::MAX, |(start, _)| *start);
        let context =
            cells.partition_point(|(start, _)| *start < edge.saturating_sub(CONTEXT_BYTES));
        self.match_until(edge);
        self.logical.keep_from(context, edge);
    }

    /// Reports the matches that start in this window and before byte `edge`.
    fn match_until(&mut self, edge: usize) {
        let mut ranges = std::mem::take(&mut self.logical.ranges);
        ranges.clear();
        self.matcher.find_from(&self.logical.text, self.logical.from, &mut ranges);
        for range in ranges.iter().filter(|range| range.start < edge) {
            if self.found.matches.len() == MAX_LIST_ITEMS {
                self.found.is_truncated = true;
                break;
            }
            self.found.matches.extend(self.logical.span(range));
        }
        self.logical.ranges = ranges;
    }
}

/// Searches the history (oldest first) and then the screen.
///
/// `cancel` is checked between rows; once set, the matches found so far are returned.
pub fn search(input: &SearchInput, matcher: &Matcher, cancel: &AtomicBool) -> Found {
    let mut search = Search { matcher, logical: Logical::default(), found: Found::default() };
    let history = input.history.len();
    let mut cursor = LineCursor::new();
    let top = -i64::try_from(history).unwrap_or(i64::MAX);
    for index in 0..history.saturating_add(input.screen.len()) {
        if cancel.load(Ordering::Acquire) || search.found.is_truncated {
            break;
        }
        let line = top.saturating_add(i64::try_from(index).unwrap_or(i64::MAX));
        let line = i32::try_from(line).unwrap_or(i32::MIN);
        let wrapped = match index.checked_sub(history) {
            None => history_row(input, (index, line), &mut cursor, &mut search.logical),
            Some(y) => screen_row(input, y, line, &mut search.logical),
        };
        if wrapped {
            search.slide();
        } else {
            search.finish();
        }
    }
    search.finish();
    search.found
}

/// Appends history row `index`; returns whether it wraps into the next row.
fn history_row(
    input: &SearchInput,
    (index, line): (usize, i32),
    cursor: &mut LineCursor,
    logical: &mut Logical,
) -> bool {
    let Ok(Some(decoded)) = input.history.read_line(index, cursor) else { return false };
    let mut col = 0;
    for cluster in ferrum_vt::clusters(&decoded.text) {
        logical.push(cluster, Point { line, col });
        col = col.saturating_add(if ferrum_vt::cluster_width(cluster) > 1 { 2 } else { 1 });
    }
    let wrapped = decoded.flags.contains(LineFlags::WRAPPED);
    if wrapped {
        pad(logical, Point { line, col }, input.cols);
    }
    wrapped
}

/// Appends screen row `y`; returns whether it wraps into the next row.
fn screen_row(input: &SearchInput, y: usize, line: i32, logical: &mut Logical) -> bool {
    let Some(row) = input.screen.get(y) else { return false };
    let ends = row.starts.iter().skip(1).map(|(start, _)| *start).chain([row.text.len()]);
    let mut next = 0;
    for (&(start, col), end) in row.starts.iter().zip(ends) {
        logical.push(row.text.get(start..end).unwrap_or(" "), Point { line, col });
        next = col.saturating_add(1);
    }
    if row.wrapped {
        pad(logical, Point { line, col: next }, input.cols);
    }
    row.wrapped
}

/// A wrapped row's trailing blanks were trimmed; they are part of the logical line.
fn pad(logical: &mut Logical, from: Point, cols: usize) {
    for col in from.col..clamp_u16(cols) {
        logical.push(" ", Point { line: from.line, col });
    }
}

#[cfg(test)]
mod tests {
    use ferrum_scrollback::{Scrollback, ScrollbackConfig};
    use ferrum_vt::{Size, Terminal};

    use super::*;
    use crate::encode::Term;
    use crate::session::sink::Sink;

    fn term(text: &str, size: Size) -> Term {
        let config = ScrollbackConfig::default();
        let mut term = Terminal::new(size, Scrollback::new(config), Sink::default());
        term.parse(text.as_bytes());
        term
    }

    fn find(term: &Term, query: &str, options: SearchOptions) -> Found {
        let matcher = Matcher::new(query, options)
            .unwrap_or(Matcher::Literal(LiteralMatcher::new("\u{0}never", CaseMode::Sensitive)));
        search(&SearchInput::of(term), &matcher, &AtomicBool::new(false))
    }

    #[test]
    fn a_match_across_a_soft_wrap_is_found_in_history_and_screen() {
        // "abcdefghijKLM" wraps after 10 columns; the next lines push it into the history.
        let term = term("abcdefghijKLM\r\n1\r\n2\r\n3\r\n4", Size::new(10, 3));
        assert_eq!(term.scrollback().len(), 3, "both rows of the wrapped line and `1` are history");
        let found = find(&term, "ijkl", SearchOptions::default());
        let start = Point { line: -3, col: 8 };
        let end = Point { line: -2, col: 2 };
        assert_eq!(found.matches, [SearchMatch { start, end }]);
        let sensitive = SearchOptions { is_case_sensitive: true, ..SearchOptions::default() };
        assert_eq!(find(&term, "ijkl", sensitive).matches, []);
        let regex = SearchOptions { is_regex: true, ..SearchOptions::default() };
        assert_eq!(find(&term, "^[0-9]$", regex).matches.len(), 4, "history 1, screen 2, 3, 4");
        assert!(Matcher::new("(", regex).is_none(), "an invalid expression matches nothing");
    }

    #[test]
    fn a_case_insensitive_regex_folds_unicode() {
        let term = term("ÉCOLE straße\r\n", Size::new(20, 3));
        let regex = SearchOptions { is_regex: true, ..SearchOptions::default() };
        assert_eq!(find(&term, "école", regex).matches.len(), 1);
        assert_eq!(find(&term, "STRASSE|STRAẞE", regex).matches.len(), 1);
    }

    #[test]
    fn a_line_longer_than_a_window_is_searched_in_bounded_windows() {
        use std::fmt::Write as _;
        // Fewer marks than the result cap, spread over every window edge.
        const MARK_EVERY: usize = 32;
        // One logical line of 3 windows' worth of text wrapped over many rows of 100 columns,
        // with a marker straddling every window edge region and one at the very end.
        let size = Size::new(100, 5);
        let mut text = String::new();
        let mut index = 0;
        while text.len() < 3 * WINDOW_BYTES {
            write!(text, "{index:07}-").unwrap_or_default();
            if index % MARK_EVERY == 0 {
                text.push('|');
            }
            index += 1;
        }
        text.push_str("NEEDLE\r\n");
        let term = term(&text, size);
        let found = find(&term, "0000042-", SearchOptions::default());
        assert_eq!(found.matches.len(), 1);
        let last = format!("{:07}-", index - 1);
        assert_eq!(find(&term, &last, SearchOptions::default()).matches.len(), 1);
        assert_eq!(find(&term, "needle", SearchOptions::default()).matches.len(), 1);
        let marks = find(&term, "|", SearchOptions::default());
        let expected = index.div_ceil(MARK_EVERY);
        assert_eq!(marks.matches.len(), expected, "no match lost or doubled at a window edge");
    }

    #[test]
    fn assertions_see_the_text_before_a_window() {
        // Lines of `Zfoo` repeated over several windows: `foo` never starts a word and `Z` only
        // starts the line. Each padding moves the window edges by one byte, so some edge falls
        // right before an `f` or a `Z`.
        let regex = SearchOptions { is_regex: true, ..SearchOptions::default() };
        for padding in 0..4 {
            let mut text = "-".repeat(padding);
            while text.len() < 3 * WINDOW_BYTES {
                text.push_str("Zfoo");
            }
            text.push_str("\r\n");
            let term = term(&text, Size::new(100, 5));
            assert_eq!(find(&term, r"\bfoo", regex).matches, [], "padding {padding}");
            let starts = find(&term, "^Z", regex).matches.len();
            assert_eq!(starts, usize::from(padding == 0), "padding {padding}");
        }
    }
}
