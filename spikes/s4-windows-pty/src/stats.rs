//! Throughput statistics over a drained session: chunk sizes, gaps and the end marker.

use crate::bytes::contains;
use crate::helpers::MARKER;
use crate::out::say;
use crate::session::{Msg, Sess};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

const MARKER_TAIL_BYTES: usize = 64;

pub(crate) struct Stats {
    pub(crate) bytes: usize,
    chunks: usize,
    max_chunk: usize,
    /// Chunk counts for <= 1 KiB, <= 4 KiB, <= 16 KiB, <= 32 KiB and larger.
    buckets: [usize; 5],
    max_gap_ms: u128,
    lf: usize,
    pub(crate) marker: bool,
    max_read_block_ms: u128,
    tail: Vec<u8>,
    first: Option<Instant>,
    last: Instant,
}

fn bucket(len: usize) -> usize {
    match len {
        0..=1024 => 0,
        1025..=4096 => 1,
        4097..=16384 => 2,
        16385..=32768 => 3,
        _ => 4,
    }
}

/// Display-only conversion; byte counts are far below 2^52.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn to_f64(n: usize) -> f64 {
    n as f64
}

impl Stats {
    fn new(start: Instant) -> Self {
        Self {
            bytes: 0,
            chunks: 0,
            max_chunk: 0,
            buckets: [0; 5],
            max_gap_ms: 0,
            lf: 0,
            marker: false,
            max_read_block_ms: 0,
            tail: Vec::new(),
            first: None,
            last: start,
        }
    }

    fn record(&mut self, at: Instant, chunk: &[u8], blocked: Duration) {
        self.first.get_or_insert(at);
        self.max_gap_ms = self.max_gap_ms.max(at.saturating_duration_since(self.last).as_millis());
        self.last = at;
        self.bytes += chunk.len();
        self.chunks += 1;
        self.max_chunk = self.max_chunk.max(chunk.len());
        self.max_read_block_ms = self.max_read_block_ms.max(blocked.as_millis());
        if let Some(n) = self.buckets.get_mut(bucket(chunk.len())) {
            *n += 1;
        }
        self.lf += chunk.iter().fold(0, |n, c| n + usize::from(*c == b'\n'));
        self.track_marker(chunk);
    }

    fn track_marker(&mut self, chunk: &[u8]) {
        self.tail.extend_from_slice(chunk);
        self.marker |= contains(&self.tail, MARKER.as_bytes());
        let keep_from = self.tail.len().saturating_sub(MARKER_TAIL_BYTES);
        self.tail.drain(..keep_from);
    }

    /// Seconds between the first and the last chunk.
    fn secs(&self) -> f64 {
        self.first.map_or(0.0, |f| self.last.saturating_duration_since(f).as_secs_f64()).max(1e-9)
    }

    pub(crate) fn print(&self, label: &str, sent: usize) {
        let secs = self.secs();
        say!(
            "[{label}] bytes_rx={} sent_by_child~={sent} ratio={:.3} chunks={} max_chunk={} mean_chunk={} buckets(<=1K,<=4K,<=16K,<=32K,>32K)={:?} throughput={:.1} MB/s (first->last chunk {secs:.2}s) max_inter_chunk_gap={} ms max_read_call_block={} ms lf_count={} end_marker_seen={}",
            self.bytes,
            to_f64(self.bytes) / to_f64(sent),
            self.chunks,
            self.max_chunk,
            self.bytes / self.chunks.max(1),
            self.buckets,
            to_f64(self.bytes) / 1_048_576.0 / secs,
            self.max_gap_ms,
            self.max_read_block_ms,
            self.lf,
            self.marker
        );
    }
}

/// Drains `s` without accumulating (reader-side memory stays bounded) until the end marker, the
/// end of the stream or `timeout`; calls `tick` about every 10 ms.
pub(crate) fn consume(
    s: &mut Sess,
    timeout: Duration,
    mut tick: Option<&mut dyn FnMut()>,
) -> Stats {
    let start = Instant::now();
    let mut stats = Stats::new(start);
    while start.elapsed() < timeout && !stats.marker && s.end.is_none() {
        match s.rx.recv_timeout(Duration::from_millis(10)) {
            Ok(Msg::Data { at, bytes, blocked }) => {
                stats.record(at, &bytes, blocked);
                s.answer_dsr(&bytes);
            }
            Ok(end @ Msg::End { .. }) => s.absorb(end),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if let Some(t) = tick.as_deref_mut() {
            t();
        }
    }
    stats
}
