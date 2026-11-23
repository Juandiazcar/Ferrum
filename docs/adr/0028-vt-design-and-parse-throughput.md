# 0028. `ferrum-vt` design decisions and parse throughput

- Status: proposed (for maintainer approval with the WP-02 PRs)
- Date: 2026-11-22

## Context

WP-02 builds the terminal state machine (`docs/02` §3, `docs/12`). Several points of the spec
could not be followed to the letter, or needed a choice the spec leaves open. NFR-PERF-02
(parse at least 400 MB/s on plain ASCII and 120 MB/s on SGR-heavy output, single core) is met by
the state machine alone and not by the product configuration, which also pushes every scrolled-off
row into the scrollback. `docs/12` WP-02 allows an ADR with the gap and the plan in that case.

## Decisions

1. **OSC strings are captured by `ferrum-vt`, not by `vte`.** `vte` 0.15 buffers an OSC without a
   bound in `std` mode and in 1 KiB in `no_std` mode; `docs/10` §3 item 9 requires 1 MiB and no
   unbounded growth. The front end (`input.rs`) finds `ESC ]` with `memchr::memmem`, brings `vte`
   back to the ground state with `ESC \`, collects the payload into its own buffer capped at
   1 MiB (released after a big one), and dispatches it. `vte` is built `default-features = false`
   (the `no_std` mode with a 1-byte OSC buffer it never uses). Parameters (32) and their
   saturation at `u16::MAX` are enforced by `vte`. CAN and SUB cancel an OSC without dispatching it.
2. **ASCII runs and plain C0 controls bypass `vte`** when it is known to be in the ground state
   (it leaves it only through ESC; a CSI or ESC dispatch proves it came back). The state is
   tracked as `Ground`, `Utf8Tail` (a UTF-8 character may be pending: one byte at a time) or
   `Unknown` (inside a sequence: through `vte`). A property test (`tests/chunking.rs`) checks that
   the final state does not depend on how the stream is cut, down to single bytes; it found three
   bugs of this design while it was written (a `print` or `execute` callback cannot prove the
   ground state because `vte` decodes a whole text run in one call).
3. **Hyperlinks and underline colors travel in the cell colors.** A cell has no room for them, the
   scrollback `Style` is `(fg, bg, attrs)`, and ids must survive in the scrollback. Six spare bits
   of each of `fg` and `bg` hold a 12-bit `ExtraId` into a per-session table. Because ids are
   persisted in scrollback rows they are never reused: the table (4 095 records, 256 KiB of
   URIs) refuses new entries when full instead of evicting (an eviction would re-point old rows to
   another URI). Text then shows without a link, and the refusal is counted.
4. **Reflow streams through the scrollback.** `ferrum-scrollback` gained `pop_front`; a width
   change pops the history oldest first, joins soft-wrapped rows, cuts them at the new width and
   pushes them back, so sealed chunks are freed as they are consumed and there is never a second
   copy of the history. The last rows become the screen; the cursor row keeps its place on the
   screen when the content allows. Trailing blanks of a soft-wrapped row are content: they are
   stored with an internal `WRAP_PAD` style so that the scrollback does not trim them.
5. **Alacritty reference tests win where they disagree with a first reading of xterm** (the
   imported tests are the conformance fixtures of FR-TERM-020): DECALN does not move the cursor,
   EL/ED/ECH/ICH/DCH keep a pending wrap, RIS drops the scrollback, the character sets are shared
   by both screens, each screen keeps its DECSC memory across alternate-screen visits. Two tests
   are not imported with reasons in `fixtures/vt/THIRD_PARTY.md`. All are in `docs/user/compat.md`.
6. **Title and icon reporting, ENQ, XTGETTCAP data and DECRQSS beyond `m`, `r`, `SP q`, `" q`
   are not implemented** (`docs/10` §3 items 1 and 3). XTGETTCAP is answered "not available" so
   that programs do not wait.

## Measurements (Windows 11, i5-9400, `cargo xtask bench`, release profile, medians; 8 MiB inputs)

Three runs of `cargo bench -p ferrum-vt --bench parse -- vt_parse` (30 samples each, medians
of the criterion confidence interval; 1 MiB/s = 1.0486 MB/s), plus the committed baseline
(`benches/baselines/windows.json`, 100 samples):

| Bench | Runs 1 / 2 / 3 (MiB/s) | In MB/s | Target | Verdict |
|---|---|---|---|---|
| `vt_parse/ascii_8mib_no_history` | 400.7 / 394.5 / 391.2 | 420 / 414 / 410 | 400 MB/s | met |
| `vt_parse/sgr_heavy_8mib_no_history` | 123.5 / 120.4 / 121.3 | 130 / 126 / 127 | 120 MB/s | met |
| `vt_parse/ascii_8mib` (scrollback push on) | 152.6 / 151.3 / 151.7 | 160 / 159 / 159 | 400 MB/s | **not met** |
| `vt_parse/sgr_heavy_8mib` (scrollback push on) | 79.0 / 80.1 / 79.7 | 83 / 84 / 84 | 120 MB/s | **not met** |
| `vt_parse/unicode_8mib` (scrollback push on) | 29.3 / 29.0 / 29.0 | 31 / 30 / 30 | none | information |
| `vt_reflow/10k_lines_200_to_100_columns` | 58.7 ms (baseline) | | 40 ms (`docs/03` §4) | **not met** |

The inputs are 8 MiB of 120-column lines (ASCII), 14 SGR changes per line (SGR-heavy) and mixed
Latin, CJK and emoji text; the screen is 200x55. The "no history" variants use a scrollback of
0 lines: rows still scroll, only the encode and the push are skipped.

## Consequences (implementation requirements)

- **The gap with the scrollback push.** Every row that scrolls off costs the encode (about
  1 ns per cell) and `Scrollback::push_line` (about 285 ns per row, WP-01's own bench). With
  120-byte lines that is about 60 % of the time of `vt_parse/ascii_8mib` (51 ms against 20 ms); shorter lines make it
  worse. NFR-PERF-02 is measured without the push, which is what a "parse" bench of the state
  machine measures; the daemon (WP-06) must do the push off the I/O thread: batch the scrolled
  rows in a bounded queue and push them from the worker of `docs/02` §5 (the same worker that
  seals and spills), so the I/O thread's parse rate is the no-history rate. WP-06 re-measures the
  daemon's end-to-end rate and the WP-08 gate decides; this ADR does not relax NFR-PERF-02.
- **Reflow of 10 000 lines is slower than the 40 ms of `docs/03` §4** (58.7 ms here). The
  budget in `docs/03` says "interruptible, UI keeps painting": `Core::reflow_primary` is a stream
  over rows and can yield at logical-line boundaries; WP-06 runs it on the worker with a time
  budget (`docs/02` §3.2) and WP-17 tunes it (a pooled `OutRow` allocation per row is the first
  cost). Until then the bench ceiling is the 2x target (80 ms) of `docs/08` §8.
- The daemon must treat `Terminal::resize` errors as "history may be shorter" (logged), never as
  a failed session.
- `ferrum-scrollback` keeps `pop_front` public; the encoding of wrapped rows with `WRAP_PAD` is
  part of its contract with `ferrum-vt` and must not be changed independently.
