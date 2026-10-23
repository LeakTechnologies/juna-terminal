# Diagnostics in Juna

Status: the lead approved the expanded Juna/current-tab diagnostics design in conversation.
The networking extension below is future roadmap work, requiring its own implementation design.

- **Tools → Diagnostics → Juna / Current tab / Reports.** Juna shows build identity,
  process CPU/private memory/working set, available graphics information and session/queue
  counters. Quick benchmark and Full benchmark launch the existing isolated synthetic
  harness, with progress, cancellation and Open report. They never inject benchmark input
  into a working tab. Current tab shows its shell and verifiably associated local child
  processes, sampled CPU/memory, session uptime and available exit/fault status. Reports
  offers Open, Review and Export instead of filling the main menu with individual controls.
- **Observe the boundary we own.** A bounded platform worker obtains process identity and
  read-only counters; PID plus creation-time validation prevents attribution after PID reuse.
  The session seam supplies owned diagnostic metadata without transferring process handles
  or OS types into the engine/renderer. The UI receives immutable values, samples at most
  once per second while the diagnostics view is open, and stops sampling when closed.
  Preserve ordered output/backpressure and existing engine ownership. CPU percentages name
  their normalization; aggregate only processes whose association is verified, and label
  partial results. Never infer which app is foreground from screen contents or process names.
  OpenCode, Git and other local tools can receive resource/process diagnostics; application
  internals, remote SSH apps and Linux processes inside WSL require explicit later adapters
  or app cooperation. Unavailable data is labelled, not guessed or replaced by zero.
- **Local, reviewable evidence.** Monitoring never records terminal text, screenshots,
  keystrokes, command lines, environment variables, private paths, IPs or app memory contents.
  Default exports use session labels and counters rather than process names/PIDs. Preview
  exactly what will be exported; store JSON/HTML locally and send nothing automatically.
  Existing application logs are separate: opening the log folder is supported, but arbitrary
  app logs are never silently included or assumed safe because a censor is enabled. Bundle
  the benchmark helper with matching version/source/hash verification and required notices;
  do not download arbitrary executables to run diagnostics. Review the concrete release/package
  changes before modifying them. Prefer existing standard-library/platform dependencies;
  justify any additional Windows feature in DECISIONS.md and keep conpty-oxide pinned.

## Delivery and acceptance

Implement in small completed cycles: first the menu, Juna/current-tab resource view and local
report review/export; then verified benchmark delivery, Quick/Full launch, cancellation and
report opening. Application-specific log adapters and internal profilers remain later work.
The existing benchmark stays available separately until its verified menu delivery is complete.

Acceptance includes keyboard/mouse navigation, a running shell while the panel is open,
inactive/closed tabs and short-lived descendants, PID reuse/access denial, partial process
trees, accurate CPU/memory labels, bounded worker cancellation and no sampling after closure.
Prove that normal working tabs receive no synthetic input, that reports omit private fields,
and that diagnostic/benchmark failure cannot close existing sessions. Review fixture-write,
ConPTY, renderer/compositor and capture overhead before attributing response delay to Juna.
Run required check/Clippy/tests, retain real-window evidence, and follow the fresh development
tag/CI/downloaded-asset cadence. Report verified results separately from hypotheses and
human/laptop acceptance. No terminal-engine implementation is imported.

## Later: network diagnostics and benchmarking

Consider a Network submenu after local diagnostics mature: latency/jitter and packet loss,
DNS lookup timing, TCP/TLS connection timing, and optional throughput tests against a
user-selected endpoint. Separate connection diagnostics from bandwidth benchmarks and label
results by protocol, destination, sample count and test conditions. A failed or blocked ICMP
probe alone does not establish general connectivity failure.

All active tests are explicitly started, bounded and cancellable. Show the destination and
expected traffic/data limits before running a bandwidth test; favour user-controlled LAN or
self-hosted endpoints. No Juna-operated measurement server, automatic third-party probes,
network discovery, packet-content capture or background bandwidth use. Protect addresses,
hostnames and endpoint URLs in views and export previews; omit them from default exports.
Do not collect credentials, URL query secrets or app network payloads. App-specific remote
service tests need explicit adapters; never infer that a terminal app's backend is healthy
from a generic network test. Tool selection, protocol details, dependencies and acceptance
criteria belong in a later reviewed design. No networking tools are implemented here.
