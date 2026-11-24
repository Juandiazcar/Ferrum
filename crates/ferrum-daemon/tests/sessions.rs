//! AC-SESS-01 and AC-SESS-02 (`docs/08` §5) with real processes: closing the UI and opening it
//! again finds every session with identical screen and scrollback; `kill -9` of the UI does not
//! affect the sessions; `kill -9` of the daemon loses the processes but a restarted daemon
//! restores the projects and layout and respawns the shells in their last working directory
//! (FR-SESS-008); the daemon exits after its grace with no session and no client, never while a
//! client is connected (FR-SESS-003).

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "test code")]

#[path = "support/harness.rs"]
mod harness;
#[path = "support/helper.rs"]
mod helper;
#[path = "support/ui.rs"]
mod ui;
#[path = "support/ui_layout.rs"]
mod ui_layout;

use std::collections::HashSet;
use std::time::{Duration, Instant};

use ferrum_proto::{ClientMessage, DaemonMessage, SessionEvent, SessionId};

use harness::{Home, kill, wait_exit, wait_success};

/// The session child and the UI processes (see `support/`); ignored when run directly.
#[test]
#[ignore = "the child processes of the daemon tests"]
fn helper_child() {
    helper::run_if_child();
    ui::run_if_ui();
}

/// How long a test waits for the idle daemon to exit (its grace is 1 s).
const EXIT_WAIT: Duration = Duration::from_secs(20);
/// The grace of the test that measures it (`performance.daemon_exit_grace_secs`).
const GRACE_SECS: u16 = 3;

/// Sets up two sessions in a UI that closes; returns what it saw and the ids.
fn two_sessions_seen(home: &Home) -> (Vec<ui::Seen>, Vec<String>) {
    let mut first = ui::start(home, "setup 2");
    let report = ui::read_report(&mut first, "SETUP-DONE");
    wait_success(&mut first, "the first UI");
    assert_eq!(report.seen.len(), 2);
    let ids = report.seen.iter().map(|seen| seen.id.to_string()).collect();
    (report.seen, ids)
}

#[test]
fn sessions_survive_closing_the_ui_with_identical_content() {
    let home = Home::new("s01");
    let mut daemon = home.start_daemon();
    let (before, ids) = two_sessions_seen(&home);
    let mut second = ui::start(&home, &format!("check {}", ids.join(" ")));
    let after = ui::read_report(&mut second, "CHECK-DONE");
    wait_success(&mut second, "the second UI");
    let listed: Vec<u64> = before.iter().map(|seen| seen.id).collect();
    assert_eq!(after.listed, listed, "the state after Welcome lists every session");
    assert_eq!(after.seen, before, "screen and scrollback changed while no UI was connected");
    assert!(before.iter().all(|seen| seen.lines > 300), "the scrollback was restored: {before:?}");
    assert_eq!(after.alive.len(), 2, "both sessions still answer");
    let mut closer = ui::start(&home, &format!("close {}", ids.join(" ")));
    wait_success(&mut closer, "closing the sessions");
    assert!(wait_exit(&mut daemon, EXIT_WAIT), "the idle daemon did not exit (FR-SESS-003)");
}

#[test]
fn killing_the_ui_does_not_affect_the_sessions() {
    let home = Home::new("s02u");
    let mut daemon = home.start_daemon();
    let mut doomed = ui::start(&home, "setup-wait 2");
    let before = ui::read_report(&mut doomed, "SETUP-DONE").seen;
    kill(&mut doomed);
    let ids: Vec<String> = before.iter().map(|seen| seen.id.to_string()).collect();
    let mut check = ui::start(&home, &format!("check {}", ids.join(" ")));
    let after = ui::read_report(&mut check, "CHECK-DONE");
    wait_success(&mut check, "the checking UI");
    assert_eq!(after.seen, before, "the sessions changed when the UI was killed");
    assert_eq!(after.alive.len(), 2);
    kill(&mut daemon);
}

#[test]
fn a_daemon_restarted_after_kill_9_restores_the_layout_and_working_directories() {
    let home = Home::new("s02d");
    let mut daemon = home.start_daemon();
    let mut setup = ui::start(&home, "layout");
    let before = ui::read_lines(&mut setup, "SETUP-DONE");
    wait_success(&mut setup, "the layout UI");
    kill(&mut daemon);
    let mut daemon = home.start_daemon();
    let mut inspect = ui::start(&home, "inspect");
    let after = ui::read_lines(&mut inspect, "CHECK-DONE");
    wait_success(&mut inspect, "the inspecting UI");
    let layout = |lines: &[String]| lines.iter().find(|line| line.starts_with("LAYOUT")).cloned();
    assert!(layout(&before).is_some());
    assert_eq!(layout(&after), layout(&before), "the layout was not restored");
    assert!(
        after.iter().any(|line| line == "RUNNING 2"),
        "the shells were not respawned: {after:?}"
    );
    assert!(after.iter().any(|line| line == "CWD-OK"));
    kill(&mut daemon);
}

