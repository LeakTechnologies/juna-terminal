# Reproducible terminal measurements

`juna-bench` is a separate Windows x64 developer utility. Normal Juna sessions do not run
the benchmark, capture windows, or collect these counters. No extra installation or system
tracing is required. Build with `cargo build --release --bin juna-bench`, then run:

```powershell
./target/release/juna-bench.exe --juna ./target/release/juna.exe --out ./target/bench-baseline --profile quick
```

The output directory must be new. Keep the desktop unlocked, test windows visible and
unobstructed, and avoid typing into them or changing their size. Close competing builds,
games and benchmarks. Keep the same resolution, scaling, refresh rate, graphics driver and
power mode for comparisons. Full runs use five trials and thirty response samples per input
trial; quick runs use three trials and five samples. A warm-up window is excluded.

Each trial launches a new isolated Juna window with the benchmark's synthetic console child,
IBM Plex Mono 16, Catppuccin Mocha and the Git bar disabled. It does not load a user's shell
profile, run project commands, or operate on existing terminal sessions. Runtime files live
under the chosen output directory. Reports contain counters, hashes and synthetic workload
names, without window images, terminal transcripts, environment values or personal paths.

The tool writes `report.json` and standalone `report.html`. Input measurements are a targeted
posted A-key message through Juna to the child, followed by observation of a distinct colour
in that owned window. Child receipt is timestamped immediately after reading input, before
writing output, with the Windows performance counter shared across processes. Only timing
deltas are exported; ordering has one counter tick of uncertainty. Frame observation is a
separate value including local file coordination, polling and capture overhead: a software response proxy,
**not physical keyboard-to-display latency**. A stale marker, missing input, capture failure
or unsuccessful process shutdown rejects the run rather than generating a passing score.
Graphics Capture requires Windows 10 1903 or later and a supported working D3D11 device.
There is no desktop/monitor fallback. A separately supervised capture process bounds driver
stalls and closes normally on success. Cleanup closes only children created by this run.

Output workloads cover ASCII, Unicode and truecolour text with scrollback growth. Their
counts describe producer bytes, not byte-exact ConPTY traffic, and final-frame observation
does not prove that every intermediate frame was displayed. The flood workload repaints
a fixed row continuously while testing input response on a separate row. Parent CPU is
reported in seconds, without conflating total-system CPU percentages or logical cores.

Memory reports include private bytes, working set, peak working set and committed regions
classified as private/mapped/image. Large private-region counts are metadata, not a claim
about allocation owners or call stacks. These counters exclude the child, capture helper and
GPU memory. Startup ends at fixture readiness; hashing the executable and warming fonts
make it a warm fixture-startup measurement, not a cold-start or first-paint result. Repeated
trial distributions and small-sample p95 values are exploratory; retain the raw samples.
The report identifies the target executable hash/version and the benchmark source hash.
The embedded build commit is labelled as a base commit because a local build can include
uncommitted source. The source hash covers the benchmark modules in their declared order.

## Comparison stages

1. Run the identical utility/profile against two Juna builds. Compare the same workload,
   viewport and resource scope, and retain executable hashes and raw reports.
2. Add Windows Terminal and Alacritty adapters after verifying equivalent owned-window,
   input, completion and cleanup semantics. No cross-terminal result is claimed yet.
3. Add scroll interaction, multiple sessions and true Ctrl+C response once their delivery
   and visible completion can be measured reliably. Measure on the affected laptop too.
4. Use optional PresentMon for presentation/frametime investigations and WPR/WPA for
   allocation stacks or scheduling traces. Review trace privacy and overhead first; the
   benchmark does not silently install them or start system-wide tracing.

[vtebench](https://github.com/alacritty/vtebench) measures terminal throughput and explicitly
does not measure latency or frame rate. Its native Windows suitability is unverified here.
[Kitty's methodology](https://sw.kovidgoyal.net/kitty/performance/) distinguishes rendering
from output processing; its default benchmark suppresses rendering. Neither project supplies
Juna's engine implementation. [PresentMon](https://github.com/GameTechDev/PresentMon) and
[Microsoft WPR/WPA](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/memory-footprint-optimization)
are optional profiling references, not current benchmark dependencies.
