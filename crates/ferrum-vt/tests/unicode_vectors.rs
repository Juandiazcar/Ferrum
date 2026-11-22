//! Unicode width, grapheme and emoji vectors (`docs/08` §6 item 3, FR-TERM-003).
//!
//! The grapheme cases are lines of `GraphemeBreakTest.txt` (Unicode UAX #29) written as the
//! clusters they must produce; the width cases follow UAX #11 (East Asian Width) and the emoji
//! presentation rules of UTS #51. Each is checked twice: on the width functions and on a terminal
//! that prints the text, where the cells must hold exactly those clusters with those widths.

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{CellFlags, NullSink, Size, Terminal, char_width, cluster_width, clusters};

const COLS: u16 = 40;

/// `(text, [(cluster, columns)])`.
type Vector = (&'static str, &'static [(&'static str, usize)]);

const VECTORS: &[Vector] = &[
    // UAX #29: GB3/GB4/GB5 do not apply to printable text; GB9 (x Extend), GB9a (x SpacingMark),
    // GB11 (emoji ZWJ sequences), GB12/13 (regional indicator pairs), GB6 to GB8 (Hangul).
    ("  ", &[(" ", 1), (" ", 1)]),
    (" \u{0308} ", &[(" \u{0308}", 1), (" ", 1)]),
    ("a\u{0308}b", &[("a\u{0308}", 1), ("b", 1)]),
    ("a\u{200D}b", &[("a\u{200D}", 1), ("b", 1)]),
    ("\u{1F1E6}\u{1F1E7}\u{1F1E8}", &[("\u{1F1E6}\u{1F1E7}", 2), ("\u{1F1E8}", 1)]),
    (
        "a\u{1F1E6}\u{1F1E7}\u{1F1E8}b",
        &[("a", 1), ("\u{1F1E6}\u{1F1E7}", 2), ("\u{1F1E8}", 1), ("b", 1)],
    ),
    ("\u{1F476}\u{1F3FF}\u{1F476}", &[("\u{1F476}\u{1F3FF}", 2), ("\u{1F476}", 2)]),
    ("\u{1F6D1}\u{200D}\u{1F6D1}", &[("\u{1F6D1}\u{200D}\u{1F6D1}", 2)]),
    ("a\u{200D}\u{1F6D1}", &[("a\u{200D}", 1), ("\u{1F6D1}", 2)]),
    ("\u{1100}\u{1100}", &[("\u{1100}\u{1100}", 2)]),
    ("\u{1100}\u{1161}\u{11A8}", &[("\u{1100}\u{1161}\u{11A8}", 2)]),
    ("\u{AC00}\u{11A8}", &[("\u{AC00}\u{11A8}", 2)]),
    ("\u{AC01}\u{11A8}", &[("\u{AC01}\u{11A8}", 2)]),
    // A spacing mark keeps its own advance: KA + VISARGA is one cluster of two columns.
    ("\u{0915}\u{0903}", &[("\u{0915}\u{0903}", 2)]),
    // UAX #11: wide, fullwidth, halfwidth, narrow, ambiguous (narrow without a CJK context).
    ("\u{3042}", &[("\u{3042}", 2)]),
    ("\u{6F22}\u{5B57}", &[("\u{6F22}", 2), ("\u{5B57}", 2)]),
    ("\u{FF21}", &[("\u{FF21}", 2)]),
    ("\u{FF71}", &[("\u{FF71}", 1)]),
    (
        "\u{00B1}\u{00D7}\u{2192}\u{03B1}",
        &[("\u{00B1}", 1), ("\u{00D7}", 1), ("\u{2192}", 1), ("\u{03B1}", 1)],
    ),
    ("\u{2500}\u{2588}", &[("\u{2500}", 1), ("\u{2588}", 1)]),
    // UTS #51: emoji presentation, text presentation, VS15/VS16, modifiers, ZWJ and keycaps.
    ("\u{1F600}", &[("\u{1F600}", 2)]),
    ("\u{2764}", &[("\u{2764}", 1)]),
    ("\u{2764}\u{FE0F}", &[("\u{2764}\u{FE0F}", 2)]),
    ("\u{2764}\u{FE0E}", &[("\u{2764}\u{FE0E}", 1)]),
    ("\u{263A}\u{FE0F}", &[("\u{263A}\u{FE0F}", 2)]),
    ("\u{1F44D}\u{1F3FD}", &[("\u{1F44D}\u{1F3FD}", 2)]),
    (
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
        &[("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}", 2)],
    ),
    ("\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}", &[("\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}", 2)]),
    ("\u{1F9D1}\u{200D}\u{1F4BB}", &[("\u{1F9D1}\u{200D}\u{1F4BB}", 2)]),
    ("\u{1F1FA}\u{1F1F8}", &[("\u{1F1FA}\u{1F1F8}", 2)]),
    ("1\u{FE0F}\u{20E3}", &[("1\u{FE0F}\u{20E3}", 2)]),
    // Zero width.
    ("e\u{0301}\u{0323}", &[("e\u{0301}\u{0323}", 1)]),
];

fn terminal() -> Terminal<NullSink> {
    let config = ScrollbackConfig::default();
    Terminal::new(Size::new(COLS, 2), Scrollback::new(config), NullSink)
}

/// The clusters of the first row: text and columns, spacers merged into their head.
fn first_row(term: &Terminal<NullSink>) -> Vec<(String, usize)> {
    let grid = term.grid();
    let Some(row) = grid.row(0) else { return Vec::new() };
    let mut found = Vec::new();
    for cell in row.cells() {
        if cell.flags().contains(CellFlags::SPACER) {
            continue;
        }
        let mut buffer = [0; 4];
        found.push((grid.cell_text(cell, &mut buffer).to_owned(), cell.columns()));
    }
    found
}

#[test]
fn the_width_functions_give_the_vector_clusters_and_widths() {
    for &(text, expected) in VECTORS {
        let got: Vec<(&str, usize)> = clusters(text).map(|c| (c, cluster_width(c))).collect();
        assert_eq!(got, expected.to_vec(), "{text:?}");
    }
}

#[test]
fn a_terminal_prints_the_vectors_as_the_same_clusters() {
    for &(text, expected) in VECTORS {
        let mut term = terminal();
        term.parse(text.as_bytes());
        let mut got = first_row(&term);
        got.truncate(expected.len());
        let want: Vec<(String, usize)> = expected.iter().map(|&(c, w)| (c.to_owned(), w)).collect();
        assert_eq!(got, want, "{text:?}");
        let columns: usize = expected.iter().map(|&(_, w)| w).sum();
        assert_eq!(term.cursor().x, columns % usize::from(COLS), "cursor after {text:?}");
    }
}

#[test]
fn single_scalar_widths_follow_east_asian_width() {
    let cases = [
        ('a', 1),
        ('\u{3042}', 2),
        ('\u{FF21}', 2),
        ('\u{FF71}', 1),
        ('\u{00E9}', 1),
        ('\u{0301}', 0),
        ('\u{200B}', 0),
        ('\u{200D}', 0),
        ('\u{FE0F}', 0),
        ('\u{1F600}', 2),
        ('\u{1F3FD}', 2),
        ('\u{1F1E6}', 1),
        ('\u{1100}', 2),
        ('\u{1161}', 0),
    ];
    for (c, width) in cases {
        assert_eq!(char_width(c), width, "U+{:04X}", u32::from(c));
    }
}

/// ZWSP is a Control for UAX #29 (a cluster of its own, no columns); the terminal then keeps it
/// in the cell before it, so that it never takes a column.
#[test]
fn a_zero_width_space_is_its_own_cluster_but_not_its_own_cell() {
    let text = "a\u{200B}b";
    let got: Vec<(&str, usize)> = clusters(text).map(|c| (c, cluster_width(c))).collect();
    assert_eq!(got, [("a", 1), ("\u{200B}", 0), ("b", 1)]);
    let mut term = terminal();
    term.parse(text.as_bytes());
    assert_eq!(
        first_row(&term).get(..2),
        Some(&[("a\u{200B}".to_owned(), 1), ("b".to_owned(), 1)][..])
    );
    assert_eq!(term.cursor().x, 2);
}
