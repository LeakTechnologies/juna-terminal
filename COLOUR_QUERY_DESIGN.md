# Terminal palette queries for system-themed applications

Requested after OpenCode/Omarchy comparison on 2026-10-13. Awaiting approval under AGENTS.md §3.

The design lead chose Defer compatibility work on 2026-10-13. Do not implement this proposal
as part of the current pass; OpenCode configuration and Juna colour queries remain unchanged.

- Add original, specification-based read-only OSC queries: 4 for indexed colours and 10/11/12 for configured foreground/background/cursor. Reply with xterm RGB syntax through the existing ordered response queue. Parse bounded query pairs, ignore unsupported/malformed requests, and keep colour-setting/reset sequences explicitly unsupported in this iteration. No borrowed terminal code or dependency.
- Supply a Juna-owned pure palette value to each engine from the app's selected/previewed theme; the engine performs no I/O or settings lookup. Keep preview/committed themes and new/inactive tabs synchronized. The renderer continues to consume owned snapshots and its presentation policy; replies report nominal configured colours, not per-cell contrast corrections. Advertise truecolour to Juna child sessions through the existing child-only environment boundary without changing global profiles.
- Verify all 16 theme ANSI colours, extended indexed colours, foreground/background/cursor, split chunks, malformed requests, inactive/new tabs, preview cancellation and byte-exact responses without changing grid cells. Record actual OpenCode system-theme acceptance when available, plus required gates. Do not override OpenCode's own chosen theme/configuration automatically or claim all existing RGB application colours update live; a client may need to re-query/reload. Selecting OpenCode's system theme is the same published configuration approach used by Omarchy.

Primary specifications and configuration references:
- https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
- https://opencode.ai/docs/themes/
- https://raw.githubusercontent.com/omacom/omarchy/quattro/config/opencode/opencode.json
