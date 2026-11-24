//! Text and style runs back into cells: the inverse of `codec::encode_row`, used when reflow
//! rebuilds the visible screen.

use ferrum_scrollback::{LineFlags, Style};

use crate::cell::{BLANK_CHAR, Cell, CellFlags};
use crate::codec::WRAP_PAD;
use crate::grid::Grid;
use crate::limits::{MAX_CLUSTER_BYTES, MAX_UTF8_BYTES};
use crate::rewrap::{OutRow, RunCursor};
use crate::width::{capped_clusters, cluster_width};

fn styled_cell(first: char, style: Style) -> Cell {
    Cell::styled(first, style.fg, style.bg, style.attrs & !WRAP_PAD)
}

/// Writes `out` into row `y` of `grid`, which must already be blank. Returns the columns used.
pub(crate) fn fill_row(grid: &mut Grid, y: usize, out: &OutRow) -> usize {
    let cols = grid.cols();
    let mut runs = RunCursor::new(&out.runs);
    let mut x = 0;
    for cluster in capped_clusters(&out.text) {
        let scalars = cluster.chars().count();
        let style = runs.take(scalars);
        match cluster_width(cluster) {
            0 => append_to_previous(grid, y, x, cluster),
            width if x + width <= cols => {
                place(grid, (y, x), cluster, style);
                x += width;
            }
            _ => break,
        }
    }
    if out.flags.contains(LineFlags::WRAPPED)
        && x + 1 == cols
        && let Some(cell) = grid.row_mut(y).and_then(|row| row.get_mut(x))
    {
        *cell = Cell::default();
        cell.insert_flags(CellFlags::LEADING_SPACER);
    }
    if let Some(row) = grid.row_mut(y) {
        row.set_flags(out.flags);
    }
    x
}

/// Puts one cluster at column `x`: a plain scalar, or an interned cluster, wide or not.
fn place(grid: &mut Grid, (y, x): (usize, usize), cluster: &str, style: Style) {
    let mut chars = cluster.chars();
    let first = chars.next().unwrap_or(BLANK_CHAR);
    let mut head = styled_cell(first, style);
    if chars.next().is_some()
        && let Some(index) = grid.intern_cluster(cluster)
    {
        head.set_raw_char(index);
        head.insert_flags(CellFlags::GRAPHEME);
    }
    let wide = cluster_width(cluster) == crate::width::WIDE_COLUMNS;
    let mut tail = styled_cell(BLANK_CHAR, style);
    if wide {
        head.insert_flags(CellFlags::WIDE);
        tail.insert_flags(CellFlags::SPACER);
    }
    let Some(row) = grid.row_mut(y) else { return };
    if let Some(cell) = row.get_mut(x) {
        *cell = head;
    }
    if let Some(cell) = row.get_mut(x + 1).filter(|_| wide) {
        *cell = tail;
    }
}

/// A zero-width cluster joins the cell before column `x` (its head when that is a spacer).
fn append_to_previous(grid: &mut Grid, y: usize, x: usize, cluster: &str) {
    let Some(mut at) = x.checked_sub(1) else { return };
    let Some(cell) = grid.row(y).and_then(|row| row.get(at)).copied() else { return };
    if cell.flags().contains(CellFlags::SPACER) {
        let Some(head) = at.checked_sub(1) else { return };
        at = head;
    }
    let Some(head) = grid.row(y).and_then(|row| row.get(at)).copied() else { return };
    let mut buffer = [0; MAX_UTF8_BYTES];
    let mut merged = String::from(grid.cell_text(&head, &mut buffer));
    merged.push_str(cluster);
    if merged.len() > MAX_CLUSTER_BYTES {
        return;
    }
    if let Some(index) = grid.intern_cluster(&merged)
        && let Some(cell) = grid.row_mut(y).and_then(|row| row.get_mut(at))
    {
        cell.set_raw_char(index);
        cell.insert_flags(CellFlags::GRAPHEME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{Encoded, encode_row};
    use crate::size::Size;
    use ferrum_scrollback::StyledRun;

    fn out_row(text: &str) -> OutRow {
        let scalars = text.chars().count();
        OutRow {
            text: text.to_owned(),
            runs: vec![StyledRun { style: Style::default(), len: scalars }],
            flags: LineFlags::NONE,
        }
    }

    #[test]
    fn filling_then_encoding_gives_the_text_back() {
        for text in ["plain", "漢字ab", "e\u{301}x", "👨‍👩‍👧!", "a\u{200b}b", "🇺🇸x"]
        {
            let mut grid = Grid::new(Size::new(12, 1));
            fill_row(&mut grid, 0, &out_row(text));
            let mut out = Encoded::default();
            if let Some(row) = grid.row(0) {
                encode_row(row, grid.graphemes(), &mut out);
            }
            assert_eq!(out.text().trim_end(), text);
        }
    }

    #[test]
    fn a_wrapped_row_one_column_short_gets_a_leading_spacer() {
        let mut grid = Grid::new(Size::new(4, 1));
        let mut row = out_row("abc");
        row.flags = LineFlags::WRAPPED;
        assert_eq!(fill_row(&mut grid, 0, &row), 3);
        let last = grid.row(0).and_then(|r| r.get(3)).copied().unwrap_or_default();
        assert!(last.flags().contains(CellFlags::LEADING_SPACER));
        assert!(grid.row(0).is_some_and(crate::row::Row::is_wrapped));
    }

    #[test]
    fn the_wrap_padding_style_is_removed() {
        let mut grid = Grid::new(Size::new(4, 1));
        let mut row = out_row("a   ");
        row.runs = vec![
            StyledRun { style: Style::default(), len: 1 },
            StyledRun { style: Style { attrs: WRAP_PAD, ..Style::default() }, len: 3 },
        ];
        fill_row(&mut grid, 0, &row);
        assert!(grid.row(0).is_some_and(|r| r.cells().iter().skip(1).all(Cell::is_default_blank)));
    }
}
