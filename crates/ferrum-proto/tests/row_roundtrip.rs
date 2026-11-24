//! Round-trip property of the row encoding (`docs/08` §2: `proto` round-trip): any cells that fit
//! the row decode back to exactly what was encoded, at the right columns.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use ferrum_proto::{
    Attrs, CellRef, CellView, CellWidth, Color, LinkTable, ProtoError, Style, decode_row,
    encode_row,
};
use proptest::prelude::*;

#[path = "support/sample.rs"]
mod sample;

#[derive(Debug, Clone)]
struct OwnedCell {
    text: String,
    width: CellWidth,
    attrs: Attrs,
    colors: [Color; 3],
    link: Option<String>,
}

impl OwnedCell {
    fn as_ref(&self) -> CellRef<'_> {
        let [fg, bg, underline_color] = self.colors;
        let style =
            Style { attrs: self.attrs, fg, bg, underline_color, link: self.link.as_deref() };
        CellRef { text: &self.text, width: self.width, style }
    }
}

fn color() -> impl Strategy<Value = Color> {
    prop_oneof![
        Just(Color::DEFAULT),
        any::<u8>().prop_map(Color::indexed),
        any::<(u8, u8, u8)>().prop_map(|(r, g, b)| Color::rgb(r, g, b)),
    ]
}

fn cell() -> impl Strategy<Value = OwnedCell> {
    let text = prop_oneof![
        4 => "[ -~]",
        1 => "\\PC{1,4}",
    ];
    let width = prop_oneof![Just(CellWidth::Single), Just(CellWidth::Double)];
    let attrs = (0_u16..0x0800).prop_filter_map("defined", Attrs::from_bits);
    let link = proptest::option::weighted(0.1, "[a-z:/.]{1,20}");
    (text, width, attrs, [color(), color(), color()], link).prop_map(
        |(text, width, attrs, colors, link)| OwnedCell { text, width, attrs, colors, link },
    )
}

fn decode<'a>(
    bytes: &'a [u8],
    links: &'a LinkTable,
) -> Result<(u16, Vec<CellView<'a>>), ProtoError> {
    let cells = decode_row(bytes, links)?;
    let cols = cells.cols();
    Ok((cols, cells.collect::<Result<_, _>>()?))
}

proptest! {
    #[test]
    fn rows_round_trip(cells in proptest::collection::vec(cell(), 0..64), spare in 0_u16..8) {
        let used: u16 = cells.iter().map(|cell| cell.width as u16).sum();
        let (mut out, mut links) = (Vec::new(), LinkTable::new());
        encode_row(used + spare, cells.iter().map(OwnedCell::as_ref), &mut out, &mut links).unwrap();
        let (cols, decoded) = decode(&out, &links).unwrap();
        prop_assert_eq!(cols, used + spare);
        prop_assert_eq!(decoded.len(), cells.len());
        let mut col = 0;
        for (got, want) in decoded.iter().zip(&cells) {
            let want = want.as_ref();
            prop_assert_eq!((got.col, got.text, got.width, got.style), (col, want.text, want.width, want.style));
            col += want.width as u16;
        }
    }

}

#[test]
fn the_sample_row_round_trips() {
    let (mut out, mut links) = (Vec::new(), LinkTable::new());
    encode_row(sample::COLS, sample::sample_cells(), &mut out, &mut links).unwrap();
    let (cols, decoded) = decode(&out, &links).unwrap();
    assert_eq!(cols, sample::COLS);
    let expected: Vec<_> = sample::sample_cells().collect();
    assert_eq!(decoded.len(), expected.len());
    for (got, want) in decoded.iter().zip(&expected) {
        assert_eq!((got.text, got.width, got.style), (want.text, want.width, want.style));
    }
    let last = decoded.last().unwrap();
    assert_eq!(last.col + last.width as u16, sample::COLS);
}

#[test]
fn a_row_too_wide_for_its_columns_is_refused() {
    let (mut out, mut links) = (Vec::new(), LinkTable::new());
    let result = encode_row(sample::COLS - 1, sample::sample_cells(), &mut out, &mut links);
    assert!(matches!(result, Err(ProtoError::TooManyCells { .. })));
    assert_eq!(out, Vec::<u8>::new());
    assert!(links.is_empty(), "the links of a refused row are rolled back");
}
