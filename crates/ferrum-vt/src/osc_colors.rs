//! OSC 4, 10, 11, 12 (set and query colors) and OSC 104, 110, 111, 112 (reset), FR-TERM-009.
//!
//! Queries are answered from the terminal's own palette, so the reply holds a color value and
//! nothing the program supplied (`docs/10` §3 item 3). Color names are not supported.

use crate::core::Core;
use crate::events::{Event, EventSink};
use crate::palette::{DynamicColor, Rgb, parse_color};
use crate::reply::Reply;

/// The text a query is asked with.
const QUERY: &[u8] = b"?";
/// String terminator used in replies.
const ST: &[u8] = b"\x1b\\";

/// Converts an 8-bit channel to the 16-bit form xterm reports (`ff` becomes `ffff`).
fn wide(channel: u8) -> u16 {
    u16::from(channel) * 257
}

/// Appends `rrrr/gggg/bbbb` to `reply`.
fn push_color(reply: Reply, color: Rgb) -> Reply {
    reply.hex4(wide(color.r)).text(b"/").hex4(wide(color.g)).text(b"/").hex4(wide(color.b))
}

impl<S: EventSink> Core<S> {
    fn palette_changed(&mut self) {
        self.screen_mut().grid.damage_mut().mark_all();
        self.sink.event(Event::PaletteChanged);
    }

    /// OSC 4: `index;spec` pairs; a spec of `?` asks for the current color.
    pub(crate) fn osc_palette(&mut self, rest: &[u8]) {
        let mut parts = rest.split(|&b| b == b';');
        let mut changed = false;
        while let (Some(index), Some(spec)) = (parts.next(), parts.next()) {
            let Some(index) = parse_index(index) else { continue };
            if spec == QUERY {
                let color = self.palette.entry(index);
                let head = Reply::new().text(b"\x1b]4;").number(usize::from(index)).text(b";rgb:");
                self.send(&push_color(head, color).text(ST));
            } else if let Some(color) = parse_color(spec) {
                changed |= self.palette.set_entry(index, color);
            }
        }
        if changed {
            self.palette_changed();
        }
    }

    /// OSC 104: resets the listed entries, or all of them when none is listed.
    pub(crate) fn reset_palette(&mut self, rest: &[u8]) {
        let mut changed = false;
        if rest.is_empty() {
            changed = self.palette.reset_all_entries();
        }
        for index in rest.split(|&b| b == b';').filter_map(parse_index) {
            changed |= self.palette.reset_entry(index);
        }
        if changed {
            self.palette_changed();
        }
    }

    /// OSC 10, 11, 12: sets or queries the foreground, background and cursor colors. Extra
    /// parameters address the following colors (`10;fg;bg`).
    pub(crate) fn osc_dynamic_colors(&mut self, command: u16, rest: &[u8]) {
        let order = [DynamicColor::Foreground, DynamicColor::Background, DynamicColor::Cursor];
        let first = usize::from(command.saturating_sub(10));
        let mut changed = false;
        for (which, spec) in order.iter().skip(first).zip(rest.split(|&b| b == b';')) {
            if spec == QUERY {
                self.report_dynamic(*which, command_for(*which));
            } else if let Some(color) = parse_color(spec) {
                changed |= self.palette.set_dynamic(*which, color);
            }
        }
        if changed {
            self.palette_changed();
        }
    }

    fn report_dynamic(&mut self, which: DynamicColor, command: usize) {
        let color = self.palette.dynamic(which);
        let head = Reply::new().text(b"\x1b]").number(command).text(b";rgb:");
        self.send(&push_color(head, color).text(ST));
    }

    /// OSC 110, 111, 112.
    pub(crate) fn reset_dynamic_color(&mut self, command: u16) {
        let which = match command {
            110 => DynamicColor::Foreground,
            111 => DynamicColor::Background,
            _ => DynamicColor::Cursor,
        };
        let changed = self.palette.reset_dynamic(which);
        if changed {
            self.palette_changed();
        }
    }
}

fn command_for(which: DynamicColor) -> usize {
    match which {
        DynamicColor::Foreground => 10,
        DynamicColor::Background => 11,
        DynamicColor::Cursor => 12,
    }
}

fn parse_index(text: &[u8]) -> Option<u8> {
    std::str::from_utf8(text).ok()?.parse().ok()
}
