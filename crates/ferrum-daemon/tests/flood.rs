//! Flood isolation at the daemon level (FR-SESS-009, NFR-PERF-04: 20 sessions, one flooding at
//! full speed, the others' echo latency p99 at most 10 ms) and the 20-session scenario without a
//! UI (`docs/12` WP-06 exit criteria), against a real `ferrumd` and real PTYs.
//!
//! The latency gate is absolute here (NFR-PERF-04); `cargo xtask flood --against <rev>` runs it
//! against the daemon of an earlier commit on the same machine as well and compares the two
//! (`docs/08` §8), reading what this test writes to `FERRUM_FLOOD_REPORT`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code; the measured latencies are printed into the test log"
)]

#[path = "support/harness.rs"]
mod harness;
#[path = "support/helper.rs"]
mod helper;

use std::time::{Duration, Instant};

use ferrum_proto::{ClientMessage, DaemonMessage, SessionEvent, SessionId, Text};
use harness::{Client, Home, kill};

/// The session child (see `support/helper.rs`); ignored when run directly.
#[test]
#[ignore = "the session child of the daemon tests, started by ferrumd"]
fn helper_child() {
    helper::run_if_child();
}

/// Sessions of the scenario (NFR-PERF-04).
const SESSIONS: usize = 20;
/// Echo round trips measured per run; `cargo xtask flood` asks for more
/// (`FERRUM_FLOOD_SAMPLES`), so its comparison rests on a steadier p50 (Q-34).
const SAMPLES: usize = 200;
const SAMPLES_VAR: &str = "FERRUM_FLOOD_SAMPLES";
/// NFR-PERF-04: the other sessions' echo latency p99.
const ECHO_P99: Duration = Duration::from_millis(10);
/// Title marks the throughput is computed from (every `FLOOD_MARK_BYTES`).
const THROUGHPUT_MARKS: usize = 4;
/// Where the measurement goes for `cargo xtask flood`.
const REPORT_VAR: &str = "FERRUM_FLOOD_REPORT";

fn start_sessions(client: &mut Client, home: &Home) -> Vec<SessionId> {
    let project = client.create_project(&home.dir("data"));
    let sessions: Vec<SessionId> = (0..SESSIONS).map(|_| client.create_session(project)).collect();
    for &session in &sessions {
        client.attach(session, 0);
        client.wait_text(session, "READY", |text| text.contains("READY"));
    }
    sessions
}

/// Echo latencies (keystroke sent to the echoed text received) of `SAMPLES` round trips over
/// `sessions`, sorted. A round is one person's step in one session: a line and its echo
/// (measured), the program's answer, then a second line whose answer the daemon sends no earlier
/// than one frame interval after the first frame (`docs/02` §4: at most one frame per interval
/// and session). So every wait is on a daemon event, nothing on the test's clock, and a session
/// is typed into again only after its frames are an interval or more old, as with a person
/// typing in 19 sessions in turn.
fn echo_latencies(client: &mut Client, sessions: &[SessionId], tag: &str) -> Vec<Duration> {
    let samples = samples();
    let mut latencies = Vec::with_capacity(samples);
    for round in 0..samples {
        let session = sessions[round % sessions.len()];
        // Fixed width, so no token is a prefix of another.
        let marker = format!("{tag}{round:04}#");
        let start = Instant::now();
        client.type_line(session, &marker);
        client.wait_text(session, &marker, |text| text.contains(&marker));
        latencies.push(start.elapsed());
        let answer = format!("OUT:{marker}");
        client.wait_text(session, &answer, |text| text.contains(&answer));
        let next = format!("{tag}{round:04}=");
        client.type_line(session, &next);
        let paced = format!("OUT:{next}");
        client.wait_text(session, &paced, |text| text.contains(&paced));
    }
    latencies.sort();
    latencies
}

fn samples() -> usize {
    std::env::var(SAMPLES_VAR).ok().and_then(|value| value.parse().ok()).unwrap_or(SAMPLES)
}

fn percentile(sorted: &[Duration], per_mille: usize) -> Duration {
    sorted[(sorted.len() * per_mille / 1_000).min(sorted.len() - 1)]
}

/// Marks of the flood seen so far: `(mark, arrival)`.
fn flood_marks(client: &Client, flooder: SessionId) -> Vec<(u64, Instant)> {
    client
        .titles
        .iter()
        .filter(|(session, _, _)| *session == flooder)
        .filter_map(|(_, title, at)| Some((title.strip_prefix("flood-")?.parse().ok()?, *at)))
        .collect()
}

