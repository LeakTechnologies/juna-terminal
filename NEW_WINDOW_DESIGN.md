# New terminal window shortcut

Requested 2026-10-13. Explicitly approved 2026-10-13 under AGENTS.md §3; a separate small release cycle.

- Reserve Ctrl+Shift+N for a new independent Juna process/window, with no Alt/AltGr activation.
  Start the configured shell interactively, without replaying one-off command arguments.
  Use the validated reported local folder when known, otherwise the launch folder. Read saved
  preferences normally. Closing either window leaves the other window and its shell running.
- Spawn the current executable by absolute path, preserving argument boundaries. Surface a
  generic actionable launch error without logging private folder/command contents. Never
  execute the shortcut inside a child shell or launch it through a script/intermediate console.
  Preserve editing/IME ownership; use the same global shortcut handling path as tab shortcuts.
- Give each running window its own diagnostic file to prevent shared juna.log truncation or
  interleaved writes; include a simple way to locate the current log. Validate shortcut modifiers,
  independent process/session cleanup, cwd/preferences inheritance and launch failure; run
  mandatory checks and a controlled two-window desktop test before the new development release.
