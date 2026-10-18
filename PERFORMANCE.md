# Performance evidence

## Dev19 output allocation improvement (2026-10-18)

The lead accepted this reliability/performance continuation under PERFORMANCE_DESIGN.md.
Published dev18 was measured with three isolated no-profile PowerShell runs, Consolas 16px,
Git footer off and no compilation during measurement. The candidate was measured the same
way; its raw executable hash identifies the working-tree build (still labelled dev18 during
measurement). Sanitised raw counters and the standalone allocation probe are retained under
`docs/performance/dev19`. No reference-terminal engine source was used.

| Three-run median | Published dev18 | Candidate |
| --- | ---: | ---: |
| Parent CPU, 4,096,000 bytes | 3,515.62 ms | 2,500.00 ms |
| Output launch to exit | 4,572.29 ms | 3,538.76 ms |
| Idle private memory | 435.4 MiB | 436.1 MiB |
| Sampled idle CPU, approximately 3 seconds | 0 ms | 15.62 ms |
| Instrumented engine allocations | 12,084,179 | 256,979 |
| Instrumented engine time | 2,045.81 ms | 988.91 ms |

The parent output CPU reduction is about 28.9%; launch-to-exit is about 22.6% lower in this
small probe. The isolated allocation probe uses the standard System allocator with atomic
counters, which affects timings; it is not uninstrumented application throughput. It processes
4 MiB in 64 KiB chunks at 100x30 cells, retaining identical history length (13,981) and revision
(63). The baseline linked an existing optimised library; `git diff v0.1.1-dev15..v0.1.1-dev18
-- src/vt` is empty, so its engine source matches the published baseline. Both probes use the
same compiler/allocator and synthetic input. Candidate probe source is supplied for reproduction;
compile against the chosen Cargo release library and its dependency/native import-library paths.

The narrowly bounded fast path skips clone/string/grapheme work only for printable ASCII
scalar pairs, which have a boundary under Unicode 17 UAX #29 GB999. Non-ASCII Prepend, Extend,
ZWJ, emoji, wide characters and controls keep existing handling. The regression checks all
9,025 printable pairs against the existing Unicode segmentation data and byte-split Unicode
examples, including a Prepend followed by ASCII. No engine ownership, parser/transport,
renderer snapshot, scrollback policy or dependency changes.

No idle-memory improvement is demonstrated. Short idle CPU samples vary at timer resolution
and do not show an idle/battery benefit. These are desktop parent-process measurements,
excluding child-shell and GPU memory; power mode/background activity is uncontrolled. Window
handle timing is not first presentation/input latency. No general terminal ranking or laptop
acceptance is claimed. Physical Explorer drops and the exact OpenCode interaction still need
human retesting; automated startup/interrupt tests cannot establish that exact scenario.

The candidate's CPU-only scrollback probe measured median owned-snapshot construction of
8.0–8.1 microseconds (p95 8.2) at 100x30 cells, across offsets 0, 128, 4,096 and 13,981, with
10 warmups and 500 samples each. Source and raw counters are retained alongside the other
reports. This excludes snapshot destruction, rendering, GPU presentation and mouse/input
latency; it is a current baseline, not a measured scrolling improvement.

Validation: check, strict all-target Clippy, all 244 standard tests, all nine real-window
checks and the isolated OpenCode startup-cancellation probe passed. Native resource and tool
pin verification passed. Full OpenCode interaction, physical drops and laptop performance
remain human acceptance; hosted release evidence follows the fresh tag.

Specification: https://www.unicode.org/reports/tr29/tr29-47.html#Grapheme_Cluster_Boundary_Rules

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
