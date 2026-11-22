//! OSC 8 hyperlinks (FR-TERM-009, FR-TERM-016). The terminal stores the URI as the program sent
//! it, restricted to printable ASCII and bounded; the UI decides which schemes may be opened
//! (`docs/10` §3 item 6).

use crate::core::Core;
use crate::events::EventSink;
use crate::extras::LinkId;
use crate::limits::{MAX_LINK_ID_BYTES, MAX_URI_BYTES};

/// True for the bytes a URI or an id may contain (printable ASCII).
fn is_printable_ascii(bytes: &[u8]) -> bool {
    bytes.iter().all(|b| (0x20..=0x7E).contains(b))
}

/// The `id=` value of an OSC 8 parameter list (`key=value` pairs separated by `:`).
fn link_id(params: &[u8]) -> &[u8] {
    params.split(|&b| b == b':').find_map(|pair| pair.strip_prefix(b"id=")).unwrap_or_default()
}

impl<S: EventSink> Core<S> {
    /// `OSC 8 ; params ; URI`. An empty URI ends the link; an invalid or over-long one is ignored
    /// and also ends the link, so text is never linked by accident.
    pub(crate) fn osc_hyperlink(&mut self, rest: &[u8]) {
        let (params, uri) = match rest.iter().position(|&b| b == b';') {
            Some(at) => {
                (rest.get(..at).unwrap_or_default(), rest.get(at + 1..).unwrap_or_default())
            }
            None => (rest, &[][..]),
        };
        let id = link_id(params);
        let valid = !uri.is_empty()
            && uri.len() <= MAX_URI_BYTES
            && id.len() <= MAX_LINK_ID_BYTES
            && is_printable_ascii(uri)
            && is_printable_ascii(id);
        let link = if valid { self.intern_link(id, uri) } else { LinkId::NONE };
        let mut pen = self.screen().cursor.pen;
        pen.link = link;
        self.set_pen(pen);
    }

    fn intern_link(&mut self, id: &[u8], uri: &[u8]) -> LinkId {
        // INVARIANT: both slices were checked to be printable ASCII, hence valid UTF-8.
        let (Ok(id), Ok(uri)) = (std::str::from_utf8(id), std::str::from_utf8(uri)) else {
            return LinkId::NONE;
        };
        self.extras.intern_link(id, uri, &mut self.scratch.short).unwrap_or(LinkId::NONE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_is_found_among_the_parameters() {
        assert_eq!(link_id(b"id=abc"), b"abc");
        assert_eq!(link_id(b"foo=1:id=x:bar=2"), b"x");
        assert_eq!(link_id(b""), b"");
        assert_eq!(link_id(b"notid=1"), b"");
    }

    #[test]
    fn only_printable_ascii_is_accepted() {
        assert!(is_printable_ascii(b"https://example.test/a?b=c"));
        assert!(!is_printable_ascii(b"a\x1bb"));
        assert!(!is_printable_ascii("é".as_bytes()));
    }
}
