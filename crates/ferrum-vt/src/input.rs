//! The byte-stream front end (`vt::parse`): fast paths in front of `vte`.
//!
//! Three jobs, all for speed and hardening (`docs/05` §6, `docs/10` §3 item 9):
//!
//! 1. **Runs of printable ASCII and plain C0 controls** go straight to the grid when `vte` is
//!    known to be in its ground state, so the common case never pays a callback per character.
//! 2. **OSC strings are captured here**, not by `vte`: its OSC buffer is unbounded in `std` mode
//!    and 1 KiB without it. Ferrum keeps its own buffer, capped at `MAX_OSC_BYTES`, and
//!    resynchronizes `vte` with a harmless `ESC \` first. `vte` therefore never sees an OSC.
//! 3. Everything else (CSI, ESC, DCS, non-ASCII text) is fed to `vte`, which calls the handlers
//!    in `perform.rs`.
//!
//! How the front end knows `vte` is in the ground state: `vte` only leaves it through an ESC byte,
//! and the handlers raise [`Core::stop`] when a CSI or ESC sequence has been dispatched (which
//! always returns `vte` to the ground state, and `vte` checks the flag before the next byte). A
//! sequence that ends without a dispatch (an ignored CSI, a CAN) leaves the state "unknown" and
//! costs speed, never correctness. Printing and executing controls cannot raise the flag: `vte`
//! decodes a whole run of text in one call and may consume an ESC after it.

use memchr::memmem::Finder;

use crate::core::Core;
use crate::events::EventSink;
use crate::limits::{MAX_OSC_BYTES, OSC_BUFFER_KEEP_BYTES};

const ESC: u8 = 0x1B;
const BEL: u8 = 0x07;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1A;
const DEL: u8 = 0x7F;
const FIRST_PRINTABLE: u8 = 0x20;
const LAST_PRINTABLE: u8 = 0x7E;
/// First byte of a multi-byte UTF-8 sequence or a stray continuation byte.
const FIRST_NON_ASCII: u8 = 0x80;
/// Bytes of OSC payload `vte` itself would buffer. It never sees an OSC, so this is the smallest
/// legal value.
const VTE_OSC_BYTES: usize = 1;

/// Consecutive steps that may consume no byte before one byte is skipped (never hang).
const MAX_STALLED_STEPS: u32 = 2;

type Vte = vte::Parser<VTE_OSC_BYTES>;

/// What the front end knows about the state of `vte`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VteState {
    /// In the ground state with no character pending: fast paths are allowed.
    Ground,
    /// In the ground state, but the last byte fed may have started a UTF-8 character that is not
    /// complete yet: bytes go one at a time.
    Utf8Tail,
    /// Inside a sequence, or not sure.
    Unknown,
}

/// Whether an OSC string is being captured.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OscState {
    Idle,
    Collecting,
    /// Collecting, but the payload went over the cap: it is read to its end and dropped.
    Overflowed,
}

/// The `vte` parser plus the front end's own state.
pub(crate) struct Input {
    parser: Vte,
    osc_finder: Finder<'static>,
    osc_buffer: Vec<u8>,
    osc: OscState,
    vte_state: VteState,
    /// The last byte of the previous chunk was an ESC that `vte` consumed.
    esc_pending: bool,
}

impl Input {
    pub(crate) fn new() -> Self {
        Self {
            parser: Vte::new_with_size(),
            osc_finder: Finder::new(&[ESC, b']']).into_owned(),
            osc_buffer: Vec::new(),
            osc: OscState::Idle,
            vte_state: VteState::Ground,
            esc_pending: false,
        }
    }

    /// Parses `bytes`, calling the handlers of `core`.
    pub(crate) fn advance<S: EventSink>(&mut self, core: &mut Core<S>, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut pos = 0;
        if self.esc_pending && bytes.first() == Some(&b']') {
            self.begin_osc(core, true);
            pos = 1;
        }
        self.esc_pending = false;
        let mut osc_at = self.next_osc(bytes, pos);
        let mut stalled = 0;
        while pos < bytes.len() {
            if self.osc != OscState::Idle {
                pos += self.collect_osc(core, bytes.get(pos..).unwrap_or_default());
                osc_at = self.next_osc(bytes, pos);
                continue;
            }
            let limit = osc_at.unwrap_or(bytes.len());
            if pos >= limit {
                self.begin_osc(core, false);
                pos += 2;
                osc_at = self.next_osc(bytes, pos);
                continue;
            }
            let used = self.step(core, bytes.get(pos..limit).unwrap_or_default());
            // A step that consumes nothing changed the parser state instead (it flushed a
            // pending UTF-8 character); more than a couple in a row would be a stall.
            stalled = if used == 0 { stalled + 1 } else { 0 };
            pos += if stalled > MAX_STALLED_STEPS { 1 } else { used };
        }
        self.esc_pending = self.osc == OscState::Idle
            && bytes.last() == Some(&ESC)
            && self.vte_state == VteState::Unknown;
    }

    fn next_osc(&self, bytes: &[u8], from: usize) -> Option<usize> {
        let tail = bytes.get(from..)?;
        self.osc_finder.find(tail).map(|offset| from + offset)
    }

