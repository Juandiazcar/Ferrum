//! Byte-stream helpers: substring search, hex excerpts and a tiny escape-sequence splitter.

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

pub(crate) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

/// First `max` bytes as space-separated hex.
pub(crate) fn hex(bytes: &[u8], max: usize) -> String {
    bytes.iter().take(max).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

fn csi_len(b: &[u8]) -> usize {
    // ESC [ params final(0x40..=0x7e)
    b.iter().skip(2).position(|c| (0x40..=0x7e).contains(c)).map_or(b.len(), |p| p + 3)
}

fn osc_len(b: &[u8]) -> usize {
    // ESC ] ... terminated by BEL or ESC \
    let mut i = 2;
    while let Some(&c) = b.get(i) {
        if c == BEL {
            return i + 1;
        }
        if c == ESC && b.get(i + 1) == Some(&b'\\') {
            return i + 2;
        }
        i += 1;
    }
    b.len()
}

/// Length of the escape sequence at the start of `b` (which begins with ESC).
fn esc_len(b: &[u8]) -> usize {
    match b.get(1) {
        Some(b'[') => csi_len(b),
        Some(b']') => osc_len(b),
        Some(_) => 2,
        None => 1,
    }
}

/// Splits a byte stream into escape sequences (`ESC...`) and text runs (`TEXT "..."`).
pub(crate) fn describe(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = bytes;
    while let Some(&first) = rest.first() {
        let len = if first == ESC {
            esc_len(rest).min(rest.len())
        } else {
            rest.iter().position(|c| *c == ESC).unwrap_or(rest.len())
        };
        let (chunk, tail) = rest.split_at(len);
        let text = String::from_utf8_lossy(chunk);
        if first == ESC {
            out.push(format!("ESC{}", text.get(1..).unwrap_or_default().escape_debug()));
        } else {
            out.push(format!("TEXT {text:?}"));
        }
        rest = tail;
    }
    out
}
