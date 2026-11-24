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
| RIS (`ESC c`) | resets the terminal | resets what Alacritty's `Term::reset_state` resets (Q-23, Q-25): also discards the scrollback and resets the cursor style; clears the title without sending an event; keeps the palette and the working directory | `alacritty_ref` `grid_reset`, `fixtures/vt/edit/ris__basic`, `fixtures/vt/edit/ris__cursor_style`, `tests/reset.rs` |
| DECSC memory of the alternate screen | one per screen | one per screen, kept while the screen is freed between visits; character sets are shared by both screens | `alacritty_ref` `saved_cursor_alt`, `fixtures/vt/altscreen/alt1049__cursor_saved_and_restored` |
| Alternate screen modes 47, 1047, 1049 | 47 keeps the contents between visits | all three start from a blank alternate screen, which is freed on exit | `fixtures/vt/altscreen/alt47__basic` |
| DECCOLM (`CSI ? 3 h/l`) | resizes the window | the terminal never resizes itself: it clears the screen, homes the cursor and resets the margins; the UI owns the size | `fixtures/vt/modes/deccolm__basic`, `alacritty_ref` `deccolm_reset` |
| Tab characters | cells stay blank | cells stay blank (Alacritty stores `\t`; the import reads it as a blank) | `alacritty_ref` `tab_rendering`, `vttest_tab_clear_set` |
| SGR 21 | double underline | cancels bold (the Alacritty reference test expects it; `4:2` is the double underline) | `alacritty_ref` `underline`, `fixtures/vt/sgr/cancel_bold__basic` |
| Hyperlink and underline-color ids | one table entry per distinct link | ids that no row uses (screen, alternate screen, history) are released when the table is full and reused; the history is not read back, a per-block summary of its ids is kept at push time; if all 4 095 are in use the new link shows as plain text and the refusal is counted; after a sweep, refusals are not swept again for a number proportional to the work the sweep did (`WORK_PER_REFUSAL`) | `tests/extras.rs`, `tests/alloc_extras.rs` |
| DCS answers | at the string terminator | DECRQSS and XTGETTCAP are answered only when the string ends with a terminator (ST, or an ESC that starts the next sequence, however the bytes are cut); CAN, SUB and the watchdog cancel it unanswered | `tests/strings.rs`, `tests/chunking.rs` |
| Terminal size | any | clamped to 1..=1 024 columns and 1..=512 rows (8 MiB of cells per grid); `Terminal::resize` returns the size applied and `Size::exact` refuses a size outside the range (ADR 0028) | `src/size.rs` tests |
| OSC 8 without an `id` | each link is distinct | the same URI is one link | `fixtures/vt/osc/osc8__id_and_semicolons` |
| Color names in OSC 4, 10, 11, 12 | X11 names accepted | only `rgb:` and `#` forms | `fixtures/vt/osc/osc4__malformed_specs`, `osc10__malformed_spec` |
| Width of a cluster | per implementation | `unicode-width` of the whole cluster (a spacing mark keeps its advance; an emoji modifier after a non-emoji joins its base) | `tests/unicode_vectors.rs`, `fixtures/vt/unicode/emoji_modifier__malformed_modifier_alone` |
| Over-limit input | varies | titles over 1 024 bytes, URIs over 4 096 bytes, clipboard over 1 MB, more than 32 parameters, clusters over 64 bytes, an OSC 4 asking for more than 256 colors and a hyperlink table whose ids are all in use are dropped or cut. An OSC over 1 MiB, and a DCS, APC, PM, SOS, CSI or escape sequence over 1 MiB, is cancelled at the first byte over the cap: its buffer is freed, a DCS is not answered, and the rest of it is discarded without being stored up to its terminator (its final byte, ST, BEL for an OSC, CAN or SUB; an ESC is kept and starts the next sequence), so nothing of the payload reaches the screen | `tests/watchdog.rs`, `tests/chunking.rs`, `tests/strings.rs`, `tests/osc.rs`, `tests/extras.rs`, `fixtures/vt/osc/title__malformed_too_long`, `fixtures/vt/sgr/params__malformed_too_many` |
| A history row over 16 KiB of text | not applicable | a full-width row of long clusters is kept in the history with its text cut to 16 KiB and without styles, and still counts as one line | `tests/print.rs` `a_row_of_long_clusters_still_counts_as_one_history_line`, `ferrum-scrollback/tests/roundtrip.rs` |
| Grapheme clusters | per implementation | a grid holds at most 4 096 distinct multi-scalar clusters; when full, the unused ones are released, at most once per `WORK_PER_REFUSAL` cells of work, and a mark that does not fit is dropped | `src/grid/clusters.rs` tests |
| 8-bit C1 controls | 8-bit mode switchable | the `vte` UTF-8 model: C1 scalars (U+0080 to U+009F) are executed as controls, raw 0x80 to 0x9F bytes are executed too; a raw C1 byte inside an unfinished CSI is ignored and the CSI completes, as in `vte` and Alacritty | `fixtures/vt/controls/ignored__malformed_c1_bytes`, `fixtures/vt/controls/c1_ind__malformed_inside_csi`, `fixtures/vt/controls/c1_ind__basic`, `c1_nel__basic`, `c1_hts__basic`, `c1_ri__basic` |
| Wide character in a one-column terminal | clipped | dropped | `fixtures/vt/print/wide__malformed_one_column` |
| Resize | reflow varies | width changes reflow the primary screen and the history, the cursor row stays on screen; the alternate screen is cut, not reflowed | `tests/reflow.rs` |

