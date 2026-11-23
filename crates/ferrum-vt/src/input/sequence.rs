//! The escape-sequence states of `vte`, mirrored by the front end (`input.rs`).
//!
//! `vte` 0.15 implements the VT500 parser of Paul Williams (<https://vt100.net/emu/dec_ansi_parser>)
//! and does not say which state it is in. The front end needs to know when a sequence ends, with
//! or without a dispatch, to go back to the fast paths. That depends only on the family of the
//! state, so this module tracks the family from the same bytes `vte` sees. Bytes that `vte`
//! executes inside a sequence (C0 controls) do not change the family.

/// The family of the `vte` state while a sequence is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Seq {
    /// After ESC.
    Escape,
    /// After ESC and at least one intermediate byte.
    EscIntermediate,
    /// A control sequence (CSI), including the state that ignores a malformed one: all of them
    /// end at a final byte.
    Csi,
    /// A device control string (DCS): its header or its data.
    Dcs,
    /// SOS, PM or APC: a string `vte` discards.
    Str,
}

/// ESC: it ends any sequence and starts a new one.
pub(super) const ESC: u8 = 0x1B;
/// CAN and SUB cancel any sequence (ECMA-48 §8.3.17, §8.3.148).
pub(super) const CAN: u8 = 0x18;
pub(super) const SUB: u8 = 0x1A;
/// The 8-bit string terminator; `vte` honors it at the end of DCS data.
pub(super) const ST: u8 = 0x9C;
/// `ESC ]` opens an OSC string, which the front end captures itself.
pub(super) const OSC_INTRODUCER: u8 = b']';
/// Intermediate bytes of an escape sequence.
const INTERMEDIATES: std::ops::RangeInclusive<u8> = 0x20..=0x2F;
/// Final bytes of an escape sequence (after its intermediates).
const ESC_FINALS: std::ops::RangeInclusive<u8> = 0x30..=0x7E;
/// Final bytes of a control sequence.
const CSI_FINALS: std::ops::RangeInclusive<u8> = 0x40..=0x7E;

impl Seq {
    /// The family after `byte`, or `None` when `byte` ends the sequence and `vte` is back in the
    /// ground state. ESC is handled by the caller (it ends this sequence and starts another).
    fn next(self, byte: u8) -> Option<Self> {
        if matches!(byte, CAN | SUB) {
            return None;
        }
        match self {
            Self::Escape => match byte {
                b'P' => Some(Self::Dcs),
                b'[' => Some(Self::Csi),
                b'X' | b'^' | b'_' => Some(Self::Str),
                b if INTERMEDIATES.contains(&b) => Some(Self::EscIntermediate),
                b if ESC_FINALS.contains(&b) => None,
                _ => Some(self),
            },
            Self::EscIntermediate if ESC_FINALS.contains(&byte) => None,
            Self::Csi if CSI_FINALS.contains(&byte) => None,
            _ => Some(self),
        }
    }

    /// True for the strings whose data can be skipped with a search for their few terminators.
    fn is_string(self) -> bool {
        matches!(self, Self::Dcs | Self::Str)
    }

    /// Offset of the first byte of `bytes` that can end a string of this family.
    fn string_end(self, bytes: &[u8]) -> Option<usize> {
        let controls = memchr::memchr3(ESC, CAN, SUB, bytes);
        if self != Self::Dcs {
            return controls;
        }
        let before = bytes.get(..controls.unwrap_or(bytes.len())).unwrap_or_default();
        memchr::memchr(ST, before).or(controls)
    }
}

/// How far a sequence went in the bytes given to [`scan`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Scan {
    /// Bytes that belong to the sequence and go to `vte`.
    pub(super) used: usize,
    /// The family after them; `None` when the sequence ended (ground state).
    pub(super) state: Option<Seq>,
}

/// Follows the sequence `seq` through `bytes`. Stops after the byte that ends it, after an ESC
/// (a new sequence), before the `]` of an OSC (the front end captures it), and after an 8-bit ST
/// in a DCS (whether it ended the string only `vte` knows: it calls `unhook`).
pub(super) fn scan(mut seq: Seq, bytes: &[u8]) -> Scan {
    let mut used = 0;
    while let Some(rest) = bytes.get(used..).filter(|rest| !rest.is_empty()) {
        if seq.is_string() {
            let Some(skip) = seq.string_end(rest) else { return Scan::open(bytes.len(), seq) };
            used += skip;
        }
        let Some(&byte) = bytes.get(used) else { break };
        if seq == Seq::Escape && byte == OSC_INTRODUCER {
            break;
        }
        used += 1;
        if byte == ESC {
            return Scan { used, state: Some(Seq::Escape) };
        }
        let Some(next) = seq.next(byte) else {
            return Scan { used, state: None };
        };
        seq = next;
        if byte == ST && seq == Seq::Dcs {
            return Scan { used, state: Some(seq) };
        }
    }
    Scan::open(used, seq)
}

impl Scan {
    /// The sequence is still open after `used` bytes.
    const fn open(used: usize, seq: Seq) -> Self {
        Self { used, state: Some(seq) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_control_sequence_ends_at_its_final_byte_even_when_it_is_ignored() {
        let scan = scan(Seq::Csi, b"1?h tail");
        assert_eq!((scan.used, scan.state), (3, None));
    }

    #[test]
    fn an_esc_starts_a_new_sequence_and_can_or_sub_cancel() {
        assert_eq!(scan(Seq::Csi, b"12\x1b[").used, 3);
        assert_eq!(scan(Seq::Csi, b"12\x1b[").state, Some(Seq::Escape));
        assert_eq!(scan(Seq::Str, b"payload\x18x").used, 8);
        assert_eq!(scan(Seq::Str, b"payload\x18x").state, None);
    }

    #[test]
    fn the_bracket_of_an_osc_is_left_to_the_front_end() {
        assert_eq!(scan(Seq::Escape, b"\n]0;t"), Scan::open(1, Seq::Escape));
    }

    #[test]
    fn strings_are_skipped_up_to_their_terminators() {
        assert_eq!(scan(Seq::Escape, b"Pq#0;2~~").state, Some(Seq::Dcs));
        assert_eq!(scan(Seq::Escape, b"Pq#0;2~~").used, 8);
        assert_eq!(scan(Seq::Dcs, b"ab\x9ccd").used, 3);
        assert_eq!(scan(Seq::Str, b"ab\x9ccd").used, 5);
    }
}
