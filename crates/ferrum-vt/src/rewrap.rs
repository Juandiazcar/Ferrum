//! Re-wrapping one logical line at a new width (FR-TERM-010, AC-TERM-02).
//!
//! A logical line is the text of a row and of the rows soft-wrapped into it. It is cut into rows
//! of at most `cols` display columns, never inside a grapheme cluster, and a double-width
//! character that does not fit starts the next row (the grid shows a leading spacer there). The
//! work happens on text and style runs, not on cells, so scrollback rows never become cells.

use ferrum_scrollback::{LineFlags, Style, StyledRun};

use crate::width::{cluster_width, clusters};

/// A logical line under reflow.
#[derive(Debug, Default)]
pub(crate) struct LogicalLine {
    pub(crate) text: String,
    pub(crate) runs: Vec<StyledRun>,
    /// Marks (prompt, command) of the rows that make up the line; they go on its first row.
    pub(crate) marks: LineFlags,
    /// Where the cursor is, in columns from the start of the line.
    pub(crate) cursor_offset: Option<usize>,
    /// Columns the rows of the line occupy so far (the offset of the next row's first cell).
    pub(crate) columns: usize,
}

impl LogicalLine {
    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.runs.clear();
        self.marks = LineFlags::NONE;
        self.cursor_offset = None;
        self.columns = 0;
    }
}

/// One output row.
#[derive(Debug, Default)]
pub(crate) struct OutRow {
    pub(crate) text: String,
    pub(crate) runs: Vec<StyledRun>,
    pub(crate) flags: LineFlags,
}

impl OutRow {
    fn push(&mut self, cluster: &str, style: Style, scalars: usize) {
        self.text.push_str(cluster);
        match self.runs.last_mut() {
            Some(last) if last.style == style => last.len += scalars,
            _ => self.runs.push(StyledRun { style, len: scalars }),
        }
    }
}

/// Where the cursor ended up: a row of the output and a column in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CursorPlace {
    pub(crate) row: usize,
    pub(crate) col: usize,
}

/// Hands out the style of each scalar of a line, run by run.
pub(crate) struct RunCursor<'a> {
    runs: &'a [StyledRun],
    index: usize,
    used: usize,
}

impl<'a> RunCursor<'a> {
    pub(crate) fn new(runs: &'a [StyledRun]) -> Self {
        Self { runs, index: 0, used: 0 }
    }

    /// The style at the current scalar; advances by `scalars`.
    pub(crate) fn take(&mut self, scalars: usize) -> Style {
        let style = self.runs.get(self.index).map_or_else(Style::default, |run| run.style);
        let mut left = scalars;
        while left > 0 {
            let Some(run) = self.runs.get(self.index) else { break };
            let available = run.len.saturating_sub(self.used);
            if left < available {
                self.used += left;
                break;
            }
            left -= available;
            self.index += 1;
            self.used = 0;
        }
        style
    }
}

