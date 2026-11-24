# ferrum-vt

**Responsibility:** the VT state machine on top of `vte`: grid, modes, charsets, SGR, OSC, DCS,
alternate screen, scroll regions, hyperlinks, shell marks, reflow with the scrollback, the
selection model, damage tracking and the event sink (`docs/02` §3).

**Status:** implemented (WP-02).

| | |
|---|---|
| Allowed internal dependencies | `ferrum-scrollback` |
| Must never depend on | `ferrum-daemon`, `ferrum-pty`, `ferrum-platform`, `ferrum-proto`, any OS or socket code |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `vte` (parser only), `unicode-width`, `unicode-segmentation`, `bitflags`, `memchr`, `base64` (OSC 52), `rustc-hash`, `thiserror`, `tracing` |
| Dev-dependencies | `insta`, `proptest`, `dhat`, `criterion` |

## Model

- `Cell` is 16 bytes (`const` size assert). Colors are packed `u32`s whose six spare bits in each of
  `fg` and `bg` carry a 12-bit `ExtraId` (hyperlink and underline color, interned in `Extras`), so a
  scrollback `Style` of `(fg, bg, attrs)` needs no conversion. Multi-scalar clusters live in the
  per-grid `GraphemeTable` (compacted when full). Every width comes from `width.rs`.
- `Grid` is a ring of rows sized to the screen. A whole-screen scroll advances the ring start.
  Rows that scroll off the top of the primary screen are encoded into `ferrum_scrollback::Scrollback`
  (`codec.rs`); trailing blanks of a soft-wrapped row are kept with the internal `WRAP_PAD` style.
- `Terminal<S: EventSink>` owns the parser front end (`input.rs`), both screens, the scrollback and
  the sink. The alternate screen is allocated on entry and freed on exit.
- `input.rs` is the byte-stream front end: bulk printing of ASCII runs, capture of OSC strings with
  a 1 MiB cap (`vte` never sees an OSC), everything else through `vte` into `perform.rs`, `csi.rs`,
  `esc.rs`, `osc*.rs`, `dcs.rs`, `reports.rs`, `sgr.rs`, `dec_modes.rs`.
- Reflow (`reflow.rs`, `rewrap.rs`, `fill.rs`): a change of width pops the history oldest first,
  joins soft-wrapped rows, cuts them at the new width and pushes them back; the last rows become
  the screen and the cursor row keeps its place.
- Events (`Event`) borrow from the terminal and never allocate; replies for the PTY are fixed text
  plus terminal-owned numbers (`Event::Reply`).

## Public API

| Item | Purpose |
|---|---|
| `Terminal::new(Size, Scrollback, sink)` | create a terminal |
| `parse(&[u8])` | feed program output (never panics, never fails) |
| `resize(Size)` | resize with reflow |
| `grid()`, `screen()`, `cursor()`, `modes()`, `scrollback()`, `palette()`, `extras()`, `title()`, `cwd()`, `keyboard_flags()`, `cursor_style()` | read the state |
| `take_pending_chunk()` / `install_sealed_chunk(SealedBlob)` | hand full history chunks to the worker between reads and put the sealed blobs back (`docs/02` §5, ADR 0028 decision 8) |
| `damage()` / `take_damage()` | dirty rows since the last take |
| `select_word`, `select_logical_line`, `selection_text`, `screen_line` | selection model (`Selection`, `Point`, `SelectionKind`) |
| `answer_clipboard_request`, `set_palette` | owner decisions fed back |
| `dump()`, `logical_text()` | text renderings for snapshots and diagnostics |
| `CellStyle::from_style` | decode a scrollback `Style` |
| `EventSink`, `Event`, `NullSink` | everything that is not grid state |

## Limits (`docs/10` §3 items 9 and 11, `limits.rs`)

32 parameters, values saturate at `u16::MAX`; OSC 1 MiB; title 1 024 bytes; OSC 52 payload 1 MB;
URI 4 096 bytes; 4 095 hyperlink/underline-color records and 256 KiB of URIs per session (ids no
row uses are released, `extras_sweep.rs`, from per-block summaries of the history,
`history_extras.rs`, never by reading it back); grapheme table 4 096 clusters per grid, clusters 64 bytes;
256 answered color queries per OSC 4; size 1 to 1 024 columns by 1 to 512 rows (`Terminal::resize`
returns the applied size); an unterminated OSC, DCS, APC, PM, SOS or CSI is reset after 1 MiB.

## Tests

`cargo test -p ferrum-vt`: unit tests, `tests/fixtures.rs` (every `fixtures/vt/<topic>/<case>.in`
against its insta snapshot; every sequence has a positive and a malformed case),
`tests/alacritty_ref.rs` (43 imported Alacritty reference tests), `tests/unicode_vectors.rs`,
`tests/reflow.rs` (AC-TERM-02 property), `tests/chunking.rs` (the state does not depend on how the
stream is cut), `tests/alloc_parse.rs` and `tests/alloc_extras.rs` (`dhat`: zero allocations in steady state; the first use of a new grapheme cluster or hyperlink allocates once, bounded by the table caps (`MAX_GRAPHEMES`, `MAX_EXTRAS`, `MAX_LINK_TABLE_BYTES`); a full extras table under a hostile SGR 58 / OSC 8 stream allocates nothing; with the scrollback push on and the history past its limit, full chunks sealed between 64 KiB reads as the daemon worker does),
`tests/strings.rs`, `tests/extras.rs`, `tests/fuzz_seeds.rs`. Fuzz: `crates/ferrum-vt/fuzz` (`cargo +nightly fuzz run parse`). Benches:
`cargo bench -p ferrum-vt` or `cargo xtask bench`. Deviations from xterm: `docs/user/compat.md`.
