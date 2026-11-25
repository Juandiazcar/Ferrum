use super::*;
use crate::font::{BOLD_WEIGHT, Family, FontSpec, Fonts, Slant, TextStyle};
use crate::synth::Underline;

const BG: Rgb = Rgb::new(16, 18, 20);
const FG: Rgb = Rgb::new(215, 218, 224);
const PAD: u32 = 3;

struct Screen {
    glyphs: Glyphs,
    painter: Painter,
    pixels: Vec<u32>,
    width: u32,
    height: u32,
    grid: Grid,
}

impl Screen {
    fn new(cols: u16, rows: u16) -> Self {
        let fonts = Fonts::new(Family::Embedded, None).unwrap();
        let spec =
            FontSpec { size_px: 12.0, scale_factor: 1.0, line_height: 1.0, letter_spacing_px: 0.0 };
        let metrics = fonts.cell_metrics(&spec).unwrap();
        let width = u32::from(cols) * metrics.width + 2 * PAD;
        let height = u32::from(rows) * metrics.height + 2 * PAD;
        let grid = Grid {
            pane: Rect::new(0, 0, width, height),
            x: PAD,
            y: PAD,
            cols,
            rows,
            metrics,
            background: BG,
        };
        let pixels = vec![0xDEAD; usize::try_from(width * height).unwrap()];
        Self {
            glyphs: Glyphs::new(fonts, 3 << 20),
            painter: Painter::new(),
            pixels,
            width,
            height,
            grid,
        }
    }

