//! The byte-stream front end (`vt::parse`): fast paths in front of `vte`.
//!
//! Three jobs, all for speed and hardening (`docs/05` §6, `docs/10` §3 item 9):
//!
//! 1. **Runs of printable ASCII and plain C0 controls** go straight to the grid when `vte` is
//!    known to be in its ground state, so the common case never pays a callback per character.
//! 2. **OSC strings are captured here**, not by `vte`: its OSC buffer is unbounded in `std` mode
//!    and 1 KiB without it. Ferrum keeps its own buffer, capped at `MAX_OSC_BYTES`, and
//!    resynchronizes `vte` with a harmless `ESC \` first. `vte` therefore never sees an OSC.
//! 3. Everything else (CSI, ESC, DCS, APC, PM, SOS, non-ASCII text) is fed to `vte`, which calls
//!    the handlers in `perform.rs`.
//!
//! How the front end knows the state of `vte`: it follows the same bytes with the small mirror of
//! the VT500 states in `sequence.rs`, so it knows where every sequence ends, with or without a
//! dispatch, however the stream is cut into reads.
//!
//! The watchdog (`docs/10` §3 item 9): a sequence or string may take at most `MAX_SEQUENCE_BYTES`
//! (an OSC payload `MAX_OSC_BYTES`). At the first byte over the cap it is cancelled: its buffer is
//! freed, `vte` is reset, a DCS string is forgotten unanswered, and the rest of it is dropped
//! without being stored up to the byte that ends it (its final byte, ST, BEL for an OSC, CAN or
//! SUB; an ESC is kept, it starts the next sequence). Nothing of its payload reaches the screen.

use memchr::memmem::Finder;

use crate::core::Core;
use crate::events::EventSink;
use crate::limits::{MAX_OSC_BYTES, MAX_SEQUENCE_BYTES, OSC_BUFFER_KEEP_BYTES};
use crate::sgr_fast;

mod sequence;

use sequence::{ESC, OSC_INTRODUCER, Seq};

const BEL: u8 = 0x07;
const DEL: u8 = 0x7F;
const FIRST_PRINTABLE: u8 = 0x20;
const LAST_PRINTABLE: u8 = 0x7E;
/// First byte of a multi-byte UTF-8 sequence or a stray continuation byte.
const FIRST_NON_ASCII: u8 = 0x80;
/// Bytes of OSC payload `vte` itself would buffer. It never sees an OSC, so this is the smallest
/// legal value.
const VTE_OSC_BYTES: usize = 1;
/// Length of `ESC ]`.
const OSC_INTRODUCER_BYTES: usize = 2;

/// Consecutive steps that may consume no byte before one byte is skipped (never hang).
const MAX_STALLED_STEPS: u32 = 2;

type Vte = vte::Parser<VTE_OSC_BYTES>;

const LANES: usize = 8;
const HIGH_BITS: u64 = 0x8080_8080_8080_8080;
const ONES: u64 = 0x0101_0101_0101_0101;
/// Added to every byte to find those below 0x20 (see `printable_run`).
const CONTROL_BIAS: u64 = 0x60;

/// Length of the leading run of printable ASCII (0x20..=0x7E) in `bytes`.
///
/// perf: eight bytes per step with word arithmetic (bench `vt_parse/ascii_8mib`). With no byte
/// above 0x7F, adding 0x60 sets the high bit of every byte that is at least 0x20 and adding 0x01
/// sets it for 0x7F, without carries between the bytes.
fn printable_run(bytes: &[u8]) -> usize {
    let mut length = 0;
    let (words, _) = bytes.as_chunks::<LANES>();
    for lanes in words {
        let value = u64::from_le_bytes(*lanes);
        let ascii = value & HIGH_BITS == 0;
        let above_control = value.wrapping_add(ONES * CONTROL_BIAS) & HIGH_BITS == HIGH_BITS;
        let below_del = value.wrapping_add(ONES) & HIGH_BITS == 0;
        if !(ascii && above_control && below_del) {
            break;
        }
        length += LANES;
    }
    length
        + bytes
            .get(length..)
            .unwrap_or_default()
            .iter()
            .take_while(|&&b| (FIRST_PRINTABLE..=LAST_PRINTABLE).contains(&b))
            .count()
}

/// What the front end knows about the state of `vte`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VteState {
    /// In the ground state with no character pending: fast paths are allowed.
    Ground,
    /// In the ground state, but the last byte fed may have started a UTF-8 character that is not
    /// complete yet: bytes go one at a time.
    Utf8Tail,
    /// Inside a sequence of this family (see `sequence.rs`).
    Sequence(Seq),
    /// A sequence of this family was cancelled at the cap and `vte` was reset: its remaining
    /// bytes are dropped, unstored, up to the byte that would have ended it.
    Discard(Seq),
}

/// Whether the ESC that opens an OSC was already fed to `vte`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EscSeen {
    /// `vte` consumed it: it is in the escape state.
    Consumed,
    /// It is in the bytes being parsed and `vte` has not seen it.
    InBytes,
}

