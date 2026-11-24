//! Output, input and size: PT-13 to PT-21, PT-24 and the startup of `docs/11` §4 (PT-18
//! measures memory too).

use std::ops::ControlFlow;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use ferrum_pty::{PtyError, PtyReader, Readiness};

use super::capture::Sequence;
use super::reader::{ChunkSink, READ_BUFFER_BYTES, spawn_reader_thread};
use super::{CURSOR_QUERY, CURSOR_REPLY, Startup};
use super::{
    Capture, DEFAULT_COLS, DEFAULT_ROWS, FLOOD_LINE_BYTES, Harness, LineCheck, contains,
    numbers_after, process_memory_bytes, size,
};

/// PT-16: the size of the session, and the lines printed before the same-size resize.
const COLS: u16 = DEFAULT_COLS;
const ROWS: u16 = DEFAULT_ROWS;
/// PT-15: the new size (the spike's).
const RESIZED: (u16, u16) = (100, 30);
const REPAINT_LINES: u64 = 60;
/// After `REPAINT_LINES` lines and `END`, the screen shows the last `ROWS - 2` numbered lines,
/// `END` and the cursor line: the lowest line a repaint may contain.
const FIRST_VISIBLE_LINE: u64 = REPAINT_LINES - (ROWS as u64 - 2);
/// PT-17: the storm alternates between the session size and this one (the spike's sizes).
const STORM_BIG: (u16, u16) = (120, 40);
/// PT-17: 16 MB flood.
const STORM_FLOOD_BYTES: u64 = 16 * 1024 * 1024;
/// PT-18: 64 MB flood.
const LARGE_FLOOD_BYTES: u64 = 64 * 1024 * 1024;
/// PT-19: a 4 MiB flood read with buffers of these sizes in turn, up to the daemon's 64 KiB.
const BUFFER_FLOOD_BYTES: u64 = 4 * 1024 * 1024;
const BUFFER_SIZES: [usize; 4] = [1, 7, 4096, READ_BUFFER_BYTES];

const SECOND: Duration = Duration::from_secs(1);
/// Generous bound for "the helper starts and prints": far above the spike's slowest (2.3 s).
const START: Duration = Duration::from_secs(10);
/// Bound for a whole flood: 64 MB at the slowest measured `ConPTY` rate under load (1.3 MB/s,
/// ADR 0026 item 4) takes about 50 s.
const FLOOD_TIMEOUT: Duration = Duration::from_secs(180);

/// What [`read_through_query`] saw.
struct Started {
    out: Capture,
    /// When the session was spawned, or when the cursor query was answered if it was emitted.
    origin: Instant,
    /// When `LINE0` first arrived.
    first_line: Option<Instant>,
}

/// Reads output, answering the startup cursor query once if the backend emits it, until `END`.
/// The spawn is raw: this case is the session layer of W1 itself.
fn read_through_query(h: &Harness<'_>, args: &[&str]) -> Started {
    let mut origin = Instant::now();
    let mut session = h.backend.spawn(&h.spec(args)).unwrap();
    let mut out = Capture::start(session.reader);
    let (mut replied, mut first_line) = (false, None);
    let deadline = Instant::now() + START;
    while !contains(&out.data, b"END") && Instant::now() < deadline {
        if !replied && contains(&out.data, CURSOR_QUERY) {
            session.writer.write_all(CURSOR_REPLY).unwrap();
            (replied, origin) = (true, Instant::now());
        }
        out.wait_until(super::capture::POLL, |output| contains(output, b"END"));
        if first_line.is_none() && contains(&out.data, b"LINE0") {
            first_line = Some(Instant::now());
        }
    }
    session.child.close().unwrap();
    Started { out, origin, first_line }
}

/// PT-13: if the backend emits `ESC[6n`, the reply `ESC[1;1R` releases the output, and a plain
/// child's first output arrives within 2 s (of the reply, or of the spawn without a query).
pub fn startup_cursor_query_is_answered(h: &Harness<'_>) {
    let startup = read_through_query(h, &["lines", "1"]);
    let first_line = startup.first_line.expect("no output");
    let elapsed = first_line.saturating_duration_since(startup.origin);
    assert!(elapsed < 2 * SECOND, "first output after {elapsed:?}");
}

