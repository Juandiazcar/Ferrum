//! Character and grapheme-cluster widths (FR-TERM-003).
//!
//! Every width in the crate comes from here, so a row encoded into the scrollback decodes to the
//! same columns it had in the grid: the scrollback stores scalars only and the consumer derives
//! widths (`docs/02` §3.2).

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::limits::MAX_CLUSTER_BYTES;

/// Columns of a wide character, the widest cell run of one cluster.
pub(crate) const WIDE_COLUMNS: usize = 2;

/// Columns of one scalar: 0 for combining and zero-width scalars, 1 or 2 otherwise.
pub fn char_width(c: char) -> usize {
    c.width().unwrap_or(0).min(WIDE_COLUMNS)
}

/// Columns of a grapheme cluster: 0, 1 or 2.
pub fn cluster_width(cluster: &str) -> usize {
    cluster.width().min(WIDE_COLUMNS)
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
pub fn clusters(text: &str) -> impl Iterator<Item = &str> {
    text.graphemes(true)
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
