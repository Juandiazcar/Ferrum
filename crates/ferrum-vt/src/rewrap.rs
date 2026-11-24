//! Re-wrapping a logical line at a new width as a stream (FR-TERM-010, AC-TERM-02).
//!
//! A logical line is the text of a row and of the rows soft-wrapped into it. It is cut into rows
//! of at most `cols` display columns, never inside a grapheme cluster, and a double-width
//! character that does not fit starts the next row (the grid shows a leading spacer there). The
//! work happens on text and style runs, not on cells, so scrollback rows never become cells.
//!
//! The line is never held whole: the input arrives one old row at a time and the output rows go
//! out as soon as they are full, so the memory of a rewrap is one row plus the cluster that
//! straddles two input rows, however long the logical line is (`docs/03` §3 rules 3 and 4).
//! Shell marks stay with the output row that is open when the input row that carried them is fed
//! (the row where the previous input row ended, give or take its last cluster), so a mark on a
//! continuation row never travels back to rows that were already handed out.

use ferrum_scrollback::{LineFlags, Style, StyledRun};

use crate::codec::WRAP_PAD;
use crate::width::{capped_clusters, cluster_width, clusters};

mod rows;

pub(crate) use rows::{CursorPlace, Feed, Finished, OutRow, Rows, RunCursor};

/// The state of the line being rewrapped.
#[derive(Debug)]
pub(crate) struct Rewrapper {
    cols: usize,
    cur: OutRow,
    col: usize,
    /// Columns of the clusters placed so far (dropped ones included).
    offset: usize,
    /// Rows of this line already handed to [`Rows`].
    emitted: usize,
    /// The last cluster of the previous row: it may continue into the next one.
    held: String,
    held_style: Style,
    cursor_offset: Option<usize>,
    cursor: Option<CursorPlace>,
    /// Scratch for the text and the runs of the row being fed.
    text: String,
    runs: Vec<StyledRun>,
}

impl Rewrapper {
    /// A rewrapper for rows of `cols` columns (at least 1).
    pub(crate) fn new(cols: usize) -> Self {
        Self {
            cols,
            cur: OutRow::default(),
            col: 0,
            offset: 0,
            emitted: 0,
            held: String::new(),
            held_style: Style::default(),
            cursor_offset: None,
            cursor: None,
            text: String::new(),
            runs: Vec::new(),
        }
    }

    /// Feeds the next old row of the line; finished output rows go to `rows`.
    pub(crate) fn feed(&mut self, input: &Feed<'_>, rows: &mut Rows) {
        let marks = LineFlags::from_bits(input.flags.bits() & !LineFlags::WRAPPED.bits());
        self.cur.flags = self.cur.flags.union(marks.unwrap_or(LineFlags::NONE));
        if let Some(x) = input.cursor_x {
            self.cursor_offset = Some(self.offset + cluster_width(&self.held) + x);
        }
        let mut text = std::mem::take(&mut self.text);
        let mut runs = std::mem::take(&mut self.runs);
        self.join(input, &mut text, &mut runs);
        let keep = held_start(&text);
        let (body, tail) = text.split_at_checked(keep).unwrap_or((&text, ""));
        let mut cursor = RunCursor::new(&runs);
        self.place_text(body, &mut cursor, rows);
        let scalars = tail.chars().count();
        self.held_style = cursor.take(scalars);
        self.held.clear();
        self.held.push_str(tail);
        self.text = text;
        self.runs = runs;
    }

    /// Ends the line: places the held cluster and closes the last row.
    pub(crate) fn finish(&mut self, rows: &mut Rows) -> Finished {
        let held = std::mem::take(&mut self.held);
        let style = self.held_style;
        for piece in capped_clusters(&held) {
            self.put_cluster(piece, style, piece.chars().count(), rows);
        }
        if self.cursor.is_none() {
            self.cursor = self.cursor_offset.map(|at| CursorPlace {
                row: self.emitted,
                col: (self.col + at.saturating_sub(self.offset)).min(self.cols - 1),
            });
        }
        self.emit(rows);
        let finished = Finished { rows: self.emitted, cursor: self.cursor };
        (self.col, self.offset, self.emitted) = (0, 0, 0);
        (self.cursor, self.cursor_offset) = (None, None);
        self.held = held;
        self.held.clear();
        finished
    }