    fn paint(&mut self, rows: &[Row<'_>], overlay: Overlay, buffer_age: u8) -> Vec<Rect> {
        let mut target = Target::new(&mut self.pixels, self.width, self.height).unwrap();
        let frame = Frame { grid: self.grid, rows, overlay, buffer_age };
        self.painter.paint(&mut self.glyphs, &mut target, &frame).to_vec()
    }

    fn pixel(&self, x: u32, y: u32) -> Rgb {
        Rgb::from_pixel(self.pixels[(y * self.width + x) as usize])
    }

    fn cell_pixels(&self, col: u16, row: u16) -> Vec<Rgb> {
        let cell = self.grid.cell_rect(col, row);
        (cell.y..cell.bottom())
            .flat_map(|y| (cell.x..cell.right()).map(move |x| (x, y)))
            .map(|(x, y)| self.pixel(x, y))
            .collect()
    }
}

fn text(line: &str) -> Vec<Cell> {
    line.chars().map(|ch| Cell { text: CellText::Char(ch), ..Cell::blank(FG, BG) }).collect()
}

#[test]
fn the_first_frame_touches_every_pixel_and_damages_the_pane() {
    let mut screen = Screen::new(6, 2);
    let line = text("hello");
    let damage = screen.paint(&[Row { cells: &line, clusters: "" }], Overlay::default(), 0);
    assert_eq!(damage, [screen.grid.pane]);
    assert!(
        screen.pixels.iter().all(|&pixel| pixel >> 24 == 0 && pixel != 0xDEAD),
        "no stale pixel"
    );
    assert_eq!(screen.pixel(0, 0), BG, "padding");
    assert!(screen.cell_pixels(0, 0).iter().any(|&pixel| pixel != BG), "the h is drawn");
    assert!(screen.cell_pixels(5, 0).iter().all(|&pixel| pixel == BG), "past the row's end");
}

#[test]
fn an_unchanged_frame_paints_nothing_and_a_changed_row_only_that_row() {
    let mut screen = Screen::new(6, 3);
    let (a, b) = (text("aaa"), text("bbb"));
    let rows = [Row { cells: &a, clusters: "" }, Row { cells: &b, clusters: "" }];
    let _first = screen.paint(&rows, Overlay::default(), 0);
    let before = screen.pixels.clone();
    assert_eq!(screen.paint(&rows, Overlay::default(), 1), []);
    assert_eq!(screen.pixels, before);
    let c = text("bcb");
    let changed = [rows[0], Row { cells: &c, clusters: "" }];
    assert_eq!(screen.paint(&changed, Overlay::default(), 1), [screen.grid.row_rect(1)]);
    assert_ne!(screen.pixels, before);
}

#[test]
fn an_older_buffer_also_repaints_what_changed_in_the_frames_it_missed() {
    let mut screen = Screen::new(4, 2);
    let (a, b) = (text("a"), text("b"));
    let _f1 = screen.paint(
        &[Row { cells: &a, clusters: "" }, Row { cells: &a, clusters: "" }],
        Overlay::default(),
        0,
    );
    let _f2 = screen.paint(
        &[Row { cells: &b, clusters: "" }, Row { cells: &a, clusters: "" }],
        Overlay::default(),
        1,
    );
    // A double-buffered backend hands back the buffer of frame 1: row 0 changed since then.
    let damage = screen.paint(
        &[Row { cells: &b, clusters: "" }, Row { cells: &a, clusters: "" }],
        Overlay::default(),
        2,
    );
    assert_eq!(damage, [screen.grid.row_rect(0)]);
    // Undefined contents or an age past the history repaint everything.
    let rows = [Row { cells: &b, clusters: "" }, Row { cells: &a, clusters: "" }];
    assert_eq!(screen.paint(&rows, Overlay::default(), 0), [screen.grid.pane]);
    assert_eq!(screen.paint(&rows, Overlay::default(), MAX_BUFFER_AGE + 1), [screen.grid.pane]);
}

#[test]
fn adjacent_dirty_rows_merge_into_one_rectangle() {
    let mut screen = Screen::new(4, 3);
    let (a, b) = (text("a"), text("b"));
    let _first = screen.paint(&[Row { cells: &a, clusters: "" }; 3], Overlay::default(), 0);
    let damage = screen.paint(&[Row { cells: &b, clusters: "" }; 3], Overlay::default(), 1);
    let area = screen.grid.area();
    assert_eq!(damage, [area]);
}

#[test]
fn cursor_shapes() {
    let mut screen = Screen::new(4, 1);
    let line = text("ab");
    let rows = [Row { cells: &line, clusters: "" }];
    let red = Rgb::new(255, 0, 0);
    let cursor = |shape| Overlay {
        cursor: Some(Cursor { col: 0, row: 0, shape, color: red, text_color: BG }),
        selection: None,
    };
    let _block = screen.paint(&rows, cursor(CursorShape::Block), 0);
    let cell = screen.cell_pixels(0, 0);
    assert!(
        cell.contains(&red) && cell.iter().all(|&pixel| pixel != FG),
        "block: cursor color, text in the text color"
    );
    let damage = screen.paint(&rows, cursor(CursorShape::Bar), 1);
    assert_eq!(damage, [screen.grid.row_rect(0)], "the cursor is part of the row hash");
    let stroke = screen.grid.metrics.stroke;
    let bar = screen.grid.cell_rect(0, 0);
    assert!((bar.y..bar.bottom()).all(|y| screen.pixel(bar.x, y) == red));
    assert_ne!(screen.pixel(bar.x + stroke, bar.y), red);
    let _hollow = screen.paint(&rows, cursor(CursorShape::Hollow), 1);
    assert_eq!(screen.pixel(bar.right() - 1, bar.bottom() - 1), red);
    assert_ne!(
        screen.pixel(bar.x + bar.width / 2, bar.y + bar.height / 2 - 1),
        red,
        "hollow inside"
    );
    let _underline = screen.paint(&rows, cursor(CursorShape::Underline), 1);
    assert_eq!(screen.pixel(bar.x + 1, bar.bottom() - 1), red);
    assert_ne!(screen.pixel(bar.x + 1, bar.y), red);
}

#[test]
fn selected_cells_take_the_selection_colors() {
    let mut screen = Screen::new(5, 1);
    let line = text("abcde");
    let blue = Rgb::new(0, 0, 200);
    let selection =
        Selection { start: (1, 0), end: (2, 0), kind: SelectionKind::Linear, bg: blue, fg: None };
    let _frame = screen.paint(
        &[Row { cells: &line, clusters: "" }],
        Overlay { cursor: None, selection: Some(selection) },
        0,
    );
    let cell = screen.grid.cell_rect(1, 0);
    assert_eq!(screen.pixel(cell.x, cell.y), blue);
    assert_eq!(screen.pixel(screen.grid.cell_rect(2, 0).x, cell.y), blue);
    assert_eq!(screen.pixel(screen.grid.cell_rect(3, 0).x, cell.y), BG);
}

#[test]
fn decorations_styles_and_clusters_are_drawn() {
    let mut screen = Screen::new(8, 1);
    let mut cells = text("ab");
    cells[0].underline = Underline::Curly;
    cells[1].strokes = Strokes::STRIKEOUT;
    cells[1].style = TextStyle { weight: BOLD_WEIGHT, slant: Slant::Italic };
    cells.push(Cell { text: CellText::Cluster { start: 0, len: 3 }, ..Cell::blank(FG, BG) });
    cells.push(Cell { text: CellText::Char('─'), ..Cell::blank(FG, BG) });
    let _frame =
        screen.paint(&[Row { cells: &cells, clusters: "e\u{301}" }], Overlay::default(), 0);
    let metrics = screen.grid.metrics;
    let first = screen.grid.cell_rect(0, 0);
    let underline_row = first.y + metrics.underline_top;
    assert!(
        (first.x..first.right())
            .any(|x| screen.pixel(x, underline_row + 1) != BG
                || screen.pixel(x, underline_row) != BG)
    );
    let cluster = screen.cell_pixels(2, 0);
    assert!(cluster.iter().filter(|&&pixel| pixel != BG).count() > 4, "e with its accent");
    let line = screen.grid.cell_rect(3, 0);
    let middle = line.y + (metrics.height - metrics.stroke) / 2;
    assert!((line.x..line.right()).all(|x| screen.pixel(x, middle) == FG), "─ spans the cell");
}

#[test]
fn a_wide_cell_draws_once_over_two_columns() {
    let mut screen = Screen::new(4, 1);
    let cells = [
        Cell { text: CellText::Char('█'), width: CellWidth::Wide, ..Cell::blank(FG, BG) },
        Cell { text: CellText::Char('█'), width: CellWidth::Spacer, ..Cell::blank(FG, BG) },
    ];
    let _frame = screen.paint(&[Row { cells: &cells, clusters: "" }], Overlay::default(), 0);
    assert!(screen.cell_pixels(0, 0).iter().all(|&pixel| pixel == FG));
    assert!(
        screen.cell_pixels(1, 0).iter().all(|&pixel| pixel == BG),
        "the spacer only has its background"
    );
}

#[test]
fn a_grid_larger_than_its_pane_is_clipped() {
    let mut screen = Screen::new(6, 2);
    screen.grid.pane = Rect::new(0, 0, screen.width / 2, screen.height / 2);
    let line = text("██████");
    let _frame = screen.paint(&[Row { cells: &line, clusters: "" }; 2], Overlay::default(), 0);
    assert_eq!(screen.pixel(screen.width - 1, screen.height - 1).pixel(), 0xDEAD);
    assert_eq!(screen.pixel(screen.width / 2 - 1, screen.height / 2 - 1), FG);
}
