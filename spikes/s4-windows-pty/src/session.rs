//! One pty session as the daemon would run it: a named reader thread (256 KiB stack, 64 KiB read
//! buffer, docs/11 §4) feeding a bounded channel, plus writer, master and child handles.

use crate::bytes::contains;
use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, ExitStatus, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::{Duration, Instant};

const READ_BUF_BYTES: usize = 64 * 1024; // docs/03 §2 "read buffer 64 KiB"
const READER_STACK_BYTES: usize = 256 * 1024; // docs/11 §4
const CHANNEL_CHUNKS: usize = 32;
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";

/// What the reader thread sends to the session owner.
pub(crate) enum Msg {
    Data { at: Instant, bytes: Vec<u8>, blocked: Duration },
    End { at: Instant, why: String },
}

pub(crate) struct Sess {
    pub(crate) master: Option<Box<dyn MasterPty + Send>>,
    pub(crate) child: Box<dyn Child + Send + Sync>,
    pub(crate) rx: Receiver<Msg>,
    pub(crate) writer: Option<Box<dyn Write + Send>>,
    pub(crate) t0: Instant,
    /// Everything received through `pump*` (not through `stats::consume`).
    pub(crate) buf: Vec<u8>,
    pub(crate) end: Option<Instant>,
    pub(crate) dsr_answers: usize,
    /// Answer the startup cursor query (`ESC[6n`) automatically.
    pub(crate) auto_dsr: bool,
}

static SEQ: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

fn read_loop(mut reader: Box<dyn Read + Send>, tx: &SyncSender<Msg>) {
    let mut buf = vec![0u8; READ_BUF_BYTES];
    loop {
        let started = Instant::now();
        let msg = match reader.read(&mut buf) {
            Ok(0) => Msg::End { at: Instant::now(), why: "eof".into() },
            Ok(n) => Msg::Data {
                at: Instant::now(),
                bytes: buf.get(..n).unwrap_or_default().to_vec(),
                blocked: started.elapsed(),
            },
            Err(e) => Msg::End { at: Instant::now(), why: format!("err {:?}: {e}", e.kind()) },
        };
        let last = matches!(msg, Msg::End { .. });
        if tx.send(msg).is_err() || last {
            return; // receiver dropped (session gone) or the stream ended
        }
    }
}

fn start_reader(reader: Box<dyn Read + Send>) -> Result<Receiver<Msg>> {
    let (tx, rx) = sync_channel::<Msg>(CHANNEL_CHUNKS);
    let id = SEQ.fetch_add(1, Ordering::Relaxed);
    // The handle is dropped on purpose: a blocked ConPTY read cannot be cancelled or joined, which
    // is exactly what experiment 6 shows.
    std::thread::Builder::new()
        .name(format!("pty-read-{id}"))
        .stack_size(READER_STACK_BYTES)
        .spawn(move || read_loop(reader, &tx))
        .context("spawn reader thread")?;
    Ok(rx)
}

/// Opens a pty of `cols` x `rows` and spawns `cmd` on it.
pub(crate) fn spawn(cmd: CommandBuilder, cols: u16, rows: u16) -> Result<Sess> {
    let pair = native_pty_system().openpty(size(cols, rows)).context("openpty")?;
    let t0 = Instant::now();
    let child = pair.slave.spawn_command(cmd).context("spawn_command")?;
    drop(pair.slave); // required on Unix for EOF delivery
    let reader = pair.master.try_clone_reader().context("try_clone_reader")?;
    let writer = pair.master.take_writer().context("take_writer")?;
    Ok(Sess {
        master: Some(pair.master),
        child,
        rx: start_reader(reader)?,
        writer: Some(writer),
        t0,
        buf: Vec::new(),
        end: None,
        dsr_answers: 0,
        auto_dsr: true,
    })
}

impl Sess {
    /// Replies to every cursor query found in `bytes` when `auto_dsr` is on.
    pub(crate) fn answer_dsr(&mut self, bytes: &[u8]) {
        if !self.auto_dsr {
            return;
        }
        for _ in bytes.windows(CURSOR_QUERY.len()).filter(|w| *w == CURSOR_QUERY) {
            self.send(CURSOR_REPLY);
            self.dsr_answers += 1;
        }
    }

    pub(crate) fn absorb(&mut self, msg: Msg) {
        match msg {
            Msg::Data { bytes, .. } => {
                self.answer_dsr(&bytes);
                self.buf.extend_from_slice(&bytes);
            }
            Msg::End { at, .. } => self.end = Some(at),
        }
    }

    pub(crate) fn send(&mut self, bytes: &[u8]) {
        if let Some(w) = self.writer.as_mut() {
            // A failed write shows up as missing output, which every experiment already reports.
            w.write_all(bytes).and_then(|()| w.flush()).ok();
        }
    }

    /// Collects output for `span`, or until the stream ends.
    pub(crate) fn pump(&mut self, span: Duration) {
        let end = Instant::now() + span;
        while Instant::now() < end && self.end.is_none() {
            match self.rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
                Ok(msg) => self.absorb(msg),
                Err(_) => break, // timeout or reader gone
            }
        }
    }

    /// Pumps until `pat` appears in `buf`; returns how long it took.
    pub(crate) fn pump_until(&mut self, pat: &str, timeout: Duration) -> Option<Duration> {
        let start = Instant::now();
        while start.elapsed() < timeout
            && self.end.is_none()
            && !contains(&self.buf, pat.as_bytes())
        {
            self.pump(Duration::from_millis(5));
        }
        contains(&self.buf, pat.as_bytes()).then(|| start.elapsed())
    }

    /// Polls `try_wait` so a missed exit cannot hang the harness.
    pub(crate) fn wait_exit(&mut self, timeout: Duration) -> Option<(Duration, ExitStatus)> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some((start.elapsed(), status));
            }
            self.pump(Duration::from_millis(1));
        }
        None
    }

    /// Waits up to `timeout` for the reader to report the end of the stream.
    pub(crate) fn wait_end(&mut self, timeout: Duration) -> Option<Instant> {
        let start = Instant::now();
        while self.end.is_none() && start.elapsed() < timeout {
            self.pump(Duration::from_millis(10));
        }
        self.end
    }

    /// Drops writer and master (closes the pseudoconsole on Windows).
    pub(crate) fn close(&mut self) {
        self.writer = None;
        self.master = None;
    }
}