/// Whether an OSC string is being captured.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OscState {
    Idle,
    Collecting,
    /// Over the cap: the rest is dropped up to its terminator.
    Discarding,
}

/// The `vte` parser plus the front end's own state.
pub(crate) struct Input {
    parser: Vte,
    osc_finder: Finder<'static>,
    osc_buffer: Vec<u8>,
    osc: OscState,
    vte_state: VteState,
    /// Bytes of the open sequence so far, its introducer included (the watchdog).
    sequence_bytes: usize,
}

impl Input {
    pub(crate) fn new() -> Self {
        Self {
            parser: Vte::new_with_size(),
            osc_finder: Finder::new(&[ESC, OSC_INTRODUCER]).into_owned(),
            osc_buffer: Vec::new(),
            osc: OscState::Idle,
            vte_state: VteState::Ground,
            sequence_bytes: 0,
        }
    }

    /// Parses `bytes`, calling the handlers of `core`.
    pub(crate) fn advance<S: EventSink>(&mut self, core: &mut Core<S>, bytes: &[u8]) {
        let mut pos = 0;
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
                self.begin_osc(core, EscSeen::InBytes);
                pos += OSC_INTRODUCER_BYTES;
                osc_at = self.next_osc(bytes, pos);
                continue;
            }
            let used = self.step(core, bytes.get(pos..limit).unwrap_or_default());
            // A step that consumes nothing changed the parser state instead (it flushed a
            // pending UTF-8 character); more than a couple in a row would be a stall.
            stalled = if used == 0 { stalled + 1 } else { 0 };
            pos += if stalled > MAX_STALLED_STEPS { 1 } else { used };
        }
    }

    fn next_osc(&self, bytes: &[u8], from: usize) -> Option<usize> {
        let tail = bytes.get(from..)?;
        self.osc_finder.find(tail).map(|offset| from + offset)
    }

    /// Handles the start of `slice` (no `ESC ]` inside) and returns the bytes consumed.
    fn step<S: EventSink>(&mut self, core: &mut Core<S>, slice: &[u8]) -> usize {
        let Some(&first) = slice.first() else { return 0 };
        match self.vte_state {
            VteState::Ground => self.step_ground(core, slice, first),
            VteState::Utf8Tail => self.feed_tail_byte(core, first),
            VteState::Sequence(Seq::Escape) if first == OSC_INTRODUCER => {
                self.begin_osc(core, EscSeen::Consumed);
                1
            }
            VteState::Sequence(seq) => self.feed_sequence(core, seq, slice),
            VteState::Discard(seq) => self.discard_sequence(seq, slice),
        }
    }

    /// The ground state: the fast paths, or `vte` for what they do not cover.
    fn step_ground<S: EventSink>(&mut self, core: &mut Core<S>, slice: &[u8], first: u8) -> usize {
        let run = printable_run(slice);
        if run > 0 {
            core.print_ascii(slice.get(..run).unwrap_or_default());
            return run;
        }
        match first {
            ESC => match sgr_fast::parse(slice) {
                Some(sgr) => {
                    core.sgr_codes(sgr.codes.get(..sgr.count).unwrap_or_default());
                    sgr.len
                }
                None => self.feed_sequence(core, Seq::Escape, slice),
            },
            DEL => 1,
            FIRST_NON_ASCII..=u8::MAX => self.feed_text(core, slice),
            _ => {
                core.control(first);
                1
            }
        }
    }

    /// `vte` holds the start of a UTF-8 character: one byte at a time, so that the state after
    /// each byte is known from the byte itself. A continuation or lead byte keeps `vte` in the
    /// ground state; an ESC flushes the partial character and starts a sequence; anything else
    /// flushes it and is handled in the ground state.
    fn feed_tail_byte<S: EventSink>(&mut self, core: &mut Core<S>, byte: u8) -> usize {
        self.parser.advance(core, &[byte]);
        self.vte_state = match byte {
            ESC => {
                self.sequence_bytes = 1;
                VteState::Sequence(Seq::Escape)
            }
            FIRST_NON_ASCII..=u8::MAX => VteState::Utf8Tail,
            _ => VteState::Ground,
        };
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

    /// The open sequence `seq` (or the one an ESC at the start of `slice` opens) goes to `vte` up
    /// to its end. At the first byte over `MAX_SEQUENCE_BYTES` it is cancelled (the watchdog).
    fn feed_sequence<S: EventSink>(&mut self, core: &mut Core<S>, seq: Seq, slice: &[u8]) -> usize {
        let budget = MAX_SEQUENCE_BYTES.saturating_sub(self.sequence_bytes);
        let window = slice.get(..slice.len().min(budget.saturating_add(1))).unwrap_or_default();
        let scan = sequence::scan(seq, window);
        if let Some(open) = scan.state.filter(|_| scan.used > budget && !scan.at_boundary) {
            self.feed_vte(core, window.get(..budget).unwrap_or_default());
            self.cancel_sequence(core, open);
            return budget;
        }
        let ended_dcs = self.feed_vte(core, window.get(..scan.used).unwrap_or_default());
        self.sequence_bytes = match scan.state {
            _ if scan.restarted => 1,
            Some(_) => self.sequence_bytes + scan.used,
            None => 0,
        };
        self.vte_state = match scan.state {
            // An 8-bit ST that `vte` took as the end of the DCS data.
            Some(Seq::Dcs) if ended_dcs => VteState::Ground,
            Some(state) => VteState::Sequence(state),
            None => VteState::Ground,
        };
        scan.used
    }

    /// Feeds `bytes` to `vte`. A DCS string that `vte` left in them, and that CAN or SUB did not
    /// cancel (`perform.rs`), ended with a terminator: it is answered here, so the answer never
    /// depends on how the stream was cut. Returns true when that happened.
    fn feed_vte<S: EventSink>(&mut self, core: &mut Core<S>, bytes: &[u8]) -> bool {
        self.parser.advance(core, bytes);
        let ended = std::mem::take(&mut core.dcs_end);
        if ended {
            core.dcs_complete();
        }
        ended
    }

    /// The watchdog: `vte` is reset (a DCS string in progress is forgotten unanswered) and the
    /// rest of the sequence `seq` is dropped up to its terminator.
    fn cancel_sequence<S: EventSink>(&mut self, core: &mut Core<S>, seq: Seq) {
        tracing::debug!("escape sequence over the cap cancelled");
        self.parser = Vte::new_with_size();
        core.dcs_end = false;
        core.dcs_cancel();
        self.sequence_bytes = 0;
        self.vte_state = VteState::Discard(seq);
    }

    /// Drops the bytes of a cancelled sequence up to its terminator.
    fn discard_sequence(&mut self, seq: Seq, slice: &[u8]) -> usize {
        let (used, state) = sequence::discard(seq, slice);
        self.vte_state = state.map_or(VteState::Ground, VteState::Discard);
        used
    }

    /// Switches to OSC capture; `vte` is first brought back to its ground state.
    fn begin_osc<S: EventSink>(&mut self, core: &mut Core<S>, esc: EscSeen) {
        match (esc, self.vte_state) {
            (EscSeen::Consumed, _) => {
                self.feed_vte(core, b"\\");
            }
            (EscSeen::InBytes, VteState::Sequence(_) | VteState::Utf8Tail) => {
                self.feed_vte(core, &[ESC, b'\\']);
            }
            (EscSeen::InBytes, VteState::Ground | VteState::Discard(_)) => {}
        }
        self.vte_state = VteState::Ground;
        self.sequence_bytes = 0;
        self.osc = OscState::Collecting;
        self.osc_buffer.clear();
    }

    /// Collects OSC bytes up to a terminator; returns the bytes consumed. An ESC terminator is
    /// left for the caller (it starts the next sequence, usually the `\` of `ESC \`). A payload
    /// that goes over `MAX_OSC_BYTES` is dropped at that byte with its buffer, and the rest of it
    /// is discarded up to its terminator (`docs/10` §3 item 9).
    fn collect_osc<S: EventSink>(&mut self, core: &mut Core<S>, bytes: &[u8]) -> usize {
        // perf: SIMD terminator search instead of a byte loop (bench vt_parse/ascii_8mib).
        let end = memchr::memchr3(BEL, ESC, sequence::CAN, bytes);
        let before = bytes.get(..end.unwrap_or(bytes.len())).unwrap_or_default();
        let end = memchr::memchr(sequence::SUB, before).or(end);
        let payload = bytes.get(..end.unwrap_or(bytes.len())).unwrap_or_default();
        let room = MAX_OSC_BYTES.saturating_sub(self.osc_buffer.len());
        if self.osc == OscState::Collecting && payload.len() > room {
            tracing::debug!("OSC payload over the cap discarded");
            self.release_osc_buffer();
            self.osc = OscState::Discarding;
            return room;
        }
        let Some(end) = end else {
            if self.osc == OscState::Collecting {
                self.osc_buffer.extend_from_slice(payload);
            }
            return bytes.len();
        };
        match bytes.get(end) {
            Some(&(BEL | ESC)) if self.osc == OscState::Collecting => {
                self.osc_buffer.extend_from_slice(payload);
                core.osc(&self.osc_buffer);
            }
            _ => {}
        }
        self.osc = OscState::Idle;
        self.release_osc_buffer();
        if bytes.get(end) == Some(&ESC) { end } else { end + 1 }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn naive(bytes: &[u8]) -> usize {
        bytes.iter().take_while(|&&b| (FIRST_PRINTABLE..=LAST_PRINTABLE).contains(&b)).count()
    }

    #[test]
    fn the_word_scan_agrees_with_the_byte_scan_for_every_byte_in_every_lane() {
        for lane in 0..2 * LANES {
            for value in 0..=u8::MAX {
                let mut bytes = [b'a'; 2 * LANES + 3];
                if let Some(slot) = bytes.get_mut(lane) {
                    *slot = value;
                }
                assert_eq!(printable_run(&bytes), naive(&bytes), "byte {value:#04x} at {lane}");
            }
        }
    }
}
