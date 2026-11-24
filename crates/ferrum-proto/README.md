# ferrum-proto

**Responsibility:** the IPC protocol between the UI (`ferrum`) and the daemon (`ferrumd`):
message types, framing, protocol versioning and the row encoding of damage (`docs/02` §4).

**Status:** WP-03 in progress: the row encoding, framing and versioning are implemented; the
messages follow.

| | |
|---|---|
| Allowed internal dependencies | none (foundation) |
| Must never depend on | any other ferrum crate, fonts or rendering |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `serde`, `postcard` (wire format), `thiserror` |
| Dev-dependencies | `proptest`, `dhat`, `criterion` |
| Features | none |

## Model

- **Frames** (`frame.rs`): `len:u32` little-endian, then one `postcard` message of `len` bytes.
  The whole frame, header included, is at most `MAX_FRAME_LEN` (4 MiB, `docs/02` §4, `docs/10`
  §2). `split_frame` refuses an oversized header before the payload arrives; `decode_message`
  refuses trailing bytes. Lists from the peer are `List<T>`, capped at `MAX_LIST_ITEMS` while
  decoding.
- **Versioning** (`version.rs`): `ProtocolVersion::negotiate` requires the same major and uses
  the lower minor.
- **Rows** (`row.rs`, `row_decode.rs`, `links.rs`): one screen row is style runs over a cell
  stream; a plain one-column scalar is its own UTF-8 bytes, any other cell is `0xFF width len
  text`. Styles carry resolved colors, attribute bits and an index into a `LinkTable`, which
  stores each hyperlink URI once (never a session-local id, ADR 0028). `encode_row` appends to
  caller buffers and does not allocate once they have their capacity (`tests/alloc_row.rs`); `decode_row` checks every byte and never allocates.

## Public API

| Item | Purpose |
|---|---|
| `encode_row`, `decode_row`, `CellRef`, `CellView`, `Style`, `Color`, `Attrs`, `LinkTable` | row encoding |
| `encode_frame`, `split_frame`, `decode_message`, `Frame`, `MAX_FRAME_LEN`, `MAX_PAYLOAD_LEN` | framing |
| `ProtocolVersion`, `PROTOCOL_VERSION` | versioning |
| `List`, `MAX_LIST_ITEMS` | length-capped lists |
| `ProtoError` | every encoding and decoding failure |

## Constants

| Constant | Value | Why |
|---|---|---|
| `MAX_FRAME_LEN` | 4 MiB, header included | `docs/02` §4, `docs/10` §2 |
| `PROTOCOL_VERSION` | 1.0 | the first protocol |
| `MAX_LIST_ITEMS` | 1 024 | bounds the memory a 4 MiB frame of tiny list items can make the decoder allocate; far above any real list |

## Tests

Row round-trip property (`tests/row_roundtrip.rs`), zero allocation in steady state
(`tests/alloc_row.rs`) and unit tests of every decoder failure. Bench `proto_encode/row_200_cols`
(no baseline yet: `benches/baselines/README.md`).

The authoritative edge table is `docs/spec/docs/04-repository-structure.md` §2, enforced by
`cargo xtask check-deps` from `xtask/deps-rules.toml`.
