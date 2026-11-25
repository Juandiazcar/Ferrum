//! `cargo xtask memcheck --daemon-only` (ADR 0022, `docs/12` WP-06): the daemon's memory in the
//! reference scenario of `docs/03` §1, against the daemon allowance of `docs/03` §2.
//!
//! Ten sessions at 200x55 each print 12 000 lines of the reference fixture mix
//! (`fixtures/output/`, seeds 1 to 10), four of them attached by the client throughout as the
//! visible ones; 10 000 must be retained; after 10 s of idleness the
//! daemon's memory is read with the metric of `docs/03` §1: PSS on Linux, `phys_footprint` on
//! macOS, Private Working Set on Windows. Child processes do not count. The test is run by `xtask`
//! against a release `ferrumd` (`FERRUM_TEST_FERRUMD`), never in the ordinary test run.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    reason = "test code; the measurement is printed for xtask"
)]

#[path = "../../../fixtures/output/output.rs"]
mod fixture;
#[path = "support/harness.rs"]
mod harness;
#[path = "support/helper.rs"]
mod helper;
#[path = "support/memory.rs"]
mod memory;

use std::io::Write;
use std::time::{Duration, Instant};

use ferrum_proto::{
    ClientMessage, DaemonMessage, ProjectId, SessionEvent, SessionId, Text, Viewport,
};
use harness::{Client, Home, kill};

/// The session child, with the `fixture <seed> <lines>` command; ignored when run directly.
#[test]
#[ignore = "the session child of the daemon tests, started by ferrumd"]
fn helper_child() {
    helper::run_if_child_with(fixture_command);
}

/// Prints `lines` logical lines of the reference fixture, then sets the title `fixture-done`.
fn fixture_command(line: &str, out: &mut dyn Write) -> Option<std::io::Result<()>> {
    let rest = line.strip_prefix("fixture ")?;
    let (seed, lines) = rest.split_once(' ')?;
    let (seed, lines): (u64, usize) = (seed.parse().ok()?, lines.parse().ok()?);
    let (rows, _) = fixture::Fixture::rows(seed, fixture::Profile::Mixed, usize::from(COLS), lines);
    let mut buffer = Vec::new();
    let written = fixture::stream::write_rows(&rows, &mut buffer)
        .and_then(|()| out.write_all(&buffer))
        .and_then(|()| out.write_all(b"\x1b]2;fixture-done\x07"))
        .and_then(|()| out.flush());
    Some(written)
}

/// The reference window's grid (`docs/03` §1: about 200x55).
const COLS: u16 = 200;
const VIEWPORT: Viewport = Viewport { cols: COLS, rows: 55 };
/// Sessions kept visible (attached) through the scenario (`docs/03` §1: 1 to 4 visible).
const VISIBLE: u64 = 4;
/// Sessions, lines fed and lines retained (`docs/03` §1).
const SESSIONS: u64 = 10;
const FED_LINES: usize = 12_000;
const RETAINED_LINES: u16 = 10_000;
/// Steady state: 10 s of idleness after the fill (`docs/03` §1).
const IDLE: Duration = Duration::from_secs(10);
/// The daemon allowance (`docs/03` §2: "Daemon ≤ 18 MB").
const DAEMON_ALLOWANCE_BYTES: u64 = 18_000_000;

/// Fills session `seed` with the fixture; the first `VISIBLE` sessions stay attached at the
/// reference size throughout (`docs/03` §1: 1 to 4 visible), the others are hidden.
fn fill(client: &mut Client, project: ProjectId, seed: u64) -> SessionId {
    let session = client.create_session(project);
    client.attach_at(session, 0, VIEWPORT);
    client.wait_text(session, "READY", |text| text.contains("READY"));
    if seed > VISIBLE {
        client.send(&ClientMessage::Detach { session_id: session });
    }
    client.type_line(session, &format!("fixture {seed} {FED_LINES}"));
    let done = Text::new("fixture-done");
    client.wait_for("the fixture", |m| {
        matches!(m, DaemonMessage::SessionEvent { session_id, event: SessionEvent::Title(title) } if *session_id == session && *title == done)
    });
    session
}

/// Every session retained `RETAINED_LINES` lines (checked after the measurement: sending the
/// history to a client is the UI's scenario).
fn check_retained(client: &mut Client, sessions: &[SessionId]) {
    for &session in sessions {
        let want_scrollback_rows = RETAINED_LINES + 1;
        let viewport = VIEWPORT;
        client.send(&ClientMessage::Attach { session_id: session, viewport, want_scrollback_rows });
        let snapshot = client.wait_for(
            "the snapshot",
            |m| matches!(m, DaemonMessage::Snapshot(s) if s.session_id == session),
        );
        let DaemonMessage::Snapshot(snapshot) = snapshot else { unreachable!() };
        assert_eq!(snapshot.history_rows, RETAINED_LINES, "10 000 lines are retained");
    }
}

#[test]
#[ignore = "the reference-scenario measurement run by cargo xtask memcheck --daemon-only"]
fn daemon_memory_in_the_reference_scenario() {
    let home = Home::new("mem");
    let mut daemon = home.start_daemon();
    let mut client = home.connect();
    let project = client.create_project(&home.dir("data"));
    let sessions: Vec<SessionId> =
        (1..=SESSIONS).map(|seed| fill(&mut client, project, seed)).collect();
    // The steady state of docs/03 §1 is defined by 10 s of idleness: a measured duration, not a
    // synchronization.
    client.drain_until(Instant::now() + IDLE);
    let measured = memory::process_memory(daemon.id());
    check_retained(&mut client, &sessions);
    println!(
        "MEMCHECK daemon {} bytes ({}), allowance {DAEMON_ALLOWANCE_BYTES}",
        measured.bytes, measured.metric
    );
    kill(&mut daemon);
    assert!(measured.bytes <= DAEMON_ALLOWANCE_BYTES, "the daemon is over its allowance");
}
