//! Values shared by the messages: ids, sizes, cursor, modes, layouts.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::error::ProtoError;
use crate::list::List;
use crate::redact::Text;

/// A session, unique for the daemon's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub u64);

/// A project: a UUID (maintainer decision Q-29, the same id as `ferrum-project`'s), sent as its
/// 16 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectId(pub Uuid);

impl Serialize for ProjectId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_bytes().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProjectId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <[u8; 16]>::deserialize(deserializer).map(|bytes| Self(Uuid::from_bytes(bytes)))
    }
}

/// What runs in a session (`docs/02` §9: `Agent` is added in Phase 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SessionKind {
    /// A shell or program in a PTY.
    Terminal,
}

/// The grid size the UI shows a session at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Viewport {
    /// Columns.
    pub cols: u16,
    /// Rows.
    pub rows: u16,
}

/// Shape of the cursor (DECSCUSR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CursorShape {
    /// A filled cell.
    Block,
    /// A line under the cell.
    Underline,
    /// A vertical bar.
    Bar,
}

/// The cursor of a session's screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cursor {
    /// Column, from 0.
    pub col: u16,
    /// Screen row, from 0.
    pub row: u16,
    /// Shape.
    pub shape: CursorShape,
    /// Whether it blinks.
    pub is_blinking: bool,
    /// Whether it is shown (DECTCEM).
    pub is_visible: bool,
}

/// Terminal modes the UI needs to encode input and draw (`docs/02` §4: "keystrokes already
/// encoded by the UI per terminal modes reported by D"). Bits not defined here are ignored, so a
/// newer minor version may add some.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Modes(pub u32);

impl Modes {
    /// DECCKM: cursor keys send application sequences.
    pub const APP_CURSOR: Self = Self(1);
    /// DECKPAM: the keypad sends application sequences.
    pub const APP_KEYPAD: Self = Self(1 << 1);
    /// LNM: Enter sends CR LF.
    pub const LINE_FEED_NEWLINE: Self = Self(1 << 2);
    /// Bracketed paste (2004).
    pub const BRACKETED_PASTE: Self = Self(1 << 3);
    /// Focus in/out reports (1004).
    pub const FOCUS_REPORTING: Self = Self(1 << 4);
    /// X10 mouse reports (9).
    pub const MOUSE_X10: Self = Self(1 << 5);
    /// Normal mouse tracking (1000).
    pub const MOUSE_NORMAL: Self = Self(1 << 6);
    /// Button-event mouse tracking (1002).
    pub const MOUSE_BUTTON: Self = Self(1 << 7);
    /// Any-event mouse tracking (1003).
    pub const MOUSE_ANY: Self = Self(1 << 8);
    /// SGR mouse encoding (1006).
    pub const MOUSE_SGR: Self = Self(1 << 9);
    /// Alternate scroll (1007).
    pub const ALTERNATE_SCROLL: Self = Self(1 << 10);
    /// The alternate screen is active.
    pub const ALT_SCREEN: Self = Self(1 << 11);
    /// DECSCNM: reverse video for the whole screen.
    pub const REVERSE_VIDEO: Self = Self(1 << 12);

    /// Whether every bit of `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// The modes of a session: the boolean modes and the kitty keyboard flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TerminalModes {
    /// Boolean modes.
    pub modes: Modes,
    /// The active kitty keyboard protocol flags (`CSI = flags u`), 0 when off.
    pub kitty_keyboard: u8,
}

/// A 24-bit color (project colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Axis {
    /// Children side by side.
    Horizontal,
    /// Children on top of each other.
    Vertical,
}

/// A share in per mille, 1 to 999: both sides of a split keep some space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct PerMille(u16);

impl PerMille {
    /// Half and half.
    pub const HALF: Self = Self(500);

    /// The share in per mille.
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for PerMille {
    type Error = ProtoError;

    fn try_from(value: u16) -> Result<Self, ProtoError> {
        if (1..=999).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ProtoError::InvalidValue("a split ratio is 1 to 999 per mille"))
        }
    }
}

impl From<PerMille> for u16 {
    fn from(value: PerMille) -> Self {
        value.0
    }
}

/// A percentage, 0 to 100.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(try_from = "u8", into = "u8")]
pub struct Percent(u8);

impl Percent {
    /// The percentage.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Percent {
    type Error = ProtoError;

    fn try_from(value: u8) -> Result<Self, ProtoError> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err(ProtoError::InvalidValue("a percentage is 0 to 100"))
        }
    }
}

impl From<Percent> for u8 {
    fn from(value: Percent) -> Self {
        value.0
    }
}

/// One node of a [`Layout`] in pre-order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LayoutNode {
    /// A pane showing a session.
    Pane(SessionId),
    /// A split; its two children follow (first, then second).
    Split {
        /// Direction.
        axis: Axis,
        /// Share of the first child.
        ratio: PerMille,
    },
}

/// A project's split tree as a flat pre-order list (`FR-PROJ-005`). The list is flat so that
/// decoding untrusted input never recurses; the daemon checks the tree shape when it converts
/// the list into its model.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Layout(pub List<LayoutNode>);

/// A cell position for selections: `line` 0 is the top screen row, negative lines are in the
/// scrollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Point {
    /// Line.
    pub line: i32,
    /// Column.
    pub col: u16,
}

/// How a selection grows (`FR-TERM-014`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SelectionKind {
    /// Cell by cell.
    Simple,
    /// Whole words (double click).
    Word,
    /// Whole lines (triple click).
    Line,
    /// A rectangle (Alt+drag).
    Block,
}

/// Search options (`FR-TERM-015`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SearchOptions {
    /// Match case.
    pub is_case_sensitive: bool,
    /// The query is a regular expression.
    pub is_regex: bool,
}

/// Progress state (`OSC 9;4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProgressState {
    /// No progress shown.
    Remove,
    /// Normal progress.
    Normal,
    /// Error state.
    Error,
    /// Busy without a value.
    Indeterminate,
    /// Warning state.
    Warning,
}

/// Where a session is in its lifecycle (`FR-SESS-006`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SessionStatus {
    /// The process runs.
    Running,
    /// The process exited by itself, with its exit code when the OS reported one.
    Exited {
        /// Exit code.
        code: Option<i32>,
    },
    /// The process was killed (closed by the user, or by a signal).
    Killed,
    /// The daemon stopped the session after an internal error (protocol 1.1, `docs/02` §10:
    /// "marked failed with the error").
    Failed {
        /// What failed, in words fixed by the daemon (never terminal content).
        error: Text,
    },
}

/// Which rows a `Rows` message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RowRegion {
    /// Screen rows; `y` is the screen row.
    Screen,
    /// History rows; `y` 0 is the row just above the screen, counting up.
    History,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_message;

    #[test]
    fn out_of_range_values_do_not_decode() {
        for value in [0_u16, 1_000, u16::MAX] {
            let bytes = postcard::to_stdvec(&value).unwrap();
            assert!(decode_message::<PerMille>(&bytes).is_err(), "{value}");
        }
        for value in [1_u16, 999] {
            let bytes = postcard::to_stdvec(&value).unwrap();
            assert_eq!(decode_message::<PerMille>(&bytes).unwrap().get(), value);
        }
        assert!(decode_message::<Percent>(&[101]).is_err());
        assert_eq!(decode_message::<Percent>(&[100]).unwrap().get(), 100);
    }

    #[test]
    fn a_project_id_is_its_16_bytes() {
        let id = ProjectId(Uuid::from_u128(1));
        let bytes = postcard::to_stdvec(&id).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(decode_message::<ProjectId>(&bytes).unwrap(), id);
    }
}
