//! CPU-only owned scrollback snapshots; excludes input, rendering and presentation.
use std::time::Instant;
fn main() {
    let workload = ("x".repeat(78) + "\r\n").repeat(51200);
    let mut terminal = juna::vt::TerminalState::new(100, 30);
    for bytes in workload.as_bytes().chunks(65536) {
        terminal.advance(bytes);
    }
    for offset in [0, 128, 4096, terminal.scrollback_len()] {
        for _ in 0..10 {
            std::hint::black_box(terminal.snapshot(offset));
        }
        let mut samples = Vec::new();
        for _ in 0..500 {
            let start = Instant::now();
            let snapshot = terminal.snapshot(offset);
            let elapsed = start.elapsed().as_secs_f64() * 1e6;
            assert_eq!(snapshot.cells.len(), 3000);
            std::hint::black_box(snapshot);
            samples.push(elapsed);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "cols=100 rows=30 offset={offset} samples=500 median_us={:.3} p95_us={:.3}",
            samples[250], samples[475]
        );
    }
}
