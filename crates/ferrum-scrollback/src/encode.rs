//! Line encoder: `docs/02` §3.2 exactly.
//!
//! ```text
//! line := flags:u8 | text_len:varint | utf8_text (trailing default-blank cells trimmed)
//!         | run_count:varint | { style_id:varint | run_len:varint }*
//! ```
//!
//! Encoding is canonical: trailing default-style spaces are trimmed, zero-length runs are dropped
//! and adjacent runs with the same style are merged. `run_len` counts Unicode scalars.

use crate::error::ScrollbackError;
use crate::limits::{MAX_ENCODED_LINE_BYTES, MAX_LINE_TEXT_BYTES};
use crate::line::{LineFlags, LineRef, StyledRun};
use crate::style::{Style, StyleId, StyleTable};
use crate::varint::{MAX_VARINT_LEN, put_len, put_varint};

/// A validated, canonicalized line ready to be written.
pub(crate) struct Planned<'a> {
    flags: LineFlags,
    text: &'a str,
    run_count: usize,
}

/// Encoder state: the scratch buffer holding the encoded runs of the line being planned.
#[derive(Debug, Default)]
pub(crate) struct Encoder {
    runs: Vec<u8>,
}

impl Encoder {
    /// Validates `line`, interns its styles and encodes its runs into the scratch buffer.
    pub(crate) fn plan<'a>(
        &mut self,
        styles: &mut StyleTable,
        line: &LineRef<'a>,
    ) -> Result<Planned<'a>, ScrollbackError> {
        if line.text.len() > MAX_LINE_TEXT_BYTES {
            return Err(ScrollbackError::LineTooLong {
                len: line.text.len(),
                max: MAX_LINE_TEXT_BYTES,
            });
        }
        // perf: an ASCII check is cheaper than counting scalars, and most lines are ASCII
        // (bench vt_parse/ascii_8mib).
        let chars = if line.text.is_ascii() { line.text.len() } else { line.text.chars().count() };
        check_coverage(line.runs, chars)?;
        let blanks = trailing_blanks(line.text, line.runs);
        let text = line.text.get(..line.text.len() - blanks).unwrap_or(line.text);
        let kept = chars - blanks;
        self.runs.clear();
        let run_count = self.encode_runs(styles, line.runs, kept);
        let planned = Planned { flags: line.flags, text, run_count };
        let len = self.max_len(&planned);
        if len > MAX_ENCODED_LINE_BYTES {
            return Err(ScrollbackError::LineTooLong { len, max: MAX_ENCODED_LINE_BYTES });
        }
        Ok(planned)
    }

    /// Upper bound of the encoded size of a planned line.
    pub(crate) fn max_len(&self, planned: &Planned<'_>) -> usize {
        1 + MAX_VARINT_LEN + planned.text.len() + MAX_VARINT_LEN + self.runs.len()
    }

    /// Appends the encoded line to `out`.
    pub(crate) fn write(&self, planned: &Planned<'_>, out: &mut Vec<u8>) {
        out.push(planned.flags.bits());
        put_len(out, planned.text.len());
        out.extend_from_slice(planned.text.as_bytes());
        put_len(out, planned.run_count);
        out.extend_from_slice(&self.runs);
    }

    /// Capacity of the scratch buffer, for memory accounting.
    pub(crate) fn capacity_bytes(&self) -> usize {
        self.runs.capacity()
    }

    /// Writes the runs covering the first `kept` scalars, merging equal neighbors.
    fn encode_runs(&mut self, styles: &mut StyleTable, runs: &[StyledRun], kept: usize) -> usize {
        if runs.is_empty() {
            return self.push_run(StyleId::DEFAULT, kept);
        }
        let mut remaining = kept;
        let mut count = 0;
        let mut pending: Option<(StyleId, usize)> = None;
        for run in runs {
            let take = run.len.min(remaining);
            if take == 0 {
                break;
            }
            remaining -= take;
            let id = styles.intern(run.style);
            pending = match pending {
                Some((last, len)) if last == id => Some((last, len + take)),
                Some((last, len)) => {
                    count += self.push_run(last, len);
                    Some((id, take))
                }
                None => Some((id, take)),
            };
        }
        if let Some((id, len)) = pending {
            count += self.push_run(id, len);
        }
        count
    }

    /// Appends one run; returns how many runs were written (0 for an empty run).
    fn push_run(&mut self, id: StyleId, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        put_varint(&mut self.runs, u64::from(id.get()));
        put_len(&mut self.runs, len);
        1
    }
}

fn check_coverage(runs: &[StyledRun], chars: usize) -> Result<(), ScrollbackError> {
    if runs.is_empty() {
        return Ok(());
    }
    let covered = runs.iter().fold(0usize, |sum, run| sum.saturating_add(run.len));
    if covered == chars {
        Ok(())
    } else {
        Err(ScrollbackError::RunsMismatch { runs: covered, chars })
    }
}

/// Number of trailing spaces that are default-styled (and so are trimmed).
fn trailing_blanks(text: &str, runs: &[StyledRun]) -> usize {
    let bytes = text.as_bytes();
    let spaces = |limit: usize, end: usize| {
        bytes.get(..end).map_or(0, |head| {
            head.iter().rev().take(limit).take_while(|&&byte| byte == b' ').count()
        })
    };
    if runs.is_empty() {
        return spaces(bytes.len(), bytes.len());
    }
    let mut blanks = 0;
    for run in runs.iter().rev() {
        if run.style != Style::default() {
            break;
        }
        let found = spaces(run.len, bytes.len() - blanks);
        blanks += found;
        if found < run.len {
            break;
        }
    }
    blanks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(text: &str, runs: &[StyledRun]) -> (Vec<u8>, StyleTable) {
        let mut styles = StyleTable::new();
        let mut encoder = Encoder::default();
        let line = LineRef { flags: LineFlags::WRAPPED, text, runs };
        let planned = encoder.plan(&mut styles, &line).map(|p| {
            let mut out = Vec::new();
            encoder.write(&p, &mut out);
            out
        });
        (planned.unwrap_or_default(), styles)
    }

    #[test]
    fn matches_the_documented_layout() {
        let (bytes, _) = encode("ab", &[]);
        // flags | text_len | "ab" | run_count | style 0 | len 2
        assert_eq!(bytes, [1, 2, b'a', b'b', 1, 0, 2]);
    }

    #[test]
    fn trims_trailing_default_blanks_but_not_styled_ones() {
        let red = Style { fg: 1, bg: 0, attrs: 0 };
        let runs = [
            StyledRun { style: Style::default(), len: 2 },
            StyledRun { style: red, len: 2 },
            StyledRun { style: Style::default(), len: 3 },
        ];
        let (bytes, styles) = encode("hi     ", &runs);
        assert_eq!(styles.len(), 2);
        // text "hi  " keeps the styled spaces; the three default blanks are gone.
        assert_eq!(bytes, [1, 4, b'h', b'i', b' ', b' ', 2, 0, 2, 1, 2]);
    }

    #[test]
    fn blank_line_has_no_runs() {
        let (bytes, _) = encode("    ", &[]);
        assert_eq!(bytes, [1, 0, 0]);
    }

    #[test]
    fn rejects_runs_that_do_not_cover_the_text() {
        let mut styles = StyleTable::new();
        let mut encoder = Encoder::default();
        let runs = [StyledRun { style: Style::default(), len: 5 }];
        let line = LineRef { flags: LineFlags::NONE, text: "ab", runs: &runs };
        assert!(matches!(
            encoder.plan(&mut styles, &line),
            Err(ScrollbackError::RunsMismatch { runs: 5, chars: 2 })
        ));
    }
}
