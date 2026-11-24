//! Resize with reflow (FR-TERM-010: a large history is reflowed on the worker, and a newer size
//! stops a running reflow) and search in the daemon (FR-TERM-015), over a real session.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failure is a failed test"
)]

#[path = "support/harness.rs"]
mod harness;
#[path = "support/helper.rs"]
mod helper;

use ferrum_proto::{
    ClientMessage, DaemonMessage, SearchMatch, SearchOptions, SessionId, Text, Viewport,
};
use harness::{Client, Home, kill};

/// The session child (see `support/helper.rs`); ignored when run directly.
#[test]
#[ignore = "the session child of the daemon tests, started by ferrumd"]
fn helper_child() {
    helper::run_if_child();
}

/// More history lines than the daemon reflows inline (1 000), so the reflow goes to the worker.
const LINES: usize = 3_000;
/// The width reflowed to: narrower than the 60-column lines of `wide`.
const NARROW: Viewport = Viewport { cols: 40, rows: 30 };

/// A session filled with `LINES` lines of 60 columns.
fn wide_session(client: &mut Client, home: &Home) -> SessionId {
    let project = client.create_project(&home.dir("data"));
    let session = client.create_session(project);
    client.attach(session, 0);
    client.wait_text(session, "READY", |text| text.contains("READY"));
    client.type_line(session, &format!("wide {LINES}"));
    let done = format!("DONE{LINES}");
    client.wait_text(session, &done, |text| text.contains(&done));
    session
}

fn search(client: &mut Client, session: SessionId, query: &str) -> Vec<SearchMatch> {
    let options = SearchOptions::default();
    client.send(&ClientMessage::Search { session_id: session, query: Text::new(query), options });
    let results =
        client.wait_for("search results", |m| matches!(m, DaemonMessage::SearchResults { .. }));
    let DaemonMessage::SearchResults { matches, is_truncated, .. } = results else {
        unreachable!()
    };
    assert!(!is_truncated);
    matches.into_vec()
}

#[test]
fn a_large_history_is_reflowed_on_the_worker_and_searched() {
    let home = Home::new("views");
    let mut daemon = home.start_daemon();
    let mut client = home.connect();
    let session = wide_session(&mut client, &home);
    // A first size, at once replaced by the final one: the reflow for the first stops early.
    let first = Viewport { cols: 50, rows: 30 };
    client.send(&ClientMessage::Resize { session_id: session, viewport: first });
    client.send(&ClientMessage::Resize { session_id: session, viewport: NARROW });
    client.wait_for(
        "a snapshot at the final size",
        |m| matches!(m, DaemonMessage::Snapshot(snapshot) if snapshot.viewport == NARROW),
    );
    // Every 60-column line is now two rows: the match of `W1500-` and the end of its line are on
    // different rows, and the match across the cut is found.
    let found = search(&mut client, session, "W1500-");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].start.line < 0, "the match is in the history");
    let across = search(&mut client, session, &format!("W1500-{}", "=".repeat(40)));
    assert_eq!(across.len(), 1, "a match across the rewrapped rows: {across:?}");
    assert_eq!(across[0].end.line, across[0].start.line + 1, "it spans two rows");
    client.mirrors.remove(&session);
    client.attach_at(session, 4, NARROW);
    client.wait_text(session, "rewrapped history", |text| {
        text.lines().any(|line| line.trim_end().len() == 20 && line.starts_with('='))
    });
    kill(&mut daemon);
}

/// Lines of the repaint test: more than a screen, so part of them is history.
const REPAINT_LINES: usize = 100;

/// Resizes `session` to `viewport` and waits until the program answered a line typed after it,
/// so whatever the terminal backend repaints for the resize has arrived (ADR 0026 W6: `ConPTY`
/// rewrites the viewport after a resize).
fn resize_and_settle(client: &mut Client, session: SessionId, viewport: Viewport, tag: &str) {
    client.send(&ClientMessage::Resize { session_id: session, viewport });
    client.type_line(session, tag);
    let answer = format!("OUT:{tag}");
    client.wait_text(session, &answer, |text| text.contains(&answer));
}

