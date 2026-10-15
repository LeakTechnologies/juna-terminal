# Performance baseline and laptop efficiency

Status: approved by the design lead on 2026-10-15 under AGENTS.md §3.

The lead also confirmed that the current update cycle must include the manual GitHub
latest-release update flow in Settings → Updates, including verified download and
Install and Restart. Finish its acceptance alongside this performance review; do not
defer the updater to a later cycle. Performance fixes must follow measured evidence.

## Purpose

Measure Juna's responsiveness and resource use before changing scheduling or rendering.
Use published behaviour and benchmark methodology from other terminals, never their
terminal-engine implementation. Keep performance evidence distinct from updater acceptance
so each claim is traceable, while retaining the updater in the current release scope.

## Initial source findings (not measured bottlenecks)

- The application schedules a wake approximately every 16 ms, including idle periods.
  Redraw itself is dirty-gated; this is not evidence of continuous idle rendering.
- Each redraw builds an owned full-viewport snapshot and visits every visible cell.
- Text preparation allocates strings and clones cached glyph vectors per populated cell.
- Atlas construction scans system fonts and copies selected font data; font-size changes
  construct a new atlas. Measure startup and repeated live-sizing costs separately.

## Baseline

Use optimised release binaries, record commit/version, Windows build, GPU/driver/backend,
power mode, display scaling, viewport cells, font and scrollback settings. Never infer
runtime RAM from executable size. Keep shell startup/profile costs separate from Juna.

Compare Juna, Alacritty and Windows Terminal on the same Windows machine. Kitty is a
behaviour/methodology reference; a Linux/WSL graphics result is a separate environment,
not a direct Windows ranking. Reference tools are optional and never installed silently.

Measure cold/warm startup to usable frame; idle/minimised CPU and process memory;
ASCII and Unicode output throughput; scrollback memory growth; scrolling and resize
frame times; input responsiveness under sustained output; and one versus multiple tabs.
Report process scope consistently, separating shell children, private memory, working set
and GPU memory. Repeat runs and report distributions, workload size and measurement limits.
Automated event timings are proxies unless actual input-to-presentation is measured.

Collect only bounded counters and timings. Do not record terminal text, commands,
environment values, repository names or private paths. No telemetry or network service.

## Follow-up optimisation gate

Select the largest measured cost first. Candidate work includes sleeping until real
activity, bounded output scheduling, reusable render allocations, font database reuse,
and damage-aware snapshot/render preparation. Preserve owned read-only snapshots,
single engine ownership, ordered output and backpressure. Coalesce frames, never bytes.
Any changed architecture needs an approved amendment before implementation.

## Acceptance

Save a reproducible baseline and a before/after table for each optimisation. All required
build/lint/tests and byte-exact transport checks must pass. Verify Unicode, alternate
screen, resize, multiple sessions and shutdown. Record real-window evidence and obtain
laptop review; a desktop improvement alone does not establish laptop acceptance.
Set numeric improvement targets after measuring the baseline, not before.

## Methodology references

- https://sw.kovidgoyal.net/kitty/performance/ — latency, scrolling, CPU and benchmark tool.
- https://github.com/alacritty/vtebench — throughput workloads; explicitly insufficient
  alone for frame-rate or latency conclusions. No reference engine source is required.
