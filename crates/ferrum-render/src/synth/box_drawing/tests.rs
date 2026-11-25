use super::*;

/// Cell sizes of `JetBrains Mono` at 9, 12, 13, 18, 26 px (odd and even sides, thin and thick).
const CELLS: [(u32, u32, u32); 5] = [(5, 12, 1), (7, 16, 1), (8, 17, 1), (11, 24, 2), (16, 34, 3)];

fn canvas(ch: char, (width, height, light): (u32, u32, u32)) -> Canvas {
    draw(ch, width, height, light).unwrap()
}

fn column(canvas: &Canvas, x: u32) -> Vec<u8> {
    (0..canvas.height).map(|y| canvas.pixels[(y * canvas.width + x) as usize]).collect()
}

fn row(canvas: &Canvas, y: u32) -> Vec<u8> {
    let start = (y * canvas.width) as usize;
    canvas.pixels[start..start + canvas.width as usize].to_vec()
}

/// The straight line whose edge pixels an arm of `weight` must match.
fn straight(weight: u8, horizontal: bool) -> char {
    match (weight, horizontal) {
        (b'l', true) => '─',
        (b'h', true) => '━',
        (b'd', true) => '═',
        (b'l', false) => '│',
        (b'h', false) => '┃',
        _ => '║',
    }
}

#[test]
fn every_arm_meets_the_neighbor_cell_on_the_same_pixels() {
    for cell in CELLS {
        let (width, height, _) = cell;
        for (offset, arms) in ARMS.chunks(4).enumerate() {
            let ch = char::from_u32(0x2500 + u32::try_from(offset).unwrap()).unwrap();
            let drawn = canvas(ch, cell);
            let [up, right, down, left] = [arms[0], arms[1], arms[2], arms[3]];
            let edge = |weight: u8, horizontal: bool, at_end: bool| {
                let line = canvas(straight(weight, horizontal), cell);
                match (horizontal, at_end) {
                    (true, false) => (column(&drawn, 0), column(&line, 0)),
                    (true, true) => (column(&drawn, width - 1), column(&line, width - 1)),
                    (false, false) => (row(&drawn, 0), row(&line, 0)),
                    (false, true) => (row(&drawn, height - 1), row(&line, height - 1)),
                }
            };
            for (weight, horizontal, at_end) in
                [(left, true, false), (right, true, true), (up, false, false), (down, false, true)]
            {
                if weight != b'.' {
                    let (got, want) = edge(weight, horizontal, at_end);
                    assert_eq!(got, want, "{ch} {cell:?} horizontal={horizontal} end={at_end}");
                }
            }
        }
    }
}

#[test]
fn arcs_continue_the_straight_lines() {
    for cell in CELLS {
        let (width, height, _) = cell;
        let (horizontal, vertical) = (canvas('─', cell), canvas('│', cell));
        let down_right = canvas('╭', cell);
        assert_eq!(column(&down_right, width - 1), column(&horizontal, width - 1), "{cell:?}");
        assert_eq!(row(&down_right, height - 1), row(&vertical, height - 1), "{cell:?}");
        let up_left = canvas('╯', cell);
        assert_eq!(column(&up_left, 0), column(&horizontal, 0), "{cell:?}");
        assert_eq!(row(&up_left, 0), row(&vertical, 0), "{cell:?}");
    }
}

#[test]
fn weights_set_the_thickness() {
    let cell = (8, 17, 1);
    let lit = |ch| column(&canvas(ch, cell), 0).into_iter().filter(|&p| p == u8::MAX).count();
    assert_eq!(lit('─'), 1);
    assert_eq!(lit('━'), 2);
    assert_eq!(lit('═'), 2, "two light lines");
    let double = column(&canvas('═', cell), 0);
    let gap = double.windows(3).any(|w| w == [255, 0, 255]);
    assert!(gap, "one light line apart: {double:?}");
}

#[test]
fn double_corners_leave_the_inside_open() {
    let cell = (11, 24, 2);
    let corner = canvas('╔', cell);
    // The inner corner: right of the outer vertical line and below the outer horizontal line,
    // above and left of the inner lines, is empty.
    let [outer, inner] =
        Axis { len: 11, cross: 24, arms: [Weight::None; 2], sides: [Weight::None; 2], light: 2 }
            .doubles(11);
    let [outer_y, inner_y] =
        Axis { len: 24, cross: 11, arms: [Weight::None; 2], sides: [Weight::None; 2], light: 2 }
            .doubles(24);
    let at = |x: u32, y: u32| corner.pixels[(y * 11 + x) as usize];
    assert_eq!(at(outer.0, outer_y.0), 255, "the outer corner is closed");
    assert_eq!(at(outer.1, outer_y.1), 0, "between the lines");
    assert_eq!(at(inner.0, inner_y.0), 255, "the inner corner is closed");
    assert_eq!(at(0, 0), 0);
}

#[test]
fn dashes_and_diagonals_draw_inside_the_cell() {
    for ch in ['┄', '┅', '┆', '┇', '┈', '┉', '┊', '┋', '╌', '╍', '╎', '╏', '╱', '╲', '╳']
    {
        let drawn = canvas(ch, (8, 17, 1));
        let lit = drawn.pixels.iter().filter(|&&p| p > 0).count();
        assert!(lit > 0 && lit < drawn.pixels.len(), "{ch}");
    }
    let dashes = column(&canvas('┆', (8, 17, 1)), 3);
    let runs = dashes.windows(2).filter(|w| w[0] == 0 && w[1] == 255).count()
        + usize::from(dashes[0] == 255);
    assert_eq!(runs, 3, "three dashes: {dashes:?}");
}
