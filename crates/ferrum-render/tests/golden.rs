//! Golden images (`docs/08` §2: "`render` small PNG comparisons with tolerance"): small scenes
//! drawn with the embedded font (deterministic on every OS), stored as `insta` binary snapshots
//! in `fixtures/render/`. A scene matches when every channel of every pixel is within
//! `TOLERANCE` of the stored image; a scene that does not is handed to `insta`, which fails and
//! writes the new image for review (`cargo insta review`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a failed expectation is a failed test"
)]

use std::path::PathBuf;

use ferrum_render::chrome::{Border, RoundedRect, Shadow, Tile};
use ferrum_render::font::{BOLD_WEIGHT, Family, FontSpec, Fonts, Slant, TextStyle};
use ferrum_render::{
    Antialias, Cell, CellText, CellWidth, Cursor, CursorShape, Frame, Glyphs, Grid, Overlay,
    Painter, Rect, Rgb, Row, Selection, SelectionKind, Strokes, Target, Underline,
};

/// Largest difference allowed per channel. Rasterization is integer and IEEE `f32` arithmetic,
/// identical on every OS, except the few `libm` calls (`sin`, `tan`, `powi`) of the curly
/// underline, the arcs and the synthetic slant, whose last-bit differences between platform
/// math libraries move a coverage value by at most one step.
const TOLERANCE: u8 = 2;

const BG: Rgb = Rgb::new(0x10, 0x12, 0x14);
const FG: Rgb = Rgb::new(0xD7, 0xDA, 0xE0);
const ACCENT: Rgb = Rgb::new(0xE8, 0x59, 0x0C);
const BLUE: Rgb = Rgb::new(0x61, 0xAF, 0xEF);
const SELECTION: Rgb = Rgb::new(0x2C, 0x31, 0x3A);
const PADDING: u32 = 4;

struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

/// How a scene is drawn: the `HiDPI` scale and the text antialiasing.
#[derive(Clone, Copy)]
struct Look {
    scale: f32,
    antialias: Antialias,
}

const ONE_X: Look = Look { scale: 1.0, antialias: Antialias::Grayscale };
const TWO_X: Look = Look { scale: 2.0, antialias: Antialias::Grayscale };
const LCD: Look = Look { scale: 1.0, antialias: Antialias::Lcd };

/// Paints `rows` on a grid of `cols` columns with the embedded font at 13 px (the default font
/// size, Q-36) × the look's scale.
fn paint(rows: &[Row<'_>], cols: u16, overlay: Overlay, look: Look) -> Image {
    let fonts = Fonts::new(Family::Embedded, None).unwrap();
    let spec = FontSpec {
        size_px: 13.0,
        scale_factor: look.scale,
        line_height: 1.0,
        letter_spacing_px: 0.0,
    };
    let metrics = fonts.cell_metrics(&spec).unwrap();
    let rows_count = u16::try_from(rows.len()).unwrap();
    let width = u32::from(cols) * metrics.width + 2 * PADDING;
    let height = u32::from(rows_count) * metrics.height + 2 * PADDING;
    let grid = Grid {
        pane: Rect::new(0, 0, width, height),
        x: PADDING,
        y: PADDING,
        cols,
        rows: rows_count,
        metrics,
        background: BG,
    };
    let mut pixels = vec![0; usize::try_from(width * height).unwrap()];
    let mut target = Target::new(&mut pixels, width, height).unwrap();
    let mut glyphs = Glyphs::new(fonts, 3 << 20);
    glyphs.set_antialias(look.antialias);
    let frame = Frame { grid, rows, overlay, buffer_age: 0 };
    let _damage = Painter::new().paint(&mut glyphs, &mut target, &frame);
    Image { width, height, pixels }
}

fn encode(image: &Image) -> Vec<u8> {
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, image.width, image.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let rgb: Vec<u8> =
        image.pixels.iter().flat_map(|pixel| pixel.to_be_bytes()[1..].to_vec()).collect();
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&rgb).unwrap();
    writer.finish().unwrap();
    png
}

fn decode(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().ok()?;
    let mut rgb = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut rgb).ok()?;
    (info.color_type == png::ColorType::Rgb).then(|| {
        (info.width, info.height, rgb.get(..info.buffer_size()).unwrap_or_default().to_vec())
    })
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/render")
}