/// Cuts `line` into rows of at most `cols` columns. `cols` is at least 1.
pub(crate) fn rewrap(line: &LogicalLine, cols: usize) -> (Vec<OutRow>, Option<CursorPlace>) {
    let mut rows = vec![OutRow { flags: line.marks, ..OutRow::default() }];
    let mut runs = RunCursor::new(&line.runs);
    let (mut col, mut offset) = (0, 0);
    let mut cursor = None;
    for cluster in clusters(&line.text) {
        let scalars = cluster.chars().count();
        let style = runs.take(scalars);
        let width = cluster_width(cluster);
        if width == 0 {
            if let Some(row) = rows.last_mut() {
                row.push(cluster, style, scalars);
            }
            continue;
        }
        if width > cols {
            offset += width;
            continue;
        }
        if col + width > cols {
            if let Some(row) = rows.last_mut() {
                row.flags = row.flags.union(LineFlags::WRAPPED);
            }
            rows.push(OutRow::default());
            col = 0;
        }
        if cursor.is_none()
            && line.cursor_offset.is_some_and(|at| (offset..offset + width).contains(&at))
        {
            cursor = Some(CursorPlace { row: rows.len() - 1, col });
        }
        if let Some(row) = rows.last_mut() {
            row.push(cluster, style, scalars);
        }
        col += width;
        offset += width;
    }
    if cursor.is_none() {
        cursor = line.cursor_offset.map(|at| CursorPlace {
            row: rows.len() - 1,
            col: (col + at.saturating_sub(offset)).min(cols - 1),
        });
    }
    (rows, cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> LogicalLine {
        let scalars = text.chars().count();
        LogicalLine {
            text: text.to_owned(),
            runs: vec![StyledRun { style: Style::default(), len: scalars }],
            ..LogicalLine::default()
        }
    }

    fn texts(rows: &[OutRow]) -> Vec<&str> {
        rows.iter().map(|row| row.text.as_str()).collect()
    }

    #[test]
    fn a_line_is_cut_at_the_width_and_all_but_the_last_row_are_wrapped() {
        let (rows, _) = rewrap(&line("abcdefgh"), 3);
        assert_eq!(texts(&rows), ["abc", "def", "gh"]);
        let wrapped: Vec<bool> =
            rows.iter().map(|r| r.flags.contains(LineFlags::WRAPPED)).collect();
        assert_eq!(wrapped, [true, true, false]);
    }

    #[test]
    fn an_exact_fit_does_not_add_an_empty_row() {
        assert_eq!(texts(&rewrap(&line("abcdef"), 3).0), ["abc", "def"]);
        assert_eq!(texts(&rewrap(&line(""), 3).0), [""]);
    }

    #[test]
    fn wide_characters_and_clusters_are_never_split() {
        assert_eq!(texts(&rewrap(&line("ab漢字"), 3).0), ["ab", "漢", "字"]);
        assert_eq!(texts(&rewrap(&line("ae\u{301}b"), 2).0), ["ae\u{301}", "b"]);
        assert_eq!(texts(&rewrap(&line("a👨‍👩‍👧b"), 2).0), ["a", "👨‍👩‍👧", "b"]);
    }

    #[test]
    fn a_wide_character_wider_than_the_terminal_is_dropped() {
        assert_eq!(texts(&rewrap(&line("a漢b"), 1).0), ["a", "b"]);
    }

    #[test]
    fn styles_follow_their_scalars_across_the_cuts() {
        let red = Style { fg: 1, ..Style::default() };
        let mut input = line("abcd");
        input.runs = vec![
            StyledRun { style: Style::default(), len: 1 },
            StyledRun { style: red, len: 2 },
            StyledRun { style: Style::default(), len: 1 },
        ];
        let (rows, _) = rewrap(&input, 2);
        let styles: Vec<Vec<(u32, usize)>> = rows
            .iter()
            .map(|r| r.runs.iter().map(|run| (run.style.fg, run.len)).collect())
            .collect();
        assert_eq!(styles, [vec![(0, 1), (1, 1)], vec![(1, 1), (0, 1)]]);
    }

    #[test]
    fn the_cursor_keeps_its_place_in_the_text_or_goes_past_the_end() {
        let mut input = line("abcdef");
        input.cursor_offset = Some(4);
        assert_eq!(rewrap(&input, 3).1, Some(CursorPlace { row: 1, col: 1 }));
        input.cursor_offset = Some(8);
        assert_eq!(rewrap(&input, 4).1, Some(CursorPlace { row: 1, col: 3 }));
        input.cursor_offset = None;
        assert_eq!(rewrap(&input, 4).1, None);
    }

    #[test]
    fn marks_go_on_the_first_row() {
        let mut input = line("abcdef");
        input.marks = LineFlags::PROMPT_MARK;
        let (rows, _) = rewrap(&input, 3);
        assert!(rows[0].flags.contains(LineFlags::PROMPT_MARK));
        assert!(!rows[1].flags.contains(LineFlags::PROMPT_MARK));
    }
}
