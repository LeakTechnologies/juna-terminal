# Terminal-wide privacy extension

Requested 2026-10-13, after the current interface iteration. Extends the approved Junafetch
privacy design; unrelated command output was previously outside that design's scope.

- Add an optional terminal-wide policy for recognized IPv4/IPv6 and MAC addresses, automatically registered local identity values, and bounded user-defined literal values. Classify owned snapshots before presentation, including wrapped text and overwritten TUI cells; preserve the original ordered byte stream and VT transitions. Junafetch's structured reports retain stronger field knowledge and can keep secrets outside VT cells. General commands' raw data remains inside the engine/process; history display and sharing are filtered, while child-process files/logs are outside Juna's control. Unknown secrets and unrecognizable early fragments cannot be guaranteed; document false positives and provide literal exceptions.
- Default intentional reveal to Shift+hover, including the Always hidden mode's explicit override. Require Shift with no Ctrl/Alt/Super, no mouse button held, and pointer movement over the protected field after Shift is pressed; hide on modifier release, pointer exit, focus loss or tab switch. Use solid masks rather than rendering readable glyphs behind blur. Normal copies remain censored even during reveal, including partial selections. Off mode is a saved user choice. Keep privacy controls compact and separate from appearance settings.
- Maintain an app-owned last committed snapshot per tab so UI/privacy redraws are immediate during synchronized terminal output without exposing partial terminal transactions. Renderer consumes only masked snapshots plus owned reveal overlays; engine/backend/layout boundaries remain intact. Verify split chunks, ANSI styling, wraps, scrollback, cursor rewrites, alternate screens, inactive tabs, partial copies, all reveal/hide conditions and exact unchanged VT responses. Record visual evidence and run required gates before delivery. Privacy protects Juna's display/sharing, not arbitrary OS captures outside Juna or data created by another program.

Implementation order: finish approved interface work, then Junafetch plus this privacy extension,
then GitHub release/updater work. No new runtime/dependency without a written requirement.

Explicitly approved by the design lead on 2026-10-13.
