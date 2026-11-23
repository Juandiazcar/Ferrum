# Compatibility notes: where Ferrum differs from xterm

Ferrum targets `xterm-256color` (FR-TERM-001). Every deliberate difference, every sequence that is
not implemented and every choice that follows the reference tests of another terminal is listed
here with the test that pins it (`docs/05` §9). Paths are relative to the repository root;
`fixtures/vt/<topic>/<name>__<variant>` is a fixture (`.in` bytes, `.md` annotation, `.snap`
snapshot).

## Deviations

| Behavior | xterm | Ferrum | Test |
|---|---|---|---|
| DECALN (`ESC # 8`) | fills with `E` and homes the cursor (as far as known) | fills with `E` in the default style; the cursor and the margins stay | `tests/alacritty_ref.rs` `decaln_reset`, `fixtures/vt/edit/decaln__basic` |
| EL / ED with a pending wrap | erases from the cursor cell | EL and ED do not cancel a pending wrap; EL 0 with a pending wrap does nothing | `alacritty_ref` `erase_in_line`, `fixtures/vt/erase/el__mode0` |
| ECH, ICH, DCH with a pending wrap | cancel it | keep it | `alacritty_ref` `erase_chars_reset`, `insert_blank_reset`, `delete_chars_reset` |
| RIS (`ESC c`) | resets the terminal | also discards the scrollback | `alacritty_ref` `grid_reset`, `fixtures/vt/edit/ris__basic` |
| DECSC memory of the alternate screen | one per screen | one per screen, kept while the screen is freed between visits; character sets are shared by both screens | `alacritty_ref` `saved_cursor_alt`, `fixtures/vt/altscreen/alt1049__cursor_saved_and_restored` |
| Alternate screen modes 47, 1047, 1049 | 47 keeps the contents between visits | all three start from a blank alternate screen, which is freed on exit | `fixtures/vt/altscreen/alt47__basic` |
| DECCOLM (`CSI ? 3 h/l`) | resizes the window | the terminal never resizes itself: it clears the screen, homes the cursor and resets the margins; the UI owns the size | `fixtures/vt/modes/deccolm__basic`, `alacritty_ref` `deccolm_reset` |
| Tab characters | cells stay blank | cells stay blank (Alacritty stores `\t`; the import reads it as a blank) | `alacritty_ref` `tab_rendering`, `vttest_tab_clear_set` |
| SGR 21 | double underline | double underline (Alacritty's reference test expects "bold off" and is not imported) | `fixtures/vt/sgr/underline_double__basic` |
| Selective erase (DECSCA, DECSED, DECSEL) | implemented | implemented (Alacritty's reference test `selective_erasure` is not imported) | `fixtures/vt/decscusr/decsca__basic`, `fixtures/vt/erase/decsed__basic` |
| OSC 8 without an `id` | each link is distinct | the same URI is one link | `fixtures/vt/osc/osc8__id_and_semicolons` |
| Color names in OSC 4, 10, 11, 12 | X11 names accepted | only `rgb:` and `#` forms | `fixtures/vt/osc/osc4__malformed_specs`, `osc10__malformed_spec` |
| Width of a cluster | per implementation | `unicode-width` of the whole cluster (a spacing mark keeps its advance; an emoji modifier after a non-emoji joins its base) | `tests/unicode_vectors.rs`, `fixtures/vt/unicode/emoji_modifier__malformed_modifier_alone` |
| Over-limit input | varies | OSC over 1 MiB, titles over 1 024 bytes, URIs over 4 096 bytes, clipboard over 1 MB, more than 32 parameters, clusters over 32 bytes and a full hyperlink table are dropped or cut | `tests/chunking.rs`, `fixtures/vt/osc/title__malformed_too_long`, `fixtures/vt/sgr/params__malformed_too_many` |
| 8-bit C1 controls | 8-bit mode switchable | the `vte` UTF-8 model: C1 scalars (U+0080 to U+009F) are executed as controls, raw 0x80 to 0x9F bytes are executed too | `fixtures/vt/controls/ignored__malformed_c1_bytes` |
| Wide character in a one-column terminal | clipped | dropped | `fixtures/vt/print/wide__malformed_one_column` |
| Resize | reflow varies | width changes reflow the primary screen and the history, the cursor row stays on screen; the alternate screen is cut, not reflowed | `tests/reflow.rs` |

## Not implemented

| Sequence or feature | Behavior | Why | Test |
|---|---|---|---|
| Title and icon reports (`CSI 21 t`, `CSI 20 t`), title stack (`CSI 22/23 t`) | ignored | injection risk (`docs/10` §3 item 1) | `fixtures/vt/reports/dsr_status__malformed_unknown_report` |
| ENQ answerback | ignored | no user data in replies (`docs/10` §3 item 3) | `fixtures/vt/controls/ignored__basic` |
| XTGETTCAP | every query answered "not available" (`DCS 0 + r ST`) | no capability database | `fixtures/vt/dcs/xtgettcap__basic` |
| DECRQSS other than `m`, `r`, `SP q`, `" q` | answered "invalid" | safe subset (`docs/10` §3 item 3) | `fixtures/vt/dcs/decrqss__malformed_unknown_and_long` |
| Mouse encodings 1005 and 1015, mouse mode 1016 | ignored | only X10/normal/button/any with SGR (FR-TERM-005) | `fixtures/vt/modes/mouse_exclusive__basic` |
| Left and right margins (DECSLRM, mode 69), DECCRA and rectangular operations | ignored | not in FR-TERM | `fixtures/vt/cursor/scosc_scorc__basic` (CSI s is save cursor) |
| `modifyOtherKeys` (`CSI > 4 ; n m`), DA3, DECLL | ignored | the UI encodes keys; kitty keyboard protocol is tracked | `fixtures/vt/reports/da2__malformed_intermediate` |
| Sixel, kitty graphics, iTerm2 inline images (OSC 1337) | discarded without storing | graphics arrive in WP-15 | `fixtures/vt/dcs/unknown__ignored`, `apc_pm_sos__ignored` |
| OSC 9 notifications and other OSC 9 forms | ignored (only `9;4` progress) | out of scope | `fixtures/vt/osc/osc9_4__malformed` |
| Color names, `OSC 5`, special colors | ignored | out of scope | `fixtures/vt/osc/osc4__malformed_specs` |