    /// Builds `text` and `runs` as the held cluster followed by the row, without the wrap padding.
    fn join(&self, input: &Feed<'_>, text: &mut String, runs: &mut Vec<StyledRun>) {
        text.clear();
        runs.clear();
        text.push_str(&self.held);
        text.push_str(input.text);
        let held = StyledRun { style: self.held_style, len: self.held.chars().count() };
        for run in std::iter::once(held).chain(input.runs.iter().copied()) {
            let style = Style { attrs: run.style.attrs & !WRAP_PAD, ..run.style };
            match runs.last_mut() {
                Some(last) if last.style == style => last.len += run.len,
                _ if run.len == 0 => {}
                _ => runs.push(StyledRun { style, len: run.len }),
            }
        }
    }

    /// Places `body` (whole clusters): ASCII runs in bulk, anything else cluster by cluster.
    fn place_text(&mut self, body: &str, runs: &mut RunCursor<'_>, rows: &mut Rows) {
        let mut rest = body;
        while !rest.is_empty() {
            let Some(cluster) = clusters(rest).next() else { break };
            for piece in capped_clusters(cluster) {
                let scalars = piece.chars().count();
                let style = runs.take(scalars);
                self.put_cluster(piece, style, scalars, rows);
            }
            rest = rest.get(cluster.len()..).unwrap_or_default();
        }
    }

    /// Places one cluster of at most `MAX_CLUSTER_BYTES`.
    fn put_cluster(&mut self, cluster: &str, style: Style, scalars: usize, rows: &mut Rows) {
        let width = cluster_width(cluster);
        if width == 0 {
            self.cur.push(cluster, style, scalars);
            return;
        }
        if width > self.cols {
            self.offset += width;
            return;
        }
        if self.col + width > self.cols {
            self.wrap(rows);
        }
        if self.cursor_in(width).is_some() {
            self.cursor = Some(CursorPlace { row: self.emitted, col: self.col });
        }
        self.cur.push(cluster, style, scalars);
        self.col += width;
        self.offset += width;
    }

    /// Where the cursor falls inside the next `width` columns, if it does and was not placed yet.
    fn cursor_in(&self, width: usize) -> Option<usize> {
        let at = self.cursor_offset.filter(|_| self.cursor.is_none())?;
        (self.offset..self.offset + width).contains(&at).then(|| at - self.offset)
    }

    /// The current row is full: it continues on the next one.
    fn wrap(&mut self, rows: &mut Rows) {
        self.cur.flags = self.cur.flags.union(LineFlags::WRAPPED);
        self.emit(rows);
        self.col = 0;
    }

    fn emit(&mut self, rows: &mut Rows) {
        let next = rows.fresh();
        rows.pending.push_back(std::mem::replace(&mut self.cur, next));
        self.emitted += 1;
    }
}

