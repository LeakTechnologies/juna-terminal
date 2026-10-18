# Dev20 native benchmark evidence

Verified on the desktop host on 2026-10-18. The final source-built dev20 utility ran
both profiles without compilation active at preflight. This is exploratory desktop evidence,
not laptop acceptance, a cross-terminal ranking or physical keyboard-to-display latency.

- `full-dev19.json` / `.html`: the published dev19 executable; five trials per workload,
  thirty input responses per input trial, and one excluded warm-up.
- `quick-dev20.json` / `.html`: the local dev20 release executable; three trials per
  workload, five responses per input trial, and one excluded warm-up.
- Each report records executable hash/version, benchmark source hash, build base commit,
  viewport, clock frequency, raw samples, median/p95 and limitations. The source hash was
  checked against the exact ordered benchmark modules, independently of the embedded commit.
- All 42 owned windows, including warm-ups, closed successfully. All 330 measured input
  responses passed receipt/unique-frame ordering checks. Synthetic staging was removed.

Full dev19 medians:

| Workload | Idle private MiB | After private MiB | Parent CPU s | Elapsed s | Response proxy ms |
|---|---:|---:|---:|---:|---:|
| Idle input | 418.3 | 417.1 | 0.08 | 2.08 | 28.70 |
| ASCII | 417.3 | 487.8 | 0.38 | 0.39 | unmeasured |
| Unicode | 418.9 | 489.0 | 0.39 | 0.44 | unmeasured |
| Colour | 418.1 | 488.0 | 0.30 | 0.32 | unmeasured |
| Continuous fixed-row output | 417.2 | 416.9 | 0.81 | 7.97 | 210.74 |

Child receipt medians were 0.30 ms idle and 5.96 ms during continuous output; receipt
p95 was 0.49 ms and 7.99 ms respectively. Receipt is timestamped before child output,
while the response proxy includes capture/polling/scheduling overhead. **Hypothesis:** much
of the observed response delay lies after input reaches the child. This does not identify
the engine, output transport, renderer, compositor or capture overhead as the cause.

Private bytes are committed process-private memory, not a measurement of resident RAM or
GPU memory. Region categories do not attribute allocation stacks. Output trials deliberately
grow scrollback. The two profiles use different output sizes/sample counts, so these reports
are not a before/after performance claim. No memory or responsiveness fix is shipped here.

Validation in this session: check, strict all-target Clippy, all 250 executed standard tests,
release builds, native x64/version/seven-icon verification and bundled-tool pins passed.
Ignored GPU/window/network acceptance tests were not rerun as part of this cycle; the
benchmark's own 42 real-window runs supply its input/capture/cleanup evidence.

The first restricted test attempt failed on temp/preference/tool/pipe access. The unchanged
assertions passed with normal native Windows access. Initial capture teardown crashed in an
unloaded GraphicsCapture DLL; process-lifetime WinRT initialisation resolved the observed
failure before the retained runs. A null optional frame was also distinguished from real
failing HRESULTs. The retained reports were generated after those corrections.

Failure evidence: invalid executable preflight emitted no success report. Separately,
deliberately closing only the fresh Juna child during warm-up caused bounded rejection,
removed synthetic staging and emitted no report. Existing user windows were not closed.

Next: attribute idle private allocations and isolate the post-receipt delay under output
pressure; then repeat identical workloads on the affected laptop. See BENCHMARKS.md for
methodology, unsupported measurements and the comparison-adapter roadmap.
