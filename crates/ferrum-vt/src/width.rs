//! Character and grapheme-cluster widths (FR-TERM-003).
//!
//! Every width in the crate comes from here, so a row encoded into the scrollback decodes to the
//! same columns it had in the grid: the scrollback stores scalars only and the consumer derives
//! widths (`docs/02` §3.2).

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Widest cell run of one cluster.
const MAX_CLUSTER_COLUMNS: usize = 2;

/// Columns of one scalar: 0 for combining and zero-width scalars, 1 or 2 otherwise.
pub fn char_width(c: char) -> usize {
    c.width().unwrap_or(0).min(MAX_CLUSTER_COLUMNS)
}

/// Columns of a grapheme cluster: 0, 1 or 2.
pub fn cluster_width(cluster: &str) -> usize {
    cluster.width().min(MAX_CLUSTER_COLUMNS)
}

/// True when `c` continues the cluster `previous` instead of starting a new one (UAX #29).
/// `scratch` is a reusable buffer, so the check does not allocate in steady state.
pub fn joins_cluster(previous: &str, c: char, scratch: &mut String) -> bool {
    if c.is_ascii() {
        // Printable ASCII always starts a cluster (prepend scalars are not supported).
        return false;
    }
    scratch.clear();
    scratch.push_str(previous);
    scratch.push(c);
    scratch.graphemes(true).nth(1).is_none()
}

/// The grapheme clusters of `text`.
pub fn clusters(text: &str) -> impl Iterator<Item = &str> {
    text.graphemes(true)
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
