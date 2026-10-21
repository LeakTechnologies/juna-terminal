# Application icon wiring

- Use the approved `assets/juna-logo.png` and its generated `assets/juna.ico` unchanged.
- Embed the ICO as a Windows executable resource using the GNU toolchain resource compiler; add no Rust dependency. Supply the window icon through Windows resource access and winit's platform icon API, without touching terminal state.
- Acceptance: validate executable icon resource entries, verify the live window has its icon, run required checks, and rebuild/push. Small-size artwork refinement stays separately reviewable.

## Taskbar class-icon repair (2026-10-21)

Explorer draws a running window's taskbar button from the window class icon, not the
per-window `WM_SETICON` handles winit installs. A window could report valid small/large
`WM_GETICON` handles while Explorer still showed a generic button. Register the same owned
HICONs with `GCLP_HICON`/`GCLP_HICONSM` before the window is shown and verify both class words.
No new dependency, file write, shortcut or AppUserModelID change.

Approved by the design lead on 2026-10-13: implement the existing approved icon.