/// PT-14: everything the child wrote while the query was unanswered, including its final line,
/// arrives after the reply.
pub fn no_output_lost_before_first_reply(h: &Harness<'_>) {
    let out = read_through_query(h, &["lines", "200"]).out;
    assert_eq!(numbers_after(&out.data, b"LINE"), (0..200).collect::<Vec<u64>>());
    assert!(contains(&out.data, b"END"), "final line lost");
}

/// PT-15: the child sees the new rows and columns.
pub fn resize_updates_child_size(h: &Harness<'_>) {
    let mut session = h.spawn(&["size"]);
    let mut out = Capture::start(session.reader);
    let initial = format!("SIZE {ROWS} {COLS}");
    assert!(out.wait_for(initial.as_bytes(), START), "wrong initial size");
    let (cols, rows) = RESIZED;
    session.resize.resize(size(cols, rows)).unwrap();
    assert_eq!(session.resize.size(), size(cols, rows));
    let mark = out.data.len();
    session.writer.write_all(b"\r").unwrap();
    let resized = format!("SIZE {rows} {cols}");
    let seen = out.wait_for_after(mark, resized.as_bytes(), 5 * SECOND);
    assert!(seen, "the child missed the resize");
    session.child.close().unwrap();
}

/// PT-16: a same-size resize returns `Ok`; any repaint is at most `rows` lines and never earlier
/// lines.
pub fn resize_same_size_is_ok(h: &Harness<'_>) {
    let mut session = h.spawn(&["repl", &REPAINT_LINES.to_string()]);
    let mut out = Capture::start(session.reader);
    assert!(out.wait_for(b"END", START), "no output");
    let mark = out.data.len();
    session.resize.resize(size(COLS, ROWS)).unwrap();
    session.writer.write_all(b"probe\r").unwrap();
    assert!(out.wait_for_after(mark, b"OUT:probe", 5 * SECOND), "no output after resize");
    let repainted = numbers_after(&out.data[mark..], b"LINE");
    assert!(repainted.len() <= usize::from(ROWS), "repaint longer than the screen: {repainted:?}");
    let first_visible = FIRST_VISIBLE_LINE;
    let earlier = repainted.iter().filter(|&&line| line < first_visible).collect::<Vec<_>>();
    assert!(
        earlier.is_empty(),
        "scrollback re-emitted: {earlier:?} (first visible {first_visible})"
    );
    session.child.close().unwrap();
}

/// Sink that checks a flood on the reader thread and reports once at the end.
struct FloodSink {
    check: LineCheck,
    last: Instant,
    max_gap: Duration,
    done: SyncSender<(LineCheck, Duration)>,
}

