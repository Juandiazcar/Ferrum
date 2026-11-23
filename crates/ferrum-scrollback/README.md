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
[ spilled chunks (SpillStore) ][ sealed (LZ4) and pending (raw) chunks, in RAM ][ hot chunk, raw ]
```

- **Hot chunk:** at most `HOT_CHUNK_LINES` (1 024) lines. Its buffer grows in exact 16 KiB steps
  (never by doubling) so it stays within 0.10 MB (ADR 0024); its line-offset index is 4 KiB.
- **Pending chunk:** a full hot chunk becomes an immutable `PendingChunk` handle (no copy; a pooled
  buffer becomes the next hot chunk). The owner hands it out with `take_pending()`, a worker
  compresses it with `ChunkSealer::seal` (any thread; the crate spawns none) and the owner puts the
  blob in place with `install()` (`docs/02` §5). Reads, search, `pop_front`, `clear`, eviction and
  spill see pending chunks; spilling seals one inline if it reaches it first. At most
  `MAX_PENDING_CHUNKS` (2) wait: then the push path seals inline (backpressure, counted in
  `MemoryReport::inline_seals`). Pending chunks and pooled buffers are reported as
  `pending_bytes`: at most two chunk buffers (0.172 MB measured, 0.18 MB allowance, Q-26). A
  buffer whose chunk is dropped while the worker holds it is reused once the worker lets go.
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
- **Hot path:** `push_line` allocates nothing in steady state, the push that retires a full chunk
  included, except a new style the first time it is seen and the hot and pooled buffers' 16 KiB
  steps until they reach their working size. `ChunkSealer::seal` allocates the blob (the retained
  data itself) and nothing else; an inline seal does the same (`tests/alloc_push_line.rs`).
- Spill I/O runs on the thread that calls `push_line`; compression runs wherever the owner calls
  `ChunkSealer::seal` (the daemon worker, WP-06). `Scrollback` is `Send`.

## Public API

| Item | Purpose |
|---|---|
| `Scrollback::new` / `with_spill` | create with a `ScrollbackConfig` (and a `Box<dyn SpillStore>`) |
| `push_line(&LineRef)` | append one row (hot path) |
| `take_pending()`, `install(SealedBlob)`, `ChunkSealer::seal(PendingChunk)` | hand full chunks to the worker, compress them there, put the blobs back |
| `read_line(index, &mut LineCursor)` | random access, 0 = oldest retained line |
| `pop_front(&mut LineCursor)` | removes and returns the oldest line; lets `ferrum-vt` rewrite history for reflow by popping the front and pushing at the back, freeing chunks as it goes |
| `search(matcher, Direction, start)` | iterator of `LineMatch` over decoded lines; `LineMatcher` is the regex hook, `LiteralMatcher` does substring with a case toggle (matching is per row) |
| `memory_report()` | capacity-based bytes by category: hot chunk, sealed, pending, misc, shared scratch, spill disk; inline seals |
| `take_event()`, `spill_state()`, `dropped_lines()` | spill and eviction status |
| `SpillStore`, `FileSpill`, `MemorySpill` | the seam, the file ring, the in-memory fake with failure injection |
| `decode_chunk`, `ChunkBuf` | validate and read a sealed blob (never panics, never over-allocates) |

## Fuzzing, benches

- `fuzz/` (own workspace, nightly toolchain, `cargo install cargo-fuzz@0.13.2`):
  `cd crates/ferrum-scrollback && cargo +nightly fuzz run decode_chunk`. The corpus is committed
  (`fuzz/corpus/decode_chunk`, seeds regenerated by `cargo test -p ferrum-scrollback write_fuzz_seeds -- --ignored`).
  `.github/workflows/nightly.yml` runs it 10 minutes per night, plus Miri, ASan and the benches.
- `benches/scrollback.rs` (criterion): push, sequential/random/spilled reads, search. Compare with
  `cargo xtask bench` (maintainer baselines in `benches/baselines/`) or `cargo xtask bench --against
  HEAD~1` (previous commit, same machine, what CI does). `cargo run --release -p ferrum-scrollback
  --example memory_report` prints the per-session memory (`dhat` next to `memory_report()`).

## Tests

`tests/roundtrip.rs` (proptest round trip, sealing, random access), `tests/pending.rs` (pending
chunks: model-based proptest, cap, eviction, clear, spill, a worker thread), `tests/spill.rs`,
`tests/search.rs`, the shared `SpillStore` contract suite (`ferrum-testkit/tests/spill_contract.rs`), unit tests next to
the code. The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced
by `cargo xtask check-deps` from `xtask/deps-rules.toml`.
