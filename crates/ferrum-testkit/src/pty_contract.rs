//! Shared contract suite for every [`PtyBackend`] implementation: PT-01 to PT-25 of ADR 0026
//! (`docs/08` §2: the same suite for real and fake implementations; `docs/05` §2 Liskov).
//!
//! Each case is a public function taking a [`Harness`], so a test file runs every case as its own
//! named test (`tests/pty_contract_native.rs`, `tests/pty_contract_fake.rs`). The child is always
//! `ferrum-pty-helper` (or the fake's interpretation of its subcommands), never a shell.
//!
//! Waits are bounded by the deadlines the contract states; nothing sleeps to synchronize.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    reason = "test-support code: a failed expectation is a failed test"
)]

mod capture;
mod io;
mod lifecycle;
mod resources;

use std::ffi::OsString;
use std::num::NonZeroU16;
use std::path::PathBuf;

use ferrum_pty::{PtyBackend, PtySession, PtySize, SpawnSpec};

pub use capture::{Capture, LineCheck, Sequence, contains, numbers_after};
pub use io::{
    large_input_completes, large_output_integrity, no_output_lost_before_first_reply,
    read_respects_buffer_size, reader_blocks_until_data_or_close, reader_thread_name_and_stack,
    resize_same_size_is_ok, resize_storm_during_flood, resize_updates_child_size,
    startup_cursor_query_is_answered, startup_output_has_no_query_or_repaint,
};
pub use lifecycle::{
    close_does_not_hang_with_unread_output, close_ends_descendants, exit_code_259_is_reported,
    exit_code_zero_nonzero_and_255, exit_detected_without_reader_eof,
    final_output_survives_fast_exit, kill_is_truthful_and_idempotent, no_panic_on_hostile_input,
    reader_ends_after_close, spawn_applies_cwd, spawn_echo_roundtrip, spawn_env_set_and_remove,
    spawn_missing_program_is_error,
};
pub use resources::{process_memory_bytes, ten_sessions_memory};

/// The size every case starts with (the spike's default).
pub const DEFAULT_COLS: u16 = 80;
/// See [`DEFAULT_COLS`].
pub const DEFAULT_ROWS: u16 = 24;

/// Filler characters of a `flood` line of `ferrum-pty-helper`.
pub const FLOOD_FILLER: usize = 60;

/// Bytes of a `flood` line, newline included: `F`, 8 digits, a space, the filler and `\n`
/// (70 visible columns, within any 80-column terminal).
pub const FLOOD_LINE_BYTES: u64 = 1 + 8 + 1 + FLOOD_FILLER as u64 + 1;

/// What every case needs: the backend under test, the helper program, and a private directory.
pub struct Harness<'a> {
    /// The backend under test.
    pub backend: &'a dyn PtyBackend,
    /// Path of `ferrum-pty-helper` (ignored by the fake, which only reads the arguments).
    pub helper: PathBuf,
    /// An existing directory owned by this test, used as cwd and for lock files.
    pub scratch: PathBuf,
}

/// Builds a [`PtySize`]; both values must be non-zero.
pub fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { cols: NonZeroU16::new(cols).unwrap(), rows: NonZeroU16::new(rows).unwrap() }
}

impl Harness<'_> {
    /// A spec running the helper with `args` in the scratch directory at 80x24.
    pub fn spec(&self, args: &[&str]) -> SpawnSpec {
        SpawnSpec {
            program: self.helper.clone().into_os_string(),
            args: args.iter().map(OsString::from).collect(),
            cwd: self.scratch.clone(),
            env: Vec::new(),
            size: size(DEFAULT_COLS, DEFAULT_ROWS),
        }
    }

    /// Spawns the helper with `args`; the spawn must succeed.
    pub fn spawn(&self, args: &[&str]) -> PtySession {
        self.backend.spawn(&self.spec(args)).unwrap()
    }
}
