//! The reference fixture as the byte stream a program writes to its terminal (`docs/03` §1:
//! what `memcheck` feeds the sessions): every logical line's rows joined (the terminal soft-wraps
//! them again at the same width), each style run opened with an SGR sequence, the line closed
//! with `SGR 0` and `\r\n`.
//!
//! The fixture's colors are fixture-local values (`FG_TAG | index`, `BG_TAG | index`); here they
//! become the 8 standard colors (`SGR 30..37`, `40..47`), and the attribute variants 1 and 2 bold
//! and italic, so the terminal interns as many distinct styles as the fixture has.

#![allow(dead_code, reason = "each includer uses a different subset of the fixture API")]

use std::io::{self, Write};

use super::Row;

const FG_TAG: u32 = 0x0100_0000;
const BG_TAG: u32 = 0x0200_0000;
const COLOR_MASK: u32 = 0x00FF_FFFF;
/// SGR bases of the standard foreground and background colors.
const SGR_FG: u32 = 30;
const SGR_BG: u32 = 40;

fn sgr(style: &ferrum_scrollback::Style, out: &mut impl Write) -> io::Result<()> {
    out.write_all(b"\x1b[0")?;
    match style.attrs {
        1 => out.write_all(b";1")?,
        2 => out.write_all(b";3")?,
        _ => {}
    }
    if style.fg & FG_TAG != 0 {
        write!(out, ";{}", SGR_FG + (style.fg & COLOR_MASK))?;
    }
    if style.bg & BG_TAG != 0 {
        write!(out, ";{}", SGR_BG + (style.bg & COLOR_MASK))?;
    }
    out.write_all(b"m")
}

/// Writes `rows` (as `Fixture::rows` made them) to `out`.
pub(crate) fn write_rows(rows: &[Row], out: &mut impl Write) -> io::Result<()> {
    for row in rows {
        let mut chars = row.text.chars();
        for run in &row.runs {
            sgr(&run.style, out)?;
            let text: String = chars.by_ref().take(run.len).collect();
            out.write_all(text.as_bytes())?;
        }
        out.write_all(chars.as_str().as_bytes())?;
        if !row.flags.contains(ferrum_scrollback::LineFlags::WRAPPED) {
            out.write_all(b"\x1b[0m\r\n")?;
        }
    }
    Ok(())
}
