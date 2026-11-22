//! Test support: an independent reference model of what the store must return.
//!
//! The model never calls the encoder: a line is a list of `(char, style)` cells; the expected
//! decoded form is derived by the rules of `docs/02` §3.2 (trim trailing default blanks, merge
//! equal neighbors) written out separately here.

#![allow(dead_code, reason = "each test binary uses a different subset of the helpers")]
#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_scrollback::{LineCursor, LineFlags, LineRef, Scrollback, Style, StyledRun};

/// A row as the grid would hold it: one `(scalar, style)` per scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenLine {
    pub(crate) flags: LineFlags,
    pub(crate) cells: Vec<(char, Style)>,
}

/// What reading the line back must give.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expected {
    pub(crate) flags: LineFlags,
    pub(crate) text: String,
    pub(crate) runs: Vec<(Style, u32)>,
}

impl GenLine {
    pub(crate) fn text(&self) -> String {
        self.cells.iter().map(|&(c, _)| c).collect()
    }

    /// One run per maximal group of equal styles, split further at every `cut` index so the
    /// encoder also sees adjacent runs of the same style.
    pub(crate) fn runs(&self, cuts: &[usize]) -> Vec<StyledRun> {
        let mut runs: Vec<StyledRun> = Vec::new();
        for (index, &(_, style)) in self.cells.iter().enumerate() {
            match runs.last_mut() {
                Some(last) if last.style == style && !cuts.contains(&index) => last.len += 1,
                _ => runs.push(StyledRun { style, len: 1 }),
            }
        }
        runs
    }

    pub(crate) fn expected(&self) -> Expected {
        let mut cells = self.cells.clone();
        while cells.last().is_some_and(|&(c, s)| c == ' ' && s == Style::default()) {
            cells.pop();
        }
        let mut runs: Vec<(Style, u32)> = Vec::new();
        for &(_, style) in &cells {
            match runs.last_mut() {
                Some((last, len)) if *last == style => *len += 1,
                _ => runs.push((style, 1)),
            }
        }
        let text = cells.iter().map(|&(c, _)| c).collect();
        Expected { flags: self.flags, text, runs }
    }
}

/// Pushes `line` with runs split at `cuts`.
pub(crate) fn push(store: &mut Scrollback, line: &GenLine, cuts: &[usize]) {
    let text = line.text();
    let runs = line.runs(cuts);
    let line_ref = LineRef { flags: line.flags, text: &text, runs: &runs };
    store.push_line(&line_ref).expect("push_line");
}

/// Reads line `index` and resolves its styles.
pub(crate) fn read(store: &Scrollback, index: usize, cursor: &mut LineCursor) -> Option<Expected> {
    let line = store.read_line(index, cursor).expect("read_line")?;
    let runs = line
        .runs
        .iter()
        .map(|run| (store.styles().get(run.style).expect("style in table"), run.len))
        .collect();
    Some(Expected { flags: line.flags, text: line.text.clone(), runs })
}

/// A deterministic line number `n`: text `line n`, three styles, wrapped every 7th line.
pub(crate) fn numbered(n: usize) -> GenLine {
    let styles =
        [Style::default(), Style { fg: 1, bg: 0, attrs: 0 }, Style { fg: 2, bg: 3, attrs: 4 }];
    let text = format!("line {n} with some filler text");
    let cells = text
        .chars()
        .enumerate()
        .map(|(i, c)| (c, styles.get((i / 5 + n) % styles.len()).copied().unwrap_or_default()))
        .collect();
    let flags = if n.is_multiple_of(7) { LineFlags::WRAPPED } else { LineFlags::NONE };
    GenLine { flags, cells }
}
