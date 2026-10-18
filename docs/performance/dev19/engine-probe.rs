use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};
struct Counter;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);
// SAFETY: every allocation and deallocation forwards to the same System allocator.
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: forward the caller-valid layout unchanged.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: pointer and original layout belong to System.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: forward the caller-valid System allocation and requested size.
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static ALLOC: Counter = Counter;
fn main() {
    let workload = ("x".repeat(78) + "\r\n").repeat(51200);
    for run in 0..3 {
        let mut terminal = juna::vt::TerminalState::new(100, 30);
        ALLOCS.store(0, Ordering::Relaxed);
        ACTIVE.store(true, Ordering::Relaxed);
        let start = Instant::now();
        for chunk in workload.as_bytes().chunks(65536) {
            terminal.advance(chunk);
        }
        let elapsed = start.elapsed();
        ACTIVE.store(false, Ordering::Relaxed);
        println!(
            "run={} bytes={} allocations={} elapsed_us={} history={} revision={}",
            run + 1,
            workload.len(),
            ALLOCS.load(Ordering::Relaxed),
            elapsed.as_micros(),
            terminal.scrollback_len(),
            terminal.revision()
        );
        std::hint::black_box(terminal);
    }
}
