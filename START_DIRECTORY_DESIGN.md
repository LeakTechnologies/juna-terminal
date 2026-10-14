# Home directory on normal startup

Requested 2026-10-14. Explicitly approved by the design lead under AGENTS.md §3.

- A normal interactive launch starts its shell in the Windows user home directory,
  rather than inheriting the portable executable folder. Resolve and validate the home
  directory at the app/session boundary; never change the process-wide working directory.
  Report an actionable error if no usable home folder exists, without logging its contents.
- Add an explicit --cwd directory argument for deliberate folder launches. Ctrl+Shift+N
  passes the validated reported folder (or launch-folder fallback) explicitly, preserving
  the approved independent-window behaviour. One-off shell command launches retain their
  inherited folder unless --cwd is supplied, so existing scripts are not silently relocated.
- Record the change in dev11 with matching notes and decisions. Test home/default/explicit
  selection, argument boundaries and new-window folder inheritance; run required checks
  and controlled portable-startup/window acceptance before pushing and verifying assets.
