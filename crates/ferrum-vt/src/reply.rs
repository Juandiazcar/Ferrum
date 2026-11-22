//! Replies to the program (DA, DSR, DECRQM, ...). They are built on the stack from fixed text and
//! numbers the terminal itself owns (cursor position, mode state), never from program-supplied
//! text (`docs/10` §3 item 3).

use crate::core::Core;
use crate::events::{Event, EventSink};

/// Longest reply the terminal builds.
const REPLY_CAPACITY: usize = 64;

/// A short reply under construction.
pub(crate) struct Reply {
    bytes: [u8; REPLY_CAPACITY],
    len: usize,
}

impl Reply {
    pub(crate) fn new() -> Self {
        Self { bytes: [0; REPLY_CAPACITY], len: 0 }
    }

    /// Appends fixed bytes; what does not fit is dropped (replies are far below the capacity).
    pub(crate) fn text(mut self, text: &[u8]) -> Self {
        for &byte in text {
            if let Some(slot) = self.bytes.get_mut(self.len) {
                *slot = byte;
                self.len += 1;
            }
        }
        self
    }

    /// Appends a decimal number.
    pub(crate) fn number(self, value: usize) -> Self {
        let mut digits = [0u8; 20];
        let mut at = digits.len();
        let mut rest = value;
        loop {
            at -= 1;
            if let Some(slot) = digits.get_mut(at) {
                // INVARIANT: `rest % 10` is below 10, so the sum is an ASCII digit.
                *slot = b'0' + u8::try_from(rest % 10).unwrap_or(0);
            }
            rest /= 10;
            if rest == 0 || at == 0 {
                break;
            }
        }
        let text = digits.get(at..).unwrap_or_default();
        self.text(text)
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }
}

impl<S: EventSink> Core<S> {
    /// Sends `reply` to the program through the sink.
    pub(crate) fn send(&mut self, reply: &Reply) {
        self.sink.event(Event::Reply(reply.as_bytes()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_are_built_from_text_and_numbers() {
        let reply = Reply::new().text(b"\x1b[").number(12).text(b";").number(0).number(1_234_567);
        assert_eq!(reply.as_bytes(), b"\x1b[12;01234567");
    }

    #[test]
    fn a_reply_never_outgrows_its_buffer() {
        let mut reply = Reply::new();
        for _ in 0..REPLY_CAPACITY * 2 {
            reply = reply.text(b"x");
        }
        assert_eq!(reply.as_bytes().len(), REPLY_CAPACITY);
        assert_eq!(Reply::new().number(usize::MAX).as_bytes().len(), 20);
    }
}
