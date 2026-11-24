//! Character and grapheme-cluster widths (FR-TERM-003).
//!
//! Every width in the crate comes from here, so a row encoded into the scrollback decodes to the
//! same columns it had in the grid: the scrollback stores scalars only and the consumer derives
//! widths (`docs/02` §3.2).

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::limits::MAX_CLUSTER_BYTES;

/// Printable ASCII is `FIRST_PRINTABLE..=LAST_PRINTABLE`.
const FIRST_PRINTABLE: u8 = 0x20;
const LAST_PRINTABLE: u8 = 0x7E;
/// Columns of a wide character, the widest cell run of one cluster.
pub(crate) const WIDE_COLUMNS: usize = 2;

/// Columns of one scalar: 0 for combining and zero-width scalars, 1 or 2 otherwise.
pub fn char_width(c: char) -> usize {
    c.width().unwrap_or(0).min(WIDE_COLUMNS)
}

/// Columns of a grapheme cluster: 0, 1 or 2.
pub fn cluster_width(cluster: &str) -> usize {
    if let [byte] = cluster.as_bytes()
        && is_plain_ascii(*byte)
    {
        return 1;
    }
    cluster.width().min(WIDE_COLUMNS)
}

/// Printable ASCII (0x20..=0x7E): one column, and a cluster of its own unless a non-ASCII scalar
/// follows.
pub(crate) fn is_plain_ascii(byte: u8) -> bool {
    (FIRST_PRINTABLE..=LAST_PRINTABLE).contains(&byte)
}

/// Length in bytes of the leading run of printable ASCII in `text` that is made of whole
/// clusters of one byte each: a byte followed by a non-ASCII scalar may be extended by it (a
/// combining mark), so it is left out.
pub(crate) fn ascii_run(text: &str) -> usize {
    let bytes = text.as_bytes();
    let run = bytes.iter().take_while(|&&byte| is_plain_ascii(byte)).count();
    match bytes.get(run) {
        Some(next) if !next.is_ascii() => run.saturating_sub(1),
        _ => run,
    }
}

/// True when `c` continues the cluster `previous` instead of starting a new one (UAX #29).
/// `scratch` is a reusable buffer, so the check does not allocate in steady state.
pub fn joins_cluster(previous: &str, c: char, scratch: &mut String) -> bool {
    scratch.clear();
    scratch.push_str(previous);
    scratch.push(c);
    scratch.graphemes(true).nth(1).is_none()
}

/// The grapheme clusters of `text`.
///
/// perf: a printable ASCII byte that is not followed by a non-ASCII scalar is a cluster by
/// itself, so ASCII text skips the UAX #29 tables (bench `vt_reflow/10k_lines_200_to_100_columns`).
pub fn clusters(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        let first = *rest.as_bytes().first()?;
        let alone = is_plain_ascii(first) && rest.as_bytes().get(1).is_none_or(u8::is_ascii);
        let cut = if alone { 1 } else { rest.graphemes(true).next()?.len() };
        let (cluster, tail) = rest.split_at_checked(cut)?;
        rest = tail;
        Some(cluster)
    })
}

/// Like [`clusters`], but a cluster longer than `MAX_CLUSTER_BYTES` comes out in pieces of at
/// most that many bytes, so no path builds a cluster the grapheme table would refuse.
pub(crate) fn capped_clusters(text: &str) -> impl Iterator<Item = &str> {
    clusters(text).flat_map(|cluster| {
        let mut rest = cluster;
        std::iter::from_fn(move || {
            if rest.is_empty() {
                return None;
            }
            let cut = rest.floor_char_boundary(MAX_CLUSTER_BYTES);
            let (piece, tail) = rest.split_at_checked(cut)?;
            rest = tail;
            Some(piece)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_cjk_and_combining_widths() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('漢'), 2);
        assert_eq!(char_width('\u{0301}'), 0);
        assert_eq!(cluster_width("e\u{0301}"), 1);
        assert_eq!(cluster_width("👨‍👩‍👧"), 2);
        assert_eq!(cluster_width("\u{2764}\u{FE0F}"), 2);
    }

    #[test]
    fn the_ascii_fast_path_agrees_with_the_segmentation_tables() {
        for text in ["plain text", "a\u{301}b", "x\r\ny", "ab\u{200d}c", "z", "ab漢c", "e\u{301}"]
        {
            let slow: Vec<&str> = text.graphemes(true).collect();
            assert_eq!(clusters(text).collect::<Vec<_>>(), slow, "{text:?}");
        }
        assert_eq!(ascii_run("abc"), 3);
        assert_eq!(ascii_run("abc\u{301}"), 2);
        assert_eq!(ascii_run("ab\tc"), 2);
    }

    #[test]
    fn long_clusters_come_out_in_capped_pieces() {
        let chain = "\u{1f468}\u{200d}".repeat(50);
        assert_eq!(clusters(&chain).count(), 1);
        let pieces: Vec<&str> = capped_clusters(&chain).collect();
        assert!(pieces.len() > 1 && pieces.iter().all(|piece| piece.len() <= MAX_CLUSTER_BYTES));
        assert_eq!(pieces.concat(), chain);
    }

    #[test]
    fn combining_marks_and_joiners_continue_a_cluster() {
        let mut scratch = String::new();
        assert!(joins_cluster("e", '\u{0301}', &mut scratch));
        assert!(joins_cluster("👨", '\u{200D}', &mut scratch));
        assert!(!joins_cluster("e", 'x', &mut scratch));
        assert!(!joins_cluster("漢", '字', &mut scratch));
        assert!(!joins_cluster("🇺🇸", '🇧', &mut scratch));
        assert!(joins_cluster("🇺", '🇸', &mut scratch));
    }
}
