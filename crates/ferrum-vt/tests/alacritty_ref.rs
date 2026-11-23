//! The reference tests imported from `alacritty_terminal` (Apache-2.0, `docs/08` §4 and §6;
//! attribution in `fixtures/vt/THIRD_PARTY.md`). Each test feeds a recording (the bytes a real
//! program wrote) to a terminal of the recorded size and compares the history and the screen
//! with the grid Alacritty produced, in the compact text form written by
//! `cargo xtask import-alacritty-refs`.
//!
//! Every upstream test is imported (maintainer decision Q-25); `fixtures/vt/THIRD_PARTY.md` gives
//! the count.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use ferrum_scrollback::{LineCursor, LineFlags, Scrollback, ScrollbackConfig, StyleId};
use ferrum_vt::{
    Attrs, Cell, CellFlags, CellStyle, Color, ColorKind, NullSink, Size, Terminal, Underline,
    cluster_width, clusters,
};

/// Smallest spill threshold the scrollback accepts.
const MIN_SPILL_THRESHOLD: usize = 1_024;
/// Bytes per read when the recording is also fed in pieces.
const PIECE: usize = 7;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vt/alacritty")
}

fn color_text(color: Color) -> Option<String> {
    match color.kind() {
        ColorKind::Indexed(index) => Some(format!("i{index}")),
        ColorKind::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        _ => None,
    }
}

/// The style words of one cluster, in the order the converter writes them.
fn style_text(term: &Terminal<NullSink>, style: &CellStyle) -> String {
    let mut parts = Vec::new();
    parts.extend(color_text(style.fg).map(|c| format!("fg={c}")));
    parts.extend(color_text(style.bg).map(|c| format!("bg={c}")));
    for (attr, name) in [
        (Attrs::BOLD, "bold"),
        (Attrs::DIM, "dim"),
        (Attrs::ITALIC, "italic"),
        (Attrs::INVERSE, "inverse"),
        (Attrs::HIDDEN, "hidden"),
        (Attrs::STRIKE, "strike"),
    ] {
        if style.attrs.contains(attr) {
            parts.push(name.to_owned());
        }
    }
    let underline = match style.underline {
        Underline::Single => Some("single"),
        Underline::Double => Some("double"),
        Underline::Curly => Some("curly"),
        Underline::Dotted => Some("dotted"),
        Underline::Dashed => Some("dashed"),
        _ => None,
    };
    parts.extend(underline.map(|name| format!("underline={name}")));
    let extras = term.extras();
    parts.extend(color_text(extras.underline_color(style.extra)).map(|c| format!("ul={c}")));
    parts.extend(extras.uri(extras.link(style.extra)).map(|uri| format!("link={uri}")));
    parts.join(" ")
}

struct Row {
    clusters: Vec<(String, String)>,
    wrapped: bool,
}

fn screen_rows(term: &Terminal<NullSink>) -> Vec<Row> {
    let grid = term.grid();
    (0..grid.lines())
        .filter_map(|y| grid.row(y))
        .map(|row| {
            let clusters = row
                .cells()
                .iter()
                .filter(|c| !c.flags().intersects(CellFlags::SPACER | CellFlags::LEADING_SPACER))
                .map(|cell| {
                    let mut buffer = [0; 4];
                    let text = grid.cell_text(cell, &mut buffer).to_owned();
                    (text, style_text(term, &cell_style(cell)))
                })
                .collect();
            Row { clusters, wrapped: row.is_wrapped() }
        })
        .collect()
}

fn cell_style(cell: &Cell) -> CellStyle {
    CellStyle::from_style(&ferrum_scrollback::Style {
        fg: cell.fg_word(),
        bg: cell.bg_word(),
        attrs: cell.attr_word(),
    })
}

fn history_rows(term: &Terminal<NullSink>) -> Vec<Row> {
    let history = term.scrollback();
    let mut cursor = LineCursor::new();
    let mut rows = Vec::new();
    for index in 0..history.len() {
        let Ok(Some(line)) = history.read_line(index, &mut cursor) else { continue };
        let mut styles: Vec<(StyleId, u32)> = line.runs.iter().map(|r| (r.style, r.len)).collect();
        styles.reverse();
        let mut found = Vec::new();
        for piece in clusters(&line.text) {
            if cluster_width(piece) == 0 {
                if let Some((text, _)) = found.last_mut() {
                    let text: &mut String = text;
                    text.push_str(piece);
                }
                continue;
            }
            let scalars = u32::try_from(piece.chars().count()).unwrap_or(1);
            let style =
                styles.last().and_then(|(id, _)| history.styles().get(*id)).unwrap_or_default();
            let mut left = scalars;
            while left > 0 {
                match styles.last_mut() {
                    Some((_, len)) if *len > left => {
                        *len -= left;
                        left = 0;
                    }
                    Some((_, len)) => {
                        left -= *len;
                        styles.pop();
                    }
                    None => left = 0,
                }
            }
            found.push((piece.to_owned(), style_text(term, &CellStyle::from_style(&style))));
        }
        rows.push(Row { clusters: found, wrapped: line.flags.contains(LineFlags::WRAPPED) });
    }
    rows
}

