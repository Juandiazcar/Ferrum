# ferrum-scrollback

**Responsibility:** compact line encoding, style interning, chunked LZ4 compression and disk spill behind a trait.

**Status:** implemented (WP-01). Reflow is `ferrum-vt` (WP-02) and is not in this crate.

| | |
|---|---|
| Allowed internal dependencies | none (foundation) |
| Must never depend on | any other ferrum crate, OS-specific code (the spill file uses only positional I/O from `std`, ADR 0027) |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `lz4_flex` (sealing), `thiserror` (errors), `tracing` (one warning when spilling is disabled) |
| Dev-dependencies | `proptest`, `dhat` |

## Model

A row that scrolled off the grid is pushed as `(flags, text, style runs)` and encoded as
`flags:u8 | text_len | utf8 text | run_count | {style_id | run_len}*` (varints, `docs/02` §3.2).
Trailing default blanks are trimmed; equal neighbor runs are merged; `run_len` counts Unicode
scalars. Wide characters and combining sequences are stored as scalars: the consumer derives column
widths from the text with its width table.

```text
oldest                                                              newest
[ spilled chunks (SpillStore) ][ sealed chunks, LZ4, in RAM ][ hot chunk, raw, <= 1 024 lines ]
```

- **Hot chunk:** at most `HOT_CHUNK_LINES` (1 024) lines. Its buffer grows in exact 16 KiB steps
  (never by doubling) so it stays within 0.10 MB (ADR 0024); its line-offset index is 4 KiB.
- **Sealed chunk:** one `Box<[u8]>` = header + delta-encoded line index + LZ4 block. The same
  bytes are the spill record. `decode_chunk` validates and decompresses it and is the fuzz target.
- **Spill:** beyond `spill_threshold_lines` (default 50 000) the oldest sealed chunks go to a
  `SpillStore`. `FileSpill` is a FIFO ring over one file (created on first use, owner-only on Unix,
  removed on drop, never larger than its cap, default 256 MiB per `docs/10` §3 item 11). Disk cap
  reached: the oldest spilled chunks are dropped. Store error (disk full): spilling is disabled for
  the session, history is capped at the threshold, and `take_event()` reports it once.
  Without a store, history is capped at the threshold too.
- **Limits:** `max_lines` 0 to 1 000 000, at most 8 192 distinct styles per session (beyond that new
  styles are stored as the default style and counted), encoded line at most 64 KiB.
- **Hot path:** `push_line` allocates nothing in steady state except the sealed blob once per
  1 024 lines (the retained data itself), a new style the first time it is seen, and the hot buffer's
  next 16 KiB step until it reaches its working size (`tests/alloc_push_line.rs`).
- Sealing and spill I/O run on the thread that calls `push_line`; the daemon decides whether to move
  a session's scrollback behind its worker (WP-06). `Scrollback` is `Send`.

## Public API

| Item | Purpose |
|---|---|
| `Scrollback::new` / `with_spill` | create with a `ScrollbackConfig` (and a `Box<dyn SpillStore>`) |
| `push_line(&LineRef)` | append one row (hot path) |
| `read_line(index, &mut LineCursor)` | random access, 0 = oldest retained line |
| `search(matcher, Direction, start)` | iterator of `LineMatch` over decoded lines; `LineMatcher` is the regex hook, `LiteralMatcher` does substring with a case toggle (matching is per row) |
| `memory_report()` | capacity-based bytes by category: hot chunk, sealed, misc, shared scratch, spill disk |
| `take_event()`, `spill_state()`, `dropped_lines()` | spill and eviction status |
| `SpillStore`, `FileSpill`, `MemorySpill` | the seam, the file ring, the in-memory fake with failure injection |
| `decode_chunk`, `ChunkBuf` | validate and read a sealed blob (never panics, never over-allocates) |

## Tests and benches

`tests/roundtrip.rs` (proptest round trip, sealing, random access), `tests/spill.rs`,
`tests/search.rs`, the shared `SpillStore` contract suite (`ferrum-testkit/tests/spill_contract.rs`), unit tests next to
the code. The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced
by `cargo xtask check-deps` from `xtask/deps-rules.toml`.
