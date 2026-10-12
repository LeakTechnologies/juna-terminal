use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const DEFAULT_CHUNK: usize = 64 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub soft: usize,
    pub hard: usize,
    pub resume: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            soft: 16 * 1024 * 1024,
            hard: 64 * 1024 * 1024,
            resume: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadState {
    Open,
    Finished,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub accepted: u64,
    pub max_depth: usize,
    pub pauses: u64,
    pub paused_total: Duration,
}

struct Inner {
    staging: VecDeque<u8>,
    finished: bool,
    cancelled: bool,
    accepted: u64,
    max_depth: usize,
    pauses: u64,
    paused_since: Option<Instant>,
    paused_total: Duration,
}

pub struct StreamBuffer {
    inner: Mutex<Inner>,
    signal: Condvar,
    limits: Limits,
    poll: Duration,
}

impl StreamBuffer {
    pub fn new(limits: Limits) -> Self {
        Self {
            inner: Mutex::new(Inner {
                staging: VecDeque::new(),
                finished: false,
                cancelled: false,
                accepted: 0,
                max_depth: 0,
                pauses: 0,
                paused_since: None,
                paused_total: Duration::ZERO,
            }),
            signal: Condvar::new(),
            limits,
            poll: Duration::from_millis(50),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    pub fn depth(&self) -> usize {
        self.lock().staging.len()
    }

    pub fn stats(&self) -> Stats {
        let g = self.lock();
        Stats {
            accepted: g.accepted,
            max_depth: g.max_depth,
            pauses: g.pauses,
            paused_total: g.paused_total,
        }
    }

    pub fn wait_for_capacity(&self, want: usize) -> ReadState {
        let mut g = self.lock();
        loop {
            if g.cancelled {
                return ReadState::Cancelled;
            }
            if g.finished {
                return ReadState::Finished;
            }
            if g.staging.len().saturating_add(want) <= self.limits.hard
                && (g.paused_since.is_none() || g.staging.len() <= self.limits.resume)
            {
                return ReadState::Open;
            }
            if g.staging.len() > self.limits.soft && g.paused_since.is_none() {
                g.pauses += 1;
                g.paused_since = Some(Instant::now());
            }
            let (ng, _) = self
                .signal
                .wait_timeout(g, self.poll)
                .unwrap_or_else(|e| e.into_inner());
            g = ng;
        }
    }

    pub fn push(&self, bytes: &[u8]) {
        let mut g = self.lock();
        if let Some(t) = g.paused_since.take() {
            g.paused_total += t.elapsed();
        }
        g.staging.extend(bytes.iter().copied());
        g.accepted += bytes.len() as u64;
        if g.staging.len() > g.max_depth {
            g.max_depth = g.staging.len();
        }
        drop(g);
        self.signal.notify_all();
    }

    pub fn take(&self) -> Vec<u8> {
        let mut g = self.lock();
        let out = std::mem::take(&mut g.staging).into_iter().collect();
        drop(g);
        self.signal.notify_all();
        out
    }
    pub fn take_up_to(&self, limit: usize) -> Vec<u8> {
        let mut g = self.lock();
        let n = limit.min(g.staging.len());
        let out = g.staging.drain(..n).collect();
        drop(g);
        self.signal.notify_all();
        out
    }

    pub fn finish(&self) {
        let mut g = self.lock();
        g.finished = true;
        drop(g);
        self.signal.notify_all();
    }

    pub fn cancel(&self) {
        let mut g = self.lock();
        g.cancelled = true;
        drop(g);
        self.signal.notify_all();
    }

    pub fn is_finished(&self) -> bool {
        self.lock().finished
    }

    pub fn is_cancelled(&self) -> bool {
        self.lock().cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(state: &mut u64) -> u8 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*state >> 33) as u8
    }

    fn payload(n: usize, seed: u64) -> Vec<u8> {
        let mut s = seed;
        (0..n).map(|_| lcg(&mut s)).collect()
    }

    #[test]
    fn byte_exact_round_trip() {
        let raw: &[u8] =
            b"\x1b[31mred\x1b[0m \xf0\x9f\x91\x8d \xff\xfe invalid \x1b[?1049h\x1b[38;2;255;0;0m";
        let buf = StreamBuffer::new(Limits::default());
        for chunk in raw.chunks(3) {
            assert_eq!(buf.wait_for_capacity(chunk.len()), ReadState::Open);
            buf.push(chunk);
        }
        buf.finish();
        let out = buf.take();
        assert_eq!(out, raw);
        assert_eq!(buf.stats().accepted, raw.len() as u64);
    }

    #[test]
    fn empty_stream_is_clean() {
        let buf = StreamBuffer::new(Limits::default());
        buf.finish();
        assert!(buf.is_finished());
        assert!(buf.take().is_empty());
        assert_eq!(buf.stats().accepted, 0);
    }

    #[test]
    fn ordering_preserved_under_pressure() {
        let limits = Limits {
            soft: 8 * 1024,
            hard: 16 * 1024,
            resume: 4 * 1024,
        };
        let buf = std::sync::Arc::new(StreamBuffer::new(limits));
        let total = 512 * 1024;
        let expected = payload(total, 42);

        let producer = {
            let buf = buf.clone();
            let expected = expected.clone();
            std::thread::spawn(move || {
                let b = buf.clone();
                for chunk in expected.chunks(1024) {
                    assert_eq!(b.wait_for_capacity(chunk.len()), ReadState::Open);
                    b.push(chunk);
                }
                b.finish();
            })
        };

        let mut got = Vec::with_capacity(total);
        while !buf.is_finished() || buf.depth() > 0 {
            let batch = buf.take();
            if batch.is_empty() {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            got.extend_from_slice(&batch);
        }
        got.extend_from_slice(&buf.take());
        producer.join().unwrap();

        assert_eq!(got, expected);
        assert_eq!(buf.stats().accepted, total as u64);
    }

    #[test]
    fn depth_stays_bounded_under_pressure() {
        let limits = Limits {
            soft: 4 * 1024,
            hard: 8 * 1024,
            resume: 2 * 1024,
        };
        let buf = std::sync::Arc::new(StreamBuffer::new(limits));
        const CHUNK: usize = 1024;
        let filler = [7u8; CHUNK];

        let producer = {
            let buf = buf.clone();
            std::thread::spawn(move || {
                let b = buf.clone();
                for _ in 0..256 {
                    assert_eq!(b.wait_for_capacity(CHUNK), ReadState::Open);
                    b.push(&filler);
                }
                b.finish();
            })
        };

        while !buf.is_finished() || buf.depth() > 0 {
            let _ = buf.take();
            std::thread::sleep(Duration::from_millis(1));
        }
        producer.join().unwrap();

        let s = buf.stats();
        assert!(
            s.max_depth <= limits.hard + CHUNK,
            "max_depth {}",
            s.max_depth
        );
        assert!(s.pauses > 0, "expected at least one pause");
        assert_eq!(s.accepted, 256 * CHUNK as u64);
    }

    #[test]
    fn cancel_unblocks_a_full_reader() {
        let limits = Limits {
            soft: 16,
            hard: 32,
            resume: 8,
        };
        let buf = std::sync::Arc::new(StreamBuffer::new(limits));
        let block = [0u8; 64];
        buf.push(&block);

        let reader = {
            let buf = buf.clone();
            std::thread::spawn(move || buf.wait_for_capacity(1024))
        };
        std::thread::sleep(Duration::from_millis(80));
        buf.cancel();

        assert_eq!(reader.join().unwrap(), ReadState::Cancelled);
        assert_eq!(buf.stats().accepted, 64);
    }

    #[test]
    fn accepted_bytes_are_never_discarded() {
        let buf = StreamBuffer::new(Limits::default());
        buf.push(b"abc");
        buf.push(b"def");
        buf.push(b"ghi");
        assert_eq!(buf.stats().accepted, 9);
        assert_eq!(buf.take(), b"abcdefghi");
        assert_eq!(buf.take(), b"");
        assert_eq!(buf.stats().accepted, 9);
    }
}
