//! Scrollback encoder exactly as docs/02 §3.2:
//!
//! ```text
//! line := flags:u8 | text_len:varint | utf8_text (trailing default blanks trimmed)
//!         | run_count:varint | { style_id:varint | run_len:varint }*
//! ```
//!
//! A per-session style table interns (fg, bg, attrs). Lines go to a hot chunk of at most 1 024
//! lines (uncompressed); a full chunk is sealed: LZ4 block (`lz4_flex`) plus a delta-encoded
//! offsets index (varint line lengths). `run_len` counts Unicode scalars of the stored text.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use anyhow::{Context, Result, anyhow};

use crate::grid::{Cell, FLAG_SPACER, Sink};

pub(crate) const CHUNK_LINES: usize = 1024;
const FLAG_WRAPPED: u8 = 1;

type StyleKey = (u32, u32, u16);

fn put_varint(out: &mut Vec<u8>, mut v: usize) {
    while v >= 0x80 {
        out.push(u8::try_from(v & 0x7F).unwrap_or(0) | 0x80); // INVARIANT: masked to 7 bits
        v >>= 7;
    }
    out.push(u8::try_from(v).unwrap_or(0)); // INVARIANT: v < 0x80 here
}

fn get_varint(buf: &[u8], pos: &mut usize) -> Option<usize> {
    let (mut v, mut shift) = (0usize, 0u32);
    loop {
        let b = *buf.get(*pos)?;
        *pos += 1;
        v |= usize::from(b & 0x7F) << shift;
        if b < 0x80 {
            return Some(v);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

fn is_blank(c: &Cell) -> bool {
    (c.ch == 0 || c.ch == u32::from(' ')) && c.fg == 0 && c.bg == 0 && c.attrs == 0
}

/// Per-session interned styles; id 0 is the default style.
pub(crate) struct StyleTable {
    list: Vec<StyleKey>,
    map: HashMap<StyleKey, u32>,
}

impl StyleTable {
    fn new() -> Self {
        let mut t = Self { list: Vec::new(), map: HashMap::new() };
        t.intern((0, 0, 0));
        t
    }

    fn intern(&mut self, k: StyleKey) -> u32 {
        if let Some(&id) = self.map.get(&k) {
            return id;
        }
        let id = u32::try_from(self.list.len()).unwrap_or(u32::MAX);
        self.list.push(k);
        self.map.insert(k, id);
        id
    }

    pub(crate) fn len(&self) -> usize {
        self.list.len()
    }

    fn get(&self, id: usize) -> Option<StyleKey> {
        self.list.get(id).copied()
    }

    /// Capacity-based estimate; the hash map part uses hashbrown's layout (entry + 1 control byte).
    fn capacity_bytes(&self) -> usize {
        self.list.capacity() * size_of::<StyleKey>()
            + self.map.capacity() * (size_of::<(StyleKey, u32)>() + 1)
    }
}

struct Sealed {
    data: Box<[u8]>,
    index: Box<[u8]>,
    lines: usize,
    raw_len: usize,
}

/// All-time counters (they include lines already evicted).
#[derive(Default)]
pub(crate) struct Counters {
    pub(crate) rows: usize,
    pub(crate) raw_bytes: usize,
    pub(crate) runs: usize,
    pub(crate) default_only_lines: usize,
    /// Bytes spent on `run_count|style|len` in lines whose only run is the default style.
    pub(crate) default_only_run_bytes: usize,
    pub(crate) sealed_lines: usize,
    pub(crate) sealed_raw: usize,
    pub(crate) sealed_compressed: usize,
    pub(crate) sealed_index: usize,
    pub(crate) encode_ns: u128,
    /// Largest number of bytes the hot chunk ever held.
    pub(crate) hot_len_max: usize,
}

/// Capacity-based accounting of the scrollback structures.
pub(crate) struct Capacity {
    pub(crate) hot_data: usize,
    pub(crate) hot_index: usize,
    pub(crate) sealed_data: usize,
    pub(crate) sealed_index: usize,
    pub(crate) sealed_structs: usize,
    pub(crate) styles: usize,
    pub(crate) line_scratch: usize,
    pub(crate) lz_scratch: usize,
}

/// The scrollback store.
pub(crate) struct Scrollback {
    cap_lines: usize,
    hot: Vec<u8>,
    hot_offsets: Vec<u32>,
    /// 0: plain `Vec` doubling growth. N > 0: `reserve_exact` in steps of at least N bytes.
    hot_step: usize,
    sealed: VecDeque<Sealed>,
    head_skip: usize,
    pub(crate) styles: StyleTable,
    text: Vec<u8>,
    runs: Vec<u8>,
    idx: Vec<u8>,
    lz: Vec<u8>,
    pub(crate) counters: Counters,
}

impl Scrollback {
    pub(crate) fn new(cap_lines: usize, hot_step: usize) -> Self {
        Self {
            cap_lines,
            hot: Vec::new(),
            hot_offsets: Vec::new(),
            hot_step,
            sealed: VecDeque::new(),
            head_skip: 0,
            styles: StyleTable::new(),
            text: Vec::new(),
            runs: Vec::new(),
            idx: Vec::new(),
            lz: Vec::new(),
            counters: Counters::default(),
        }
    }

    /// Lines currently retained (the oldest chunk may be partially skipped, not yet freed).
    pub(crate) fn retained(&self) -> usize {
        let sealed: usize = self.sealed.iter().map(|c| c.lines).sum();
        sealed + self.hot_offsets.len() - self.head_skip
    }

    pub(crate) fn hot_lines(&self) -> usize {
        self.hot_offsets.len()
    }

    pub(crate) fn sealed_chunks(&self) -> usize {
        self.sealed.len()
    }

    pub(crate) fn capacity(&self) -> Capacity {
        Capacity {
            hot_data: self.hot.capacity(),
            hot_index: self.hot_offsets.capacity() * size_of::<u32>(),
            sealed_data: self.sealed.iter().map(|c| c.data.len()).sum(),
            sealed_index: self.sealed.iter().map(|c| c.index.len()).sum(),
            sealed_structs: self.sealed.capacity() * size_of::<Sealed>(),
            styles: self.styles.capacity_bytes(),
            line_scratch: self.text.capacity() + self.runs.capacity() + self.idx.capacity(),
            lz_scratch: self.lz.capacity(),
        }
    }

    fn encode(&mut self, cells: &[Cell], wrapped: bool) {
        let mut end = cells.len();
        while end > 0 && cells.get(end - 1).is_some_and(is_blank) {
            end -= 1;
        }
        self.text.clear();
        self.runs.clear();
        let (mut run_count, mut run_len, mut cur) = (0usize, 0usize, None::<StyleKey>);
        let mut buf = [0u8; 4];
        for c in cells.iter().take(end).filter(|c| c.flags & FLAG_SPACER == 0) {
            let ch = if c.ch == 0 { ' ' } else { char::from_u32(c.ch).unwrap_or('\u{FFFD}') };
            self.text.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            let key = (c.fg, c.bg, c.attrs);
            if cur != Some(key) {
                if let Some(k) = cur {
                    let id = self.styles.intern(k);
                    put_varint(&mut self.runs, usize::try_from(id).unwrap_or(0));
                    put_varint(&mut self.runs, run_len);
                    run_count += 1;
                }
                cur = Some(key);
                run_len = 0;
            }
            run_len += 1;
        }
        if let Some(k) = cur {
            let id = self.styles.intern(k);
            put_varint(&mut self.runs, usize::try_from(id).unwrap_or(0));
            put_varint(&mut self.runs, run_len);
            run_count += 1;
        }
        let needed = self.text.len() + self.runs.len() + 24;
        if self.hot_step > 0 && self.hot.capacity() - self.hot.len() < needed {
            self.hot.reserve_exact(self.hot_step.max(needed));
        }
        let start = self.hot.len();
        self.hot_offsets.push(u32::try_from(start).unwrap_or(u32::MAX));
        self.hot.push(if wrapped { FLAG_WRAPPED } else { 0 });
        put_varint(&mut self.hot, self.text.len());
        self.hot.extend_from_slice(&self.text);
        let runs_start = self.hot.len();
        put_varint(&mut self.hot, run_count);
        self.hot.extend_from_slice(&self.runs);
        let c = &mut self.counters;
        c.rows += 1;
        c.raw_bytes += self.hot.len() - start;
        c.runs += run_count;
        c.hot_len_max = c.hot_len_max.max(self.hot.len());
        if run_count == 1 && self.runs.first() == Some(&0) {
            c.default_only_lines += 1;
            c.default_only_run_bytes += self.hot.len() - runs_start;
        }
    }

    fn seal(&mut self) -> Result<()> {
        let raw_len = self.hot.len();
        self.lz.resize(lz4_flex::block::get_maximum_output_size(raw_len), 0);
        let n = lz4_flex::block::compress_into(&self.hot, &mut self.lz)
            .map_err(|e| anyhow!("lz4 compress: {e}"))?;
        self.idx.clear();
        let mut offs = self.hot_offsets.iter().map(|&o| o as usize).peekable();
        while let Some(o) = offs.next() {
            let next = offs.peek().copied().unwrap_or(raw_len);
            put_varint(&mut self.idx, next - o); // delta-encoded offsets
        }
        let packed: Box<[u8]> = self.lz.get(..n).context("lz4 length")?.into();
        let index: Box<[u8]> = self.idx.as_slice().into();
        let c = &mut self.counters;
        c.sealed_lines += self.hot_offsets.len();
        c.sealed_raw += raw_len;
        c.sealed_compressed += packed.len();
        c.sealed_index += index.len();
        self.sealed.push_back(Sealed {
            data: packed,
            index,
            lines: self.hot_offsets.len(),
            raw_len,
        });
        self.hot.clear();
        self.hot_offsets.clear();
        Ok(())
    }

    fn evict(&mut self) {
        while self.retained() > self.cap_lines {
            let Some(front) = self.sealed.front() else { return };
            self.head_skip += 1;
            if self.head_skip == front.lines {
                self.sealed.pop_front();
                self.head_skip = 0;
            }
        }
    }

    /// Calls `f(flags, text, runs)` for every retained line, oldest first; `runs` yields
    /// `(style, run_len)` pairs. Used by the round-trip verification only.
    pub(crate) fn for_each_line(&self, mut f: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
        let mut raw = Vec::new();
        for (i, ch) in self.sealed.iter().enumerate() {
            raw.resize(ch.raw_len, 0);
            let n = lz4_flex::block::decompress_into(&ch.data, &mut raw)
                .map_err(|e| anyhow!("lz4 decompress: {e}"))?;
            if n != ch.raw_len {
                return Err(anyhow!("short decompress"));
            }
            let (mut pos, mut ipos) = (0usize, 0usize);
            for l in 0..ch.lines {
                let len = get_varint(&ch.index, &mut ipos).context("index")?;
                let line = raw.get(pos..pos + len).context("line bounds")?;
                pos += len;
                if !(i == 0 && l < self.head_skip) {
                    f(line)?;
                }
            }
        }
        for (i, &o) in self.hot_offsets.iter().enumerate() {
            let end = self.hot_offsets.get(i + 1).map_or(self.hot.len(), |&e| e as usize);
            f(self.hot.get(o as usize..end).context("hot bounds")?)?;
        }
        Ok(())
    }

    /// FNV-1a over (wrapped, then per scalar: char, fg, bg, attrs) of an encoded line.
    pub(crate) fn hash_encoded(&self, line: &[u8]) -> Result<u64> {
        let mut h = Fnv::new();
        let flags = *line.first().context("flags")?;
        let mut pos = 1;
        let tlen = get_varint(line, &mut pos).context("text_len")?;
        let text = std::str::from_utf8(line.get(pos..pos + tlen).context("text")?)?;
        pos += tlen;
        let nruns = get_varint(line, &mut pos).context("run_count")?;
        h.byte(flags & FLAG_WRAPPED);
        let mut chars = text.chars();
        for _ in 0..nruns {
            let id = get_varint(line, &mut pos).context("style")?;
            let len = get_varint(line, &mut pos).context("run_len")?;
            let (fg, bg, at) = self.styles.get(id).context("style id")?;
            for _ in 0..len {
                h.cell(u32::from(chars.next().context("run past text")?), fg, bg, at);
            }
        }
        if chars.next().is_some() || pos != line.len() {
            return Err(anyhow!("runs do not cover text or trailing bytes"));
        }
        Ok(h.0)
    }
}

impl Sink for Scrollback {
    fn push_row(&mut self, cells: &[Cell], wrapped: bool) -> Result<()> {
        let t = Instant::now();
        self.encode(cells, wrapped);
        if self.hot_offsets.len() == CHUNK_LINES {
            self.seal()?;
        }
        self.evict();
        self.counters.encode_ns += t.elapsed().as_nanos();
        Ok(())
    }
}

/// FNV-1a 64.
pub(crate) struct Fnv(pub(crate) u64);

impl Fnv {
    pub(crate) fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub(crate) fn byte(&mut self, b: u8) {
        self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }

    pub(crate) fn cell(&mut self, ch: u32, fg: u32, bg: u32, attrs: u16) {
        for v in [ch, fg, bg, u32::from(attrs)] {
            for b in v.to_le_bytes() {
                self.byte(b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trips() {
        for v in [0, 1, 127, 128, 300, 16_383, 16_384, usize::MAX >> 8] {
            let mut b = Vec::new();
            put_varint(&mut b, v);
            let mut pos = 0;
            assert_eq!(get_varint(&b, &mut pos), Some(v));
            assert_eq!(pos, b.len());
        }
    }

    #[test]
    fn chunks_seal_evict_and_decode() -> Result<()> {
        let mut sb = Scrollback::new(2_500, 0);
        let mut cells = vec![Cell::default(); 8];
        for i in 0..3_000u32 {
            if let Some(c) = cells.first_mut() {
                *c = Cell { ch: 65 + i % 26, fg: i % 3, ..Cell::default() };
            }
            sb.push_row(&cells, i % 7 == 0)?;
        }
        assert_eq!(sb.retained(), 2_500);
        let mut n = 0;
        sb.for_each_line(|l| {
            sb.hash_encoded(l)?;
            n += 1;
            Ok(())
        })?;
        assert_eq!(n, 2_500);
        Ok(())
    }
}
