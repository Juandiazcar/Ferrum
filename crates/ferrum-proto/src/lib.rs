//! `ferrum-proto`: the IPC protocol between the UI (`ferrum`) and the daemon (`ferrumd`):
//! message types, framing, protocol versioning and the row encoding of damage (`docs/02` §4).
//!
//! # Model
//!
//! A connection carries frames ([`encode_frame`], [`split_frame`]): a little-endian `u32` length
//! and one `postcard`-encoded message, at most [`MAX_FRAME_LEN`] bytes header included. Peers
//! negotiate a [`ProtocolVersion`]. Screen rows travel in a compact format of their own ([`encode_row`], [`decode_row`]) whose
//! encoder does not allocate; hyperlinks are stored once per block in a [`LinkTable`]. Every
//! decoder takes untrusted bytes and returns a [`ProtoError`] instead of panicking; list lengths are capped at [`MAX_LIST_ITEMS`].
//!
//! Allowed internal dependencies: none (foundation).
//! Must never depend on: any other ferrum crate, fonts or rendering.

#![forbid(unsafe_code)]

mod error;
mod frame;
mod links;
mod list;
mod row;
mod row_decode;
mod style;
mod version;

pub use error::ProtoError;
pub use frame::{
    FRAME_HEADER_LEN, Frame, MAX_FRAME_LEN, MAX_PAYLOAD_LEN, decode_message, encode_frame,
    split_frame,
};
pub use links::LinkTable;
pub use list::{List, MAX_LIST_ITEMS};
pub use row::{CellRef, CellWidth, encode_row};
pub use row_decode::{CellView, RowCells, decode_row};
pub use style::{Attrs, Color, ColorKind, Style, Underline};
pub use version::{PROTOCOL_VERSION, ProtocolVersion};
