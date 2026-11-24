# ferrum-proto

**Responsibility:** the IPC protocol between the UI (`ferrum`) and the daemon (`ferrumd`):
message types, framing, protocol versioning and the row encoding of damage (`docs/02` §4).

**Status:** WP-03 in progress: the row encoding is implemented; framing, versioning and the
messages follow.

| | |
|---|---|
| Allowed internal dependencies | none (foundation) |
| Must never depend on | any other ferrum crate, fonts or rendering |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `thiserror` |
| Dev-dependencies | `proptest`, `dhat` |
| Features | none |

## Model

- **Rows** (`row.rs`, `row_decode.rs`, `links.rs`): one screen row is style runs over a cell
  stream; a plain one-column scalar is its own UTF-8 bytes, any other cell is `0xFF width len
  text`. Styles carry resolved colors, attribute bits and an index into a `LinkTable`, which
  stores each hyperlink URI once (never a session-local id, ADR 0028). `encode_row` appends to
  caller buffers and does not allocate once they have their capacity (`tests/alloc_row.rs`); `decode_row` checks every byte and never allocates.

## Public API

| Item | Purpose |
|---|---|
| `encode_row`, `decode_row`, `CellRef`, `CellView`, `Style`, `Color`, `Attrs`, `LinkTable` | row encoding |
| `ProtoError` | every encoding and decoding failure |

## Tests

Row round-trip property (`tests/row_roundtrip.rs`), zero allocation in steady state
(`tests/alloc_row.rs`) and unit tests of every decoder failure.

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