impl ChunkSink for FloodSink {
    fn chunk(&mut self, bytes: &[u8]) -> ControlFlow<()> {
        self.max_gap = self.max_gap.max(self.last.elapsed());
        self.last = Instant::now();
        self.check.feed(bytes);
        if self.check.end.is_some() { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    }

    fn end(&mut self, _result: Result<(), PtyError>) {
        let report = (std::mem::take(&mut self.check), self.max_gap);
        // The test may have timed out and left; nothing is left to report then.
        let _ignored = self.done.send(report);
    }
}

fn check_flood(reader: Box<dyn PtyReader>, sequence: Sequence) -> Receiver<(LineCheck, Duration)> {
    let (done, rx) = std::sync::mpsc::sync_channel(1);
    let check = LineCheck::new(sequence);
    let sink = FloodSink { check, last: Instant::now(), max_gap: Duration::ZERO, done };
    let _thread = spawn_reader_thread(1, reader, sink).unwrap();
    rx
}

/// PT-17: a 16 MB flood with a resize every 40 ms: the end marker arrives, no resize fails, and
/// the longest gap between reads is under 1 s.
pub fn resize_storm_during_flood(h: &Harness<'_>) {
    let session = h.spawn(&["flood", &STORM_FLOOD_BYTES.to_string()]);
    let report = check_flood(session.reader, Sequence::Ignored);
    let resize = session.resize;
    let (stop, stopped) = std::sync::mpsc::channel::<()>();
    let storm = std::thread::spawn(move || {
        let mut results = Vec::new();
        let mut big = false;
        // The receive timeout is the 40 ms cadence; a message (or hang-up) stops the storm.
        while stopped
            .recv_timeout(Duration::from_millis(40))
            .is_err_and(|error| error == RecvTimeoutError::Timeout)
        {
            big = !big;
            let (cols, rows) = if big { STORM_BIG } else { (COLS, ROWS) };
            results.push(resize.resize(size(cols, rows)));
        }
        results
    });
    let (check, max_gap) = report.recv_timeout(FLOOD_TIMEOUT).expect("flood did not finish");
    stop.send(()).unwrap();
    let results = storm.join().unwrap();
    session.child.close().unwrap();
    assert!(check.end.is_some(), "end marker lost");
    assert!(results.iter().all(Result::is_ok), "a resize failed");
    assert!(!results.is_empty(), "no resize happened during the flood");
    assert!(max_gap < SECOND, "read gap {max_gap:?}");
}

/// PT-18: 64 MB: the end marker arrives, lines are complete and in order, and the process memory
/// grows by less than 1 MiB while reading. Throughput is recorded in the failure message only.
pub fn large_output_integrity(h: &Harness<'_>) {
    let before = process_memory_bytes().unwrap();
    let started = Instant::now();
    let session = h.spawn(&["flood", &LARGE_FLOOD_BYTES.to_string()]);
    let (check, _) =
        check_flood(session.reader, Sequence::Checked).recv_timeout(FLOOD_TIMEOUT).unwrap();
    let after = process_memory_bytes().unwrap();
    session.child.close().unwrap();
    let elapsed = started.elapsed();
    let rate = u128::from(check.bytes) / elapsed.as_millis().max(1);
    report_throughput(check.bytes, elapsed, rate);
    assert_eq!(check.error, None, "throughput {rate} kB/s");
    assert_eq!(check.end, Some(check.next), "lines lost or end marker missing");
    assert!(check.next * FLOOD_LINE_BYTES >= LARGE_FLOOD_BYTES, "too few lines: {}", check.next);
    let delta = after.saturating_sub(before);
    assert!(delta < 1024 * 1024, "memory grew by {delta} bytes");
}

/// PT-19: no read returns more than the buffer given, up to the daemon's 64 KiB.
pub fn read_respects_buffer_size(h: &Harness<'_>) {
    let mut session = h.spawn(&["flood", &BUFFER_FLOOD_BYTES.to_string()]);
    let mut reader = session.reader;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = vec![0; READ_BUFFER_BYTES];
        let mut tail = Vec::new();
        for len in BUFFER_SIZES.into_iter().cycle() {
            let read = reader.read(&mut buf[..len]).unwrap();
            if read > len || read == 0 {
                return tx.send(false);
            }
            tail.extend_from_slice(&buf[..read]);
            if contains(&tail, b"FLOOD-END") {
                return tx.send(true);
            }
            let keep = tail.len().saturating_sub(16);
            tail.drain(..keep);
        }
        tx.send(false)
    });
    assert!(rx.recv_timeout(FLOOD_TIMEOUT).unwrap(), "a read exceeded its buffer");
    session.child.kill().unwrap();
    session.child.close().unwrap();
}

/// PT-20: 4 MiB written to a child that never reads completes within 30 s while the output is
/// drained, and never blocks `read`.
pub fn large_input_completes(h: &Harness<'_>) {
    let session = h.spawn(&["sleep"]);
    let mut out = Capture::start(session.reader);
    let mut writer = session.writer;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let chunk = vec![b'a'; 4096];
        let result = (0..1024).try_for_each(|_| writer.write_all(&chunk));
        let _ignored = tx.send(result.is_ok());
    });
    let deadline = Instant::now() + 30 * SECOND;
    let done = loop {
        if let Ok(done) = rx.try_recv() {
            break Some(done);
        }
        if Instant::now() >= deadline {
            break None;
        }
        // Draining the echo keeps the output side from blocking the child.
        out.wait_until(super::capture::POLL, |_| false);
        assert!(out.end.is_none(), "read ended while writing");
    };
    session.child.close().unwrap();
    assert_eq!(done, Some(true), "the 4 MiB write did not complete within 30 s");
}

