//! The kitty keyboard protocol mode stack (FR-TERM-008). The terminal only tracks the state; the
//! UI reads [`KeyboardFlags`] to encode keys.

use bitflags::bitflags;

use crate::limits::KEYBOARD_STACK_DEPTH;

bitflags! {
    /// Progressive enhancement flags (`CSI > flags u`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct KeyboardFlags: u32 {
        /// Disambiguate escape codes.
        const DISAMBIGUATE_ESCAPE_CODES = 1;
        /// Report event types (press, repeat, release).
        const REPORT_EVENT_TYPES = 1 << 1;
        /// Report alternate keys.
        const REPORT_ALTERNATE_KEYS = 1 << 2;
        /// Report all keys as escape codes.
        const REPORT_ALL_KEYS_AS_ESCAPE_CODES = 1 << 3;
        /// Report associated text.
        const REPORT_ASSOCIATED_TEXT = 1 << 4;
    }
}

/// How `CSI = flags ; mode u` combines the flags with the current ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SetMode {
    /// Replace the current flags (mode 1, the default).
    Replace,
    /// Set the given bits (mode 2).
    Add,
    /// Clear the given bits (mode 3).
    Remove,
}

impl SetMode {
    pub(crate) fn from_code(code: u16) -> Option<Self> {
        match code {
            0 | 1 => Some(Self::Replace),
            2 => Some(Self::Add),
            3 => Some(Self::Remove),
            _ => None,
        }
    }
}

/// The bounded stack of one screen. The current flags are the top entry; an empty stack means 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardStack {
    entries: [KeyboardFlags; KEYBOARD_STACK_DEPTH],
    len: usize,
}

impl Default for KeyboardStack {
    fn default() -> Self {
        Self { entries: [KeyboardFlags::empty(); KEYBOARD_STACK_DEPTH], len: 0 }
    }
}

impl KeyboardStack {
    /// The active flags.
    pub fn current(&self) -> KeyboardFlags {
        self.len.checked_sub(1).and_then(|top| self.entries.get(top)).copied().unwrap_or_default()
    }

    /// Pushes flags; when the stack is full the oldest entry is dropped.
    pub(crate) fn push(&mut self, flags: KeyboardFlags) {
        if self.len == KEYBOARD_STACK_DEPTH {
            self.entries.copy_within(1.., 0);
            self.len -= 1;
        }
        if let Some(slot) = self.entries.get_mut(self.len) {
            *slot = flags;
            self.len += 1;
        }
    }

    /// Pops `count` entries; popping more than there are leaves the stack empty.
    pub(crate) fn pop(&mut self, count: usize) {
        self.len = self.len.saturating_sub(count);
    }

    /// Changes the active flags in place.
    pub(crate) fn set(&mut self, flags: KeyboardFlags, mode: SetMode) {
        if self.len == 0 {
            self.push(KeyboardFlags::empty());
        }
        let Some(top) = self.len.checked_sub(1).and_then(|top| self.entries.get_mut(top)) else {
            return;
        };
        match mode {
            SetMode::Replace => *top = flags,
            SetMode::Add => top.insert(flags),
            SetMode::Remove => top.remove(flags),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: KeyboardFlags = KeyboardFlags::DISAMBIGUATE_ESCAPE_CODES;
    const B: KeyboardFlags = KeyboardFlags::REPORT_EVENT_TYPES;

    #[test]
    fn push_pop_and_the_empty_stack() {
        let mut stack = KeyboardStack::default();
        assert_eq!(stack.current(), KeyboardFlags::empty());
        stack.push(A);
        stack.push(B);
        assert_eq!(stack.current(), B);
        stack.pop(1);
        assert_eq!(stack.current(), A);
        stack.pop(9);
        assert_eq!(stack.current(), KeyboardFlags::empty());
    }

    #[test]
    fn the_stack_is_bounded_and_drops_the_oldest() {
        let mut stack = KeyboardStack::default();
        stack.push(A);
        for _ in 0..KEYBOARD_STACK_DEPTH {
            stack.push(B);
        }
        stack.pop(KEYBOARD_STACK_DEPTH - 1);
        assert_eq!(stack.current(), B, "the oldest entry (A) was dropped");
    }

    #[test]
    fn set_replaces_adds_and_removes_in_place() {
        let mut stack = KeyboardStack::default();
        stack.set(A, SetMode::Replace);
        stack.set(B, SetMode::Add);
        assert_eq!(stack.current(), A | B);
        stack.set(A, SetMode::Remove);
        assert_eq!(stack.current(), B);
        assert_eq!(SetMode::from_code(9), None);
    }
}
