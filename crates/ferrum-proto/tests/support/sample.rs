//! A realistic screen row shared by the tests and the bench: 200 columns of mostly ASCII text in
//! a few colored runs, with a hyperlink, a wide character and a multi-scalar cluster.

use ferrum_proto::{Attrs, CellRef, CellWidth, Color, Style, Underline};

/// Columns of the sample row (the reference window is about 200 columns wide, `docs/03` §1).
pub(crate) const COLS: u16 = 200;

const LINK: &str = "https://example.test/build/42";
const TEXT: &str =
    "error[E0277]: the trait bound `Cell: Default` is not satisfied -> src/grid.rs:42 ";

/// The cells of the sample row, left to right, filling exactly [`COLS`] columns.
pub(crate) fn sample_cells() -> impl Iterator<Item = CellRef<'static>> + Clone {
    let red = Style { attrs: Attrs::BOLD, fg: Color::indexed(1), ..Style::default() };
    let link = Style {
        attrs: Attrs::NONE.with_underline(Underline::Single),
        fg: Color::rgb(0x4E, 0x9A, 0xF0),
        link: Some(LINK),
        ..Style::default()
    };
    let plain = Style::default();
    let text = TEXT.split_inclusive(|_: char| true).cycle();
    let ascii = text.zip(0..COLS - 4).map(move |(text, col)| {
        let style = match col {
            0..12 => red,
            60..90 => link,
            _ => plain,
        };
        CellRef { text, width: CellWidth::Single, style }
    });
    let wide = CellRef { text: "界", width: CellWidth::Double, style: plain };
    let cluster = CellRef { text: "e\u{301}", width: CellWidth::Single, style: plain };
    let flag = CellRef { text: "\u{1F1E8}\u{1F1F4}", width: CellWidth::Single, style: red };
    ascii.chain([wide, cluster, flag])
}