#[test]
fn a_resize_repaint_does_not_enter_the_history() {
    let home = Home::new("repaint");
    let mut daemon = home.start_daemon();
    let mut client = home.connect();
    let project = client.create_project(&home.dir("data"));
    let session = client.create_session(project);
    client.attach(session, 0);
    client.wait_text(session, "READY", |text| text.contains("READY"));
    client.type_line(session, &format!("lines {REPAINT_LINES}"));
    let done = format!("DONE{REPAINT_LINES}");
    client.wait_text(session, &done, |text| text.contains(&done));
    // Height only: no reflow; the backend repaints the screen each time.
    resize_and_settle(&mut client, session, Viewport { cols: 80, rows: 30 }, "taller");
    resize_and_settle(&mut client, session, Viewport { cols: 80, rows: 12 }, "shorter");
    client.mirrors.remove(&session);
    client.attach_at(session, 2_000, Viewport { cols: 80, rows: 12 });
    client.wait_text(session, "the last answer", |text| text.contains("OUT:shorter"));
    client.pump_until("the history rows", |c| c.mirrors[&session].is_complete());
    let text = client.mirrors[&session].text();
    for index in 0..REPAINT_LINES {
        let line = format!("LINE{index}");
        let count = text.lines().filter(|row| row.trim_end() == line).count();
        assert_eq!(count, 1, "{line} appears {count} times in history and screen:\n{text}");
    }
    kill(&mut daemon);
}

/// Time to print the long history: 24 MB through `ConPTY` on a hosted Windows runner can take
/// most of a minute with a debug daemon.
const FILL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(180);
/// History of the yield test: far more than one reflow budget (16 ms) can rewrap on a fast
/// machine (50 000 lines take less).
const LONG_HISTORY: u32 = 400_000;

/// Counts the snapshots of `session` at `viewport` and notes when the answer to `line` arrived.
#[derive(Default)]
struct Watch {
    snapshots: usize,
    echo_at: Option<usize>,
}

#[test]
fn a_long_reflow_yields_and_the_session_output_is_read_meanwhile() {
    let home = Home::new("yield");
    home.write_settings(|settings| {
        settings.terminal.scrollback_lines =
            ferrum_config::ScrollbackLines::new(LONG_HISTORY).unwrap();
    });
    let mut daemon = home.start_daemon();
    let mut client = home.connect();
    let project = client.create_project(&home.dir("data"));
    let session = client.create_session(project);
    client.attach(session, 0);
    client.wait_text(session, "READY", |text| text.contains("READY"));
    client.type_line(session, &format!("wide {LONG_HISTORY}"));
    let done = format!("DONE{LONG_HISTORY}");
    client.wait_text_within(FILL_DEADLINE, session, &done, |text| text.contains(&done));
    // The reflow goes to the worker; the line typed now is answered while it runs, and the
    // answer arrives before the reflow's last snapshot.
    client.send(&ClientMessage::Resize { session_id: session, viewport: NARROW });
    client.type_line(session, "during");
    let mut watch = Watch::default();
    let start = std::time::Instant::now();
    while watch.echo_at.is_none_or(|echo| watch.snapshots <= echo) {
        let left = harness::DEADLINE.checked_sub(start.elapsed()).unwrap_or_default();
        let Some(message) = client.next_message(left) else {
            panic!(
                "no snapshot followed the answer (snapshots {}, answer after {:?}): the reflow \
                 did not yield",
                watch.snapshots, watch.echo_at
            );
        };
        if matches!(&message, DaemonMessage::Snapshot(s) if s.viewport == NARROW) {
            watch.snapshots += 1;
        }
        if watch.echo_at.is_none() && client.mirrors[&session].text().contains("OUT:during") {
            watch.echo_at = Some(watch.snapshots);
        }
    }
    kill(&mut daemon);
}
