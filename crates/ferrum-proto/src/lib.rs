//! `ferrum-proto`: the IPC protocol between the UI (`ferrum`) and the daemon (`ferrumd`):
//! message types, framing, protocol versioning and the row encoding of damage (`docs/02` §4).
//!
//! # Model
//!
//! Screen rows travel in a compact format of their own ([`encode_row`], [`decode_row`]) whose
//! encoder does not allocate; hyperlinks are stored once per block in a [`LinkTable`]. Every
//! decoder takes untrusted bytes and returns a [`ProtoError`] instead of panicking.
//!
//! Allowed internal dependencies: none (foundation).
//! Must never depend on: any other ferrum crate, fonts or rendering.

#![forbid(unsafe_code)]

mod error;
mod links;
mod row;
mod row_decode;
mod style;

pub use error::ProtoError;
pub use links::LinkTable;
pub use row::{CellRef, CellWidth, encode_row};
pub use row_decode::{CellView, RowCells, decode_row};
pub use style::{Attrs, Color, ColorKind, Style, Underline};