/// PT-21: a silent child does not make `read` return early or fail.
pub fn reader_blocks_until_data_or_close(h: &Harness<'_>) {
    let session = h.spawn(&["sleep"]);
    let mut out = Capture::start(session.reader);
    assert_eq!(out.wait_end(SECOND), None, "read ended while the child was silent");
    session.child.close().unwrap();
    assert_eq!(out.wait_end(2 * SECOND), Some(Ok(())));
}

/// PT-24 (maintainer decision Q-31): no session needs a thread of its own. A native reader is a
/// pollable descriptor (Unix) or is notified by the backend's single I/O thread, `pty-io`, the
/// same thread for every session (Windows); then `try_read` makes progress.
pub fn no_reader_thread_per_session(h: &Harness<'_>) {
    const SESSIONS: usize = 3;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut sessions = Vec::new();
    for _ in 0..SESSIONS {
        let mut session = h.spawn(&["repl", "1"]);
        match session.reader.readiness() {
            Readiness::Notify => {
                let tx = tx.clone();
                session.reader.set_notify(Box::new(move || {
                    let thread = std::thread::current();
                    // The test may have stopped listening; nothing is left to record then.
                    let _sent = tx.send((thread.id(), thread.name().map(str::to_owned)));
                }));
            }
            #[cfg(unix)]
            Readiness::Fd(_) => {}
            other => panic!("a native reader must not need a thread: {other:?}"),
        }
        sessions.push(session);
    }
    let mut threads = Vec::new();
    for session in &mut sessions {
        let notified = session.reader.readiness() == Readiness::Notify;
        let mut seen = Vec::new();
        let mut buf = vec![0; READ_BUFFER_BYTES];
        while !contains(&seen, b"END") {
            let read = if notified {
                let Some(read) = session.reader.try_read(&mut buf).unwrap() else {
                    let (id, name) = rx.recv_timeout(START).expect("no notification");
                    assert_eq!(name.as_deref(), Some("pty-io"), "notified on {name:?}");
                    threads.push(id);
                    continue;
                };
                read
            } else {
                // A descriptor the daemon polls; this test simply reads it.
                session.reader.read(&mut buf).unwrap()
            };
            assert!(read > 0, "the output ended early");
            seen.extend_from_slice(&buf[..read]);
        }
    }
    let one_thread = threads.iter().all(|id| Some(id) == threads.first());
    assert!(one_thread, "more than one I/O thread served the sessions: {threads:?}");
    for session in sessions {
        session.child.close().unwrap();
    }
}

/// PT-18 records the throughput (ADR 0026: recorded, not gated) in the test output.
#[allow(clippy::print_stderr, reason = "PT-18 records the measured throughput in the test log")]
fn report_throughput(bytes: u64, elapsed: Duration, rate: u128) {
    eprintln!("PT-18 large_output_integrity: {bytes} bytes in {elapsed:?} ({rate} kB/s)");
}

/// `docs/11` §4 points 1 and 2 at startup: a `ConPTY` session's first and only output is the cursor
/// query until it is answered (W1); no backend re-renders the screen (no clear screen) before the
/// child's first output. The bytes before it are recorded in the test output.
pub fn startup_output_follows_docs_11(h: &Harness<'_>) {
    let mut session = h.backend.spawn(&h.spec(&["lines", "1"])).unwrap();
    let mut out = Capture::start(session.reader);
    if h.startup == Startup::CursorQuery {
        assert!(out.wait_until(START, |output| output.len() >= CURSOR_QUERY.len()), "no output");
        assert_eq!(&out.data[..CURSOR_QUERY.len()], CURSOR_QUERY, "the query is not first");
        session.writer.write_all(CURSOR_REPLY).unwrap();
    }
    assert!(out.wait_for(b"END", START), "no output");
    session.child.close().unwrap();
    let first_line = out.data.windows(5).position(|window| window == b"LINE0").unwrap();
    let startup = &out.data[..first_line];
    report_startup(startup);
    if h.startup == Startup::Plain {
        assert!(!contains(&out.data, CURSOR_QUERY), "a plain backend sent the cursor query");
    }
    assert!(!contains(startup, b"\x1b[2J"), "the screen was cleared at startup");
}

/// Records the startup bytes in the test output.
#[allow(clippy::print_stderr, reason = "records the measured startup bytes in the test log")]
fn report_startup(startup: &[u8]) {
    eprintln!("startup bytes before the first line: {:?}", startup.escape_ascii().to_string());
}