fn check(name: &str, image: &Image) {
    let png = encode(image);
    let stored = std::fs::read(fixtures().join(format!("{name}.snap.png"))).ok();
    let close = stored.as_deref().and_then(decode).is_some_and(|(width, height, rgb)| {
        let ours = decode(&png).expect("our own image decodes").2;
        (width, height) == (image.width, image.height)
            && rgb.iter().zip(&ours).all(|(&a, &b)| a.abs_diff(b) <= TOLERANCE)
    });
    if close {
        return;
    }
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(fixtures());
    settings.set_prepend_module_to_snapshot(false);
    settings.set_omit_expression(true);
    settings.bind(|| {
        insta::assert_binary_snapshot!(&format!("{name}.png"), png);
    });
}

fn text(line: &str, fg: Rgb, style: TextStyle) -> Vec<Cell> {
    line.chars().map(|ch| Cell { text: CellText::Char(ch), style, ..Cell::blank(fg, BG) }).collect()
}

fn styled_rows() -> Vec<Vec<Cell>> {
    let bold = TextStyle { weight: BOLD_WEIGHT, slant: Slant::Upright };
    let italic = TextStyle { slant: Slant::Italic, ..TextStyle::REGULAR };
    let bold_italic = TextStyle { weight: BOLD_WEIGHT, slant: Slant::Italic };
    vec![
        text("$ cargo run -- --help  0Oo1lI|", FG, TextStyle::REGULAR),
        text("Bold: fn main() { println!(); }", ACCENT, bold),
        text("Italic: // Grüße, café, Ærø ½", BLUE, italic),
        text("Bold italic: <=> != -> www", FG, bold_italic),
        underlined_row(),
        struck_row(),
        wide_row(),
    ]
}

/// One word per underline style.
fn underlined_row() -> Vec<Cell> {
    let mut row = text("single double curly dotted dash", FG, TextStyle::REGULAR);
    let styles = [
        Underline::Single,
        Underline::Double,
        Underline::Curly,
        Underline::Dotted,
        Underline::Dashed,
    ];
    let mut style = styles.iter().copied().cycle();
    let mut current = style.next().unwrap();
    for cell in &mut row {
        if cell.text == CellText::Char(' ') {
            current = style.next().unwrap();
        }
        cell.underline = current;
        cell.underline_color = ACCENT;
    }
    row
}

/// Strikeout, overline, both, and a combining-mark cluster.
fn struck_row() -> Vec<Cell> {
    let mut row = text("strikeout overline both e\u{301}", FG, TextStyle::REGULAR);
    for (i, cell) in row.iter_mut().enumerate() {
        cell.strokes = match i {
            0..=9 => Strokes::STRIKEOUT,
            10..=18 => Strokes::OVERLINE,
            _ => Strokes::STRIKEOUT.with(Strokes::OVERLINE),
        };
    }
    let last = row.len() - 2;
    row.truncate(last);
    row.push(Cell { text: CellText::Cluster { start: 0, len: 3 }, ..Cell::blank(FG, BG) });
    row
}

/// A wide cell and its spacer.
fn wide_row() -> Vec<Cell> {
    let mut row = text("wide:", FG, TextStyle::REGULAR);
    let wide =
        Cell { text: CellText::Char('█'), width: CellWidth::Wide, ..Cell::blank(ACCENT, BG) };
    row.push(wide);
    row.push(Cell { width: CellWidth::Spacer, ..Cell::blank(ACCENT, BG) });
    row
}

fn text_scene() -> (Vec<Vec<Cell>>, Overlay) {
    let selection = Selection {
        start: (2, 1),
        end: (9, 2),
        kind: SelectionKind::Linear,
        bg: SELECTION,
        fg: None,
    };
    let cursor = Cursor { col: 2, row: 0, shape: CursorShape::Block, color: FG, text_color: BG };
    (styled_rows(), Overlay { cursor: Some(cursor), selection: Some(selection) })
}

fn cluster_rows(lines: &[Vec<Cell>]) -> Vec<Row<'_>> {
    lines.iter().map(|cells| Row { cells, clusters: "e\u{301}" }).collect()
}

