//! OSC 52 clipboard access (FR-TERM-009). The terminal only reports the request as an event; the
//! policy (deny, write-only with notification, ask) is the UI's (`docs/10` §3 item 4, WP-11).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::core::Core;
use crate::events::{Event, EventSink};
use crate::limits::{MAX_CLIPBOARD_BYTES, OSC_BUFFER_KEEP_BYTES};

/// Selection letters and digits OSC 52 may name.
const SELECTIONS: &[u8] = b"cpqs01234567";
/// Most selection characters of one request.
const MAX_SELECTION_CHARS: usize = 8;
/// What `Pc` defaults to when it is empty (xterm: `s0`).
const DEFAULT_SELECTION: &str = "s0";
/// The data part of a read request.
const QUERY: &[u8] = b"?";

fn valid_selection(selection: &[u8]) -> bool {
    selection.len() <= MAX_SELECTION_CHARS && selection.iter().all(|b| SELECTIONS.contains(b))
}

impl<S: EventSink> Core<S> {
    /// `OSC 52 ; Pc ; Pd`: `Pd` is base64 data to store, or `?` to read.
    pub(crate) fn osc_clipboard(&mut self, rest: &[u8]) {
        let Some(split) = rest.iter().position(|&b| b == b';') else { return };
        let selection = rest.get(..split).unwrap_or_default();
        let payload = rest.get(split + 1..).unwrap_or_default();
        if !valid_selection(selection) || payload.len() > MAX_CLIPBOARD_BYTES {
            return;
        }
        let selection = std::str::from_utf8(selection)
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_SELECTION);
        if payload == QUERY {
            self.sink.event(Event::ClipboardLoad { selection });
            return;
        }
        self.scratch.bytes.clear();
        if STANDARD.decode_vec(payload, &mut self.scratch.bytes).is_err() {
            return;
        }
        self.sink.event(Event::ClipboardStore { selection, bytes: &self.scratch.bytes });
        if self.scratch.bytes.capacity() > OSC_BUFFER_KEEP_BYTES {
            self.scratch.bytes = Vec::new();
        }
    }

    /// Answers a [`Event::ClipboardLoad`] the owner approved: the reply is `OSC 52 ; selection ;
    /// base64 ST`. The clipboard text is the owner's to give; the terminal only frames it.
    pub(crate) fn answer_clipboard(&mut self, selection: &str, clipboard: &[u8]) {
        let clipboard =
            clipboard.get(..clipboard.len().min(MAX_CLIPBOARD_BYTES * 3 / 4)).unwrap_or_default();
        let mut out = Vec::with_capacity(clipboard.len() * 4 / 3 + selection.len() + 16);
        out.extend_from_slice(b"\x1b]52;");
        out.extend_from_slice(selection.as_bytes());
        out.push(b';');
        out.extend_from_slice(STANDARD.encode(clipboard).as_bytes());
        out.extend_from_slice(b"\x1b\\");
        self.sink.event(Event::Reply(&out));
    }
}