/// The expected-file form of the history and the screen.
fn render(term: &Terminal<NullSink>) -> String {
    let mut rows = history_rows(term);
    rows.extend(screen_rows(term));
    let mut lines: Vec<String> = Vec::new();
    for mut row in rows {
        if !row.wrapped {
            while row.clusters.last().is_some_and(|(text, style)| text == " " && style.is_empty()) {
                row.clusters.pop();
            }
        }
        let text: String = row.clusters.iter().map(|(text, _)| text.as_str()).collect();
        let mut out = format!("row |{text}|{}\n", if row.wrapped { "W" } else { "" });
        let mut runs: Vec<(usize, usize, &str)> = Vec::new();
        for (index, (_, style)) in
            row.clusters.iter().enumerate().filter(|(_, (_, s))| !s.is_empty())
        {
            match runs.last_mut() {
                Some((_, end, last)) if *end + 1 == index && *last == style.as_str() => {
                    *end = index;
                }
                _ => runs.push((index, index, style)),
            }
        }
        for (from, to, style) in runs {
            let _ = writeln!(out, "  {from}-{to} {style}");
        }
        lines.push(out);
    }
    let first_used = lines.iter().position(|row| row != "row ||\n").unwrap_or(lines.len());
    lines.split_off(first_used).concat()
}

fn terminal_for(header: &str) -> Terminal<NullSink> {
    let mut words = header.split_whitespace();
    let size = words.nth(1).expect("size").split_once('x').expect("COLSxLINES");
    let history: usize = words.nth(1).expect("history").parse().expect("history lines");
    let config = ScrollbackConfig::new(history, history.max(MIN_SPILL_THRESHOLD))
        .expect("scrollback config");
    let size = Size::new(size.0.parse().expect("columns"), size.1.parse().expect("lines"));
    Terminal::new(size, Scrollback::new(config), NullSink)
}

fn run(name: &str) {
    let expected =
        fs::read_to_string(root().join(format!("{name}.expected"))).expect("expected file");
    let recording = fs::read(root().join(format!("{name}.recording"))).expect("recording");
    let body = expected.split_once('\n').map(|(_, rest)| rest).unwrap_or_default();
    let (header, rows) = body.split_once('\n').expect("header and rows");
    for pieces in [None, Some(PIECE)] {
        let mut term = terminal_for(header);
        match pieces {
            None => term.parse(&recording),
            Some(size) => recording.chunks(size).for_each(|chunk| term.parse(chunk)),
        }
        let got = render(&term);
        let first = got.lines().zip(rows.lines()).position(|(a, b)| a != b).unwrap_or(0);
        let show = |text: &str| text.lines().skip(first).take(3).collect::<Vec<_>>().join("\n");
        let how = pieces.map_or("whole".to_owned(), |n| format!("{n}-byte reads"));
        assert!(
            got == rows,
            "{name} ({how}) differs from the Alacritty grid at line {first}:\nexpected:\n{}\ngot:\n{}",
            show(rows),
            show(&got)
        );
    }
}

macro_rules! ref_tests {
    ($($name:ident)*) => {
        /// The imported tests, by name.
        const IMPORTED: &[&str] = &[$(stringify!($name)),*];

        $(
            #[test]
            fn $name() {
                run(stringify!($name));
            }
        )*
    };
}

ref_tests! {
    alt_reset
    clear_underline
    colored_reset
    colored_underline
    csi_rep
    decaln_reset
    deccolm_reset
    delete_chars_reset
    delete_lines
    erase_chars_reset
    erase_in_line
    fish_cc
    grid_reset
    history
    hyperlinks
    indexed_256_colors
    insert_blank_reset
    issue_855
    ll
    newline_with_cursor_beyond_scroll_region
    origin_goto
    region_scroll_down
    row_reset
    saved_cursor
    saved_cursor_alt
    scroll_in_region_up_preserves_history
    scroll_up_reset
    selective_erasure
    sgr
    tab_rendering
    tmux_git_log
    tmux_htop
    underline
    vim_24bitcolors_bce
    vim_large_window_scroll
    vim_simple_edit
    vttest_cursor_movement_1
    vttest_insert
    vttest_origin_mode_1
    vttest_origin_mode_2
    vttest_scroll
    vttest_tab_clear_set
    wrapline_alt_toggle
    zerowidth
    zsh_tab_completion
}

#[test]
fn the_imported_set_matches_the_files_and_the_notes() {
    let mut on_disk: Vec<String> = fs::read_dir(root())
        .expect("fixtures/vt/alacritty")
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".expected").map(str::to_owned))
        .collect();
    on_disk.sort();
    let mut imported: Vec<&str> = IMPORTED.to_vec();
    imported.sort_unstable();
    assert_eq!(on_disk, imported);
    let notes = fs::read_to_string(root().join("../THIRD_PARTY.md")).expect("THIRD_PARTY.md");
    assert!(notes.contains(&format!("{} imported", IMPORTED.len())), "the imported count is stale");
}