#[test]
fn a_connected_client_keeps_a_daemon_without_sessions_alive() {
    let home = Home::new("idle");
    let mut daemon = home.start_daemon();
    // A UI with a project and no session, connected until killed.
    let mut client = ui::start(&home, "setup-wait 0");
    ui::read_lines(&mut client, "SETUP-DONE");
    // Three times the grace with a client and no session: the daemon must still run.
    let three_graces = Duration::from_secs(3 * u64::from(harness::GRACE_SECS));
    assert!(!wait_exit(&mut daemon, three_graces), "it exited with a client connected");
    kill(&mut client);
    assert!(wait_exit(&mut daemon, EXIT_WAIT), "the daemon did not exit after the client left");
}

#[test]
fn the_daemon_waits_its_grace_before_exiting() {
    let home = Home::new("grace");
    home.write_settings(|settings| {
        settings.performance.daemon_exit_grace_secs =
            ferrum_config::DaemonExitGraceSecs::new(GRACE_SECS).unwrap();
    });
    let mut daemon = home.start_daemon();
    let mut client = ui::start(&home, "setup 0");
    ui::read_lines(&mut client, "SETUP-DONE");
    // Taken before the UI leaves, so the measured time is never longer than the daemon's idle.
    let left = Instant::now();
    wait_success(&mut client, "the UI");
    assert!(wait_exit(&mut daemon, EXIT_WAIT), "the idle daemon did not exit");
    let grace = Duration::from_secs(u64::from(GRACE_SECS));
    assert!(left.elapsed() >= grace, "it exited after {:?}, before its grace", left.elapsed());
}

#[test]
fn events_before_a_session_is_announced_arrive_after_its_created() {
    let home = Home::new("held");
    let mut daemon = home.start_daemon();
    let mut client = home.connect();
    let project = client.create_project(&home.dir("data"));
    // The child reports its directory (OSC 7) as soon as it starts, usually before the session
    // is announced: the event is held, then delivered right after `Created`.
    let session = client.create_session(project);
    client.wait_for("the early working directory", |m| {
        matches!(m, DaemonMessage::SessionEvent { session_id, event: SessionEvent::CwdChanged(_) } if *session_id == session)
    });
    kill(&mut daemon);
}

/// The sessions a newly connected client is told about, up to a round trip after `Synced`: each
/// must appear once (in the state or as a later `Created`).
fn created_once(home: &Home) -> Vec<SessionId> {
    let mut client = home.connect();
    let mut seen: Vec<SessionId> = client
        .state
        .iter()
        .filter_map(|m| match m {
            DaemonMessage::SessionEvent { session_id, event: SessionEvent::Created(_) } => {
                Some(*session_id)
            }
            _ => None,
        })
        .collect();
    let roots =
        ferrum_proto::List::new(vec![ferrum_proto::Text::new(home.dir("data").to_string_lossy())])
            .unwrap();
    client.send(&ClientMessage::CreateProject { name: ferrum_proto::Text::new("sync"), roots });
    client.wait_for("the round trip", |m| {
        if let DaemonMessage::SessionEvent { session_id, event: SessionEvent::Created(_) } = m {
            seen.push(*session_id);
        }
        matches!(m, DaemonMessage::ProjectEvent(ferrum_proto::ProjectEvent::Created(_)))
    });
    seen
}

#[test]
fn a_client_accepted_while_sessions_start_gets_each_created_once() {
    let home = Home::new("dup");
    let mut daemon = home.start_daemon();
    let mut creator = home.connect();
    let project = creator.create_project(&home.dir("data"));
    let starter = std::thread::Builder::new()
        .name("test-session-creator".into())
        .spawn(move || {
            let mut creator = creator;
            for _ in 0..SESSIONS_RACED {
                creator.create_session(project);
            }
        })
        .unwrap();
    for _ in 0..CLIENTS_RACED {
        let seen = created_once(&home);
        let unique: HashSet<SessionId> = seen.iter().copied().collect();
        assert_eq!(unique.len(), seen.len(), "a session was announced twice: {seen:?}");
    }
    starter.join().unwrap();
    kill(&mut daemon);
}

/// Sessions created while other clients connect (`a_client_accepted_while_sessions_start...`).
const SESSIONS_RACED: usize = 6;
/// Clients connected meanwhile.
const CLIENTS_RACED: usize = 6;
