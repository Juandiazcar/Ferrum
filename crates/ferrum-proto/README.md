# ferrum-proto

**Responsibility:** the IPC protocol between the UI (`ferrum`) and the daemon (`ferrumd`):
message types, framing, protocol versioning and the row encoding of damage (`docs/02` §4).

**Status:** implemented (WP-03).

| | |
|---|---|
| Allowed internal dependencies | none (foundation) |
| Must never depend on | any other ferrum crate, fonts or rendering |
| `unsafe` | forbidden (`#![forbid(unsafe_code)]`) |
| Dependencies | `serde` (derive), `postcard` (wire format), `uuid` (project ids), `thiserror` |
| Dev-dependencies | `proptest`, `dhat`, `criterion` |
| Features | none |

## Model

- **Frames** (`frame.rs`): `len:u32` little-endian, then one `postcard` message of `len` bytes.
  The whole frame, header included, is at most `MAX_FRAME_LEN` (4 MiB, `docs/02` §4, `docs/10`
  §2). `split_frame` refuses an oversized header before the payload arrives; `decode_message`
  refuses trailing bytes.
- **Messages**: `ClientMessage` (UI to daemon) and `DaemonMessage` (daemon to UI), the set of
  `docs/02` §4, plus one reserved variant per Phase 2 area (agent, usage, vault; `docs/02` §9).
  `postcard` encodes a variant by its position: variants are only appended within a major
  version. After `Welcome` the daemon sends its whole state, one item per message
  (`ProjectEvent::Created` with every `FR-PROJ-001` field, `SessionEvent::Created` with the
  session's status), then `Synced`. Lists from the peer are `List<T>`, capped at
  `MAX_LIST_ITEMS` while decoding, and no message nests lists inside lists; layouts are a flat
  pre-order `List<LayoutNode>`, so decoding never recurses. Ranged values are validated newtypes
  (`Percent`, `PerMille`); `ProjectId` is a UUID sent as its 16 bytes (maintainer decision Q-29).
- **Versioning** (`version.rs`, `handshake.rs`): the first client frame is `Hello { protocol_version,
  client_kind, nonce }`. `answer_hello` refuses a wrong nonce (`Nonce` equality is constant
  time), then another major version (`Refused`, with the daemon's version for the UI's error),
  and otherwise answers `Welcome` with the lower of the two minor versions. In every major,
  `Hello` is client variant 0, `Welcome` and `Refused` are daemon variants 0 and 1 and the
  handshake types keep their layout (pinned by `tests/handshake.rs`).
- **Rows** (`row.rs`, `row_decode.rs`, `links.rs`, `block.rs`): one screen row is style runs over a
  cell stream; a plain one-column scalar is its own UTF-8 bytes, any other cell is `0xFF width len
  text`. Styles carry resolved colors, attribute bits and an index into the block's `LinkTable`,
  which stores each hyperlink URI once (never a session-local id, ADR 0028). `encode_row` appends
  to caller buffers and does not allocate once they have their capacity (`tests/alloc_row.rs`);
  `decode_row` checks every byte and never allocates. A `RowBlock` never grows past
  `MAX_ROW_BLOCK_BYTES`, so a `Snapshot`, `Rows` or `Damage` message always fits one frame; a
  screen that needs more goes out as several messages (`tests/frame_budget.rs`).
- **Privacy**: user content is `Text` or `Bytes` (keystrokes, queries, titles, paths, names) and
  rows are a `RowBlock`; their `Debug` prints sizes only, and `Nonce`'s is redacted
  (`docs/05` §8, `docs/10` §4).

## Public API

| Item | Purpose |
|---|---|
| `encode_frame`, `split_frame`, `decode_message`, `Frame`, `MAX_FRAME_LEN`, `MAX_PAYLOAD_LEN` | framing |
| `ClientMessage`, `DaemonMessage`, `Snapshot`, `Damage`, `SessionEvent`, `ProjectEvent`, `ProjectInfo`, `SessionInfo`, ... | messages |
| `ProtocolVersion`, `PROTOCOL_VERSION`, `Hello`, `Nonce`, `answer_hello`, `Refusal` | versioning and handshake |
| `encode_row`, `decode_row`, `CellRef`, `CellView`, `Style`, `Color`, `Attrs`, `LinkTable`, `RowBlock` | row encoding |
| `List`, `MAX_LIST_ITEMS`, `Text`, `Bytes`, `Percent`, `PerMille`, `ProjectId` | checked and redacted values |
| `ProtoError` | every encoding and decoding failure |

## Constants

| Constant | Value | Why |
|---|---|---|
| `MAX_FRAME_LEN` | 4 MiB, header included | `docs/02` §4, `docs/10` §2 |
| `PROTOCOL_VERSION` | 1.1 | 1.0 was the first protocol; 1.1 adds `SearchResults` (with `SearchMatch`), `FileSetAside`, the failed session state with its error (`SessionEvent::Failed`, `SessionStatus::Failed`) and `SessionEvent::SpillDisabled` |
| `NONCE_LEN` | 32 bytes | 256 random bits cannot be guessed by a process that cannot read the owner-only nonce file |
| `MAX_LIST_ITEMS` | 1 024 | bounds the memory a 4 MiB frame of tiny list items can make the decoder allocate (each list at most 1 024 items, no list nested in another); far above any real list |
| `ROW_BLOCK_HEADROOM` | 4 KiB | room in a frame for the fields of a `Snapshot`/`Rows`/`Damage` message besides its block: under 64 bytes plus a title, which `ferrum-vt` caps at 1 024 bytes |

## Tests

Golden bytes for every message variant and every nested enum variant (`tests/golden.rs`,
`fixtures/proto/*.hex`), version negotiation and the pinned handshake (`tests/handshake.rs`),
round-trip properties of rows and of both message enums (`tests/row_roundtrip.rs`,
`tests/message_roundtrip.rs`), the worst-case screen in frames under the cap
(`tests/frame_budget.rs`), `Debug` redaction (`tests/redaction.rs`), zero allocation in steady
state (`tests/alloc_row.rs`), the fuzz body on the corpus and random bytes
(`tests/fuzz_seeds.rs`). Fuzz target `decode` in `fuzz/` (nightly job). Benches
`proto_encode/row_200_cols` and `proto_encode/damage_200x60` (no baseline yet:
`benches/baselines/README.md`).
