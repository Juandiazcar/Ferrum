# Third-party material in `fixtures/vt`

## Alacritty reference tests (`fixtures/vt/alacritty/`)

- **Source:** `alacritty_terminal` 0.26.0, `tests/ref/<name>/` (`alacritty.recording`, `size.json`,
  `config.json`, `grid.json`), <https://github.com/alacritty/alacritty>.
- **License:** Apache License 2.0 (the full text is in `LICENSE-APACHE` of that crate and of this
  repository's `LICENSE-APACHE`). Copyright (c) the Alacritty contributors.
- **What was changed:** the recordings are copied byte for byte (`<name>.recording`). The expected
  grids (0.1 to 1 MB of JSON each) were rewritten by `cargo xtask import-alacritty-refs` into one
  text line per row plus one line per run of styled clusters (`<name>.expected`); the format is
  described in `xtask/src/import_refs.rs`. The conversion drops the spacer cells of wide
  characters (the cluster carries its width), omits never-filled history rows at the top, and
  reads the tab character that Alacritty stores in the first cell a tab crosses as a blank
  (Ferrum leaves the cells blank, so tab stops are still tested by the position of the text).
- **Runner:** `crates/ferrum-vt/tests/alacritty_ref.rs` feeds each recording to a terminal of the
  recorded size, once whole and once in 7-byte reads, and compares history and screen.

45 reference tests exist upstream; 43 imported, 2 excluded.

### Excluded tests

| Test | Reason |
|---|---|
| `selective_erasure` | Alacritty does not implement DECSCA, DECSED and DECSEL: its expected grid keeps the text of `ESC [ ? 2 J` and `ESC [ ? 2 K`. Ferrum implements them as xterm does (the unprotected cells are erased); covered by `fixtures/vt/decscusr/decsca__*` and `fixtures/vt/erase/decsed__*`, `decsel__*`. |
| `underline` | One of its rows feeds `ESC [ 4:3;21 m` and expects SGR 21 to cancel bold (Alacritty's reading). xterm, and Ferrum, treat SGR 21 as a double underline; covered by `fixtures/vt/sgr/underline_double__*`; the other underline sequences it feeds (`4`, `4:n`, `24`) are covered by `fixtures/vt/sgr/underline__*` and `sgr/underline_styles__*`. |

### Where Ferrum follows the Alacritty tests instead of its first reading of xterm

These are recorded in `docs/user/compat.md` with their tests: DECALN leaves the cursor and the
margins alone (`decaln_reset`), EL and ED do not cancel a pending wrap and EL 0 with a pending wrap
does nothing (`erase_in_line`), RIS discards the scrollback (`grid_reset`), each screen keeps its own
DECSC memory across alternate-screen visits and the character sets are shared by both screens
(`saved_cursor_alt`).
