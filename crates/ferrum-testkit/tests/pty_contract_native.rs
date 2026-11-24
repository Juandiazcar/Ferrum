//! The `PtyBackend` contract suite (PT-01 to PT-24, ADR 0026, and the startup bytes of `docs/11`
//! §4) on this OS's backend
//! (`ferrum_pty::native_backend`), one named test per case.

#![allow(clippy::unwrap_used, reason = "test code: a failed expectation is a failed test")]

use std::path::PathBuf;

use ferrum_testkit::pty_contract::{self as pt, Harness};

fn harness(name: &str) -> Harness<'static> {
    let scratch = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pty-contract").join(name);
    std::fs::create_dir_all(&scratch).unwrap();
    let backend = Box::leak(ferrum_pty::native_backend());
    let helper = env!("CARGO_BIN_EXE_ferrum-pty-helper").into();
    Harness { backend, helper, scratch, startup: pt::NATIVE_STARTUP }
}

macro_rules! contract {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                pt::$name(&harness(stringify!($name)));
            }
        )*
    };
}

contract!(
    spawn_echo_roundtrip,                   // PT-01
    spawn_missing_program_is_error,         // PT-02
    spawn_applies_cwd,                      // PT-03
    spawn_env_set_and_remove,               // PT-04
    exit_code_zero_nonzero_and_255,         // PT-05
    exit_code_259_is_reported,              // PT-06
    exit_detected_without_reader_eof,       // PT-07
    reader_ends_after_close,                // PT-08
    final_output_survives_fast_exit,        // PT-09
    kill_is_truthful_and_idempotent,        // PT-10
    close_ends_descendants,                 // PT-11
    close_does_not_hang_with_unread_output, // PT-12
    startup_cursor_query_is_answered,       // PT-13
    no_output_lost_before_first_reply,      // PT-14
    resize_updates_child_size,              // PT-15
    resize_same_size_is_ok,                 // PT-16
    resize_storm_during_flood,              // PT-17
    large_output_integrity,                 // PT-18
    read_respects_buffer_size,              // PT-19
    large_input_completes,                  // PT-20
    reader_blocks_until_data_or_close,      // PT-21
    no_panic_on_hostile_input,              // PT-22
    ten_sessions_memory,                    // PT-23
    no_reader_thread_per_session,           // PT-24
    startup_output_follows_docs_11,         // docs/11 §4 points 1 and 2
);