    /// Handles the start of `slice` (no `ESC ]` inside) and returns the bytes consumed.
    fn step<S: EventSink>(&mut self, core: &mut Core<S>, slice: &[u8]) -> usize {
        let Some(&first) = slice.first() else { return 0 };
        if self.vte_state == VteState::Utf8Tail {
            return self.feed_tail_byte(core, first);
        }
        if self.vte_state == VteState::Ground {
            let run = slice
                .iter()
                .take_while(|&&b| (FIRST_PRINTABLE..=LAST_PRINTABLE).contains(&b))
                .count();
            if run > 0 {
                core.print_ascii(slice.get(..run).unwrap_or_default());
                return run;
            }
            if first == DEL {
                return 1;
            }
            if first < FIRST_PRINTABLE && first != ESC {
                core.control(first);
                return 1;
            }
            if first >= FIRST_NON_ASCII {
                return self.feed_text(core, slice);
            }
        }
        self.feed_sequence(core, slice)
    }

    /// `vte` holds the start of a UTF-8 character: one byte at a time, so that the state after
    /// each byte is known from the byte itself. A continuation or lead byte keeps `vte` in the
    /// ground state; an ESC flushes the partial character and starts a sequence; anything else
    /// flushes it and is handled in the ground state.
    fn feed_tail_byte<S: EventSink>(&mut self, core: &mut Core<S>, byte: u8) -> usize {
        self.parser.advance(core, &[byte]);
        match byte {
            ESC => self.vte_state = VteState::Unknown,
            FIRST_NON_ASCII..=0xFF => {}
            _ => self.vte_state = VteState::Ground,
        }
        1
    }

    /// Non-ASCII text in the ground state: `vte` decodes it up to the next ESC.
    fn feed_text<S: EventSink>(&mut self, core: &mut Core<S>, slice: &[u8]) -> usize {
        let end = memchr::memchr(ESC, slice).unwrap_or(slice.len());
        let text = slice.get(..end).unwrap_or_default();
        self.parser.advance(core, text);
        let tail = text.last().is_some_and(|&b| b >= FIRST_NON_ASCII);
        self.vte_state = if tail { VteState::Utf8Tail } else { VteState::Ground };
        end
    }

    /// An escape sequence, or the rest of one: `vte` runs until a sequence is dispatched.
    fn feed_sequence<S: EventSink>(&mut self, core: &mut Core<S>, slice: &[u8]) -> usize {
        core.stop = false;
        let used = self.parser.advance_until_terminated(core, slice);
        // A dispatch returns `vte` to the ground state with no character pending.
        self.vte_state = if core.stop { VteState::Ground } else { VteState::Unknown };
        used
    }

    /// Switches to OSC capture; `vte` is first brought back to its ground state.
    fn begin_osc<S: EventSink>(&mut self, core: &mut Core<S>, esc_consumed: bool) {
        if esc_consumed {
            self.parser.advance(core, b"\\");
        } else if self.vte_state != VteState::Ground {
            self.parser.advance(core, &[ESC, b'\\']);
        }
        self.vte_state = VteState::Ground;
        self.osc = OscState::Collecting;
        self.osc_buffer.clear();
    }

    /// Collects OSC bytes up to a terminator; returns the bytes consumed. An ESC terminator is
    /// left for the caller (it starts the next sequence, usually the `\` of `ESC \`).
    fn collect_osc<S: EventSink>(&mut self, core: &mut Core<S>, bytes: &[u8]) -> usize {
        let end = bytes.iter().position(|&b| matches!(b, BEL | ESC | CAN | SUB));
        let payload = bytes.get(..end.unwrap_or(bytes.len())).unwrap_or_default();
        if self.osc_buffer.len() + payload.len() > MAX_OSC_BYTES {
            self.osc = OscState::Overflowed;
        } else {
            self.osc_buffer.extend_from_slice(payload);
        }
        let Some(end) = end else { return bytes.len() };
        match bytes.get(end) {
            Some(&(CAN | SUB)) => self.abort_osc(),
            _ => self.finish_osc(core),
        }
        if bytes.get(end) == Some(&ESC) { end } else { end + 1 }
    }

    /// CAN and SUB cancel the string: nothing is dispatched (ECMA-48).
    fn abort_osc(&mut self) {
        self.osc = OscState::Idle;
        self.release_osc_buffer();
    }

    fn finish_osc<S: EventSink>(&mut self, core: &mut Core<S>) {
        if self.osc == OscState::Overflowed {
            tracing::debug!("OSC payload over the cap discarded");
        } else {
            core.osc(&self.osc_buffer);
        }
        self.osc = OscState::Idle;
        self.release_osc_buffer();
    }

    /// Empties the buffer, and frees it when a big payload made it grow.
    fn release_osc_buffer(&mut self) {
        self.osc_buffer.clear();
        if self.osc_buffer.capacity() > OSC_BUFFER_KEEP_BYTES {
            self.osc_buffer = Vec::new();
        }
    }
}

impl std::fmt::Debug for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Input")
            .field("vte_state", &self.vte_state)
            .field("osc", &self.osc)
            .finish_non_exhaustive()
    }
}