/// Byte offset where the last cluster of `text` starts; the whole text when that cluster is
/// already too long to be extended (it is placed in pieces).
fn held_start(text: &str) -> usize {
    let Some(last) = clusters(text).last() else { return 0 };
    if last.len() > crate::limits::MAX_CLUSTER_BYTES { text.len() } else { text.len() - last.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rewraps `text` (one logical line of one old row) with the cursor at `cursor_x`.
    fn rewrap_with(
        text: &str,
        runs: &[StyledRun],
        cols: usize,
        cursor_x: Option<usize>,
    ) -> (Vec<OutRow>, Option<CursorPlace>) {
        let scalars = text.chars().count();
        let default = [StyledRun { style: Style::default(), len: scalars }];
        let mut rewrapper = Rewrapper::new(cols);
        let mut rows = Rows::default();
        let input = Feed {
            flags: LineFlags::NONE,
            text,
            runs: if runs.is_empty() { &default } else { runs },
            cursor_x,
        };
        rewrapper.feed(&input, &mut rows);
        let finished = rewrapper.finish(&mut rows);
        (rows.pending.into_iter().collect(), finished.cursor)
    }

    fn rewrap(text: &str, cols: usize) -> Vec<OutRow> {
        rewrap_with(text, &[], cols, None).0
    }

    fn texts(rows: &[OutRow]) -> Vec<&str> {
        rows.iter().map(|row| row.text.as_str()).collect()
    }

    #[test]
    fn a_line_is_cut_at_the_width_and_all_but_the_last_row_are_wrapped() {
        let rows = rewrap("abcdefgh", 3);
        assert_eq!(texts(&rows), ["abc", "def", "gh"]);
        let wrapped: Vec<bool> =
            rows.iter().map(|r| r.flags.contains(LineFlags::WRAPPED)).collect();
        assert_eq!(wrapped, [true, true, false]);
    }

    #[test]
    fn an_exact_fit_does_not_add_an_empty_row() {
        assert_eq!(texts(&rewrap("abcdef", 3)), ["abc", "def"]);
        assert_eq!(texts(&rewrap("", 3)), [""]);
    }

    #[test]
    fn wide_characters_and_clusters_are_never_split() {
        assert_eq!(texts(&rewrap("ab漢字", 3)), ["ab", "漢", "字"]);
        assert_eq!(texts(&rewrap("ae\u{301}b", 2)), ["ae\u{301}", "b"]);
        assert_eq!(texts(&rewrap("a👨‍👩‍👧b", 2)), ["a", "👨‍👩‍👧", "b"]);
    }

    #[test]
    fn a_wide_character_wider_than_the_terminal_is_dropped() {
        assert_eq!(texts(&rewrap("a漢b", 1)), ["a", "b"]);
    }

    #[test]
    fn styles_follow_their_scalars_across_the_cuts() {
        let red = Style { fg: 1, ..Style::default() };
        let runs = [
            StyledRun { style: Style::default(), len: 1 },
            StyledRun { style: red, len: 2 },
            StyledRun { style: Style::default(), len: 1 },
        ];
        let (rows, _) = rewrap_with("abcd", &runs, 2, None);
        let styles: Vec<Vec<(u32, usize)>> = rows
            .iter()
            .map(|r| r.runs.iter().map(|run| (run.style.fg, run.len)).collect())
            .collect();
        assert_eq!(styles, [vec![(0, 1), (1, 1)], vec![(1, 1), (0, 1)]]);
    }

    #[test]
    fn the_cursor_keeps_its_place_in_the_text_or_goes_past_the_end() {
        assert_eq!(rewrap_with("abcdef", &[], 3, Some(4)).1, Some(CursorPlace { row: 1, col: 1 }));
        assert_eq!(rewrap_with("abcdef", &[], 4, Some(8)).1, Some(CursorPlace { row: 1, col: 3 }));
        assert_eq!(rewrap_with("abcdef", &[], 4, None).1, None);
    }

    #[test]
    fn marks_stay_on_the_row_where_their_input_row_resumes() {
        let mut rewrapper = Rewrapper::new(3);
        let mut rows = Rows::default();
        let rows_in = [
            ("abcdefg", LineFlags::PROMPT_MARK.union(LineFlags::WRAPPED)),
            ("hij", LineFlags::COMMAND_START),
        ];
        for (text, flags) in rows_in {
            let runs = [StyledRun { style: Style::default(), len: text.len() }];
            rewrapper.feed(&Feed { flags, text, runs: &runs, cursor_x: None }, &mut rows);
        }
        rewrapper.finish(&mut rows);
        let flags: Vec<LineFlags> = rows.pending.iter().map(|row| row.flags).collect();
        assert!(flags[0].contains(LineFlags::PROMPT_MARK));
        assert!(!flags[0].contains(LineFlags::COMMAND_START));
        assert!(flags[1].contains(LineFlags::COMMAND_START));
    }

    #[test]
    fn a_cluster_split_across_two_input_rows_is_one_cluster_again() {
        let mut rewrapper = Rewrapper::new(10);
        let mut rows = Rows::default();
        for text in ["ae", "\u{301}b"] {
            let runs = [StyledRun { style: Style::default(), len: text.chars().count() }];
            let flags = LineFlags::WRAPPED;
            rewrapper.feed(&Feed { flags, text, runs: &runs, cursor_x: None }, &mut rows);
        }
        rewrapper.finish(&mut rows);
        assert_eq!(texts(&rows.pending.into_iter().collect::<Vec<_>>()), ["ae\u{301}b"]);
    }
}
