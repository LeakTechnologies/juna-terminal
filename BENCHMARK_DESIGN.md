# Native benchmark utility

Status: the design lead approved the benchmark plan on 2026-10-18. This records the
implementation scope of that accepted plan before code changes.

- Build a separate Rust developer utility using the standard library and existing infrastructure
  crates. Drive only freshly launched, isolated Juna windows and deterministic synthetic child
  programs. Measure CPU/private memory/working set and committed region categories, varied
  output, startup, scrollback and responsiveness under load. Quick/full profiles run sequentially;
  preserve raw samples and report median/p95 without a composite speed score. No terminal engine
  implementation is imported, and no application/VT ownership boundary changes.
- Validate input delivery and a uniquely identifiable rendered response in the owned test window
  before publishing a latency number. Hide the cursor, use distinct per-sample markers and reject
  stale/ambiguous captures. Timestamp injection, child receipt and observed frame independently;
  software capture is a proxy for physical display latency. Unsupported or failed measurements
  are explicit, never zero or silently replaced by parser/shell acknowledgments. Bound capture,
  input, output and process cleanup. Never inject into arbitrary foreground windows or close
  existing sessions. Keep captured pixels ephemeral; reports contain synthetic counters only.
- Produce local versioned JSON plus an escaped standalone HTML comparison. Record executable
  identity, workload, viewport/font, sample counts, timing limits and capability gaps. WPR/WPA and
  PresentMon are optional external profiling tools; never install/start system tracing silently.
  Extend comparison adapters only after verifying equivalent completion/measurement semantics.
  Juna/Windows Terminal/Alacritty comparison and Kitty/vtebench methodology stay separate from
  importing engine code. Test report validation/statistics/failure paths and real owned windows.

Implementation proceeds in small completed releases. An unavailable capture backend is a
reported limitation to resolve, not evidence of keyboard-to-paint latency. Release/workflow
changes receive the inherited explicit review before modification. The utility is separate
from the normal terminal UI and package; initial developer builds remain locally available.

Acceptance: deterministic workloads, bounded cleanup after failures, no user-profile/session
changes, no transcripts/private paths in reports, no claimed paint timing without observed
markers, required check/Clippy/tests, real-window evidence and fresh verified release cadence.

The first completed slice is the owned-Juna adapter: idle input, ASCII/Unicode/colour
output (including scrollback growth), and A-key response during continuous fixed-row output.
Cross-terminal adapters, scroll interaction, multiple tabs and true Ctrl+C timing follow only
after equivalent input/completion semantics can be verified. A supervised capture subprocess
bounds blocking driver calls; only its owned child may be terminated on timeout. Windows
Graphics Capture requires Windows 10 1903 and a working supported D3D11 device; failure rejects
the run. Capture has no monitor/desktop fallback.

WinRT initialisation remains active for the capture subprocess lifetime. Local session/pool
resources still close normally. Uninitialising COM before process exit caused an observed
GraphicsCapture.dll_unloaded access violation after a successful marker read. Keeping COM
alive follows Microsoft's cached-factory guidance; the targeted probe then exited normally.
This is native lifetime evidence on this host, not proof across all graphics drivers.
https://devblogs.microsoft.com/oldnewthing/20211105-00/?p=105878
