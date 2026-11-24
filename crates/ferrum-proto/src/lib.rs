//! `ferrum-proto`: the IPC protocol between the UI (`ferrum`) and the daemon (`ferrumd`):
//! message types, framing, protocol versioning and the row encoding of damage (`docs/02` §4).
//!
//! # Model
//!
//! A connection carries frames ([`encode_frame`], [`split_frame`]): a little-endian `u32` length
//! and one `postcard`-encoded [`ClientMessage`] or [`DaemonMessage`], at most [`MAX_FRAME_LEN`]
//! bytes header included. The first client frame is a [`Hello`]; [`answer_hello`] checks its
//! nonce and negotiates the [`ProtocolVersion`]. Screen rows travel in a [`RowBlock`] in a
//! compact format of their own ([`encode_row`], [`decode_row`]) whose encoder does not allocate;
//! a block never outgrows a frame. Every decoder takes untrusted bytes and returns a
//! [`ProtoError`] instead of panicking; list lengths are capped at [`MAX_LIST_ITEMS`]. User
//! content ([`Text`], [`Bytes`], rows, nonces) never shows in `Debug` output.
//!
//! Allowed internal dependencies: none (foundation).
//! Must never depend on: any other ferrum crate, fonts or rendering.

#![forbid(unsafe_code)]

mod block;
mod client;
mod daemon;
mod error;
mod frame;
mod handshake;
mod links;
mod list;
mod redact;
mod row;
mod row_decode;
mod style;
mod types;
mod version;

pub use block::{MAX_ROW_BLOCK_BYTES, ROW_BLOCK_HEADROOM, RowBlock};
pub use client::{ClientMessage, ProjectChange, SelectAction};
pub use daemon::{
    DaemonMessage, Damage, ProjectEvent, ProjectInfo, SessionEvent, SessionInfo, Snapshot,
};
pub use error::ProtoError;
pub use frame::{
    FRAME_HEADER_LEN, Frame, MAX_FRAME_LEN, MAX_PAYLOAD_LEN, decode_message, encode_frame,
    split_frame,
};
pub use handshake::{ClientKind, Hello, NONCE_LEN, Nonce, Refusal, RefusalReason, answer_hello};
pub use links::LinkTable;
pub use list::{List, MAX_LIST_ITEMS};
pub use redact::{Bytes, Text};
pub use row::{CellRef, CellWidth, encode_row};
pub use row_decode::{CellView, RowCells, decode_row};
pub use style::{Attrs, Color, ColorKind, Style, Underline};
pub use types::{
    Axis, Cursor, CursorShape, Layout, LayoutNode, Modes, PerMille, Percent, Point, ProgressState,
    ProjectId, Rgb, RowRegion, SearchOptions, SelectionKind, SessionId, SessionKind, SessionStatus,
    TerminalModes, Viewport,
};
pub use uuid::Uuid;
pub use version::{PROTOCOL_VERSION, ProtocolVersion};
