//! `ferrum-vt`: the VT state machine on top of `vte`: grid, modes, charsets, selection model,
//! hyperlinks, shell marks (`docs/02` §3).
//!
//! # Model
//!
//! The visible screen is a ring of [`Row`]s of 16-byte [`Cell`]s. Rows that scroll off the top are
//! encoded into a [`ferrum_scrollback::Scrollback`] and leave the grid; they are never kept as
//! cells. Colors, hyperlinks and underline colors are packed into the cell (see `color.rs`
//! and [`Extras`]), multi-scalar grapheme clusters are interned per grid ([`GraphemeTable`]).
//! [`Terminal`] owns both screens, the scrollback and the parser; everything that is not grid
//! state leaves through the [`EventSink`].
//!
//! Allowed internal dependencies: `ferrum-scrollback`.
//! Must never depend on: `ferrum-daemon`, `ferrum-pty`, `ferrum-platform`, `ferrum-proto`, any OS
//! or socket code.

#![forbid(unsafe_code)]

mod args;
mod cell;
mod charset;
mod codec;
mod color;
mod core;
mod csi;
mod cursor;
mod damage;
mod dcs;
mod dec_modes;
mod dump;
mod erase;
mod error;
mod esc;
mod events;
mod extras;
mod extras_sweep;
mod graphemes;
mod grid;
mod history_extras;
mod input;
mod keyboard;
mod limits;
mod modes;
mod motion;
mod osc;
mod osc_clipboard;
mod osc_colors;
mod osc_link;
mod osc_marks;
mod palette;
mod perform;
mod print;
mod reply;
mod reports;
mod row;
mod screen;
mod scroll;
mod sgr;
mod size;
mod slot_set;
mod style_modes;
mod tabs;
mod terminal;
mod width;

pub use cell::{Attrs, BLANK_CHAR, Cell, CellFlags, Underline};
pub use charset::{Charset, Charsets};
pub use color::{Color, ColorKind};
pub use cursor::{Cursor, CursorShape, CursorStyle, Pen, SavedCursor};
pub use damage::Damage;
pub use error::VtError;
pub use events::{Event, EventSink, NullSink, ProgressState, ShellMark};
pub use extras::{ExtraId, Extras, LinkId};
pub use graphemes::GraphemeTable;
pub use grid::Grid;
pub use keyboard::{KeyboardFlags, KeyboardStack};
pub use limits::*;
pub use modes::{Mode, MouseTracking};
pub use palette::{ANSI_COLORS, PALETTE_SIZE, Palette, Rgb, Role};
pub use row::Row;
pub use screen::{Region, Screen};
pub use size::Size;
pub use tabs::TabStops;
pub use terminal::Terminal;
pub use width::{char_width, cluster_width, clusters, joins_cluster};