## Not implemented

| Sequence or feature | Behavior | Why | Test |
|---|---|---|---|
| Protected erase: DECSCA (`CSI Ps " q`), DECSED (`CSI ? Ps J`), DECSEL (`CSI ? Ps K`) | ignored; protected erase is not supported | Alacritty has none and its reference test expects the text to stay (maintainer decision Q-25) | `tests/alacritty_ref.rs` `selective_erasure` |
| Title and icon reports (`CSI 21 t`, `CSI 20 t`), title stack (`CSI 22/23 t`) | ignored | injection risk (`docs/10` §3 item 1) | `fixtures/vt/reports/dsr_status__malformed_unknown_report` |
| ENQ answerback | ignored | no user data in replies (`docs/10` §3 item 3) | `fixtures/vt/controls/ignored__basic` |
| XTGETTCAP | every query answered "not available" (`DCS 0 + r ST`) | no capability database | `fixtures/vt/dcs/xtgettcap__basic` |
| DECRQSS other than `m`, `r`, `SP q` | answered "invalid" | safe subset (`docs/10` §3 item 3) | `fixtures/vt/dcs/decrqss__malformed_unknown_and_long` |
| Mouse encodings 1005 and 1015, mouse mode 1016 | ignored | only X10/normal/button/any with SGR (FR-TERM-005) | `fixtures/vt/modes/mouse_unsupported__basic`, `mouse_unsupported__malformed_after_sgr_encoding` |
| Left and right margins (DECSLRM, mode 69), DECCRA and rectangular operations | ignored | not in FR-TERM | `fixtures/vt/modes/margins_unsupported__basic`, `margins_unsupported__malformed_deccra_ignored` (CSI s stays save cursor: `fixtures/vt/cursor/scosc_scorc__basic`) |
| `modifyOtherKeys` (`CSI > 4 ; n m`), DA3, DECLL | ignored | the UI encodes keys; kitty keyboard protocol is tracked | `fixtures/vt/reports/modify_other_keys__basic`, `fixtures/vt/reports/da3__basic`, `fixtures/vt/modes/decll__basic` |
| Sixel, kitty graphics, iTerm2 inline images (OSC 1337) | discarded without storing | graphics arrive in WP-15 | `fixtures/vt/dcs/unknown__ignored`, `apc_pm_sos__ignored` |
| OSC 9 notifications and other OSC 9 forms | ignored (only `9;4` progress) | out of scope | `fixtures/vt/osc/osc9_4__malformed` |
| Color names, `OSC 5`, special colors | ignored | out of scope | `fixtures/vt/osc/osc4__malformed_specs` |
