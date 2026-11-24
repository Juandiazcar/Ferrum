//! A shortcut for the most common control sequence, plain `CSI Pm m` (colors and attributes).
//!
//! Colored output is mostly SGR sequences between short runs of text, and walking each one through
//! the generic `vte` state machine costs more than applying it. This recognizes only the
//! unambiguous shape `ESC [ {digits or ;}* m` that is complete inside the bytes at hand;
//! anything else (sub-parameters, intermediates, other finals, a sequence cut by the end of the
//! read, too many or too large parameters) is left to `vte`, which gives the same result.

const ESC: u8 = 0x1B;
const CSI_INTRODUCER: u8 = b'[';
const SGR_FINAL: u8 = b'm';
const PARAM_SEPARATOR: u8 = b';';
/// Most parameters handled here; `vte` allows more (`MAX_PARAMS`), the rest goes to it.
const MAX_FAST_PARAMS: usize = 16;
/// Bytes of the introducer `ESC [`.
const INTRODUCER_LEN: usize = 2;
/// Most digits of a parameter handled here; four digits never reach the `u16` saturation `vte`
/// applies, so the values are the same on both paths.
const MAX_DIGITS: u8 = 4;
const DECIMAL: u16 = 10;

/// The parameters of an SGR sequence and the bytes it took.
pub(crate) struct Sgr {
    pub(crate) codes: [u16; MAX_FAST_PARAMS],
    pub(crate) count: usize,
    pub(crate) len: usize,
}

/// Recognizes a complete plain SGR sequence at the start of `bytes`.
pub(crate) fn parse(bytes: &[u8]) -> Option<Sgr> {
    let body = bytes.strip_prefix(&[ESC, CSI_INTRODUCER])?;
    let mut sgr = Sgr { codes: [0; MAX_FAST_PARAMS], count: 0, len: 0 };
    let (mut value, mut digits) = (0u16, 0u8);
    for (at, &byte) in body.iter().enumerate() {
        match byte {
            b'0'..=b'9' => {
                digits += 1;
                if digits > MAX_DIGITS {
                    return None;
                }
                value = value * DECIMAL + u16::from(byte - b'0');
            }
            PARAM_SEPARATOR => {
                *sgr.codes.get_mut(sgr.count)? = value;
                sgr.count += 1;
                (value, digits) = (0, 0);
            }
            SGR_FINAL => {
                // `ESC [ m` has no parameters; after a separator the last one is empty (0).
                if sgr.count > 0 || digits > 0 {
                    *sgr.codes.get_mut(sgr.count)? = value;
                    sgr.count += 1;
                }
                sgr.len = INTRODUCER_LEN + at + 1;
                return Some(sgr);
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(bytes: &[u8]) -> Option<(Vec<u16>, usize)> {
        parse(bytes).map(|sgr| (sgr.codes.get(..sgr.count).unwrap_or_default().to_vec(), sgr.len))
    }

    #[test]
    fn plain_sequences_give_their_parameters_and_length() {
        assert_eq!(codes(b"\x1b[m"), Some((vec![], 3)));
        assert_eq!(codes(b"\x1b[0mtext"), Some((vec![0], 4)));
        assert_eq!(codes(b"\x1b[1;38;5;208m"), Some((vec![1, 38, 5, 208], 13)));
        assert_eq!(codes(b"\x1b[;m"), Some((vec![0, 0], 4)));
        assert_eq!(codes(b"\x1b[1;m"), Some((vec![1, 0], 5)));
    }

    #[test]
    fn everything_else_is_left_to_vte() {
        for bytes in [
            &b"\x1b[38:2::1:2:3m"[..],
            b"\x1b[?25m",
            b"\x1b[1;2",
            b"\x1b[1",
            b"\x1b[",
            b"\x1b[12345m",
            b"\x1b[1 m",
            b"\x1b[1H",
            b"\x1bm",
            b"ab",
        ] {
            assert!(parse(bytes).is_none(), "{bytes:?}");
        }
        let many = format!("\x1b[{}m", vec!["1"; MAX_FAST_PARAMS + 1].join(";"));
        assert!(parse(many.as_bytes()).is_none());
        let max = format!("\x1b[{}m", vec!["1"; MAX_FAST_PARAMS].join(";"));
        assert_eq!(parse(max.as_bytes()).map(|sgr| sgr.count), Some(MAX_FAST_PARAMS));
    }
}
