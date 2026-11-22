//! The `vte` callbacks: Ferrum's own handlers (`docs/02` §3.3).

use vte::{Params, Perform};

use crate::args::Args;
use crate::core::Core;
use crate::events::{Event, EventSink};

/// C0 and C1 controls handled in the ground state.
mod control {
    pub(super) const BEL: u8 = 0x07;
    pub(super) const BS: u8 = 0x08;
    pub(super) const HT: u8 = 0x09;
    pub(super) const LF: u8 = 0x0A;
    pub(super) const VT: u8 = 0x0B;
    pub(super) const FF: u8 = 0x0C;
    pub(super) const CR: u8 = 0x0D;
    pub(super) const SO: u8 = 0x0E;
    pub(super) const SI: u8 = 0x0F;
    pub(super) const IND: u8 = 0x84;
    pub(super) const NEL: u8 = 0x85;
    pub(super) const HTS: u8 = 0x88;
    pub(super) const RI: u8 = 0x8D;
}

impl<S: EventSink> Core<S> {
    /// Handles a C0 or C1 control byte (shared by the `vte` path and the front end's fast path).
    pub(crate) fn control(&mut self, byte: u8) {
        match byte {
            control::BEL => self.sink.event(Event::Bell),
            control::BS => self.backspace(),
            control::HT => self.tab_forward(1),
            control::LF | control::VT | control::FF => self.line_feed(),
            control::CR => self.carriage_return(),
            control::SO => self.screen_mut().charsets.shift(1),
            control::SI => self.screen_mut().charsets.shift(0),
            control::IND => self.index(),
            control::NEL => self.next_line(),
            control::HTS => {
                let x = self.screen().cursor.x;
                self.tabs.set(x);
            }
            control::RI => self.reverse_index(),
            _ => {}
        }
    }
}

impl<S: EventSink> Perform for Core<S> {
    fn print(&mut self, c: char) {
        self.print_char(c);
    }

    fn execute(&mut self, byte: u8) {
        self.control(byte);
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        self.stop = true;
        if !ignore {
            self.csi(Args::new(params), intermediates, action);
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        self.stop = true;
        if !ignore {
            self.esc(intermediates, byte);
        }
    }

    fn terminated(&self) -> bool {
        self.stop
    }
}
