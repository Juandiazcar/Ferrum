# 0028. `ferrum-vt` design decisions and parse throughput

- Status: accepted (maintainer decisions Q-21 to Q-27, 2026-11-23, and Q-28, 2026-11-24;
  Q-21 amended the same day: decision 8; Q-25 amends Q-23 and Q-26 amends the Q-21 amendment)
- Date: 2026-11-23

## Context

WP-02 builds the terminal state machine (`docs/02` §3, `docs/12`). Several points of the spec
could not be followed to the letter, or needed a choice the spec leaves open. The first
implementation measured NFR-PERF-02 (parse at least 400 MB/s on plain ASCII and 120 MB/s on
SGR-heavy output, single core) without the scrollback push and proposed moving the push to the
WP-06 worker, and it measured the reflow of 10 000 lines above the 40 ms of `docs/03` §4 and
proposed deferring it. The maintainer rejected both proposals on 2026-11-23 (Q-21, Q-22): the
budgets stand as written and apply to the product configuration. With the push on, ASCII then
measured 253 MB/s on the M2, 44 % of it in LZ4; the maintainer amended Q-21 ("seal on the
worker", decision 8). This ADR records the decisions that remain and the measurements.

## Decisions

1. **Strings are captured or bounded by `ferrum-vt`, not left to `vte`.** `vte` 0.15 buffers an
   OSC without a bound in `std` mode and in 1 KiB in `no_std` mode; `docs/10` §3 item 9 requires
   1 MiB and no unbounded growth. The front end (`input.rs`) finds `ESC ]` with
   `memchr::memmem`, brings `vte` back to the ground state with `ESC \`, collects the payload
   into its own buffer and dispatches it. DCS, APC, PM, SOS, CSI and escape sequences stay in
   `vte` (it stores nothing for them). The watchdog of `docs/10` §3 item 9 ("capped and
   discarded", "the parser is reset after the cap"): an OSC payload may take `MAX_OSC_BYTES`
   (1 MiB), any other sequence or string `MAX_SEQUENCE_BYTES` (1 MiB, counted from its ESC). At
   the first byte over the cap the sequence is cancelled at that exact byte, whatever the reads:
   its buffer is freed, `vte` is reset, a DCS string is forgotten unanswered, and the rest of it
   is dropped without being stored up to the byte that would have ended it (its final byte, ST,
   BEL for an OSC, CAN or SUB; an ESC is kept, it starts the next sequence), so nothing of its
   payload (a sixel image, an OSC 1337 file, an over-long OSC 52) reaches the screen
   (`tests/watchdog.rs`). A DCS string is answered (DECRQSS, XTGETTCAP "not available") when
   `vte` leaves it at a terminator, also when that terminator is the ESC of a following OSC, and
   cancelled when a CAN or SUB ends it (`vte` executes it right after `unhook`), so the answer
   never depends on how the stream is cut (`tests/chunking.rs`). `vte` is built `default-features = false`. Parameters
   (32) and their saturation at `u16::MAX` are enforced by `vte`. An OSC 4 answers at most 256
   color queries (`MAX_COLOR_QUERIES`) and an OSC 52 payload is at most 1 MB (`MAX_CLIPBOARD_BYTES`,
   1 000 000 bytes, below the OSC cap so the check is reachable).
2. **ASCII runs, plain C0 controls and plain SGR bypass `vte`** when it is known to be in the
   ground state. `vte` does not expose its state, so the front end mirrors the families of the
   VT500 states it implements (escape, escape with intermediates, CSI, DCS, SOS/PM/APC;
   `input/sequence.rs`) from the same bytes: it knows where every sequence ends, also the ones
   that end without a dispatch (an ignored CSI, a CAN), and counts the bytes of the open one for
   the watchdog; text after a sequence is never counted. The state is `Ground`, `Utf8Tail` (a
   UTF-8 character may be pending: one byte at a time), `Sequence` (through `vte`) or `Discard`
   (after the cap). A property test (`tests/chunking.rs`)
   checks that the final state does not depend on how the stream is cut, down to single bytes.
3. **Hyperlinks and underline colors travel in the cell colors, and their ids are released when no
   row uses them (Q-24).** A cell has no room for them, the scrollback `Style` is `(fg, bg,
   attrs)`, and ids must survive in the scrollback. Six spare bits of each of `fg` and `bg` hold a
   12-bit `ExtraId` into a per-session table (4 095 records, 256 KiB of URIs). Because ids are
   persisted in scrollback rows, an id is handed out again only when nothing carries it. The
   history is never read back for this: every history push (scrolling and reflow alike, through
   `HistoryExtras::push`) ORs the ids of the line's runs into a 4 096-bit set (512 bytes) for its
   block of 1 024 pushed lines (the chunk size). The retained history is always the pushed lines
   `[pushed - len, pushed)`, so after each push the blocks entirely below the oldest retained line
   are dropped (their ring slots are reused: no steady-state allocation), and `CSI 3 J` and RIS
   drop them all. A cached union of the blocks is ORed on push and recomputed at the next sweep
   after a block was dropped. When the table is full, `Core::sweep_extras` marks the ids of both
   screens' cells, of every pen and saved cursor and the union, and releases the rest. This is
   conservative (an evicted line keeps its ids while its block still holds a retained line: at
   most the 1 023 evicted lines of the front block) and never short.
   A table whose ids are all in use refuses the new entry (the text shows without a link) and
   counts the refusal (`Extras::overflowed`). After a sweep that did `w` units of work (cells
   scanned plus 64-bit summary words) and freed `f` entries, the next `w / WORK_PER_REFUSAL - f`
   refusals (64 units each, saturating) do not sweep, so the amortized cost of an SGR 58 or OSC 8
   is bounded independently of the grid and history sizes, even for a stream where every new
   color frees exactly one id. The marking set is a 512-byte bitset on the stack, so a sweep
   allocates nothing and does no I/O; a refused link allocates nothing either (the room is checked
   before the key is copied). The summaries cost 512 bytes per block plus the 512-byte union: at
   most 5 632 bytes of heap per session at 10 000 lines. `tests/extras.rs` proves that reuse never
   re-points a row that still exists (screen and history, after reflow and eviction) and that the
   full-table path never reads the spill store; `tests/alloc_extras.rs` (dhat) that it allocates
   nothing. The grapheme table of a grid follows the same amortization rule (a compaction that
   scanned `C` cells and freed `f` clusters waits `C / WORK_PER_REFUSAL - f` refusals) and an
   allocation-free compaction (`Grid::intern_cluster`).
4. **Reflow streams through the scrollback with bounded memory.** `ferrum-scrollback` gained
   `pop_front` and `skip_front`; a width change pops the history oldest first, feeds the rows to
   a streaming re-cutter that keeps one output row and the cluster that straddles two input rows,
   and pushes the rows back, so sealed chunks are freed as they are consumed and there is never a
   second copy of the history (`tests/alloc_reflow.rs` bounds the peak for an 8 MB soft-wrapped
   line). A row that cannot be read is skipped with `skip_front`, the history keeps its order,
   the logical line it belonged to ends at the lost row (the text before and after it is never
   joined) and the first error is returned. A row whose text is over the scrollback's
   `MAX_LINE_TEXT_BYTES` (16 KiB; a full-width row of long clusters can hold 64 KiB) is stored with
   its text cut at a character boundary and without styles, and always counted, so stable line
   numbers never shift (`Scrollback::push_line`). Clusters are capped at `MAX_CLUSTER_BYTES` on every path. A shell
   mark on a continuation row stays with the output row that is open when that input row is fed.
   Trailing blanks of a soft-wrapped row are content: they are stored with an internal `WRAP_PAD`
   style so that the scrollback does not trim them.
5. **Alacritty's behavior wins, literally, where it disagrees with a first reading of xterm (Q-23,
   amended by Q-25)** (the imported tests are the conformance fixtures of FR-TERM-020; all 45 are
   imported and pass): DECALN does not move the cursor, EL/ED/ECH/ICH/DCH keep a pending wrap,
   the character sets are shared by both screens, each screen keeps its DECSC memory across
   alternate-screen visits, SGR 21 cancels bold (the `underline` test), and protected erase
   (DECSCA, DECSED, DECSEL) is not implemented (`selective_erasure`). RIS resets what
   `Term::reset_state` of `alacritty_terminal` 0.26.0 resets: the screens, the history
   (`grid_reset`), the modes, the tab stops, the character sets, the keyboard stacks, the cursor
   style and the title; like Alacritty it sends no event for the title and keeps the palette (the
   working directory, which Alacritty does not track, stays too). All are listed in
   `docs/user/compat.md` with the test that pins them.
6. **Title and icon reporting, ENQ, XTGETTCAP data and DECRQSS beyond `m`, `r` and `SP q` are not
   implemented** (`docs/10` §3 items 1 and 3). XTGETTCAP is answered "not available" so
   that programs do not wait.
7. **The grid is limited to 1 024 columns by 512 rows** (`docs/10` §3 item 11, a per-session
   limit that the spec did not name). A grid of that size is 8 MiB of 16-byte cells; the primary
   and the alternate screen together are 16 MiB, the worst a hostile resize can cost one session.
   `Size::new` clamps; `Size::exact` refuses a size outside the range and `Terminal::resize`
   returns the size that was applied, so the daemon can tell the UI when it differs from the
   request instead of letting the two disagree silently.

8. **Full history chunks are sealed on the worker (Q-21 amendment, 2026-11-23).** The encode and
   `Scrollback::push_line` stay on the parse path (the I/O thread), as Q-21 requires; only the
   LZ4 compression of a full hot chunk moves off it, which is where `docs/02` §5 already puts
   "compression of sealed chunks" ("compression/search receive immutable chunk handles", bounded
   channels). In `ferrum-scrollback`, a full hot chunk becomes an immutable `PendingChunk` (an
   `Arc` of the chunk buffers, no copy) and a pooled buffer becomes the next hot chunk, so the
   push path does not allocate. The owner hands it out with `take_pending`
   (`Terminal::take_pending_chunk`), the worker compresses it with `ChunkSealer::seal` and the
   owner installs the blob with `install` (`Terminal::install_sealed_chunk`); the crate spawns no
   thread (threads belong to the daemon, WP-06). Reads, search, `pop_front`, `clear`, eviction
   and spill see pending chunks; a late blob of a chunk that is gone is dropped, and spilling
   seals a pending chunk itself when it reaches one. Memory stays bounded (`docs/03` §3 rule 4,
   `docs/10` §3 item 11): at most `MAX_PENDING_CHUNKS` = 2 chunks wait (maintainer decision
   Q-26, amended 2026-11-23); then the push path seals inline (backpressure), counted in
   `MemoryReport::inline_seals`. Pending chunks and pooled buffers are reported as
   `pending_bytes`: at most two chunk buffers, 0.172 MB measured (0.086 MB in steady state with a
   worker that keeps up) and 0.18 MB by allowance, the "per session pending chunks" line of
   `docs/03` §2. Its first 0.09 MB came from the misc line (0.20 -> 0.11 MB) and the second
   0.09 MB from the sealed chunks line (0.45 -> 0.36 MB; the reference mix measures 0.338-0.343
   MB), so a session stays at 1.00 MB: 0.25 grid + 0.10 hot + 0.36 sealed + 0.18 pending + 0.11
   misc. A chunk dropped while the worker holds it keeps its buffer in the pool, reused once the
   worker lets go, so the push path never allocates a new one. A blob carries the identity of its
   scrollback and `install` refuses a foreign one; pooled buffers and the sealer scratch above
   `POOLED_BUFFER_KEEP_BYTES` are freed after use (`docs/03` §3 rule 7). Three pending chunks
   (0.26 MB measured, 0.30 MB by allowance) were the first proposal; Q-26 first kept one, with
   which the ASCII parse bench sealed about half of its chunks inline and missed 400 MB/s on the
   M2, and then two (see Measurements). The moved work is still gated: `scrollback_seal/
   fixture_chunks` has an absolute ceiling of 4.29 ms (2x the time to seal its 857 466 bytes at
   400 MB/s, since the worker must keep up with the parser). `tests/pending.rs` (model-based
   proptest, cap, eviction, clear, spill, a worker thread), `tests/memory_budget.rs` with and
   without a worker, and the `dhat` tests prove it: parse with the push on allocates nothing in
   steady state, the pending path included, and a seal allocates exactly its blob.

## Measurements

Machine: Apple M2, macOS, `cargo bench -p ferrum-vt --bench parse` and `cargo bench -p
ferrum-scrollback --bench scrollback -- scrollback_seal` (criterion medians). The machine was
shared with other builds (load average 5 to 16), so the last digit is noise. This is **not** the
reference machine: the verdict there is the maintainer's run on the i5-9400 (Q-27,
`docs/manual-verification.md` MV-WP02-1), **pending**. Inputs are 8 MiB from the seeded
`fixtures/output/` generator (profile Mixed, seed 1, ASCII lines; the unicode input is a stress
mix), screen 200x55, history 10 000 lines. With history on, the parse benches model the daemon:
64 KiB reads (`docs/02` §5), and between reads the I/O thread installs the blobs the worker sent
back and hands it the full chunks over bounded channels; the `_inline_seal` variants compress on
the parse thread instead.

| Bench | First build (M2, MiB/s) | Round-3 tip (M2, MiB/s; MB/s) | Target |
|---|---|---|---|
| `vt_parse/ascii_8mib` (push on, `MAX_PENDING_CHUNKS` = 2) | 72.0 | 418.5; 438.8 | 400 MB/s |
| `vt_parse/ascii_8mib_inline_seal` | | 265.3; 278.2 | information |
| `vt_parse/ascii_8mib_no_history` | 94.2 | 793.7; 832.3 | 400 MB/s |
| `vt_parse/sgr_heavy_8mib` (push on, `MAX_PENDING_CHUNKS` = 2) | 87.0 | 211.5; 221.8 | 120 MB/s |
| `vt_parse/sgr_heavy_8mib_inline_seal` | | 177.6; 186.3 | information |
| `vt_parse/sgr_heavy_8mib_no_history` | 115.7 | 335.9; 352.2 | 120 MB/s |
| `vt_parse/unicode_8mib` (push on) | 43.2 | 45.5; 47.8 | none |
| `vt_reflow/10k_lines_200_to_100_columns` | 36.8 ms | 3.44 ms | 40 ms |
| `scrollback_seal/fixture_chunks` | | 1.81 ms (451.7 MiB/s) | ceiling 4.29 ms |

The two push-on rows were measured with `MAX_PENDING_CHUNKS` = 2 (load average 11 to 16); the other
rows do not depend on the cap and were measured with one pending chunk at the same tip. Inline
seals with two pending chunks: `ascii_8mib` 5 286 of 64 550 chunks (8.2 %), `sgr_heavy_8mib` 7 of
16 261. With one pending chunk the same tip gave `ascii_8mib` 336.5 MiB/s (352.9 MB/s) with
23 952 of 49 663 chunks (48.2 %) sealed inline and `sgr_heavy_8mib` 216.9 MiB/s with 11 of
16 261; with three, the round-2 tip gave 505.5 MiB/s with 13 of 76 181. The cause is the
hand-over cadence, not the worker's speed (it seals at about 450 to 600 MiB/s): blobs are
installed and chunks handed out only between 64 KiB reads, and a chunk of these ASCII lines is
about 80 KB of input, so with fewer slots the next chunk is more often full before the previous
blob is back. Per session, `memory_report` (reference mix, seeds 1-3) gives sealed 0.338-0.343
MB and pending 0.086 MB with a worker that keeps up, within the new allowances
(`tests/memory_budget.rs` asserts them). The per-commit before/after numbers of the performance
work are in the commit messages (measured on this machine before the round-3 rebuild of the
stack, except `perf(vt): speed up the ASCII path`).

**Q-22 (reflow, 40 ms): met on this machine** (3.44 ms). **Q-21 with `MAX_PENDING_CHUNKS` = 2
(Q-26 amended): met on this machine**: ASCII 439 MB/s against 400, SGR-heavy 222 MB/s against
120. On the i5-9400: pending maintainer run (Q-27).

**Q-28 (2026-11-24), ASCII with the push on x86.** On GitHub-hosted x86 runners `vt_parse/ascii_8mib`
stayed under 400 MB/s, and each run gets a different CPU, so an absolute MB/s gate there is
meaningless. The x86 work was A/B tested over three rounds on those runners: the ring index without
a division (`grid.rs`) and the last-style cache plus the ASCII scalar-count shortcut
(`ferrum-scrollback` `style.rs`, `encode.rs`) gave +4 to 7 % and were kept; an 8-cell copy in
`encode_uniform`, a 16-byte aligned `Cell` and a direct return of the push result were neutral or
worse and were dropped. Best measured with the push on: 292 MiB/s (EPYC 9V74, Windows), 369 MiB/s
(Xeon 6975P-C, Linux), 406 MiB/s / 426 MB/s (Apple M2); without history 541 to 607 MiB/s on x86.
The maintainer accepted the measured ASCII-with-push result (Q-28); SGR-heavy (at least 120 MB/s)
and the reflow (at most 40 ms) remain met.

## Consequences

- The daemon (WP-06) wires decision 8: after each PTY read the I/O thread installs the blobs the
  worker sent back and hands it the session's pending chunks over bounded channels, and the worker
  owns one `ChunkSealer` for all sessions. A session whose chunks the worker does not seal in
  time seals inline (`inline_seals` counts it); memory never grows past `MAX_PENDING_CHUNKS`.

- The daemon (WP-06) treats `Terminal::resize` errors as "history may be shorter" (logged), never as
  a failed session, and reports the applied size to the UI.
- ExtraIds (and grapheme indices) are per-session and may be reused after a sweep: WP-03/WP-06
  must resolve ids to URIs/colors before rows leave the daemon; a UI or protocol row must never
  carry a raw ExtraId.
- `ferrum-scrollback` keeps `pop_front`, `skip_front` and `clear` public; the encoding of wrapped
  rows with `WRAP_PAD` is part of its contract with `ferrum-vt`.