/// Bytes per second the daemon takes from the flooding session, from [`THROUGHPUT_MARKS`]
/// marks (the flood never stops, so this also shows it ran through the measurement).
fn flood_throughput(client: &mut Client, flooder: SessionId) -> Option<f64> {
    client.pump_until("the flood's title marks", |c| {
        flood_marks(c, flooder).len() >= THROUGHPUT_MARKS
    });
    let marks = flood_marks(client, flooder);
    let (&(first, start), &(last, end)) = (marks.first()?, marks.last()?);
    let seconds = end.checked_duration_since(start)?.as_secs_f64();
    let mib = u32::try_from((last - first) * (helper::FLOOD_MARK_BYTES as u64 >> 20)).ok()?;
    let bytes = f64::from(mib) * f64::from(1_u32 << 20);
    (seconds > 0.0).then(|| bytes / seconds)
}

/// Starts the flood in the hidden first session and waits until it runs.
fn start_flood(client: &mut Client, flooder: SessionId) {
    // The flooding session runs hidden, as a background session does (docs/03 §1).
    client.send(&ClientMessage::Detach { session_id: flooder });
    client.type_line(flooder, "flood");
    client.wait_for("the flood", |m| {
        matches!(m, DaemonMessage::SessionEvent { session_id, event: SessionEvent::Activity } if *session_id == flooder)
    });
}

fn report(quiet: &[Duration], busy: &[Duration], throughput: Option<f64>) {
    let (p50, p99) = (percentile(busy, 500), percentile(busy, 990));
    let mib = throughput.map(|bytes| bytes / f64::from(1_u32 << 20));
    eprintln!(
        "NFR-PERF-04 echo latency over {} round trips, 19 sessions: quiet p50 {:?} p99 \
         {:?}; with one session flooding p50 {p50:?} p99 {p99:?}; flood {mib:.1?} MiB/s",
        samples(),
        percentile(quiet, 500),
        percentile(quiet, 990),
    );
    if let Some(path) = std::env::var_os(REPORT_VAR) {
        let (p50_ns, p99_ns) = (p50.as_nanos(), p99.as_nanos());
        let text =
            format!("p50_ns {p50_ns}\np99_ns {p99_ns}\nthroughput {}\n", throughput.unwrap_or(0.0));
        std::fs::write(path, text).unwrap();
    }
}

#[test]
fn a_flooding_session_does_not_delay_the_other_nineteen() {
    let home = Home::new("flood");
    let mut daemon = home.start_daemon();
    let mut client = home.connect();
    let sessions = start_sessions(&mut client, &home);
    let (&flooder, others) = sessions.split_first().unwrap();
    let quiet = echo_latencies(&mut client, others, "q");
    start_flood(&mut client, flooder);
    let busy = echo_latencies(&mut client, others, "f");
    let throughput = flood_throughput(&mut client, flooder);
    report(&quiet, &busy, throughput);
    let p99 = percentile(&busy, 990);
    assert!(throughput.is_some(), "the flood's throughput could not be measured");
    assert!(p99 <= ECHO_P99, "p99 {p99:?} with a flooding session is above {ECHO_P99:?}");
    kill(&mut daemon);
}

/// Detaches every session, then makes a round trip, after which no frame may arrive.
fn hide_all(client: &mut Client, sessions: &[SessionId]) {
    for &session in sessions {
        client.send(&ClientMessage::Detach { session_id: session });
    }
    let last = *sessions.last().unwrap();
    client.send(&ClientMessage::RenameSession { session_id: last, name: Text::new("hidden") });
    client.wait_for("the rename", |m| {
        matches!(m, DaemonMessage::SessionEvent { event: SessionEvent::Renamed(_), .. })
    });
    client.frames.clear();
}

/// Waits until every session reported the title `done-<index>`.
fn wait_titles(client: &mut Client, sessions: &[SessionId]) {
    let expected: Vec<String> = (0..sessions.len()).map(|index| format!("done-{index}")).collect();
    let is_done = |client: &Client, index: usize| {
        client.titles.iter().any(|(s, title, _)| *s == sessions[index] && *title == expected[index])
    };
    client.pump_until("every session's title", |c| {
        (0..sessions.len()).all(|index| is_done(c, index))
    });
}

#[test]
fn twenty_sessions_run_without_a_ui() {
    let home = Home::new("twenty");
    let mut daemon = home.start_daemon();
    let mut client = home.connect();
    let sessions = start_sessions(&mut client, &home);
    hide_all(&mut client, &sessions);
    for (index, &session) in sessions.iter().enumerate() {
        client.type_line(session, "lines 3000");
        client.type_line(session, &format!("title done-{index}"));
    }
    wait_titles(&mut client, &sessions);
    // Hidden sessions report through session events only (docs/02 §4).
    assert!(client.frames.is_empty(), "hidden sessions sent frames: {:?}", client.frames);
    let last = *sessions.last().unwrap();
    client.mirrors.remove(&last);
    client.attach(last, 3_100);
    client.wait_text(last, "DONE3000", |text| text.contains("DONE3000"));
    client.pump_until("the history rows", |c| c.mirrors[&last].is_complete());
    let text = client.mirrors[&last].text();
    assert!(text.contains("LINE10\n"), "the history of a hidden session was kept");
    kill(&mut daemon);
}
