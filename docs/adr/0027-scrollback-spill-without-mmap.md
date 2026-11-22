# 0027. Scrollback spill without mmap

- Status: accepted
- Date: 2026-11-22 (maintainer approved 2026-11-22)

## Context

`docs/01` FR-TERM-011 and `docs/02` §3.2 say that cold scrollback history spills to disk "(mmap)":
sealed chunks are written to a spill file in the run directory and "reading faults them back".
`docs/07` lists `memmap2` as a `scrollback` dependency for the spill file.

`ferrum-scrollback` is a pure foundation crate with `#![forbid(unsafe_code)]` (`docs/05` §4).
Mapping a file is `unsafe` (`memmap2::Mmap::map` is an `unsafe fn`: another process or a
truncation can change the mapped bytes under a safe `&[u8]`). The spill file is exactly that case:
it lives in a directory other processes of the same user can reach, and a disk-full or truncation
event must never become undefined behavior in the daemon (NFR-REL-01).

## Decision

The spill file implementation lives in `ferrum-scrollback` behind the `SpillStore` trait
(`docs/04` §2, `docs/05` §2) and does **not** use mmap:

- sealed chunks are written to the spill file with plain positional writes and read back with plain
  positional reads: `std::os::unix::fs::FileExt::{read_exact_at, write_all_at}` on Unix and
  `std::os::windows::fs::FileExt::{seek_read, seek_write}` on Windows, behind one tiny `cfg`-ed
  helper module;
- no `unsafe` and no new dependency (`memmap2` stays in the workspace table for fonts in
  `ferrum-render`, where it is used through `fontdb`);
- the file is created on the first spill (`docs/03` §3 rule 11), is owner-only on Unix, and is
  removed when the store is dropped;
- the on-disk space is reused (FIFO ring of records) so a long session does not grow the file past
  the per-session disk cap (`docs/10` §3 item 11, default 256 MB);
- a read copies the record into a caller-owned buffer, so a spilled chunk costs resident memory
  only while it is being decoded (one chunk, about 40 KB compressed), the same property mmap
  provided with page faults.

## Consequences

- Reading a cold chunk is a `read` syscall plus an LZ4 decompress instead of a page fault; the
  resident cost is the same bounded buffer. The cost is one copy of about 40 KB per cold chunk read.
  WP-01 benches the spilled read path.
- `docs/01` FR-TERM-011, `docs/02` §3.2 and `docs/07` (the `memmap2` row) are reworded in the same
  change; FR-TERM-011 keeps its numbers (default 10 000, min 0, max 1 000 000, spill beyond 50 000).
- The `SpillStore` trait keeps the seam open: a mapped implementation can be added later in a crate
  that allows `unsafe` if a measurement ever shows the copy matters (a new ADR would be needed).
- Platform behavior (positional I/O differs per OS) is exercised by the shared `SpillStore`
  contract suite in `ferrum-testkit`, run on all three OS in CI.
