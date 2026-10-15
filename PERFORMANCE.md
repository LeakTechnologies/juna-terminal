# Performance evidence

## Dev13 privacy regression probe (2026-10-15)

Repeated the isolated-window probe against the published dev12 executable and the dev13
release build: three runs each, Consolas 16px, no-profile PowerShell, fresh app caches and
Git footer off. No compilation ran during measurements. Sanitized raw reports are in
`docs/performance/dev13` and identify both executable hashes.

Median parent CPU for 4,096,000 bytes was 3.516 seconds in both builds. Launch-to-exit medians
were 6.032 and 5.886 seconds respectively; this small sample does not establish a speedup.
Median idle private memory was 433.9 versus 433.5 MiB; median sampled idle CPU was zero in
both builds over approximately three seconds. One dev12 idle sample recorded 93.75 ms.
The privacy broker adds no periodic I/O wakeup and protected metadata fits within the existing
48-byte cell ceiling. These results show no evident regression in this limited desktop probe;
they do not resolve high memory use, bulk-output cost, laptop performance or battery behaviour.

## 2026-10-15 desktop baseline

Windows 11 build 26200, 12 logical processors, NVIDIA RTX 3060/Vulkan for Juna.
Optimised development builds from the dev12 working tree based on dev11; source changes
are distinguished by executable SHA-256 in local reports. This is desktop evidence,
not laptop or stable acceptance. No other terminal's engine source was used.

The isolated-window probe uses PowerShell without a profile, Consolas at 16px
(12pt in Alacritty), fresh configuration/cache per run, and three runs. Juna's Git
footer is disabled. Alacritty 0.17.0 is the official portable Windows release; its
SHA-256 matches the GitHub asset digest. Reference executable SHA-256:
`5ad70ddc5c2a2bfe84084a2c4c73558e9360914f035a902d7a69277be7249f1f`.

Initial median observations:

| Measurement | Juna before optimisation | Alacritty 0.17.0 |
| --- | ---: | ---: |
| Idle private memory | 433.5 MiB | 147.8 MiB |
| Idle working set | 249.7 MiB | 138.5 MiB |
| CPU time over approximately 10 seconds idle | 78.12 ms | 15.62 ms |
| 4,096,000-byte ASCII output, launch to exit | 5.72 s | 2.95 s |
| Parent CPU time for that output workload | 3.31 s | 0.98 s |

These are small exploratory samples. Memory/CPU cover the parent process, not child
shells or GPU allocations. The output duration includes startup and shutdown, and
does not prove input latency, presentation rate or pure parsing speed. Viewport/font
geometry and scrollback policies are not identical; this is a practical comparison,
not a terminal ranking. Window-handle timing is recorded only as a startup proxy.
The first run may warm OS caches. Battery/power mode was not controlled.

## Redraw preparation improvement

The release-only probe uses 140 × 45 cells, IBM Plex Mono 16px, ten warm-up frames
and 200 timed samples per synthetic workload. It measures CPU preparation without
GPU submission/presentation. The before/after runs use the same desktop and probe.

| Workload | Before median | After median | Before p95 | After p95 |
| --- | ---: | ---: | ---: | ---: |
| ASCII text | 2.119 ms | 0.951 ms | 2.637 ms | 1.241 ms |
| Light coloured boxes | 14.708 ms | 0.989 ms | 17.754 ms | 1.283 ms |
| Unicode text | 2.086 ms | 0.960 ms | 2.634 ms | 1.061 ms |

Reuse the exact existing contrast result for adjacent equal foreground/background
pairs, borrow cached glyph slices, encode scalar text on the stack and reuse foreground
instance storage. Colour correction, shaping and clipping rules are unchanged. Renderer
ownership and terminal state are unchanged. No dependencies were added for performance.

The initial atlas construction median was approximately 76 ms. Snapshot copying was
approximately 17–33 microseconds in these workloads. These observations favour fixing
render preparation first; they do not establish the cause of the high process memory.

The repeated window measurements did not show an end-to-end output improvement:
the post-change median was 5.89 seconds and 3.36 seconds of parent CPU, versus 5.72
seconds and 3.31 seconds before. Private memory remained approximately 435 MiB.
The small change is within the variation of these exploratory runs; the bulk-output
and high-memory problems remain open. Faster redraw preparation is the demonstrated
improvement, not a claim that the terminal is now generally faster or lighter.

## Reproduce

Build the release executable, then run `scripts/measure-performance.ps1 -Executable
target/release/juna.exe` for idle measurements and add `-Workload Flood` for output.
Supply separate `-Output` filenames for before/after. A verified portable Alacritty
binary can use the same script with `-Terminal Alacritty`. It is not installed by the script.

Run `cargo test --release --test performance_acceptance -- --ignored --nocapture`
for CPU preparation timings. `JUNA_PERF_REPORT` selects the output JSON path. Keep
compilation and other benchmarks out of the measurement interval. Reports are under
ignored `target/acceptance/performance`; they contain counters/timings rather than
terminal transcripts or environment values.
Sanitised counter reports from this iteration are retained in `docs/performance/dev12`.

Next investigations: private-memory attribution, font reuse during live sizing, idle
wakeups, output scheduling and input responsiveness under load. Reproduce on the laptop
before declaring its performance problem resolved. Kitty's published methodology is useful,
but its current releases supply no native Windows binary for a direct host comparison.

References: [Kitty performance](https://sw.kovidgoyal.net/kitty/performance/),
[Alacritty throughput benchmark limits](https://github.com/alacritty/vtebench),
[Alacritty 0.17.0 release](https://github.com/alacritty/alacritty/releases/tag/v0.17.0).
