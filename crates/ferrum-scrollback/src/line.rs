//! The line types shared by the encoder, the decoder and the readers.

use crate::style::{Style, StyleId};

/// Per-line flags (`docs/02` §3.1 "Line flags", the `flags:u8` of §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LineFlags(u8);

impl LineFlags {
    /// No flag set.
    pub const NONE: Self = Self(0);
    /// The row continues on the next row (soft wrap); reflow needs it.
    pub const WRAPPED: Self = Self(1);
    /// A shell prompt starts on this row (OSC 133 `A`).
    pub const PROMPT_MARK: Self = Self(1 << 1);
    /// A command starts on this row (OSC 133 `B`/`C`).
    pub const COMMAND_START: Self = Self(1 << 2);
    /// A command ends on this row (OSC 133 `D`).
    pub const COMMAND_END: Self = Self(1 << 3);

    const DEFINED_BITS: u8 = 0b1111;

    /// Flags from stored bits; `None` when undefined bits are set.
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits & !Self::DEFINED_BITS == 0 { Some(Self(bits)) } else { None }
    }

    /// The stored bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// True when every flag of `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of both flag sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A run of `len` consecutive Unicode scalars sharing one style (input side).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledRun {
    /// The style of the run.
    pub style: Style,
    /// Number of Unicode scalar values (not columns, not bytes).
    pub len: usize,
}

/// One row to store. Borrowed, so pushing it never allocates.
///
/// `runs` must cover exactly the scalars of `text`; an empty `runs` means "all default style".
/// Trailing default-styled spaces are trimmed by the store (`docs/02` §3.2). Wide characters and
/// combining sequences are *not* described here: the consumer derives column widths from the
/// text with its width table (the encoding stores scalars, `docs/02` §3.2).
#[derive(Debug, Clone, Copy)]
pub struct LineRef<'a> {
    /// Line flags.
    pub flags: LineFlags,
    /// The row text, without the cell padding of wide characters.
    pub text: &'a str,
    /// Style runs over the scalars of `text`.
    pub runs: &'a [StyledRun],
}

/// A style run of a decoded line; resolve the id with [`StyleTable::get`](crate::StyleTable::get).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// Interned style.
    pub style: StyleId,
    /// Number of Unicode scalar values.
    pub len: u32,
}

/// A decoded line. Reusable: decoding into it keeps its allocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedLine {
    /// Line flags.
    pub flags: LineFlags,
    /// Row text.
    pub text: String,
    /// Style runs over the scalars of `text`, in order; they cover it exactly.
    pub runs: Vec<Run>,
}

impl DecodedLine {
    /// An empty line.
    pub const fn new() -> Self {
        Self { flags: LineFlags::NONE, text: String::new(), runs: Vec::new() }
    }

    /// Heap bytes held by the buffers.
    pub fn capacity_bytes(&self) -> usize {
        self.text.capacity() + self.runs.capacity() * size_of::<Run>()
    }
}

impl Default for DecodedLine {
    fn default() -> Self {
        Self::new()
    }
}
