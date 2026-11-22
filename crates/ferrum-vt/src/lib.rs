//! `ferrum-vt`: the VT state machine on top of `vte`: grid, modes, charsets, selection model,
//! hyperlinks, shell marks (`docs/02` §3).
//!
//! # Model
//!
//! The visible screen is a ring of [`Row`]s of 16-byte [`Cell`]s. Rows that scroll off the top are
//! encoded into a [`ferrum_scrollback::Scrollback`] and leave the grid; they are never kept as
//! cells. Colors, hyperlinks and underline colors are packed into the cell (see `color.rs`
//! and [`Extras`]), multi-scalar grapheme clusters are interned per grid ([`GraphemeTable`]).
//!
//! Allowed internal dependencies: `ferrum-scrollback`.
//! Must never depend on: `ferrum-daemon`, `ferrum-pty`, `ferrum-platform`, `ferrum-proto`, any OS
//! or socket code.

#![forbid(unsafe_code)]

mod cell;
mod color;
mod damage;
mod error;
mod extras;
mod graphemes;
mod grid;
mod limits;
mod row;
mod size;
mod width;

pub use cell::{Attrs, BLANK_CHAR, Cell, CellFlags, Underline};
pub use color::{Color, ColorKind};
pub use damage::Damage;
pub use error::VtError;
pub use extras::{ExtraId, Extras, LinkId};
pub use graphemes::GraphemeTable;
pub use grid::Grid;
pub use limits::*;
pub use row::Row;
pub use size::Size;
pub use width::{char_width, cluster_width, clusters, joins_cluster};