#[test]
fn text_styles_decorations_cursor_and_selection() {
    let (lines, overlay) = text_scene();
    check("text_styles", &paint(&cluster_rows(&lines), 32, overlay, ONE_X));
}

#[test]
fn text_styles_at_2x() {
    let (lines, overlay) = text_scene();
    check("text_styles_2x", &paint(&cluster_rows(&lines), 32, overlay, TWO_X));
}

#[test]
fn lcd_text_on_an_opaque_background() {
    let (lines, overlay) = text_scene();
    check("text_styles_lcd", &paint(&cluster_rows(&lines), 32, overlay, LCD));
}

#[test]
fn cursor_shapes() {
    let line = text("block bar under hollow", FG, TextStyle::REGULAR);
    let rows = [Row { cells: &line, clusters: "" }];
    let shapes = [
        ("block", CursorShape::Block, 0),
        ("bar", CursorShape::Bar, 6),
        ("underline", CursorShape::Underline, 10),
        ("hollow", CursorShape::Hollow, 16),
    ];
    for (name, shape, col) in shapes {
        let cursor = Cursor { col, row: 0, shape, color: ACCENT, text_color: BG };
        let overlay = Overlay { cursor: Some(cursor), selection: None };
        check(&format!("cursor_{name}"), &paint(&rows, 22, overlay, ONE_X));
    }
}

fn box_rows() -> Vec<Vec<Cell>> {
    let chars: Vec<char> =
        (0x2500..=0x259F).chain(0xE0B0..=0xE0B3).filter_map(char::from_u32).collect();
    chars
        .chunks(32)
        .map(|chunk| {
            chunk
                .iter()
                .map(|&ch| Cell { text: CellText::Char(ch), ..Cell::blank(FG, BG) })
                .collect()
        })
        .collect()
}

#[test]
fn box_drawing_blocks_and_powerline() {
    let lines = box_rows();
    let rows: Vec<Row<'_>> = lines.iter().map(|cells| Row { cells, clusters: "" }).collect();
    check("box_drawing", &paint(&rows, 32, Overlay::default(), ONE_X));
}

#[test]
fn box_drawing_at_2x() {
    let lines = box_rows();
    let rows: Vec<Row<'_>> = lines.iter().map(|cells| Row { cells, clusters: "" }).collect();
    check("box_drawing_2x", &paint(&rows, 32, Overlay::default(), TWO_X));
}

#[test]
fn a_box_drawn_frame_is_seamless() {
    let frame =
        ["╭──┬──╮", "│  │  │", "├──┼──┤", "╞══╪══╡", "╰──┴──╯", "┏━┳━┓ ", "┗━┻━┛ ", "▀▄█▌▐░"];
    let lines: Vec<Vec<Cell>> =
        frame.iter().map(|line| text(line, FG, TextStyle::REGULAR)).collect();
    let rows: Vec<Row<'_>> = lines.iter().map(|cells| Row { cells, clusters: "" }).collect();
    check("box_frame", &paint(&rows, 7, Overlay::default(), ONE_X));
}

#[test]
#[cfg_attr(miri, ignore = "tiny-skia compares function pointers, which Miri makes unequal")]
fn chrome_tiles() {
    let (width, height) = (160, 72);
    let mut pixels = vec![BG.pixel(); usize::try_from(width * height).unwrap()];
    let mut target = Target::new(&mut pixels, width, height).unwrap();
    let shadow = Shadow { radius: 6, blur: 8, color: Rgb::BLACK, alpha: 160 };
    let card = Rect::new(16, 12, 128, 40);
    shadow.draw(&Tile::shadow(&shadow).unwrap(), &mut target, card, (0, 4));
    let border = Border { color: SELECTION, width: 1.0 };
    let style = RoundedRect { radius: 6.0, fill: Rgb::new(0x1A, 0x1D, 0x21), border: Some(border) };
    Tile::rounded_rect(&style).unwrap().draw(&mut target, card);
    let button = RoundedRect { radius: 6.0, fill: ACCENT, border: None };
    Tile::rounded_rect(&button).unwrap().draw(&mut target, Rect::new(28, 22, 64, 20));
    check("chrome_tiles", &Image { width, height, pixels });
}
