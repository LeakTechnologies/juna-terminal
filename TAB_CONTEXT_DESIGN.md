# Separate tab and terminal context menus

Requested 2026-10-13 under AGENTS.md §3.

Explicitly approved by the design lead on 2026-10-13; existing keyboard shortcuts retained.

- Right-click on a tab opens a tab-only menu: Open tab, Rename tab and Close tab. Actions target the clicked tab, even if inactive, without first switching or changing its terminal state. Right-click empty tab-strip space offers Open tab only. The plus button remains available. Themes, settings and clipboard actions never appear in the tab-strip menu.
- Right-click terminal content retains Copy, Paste, Select all, Themes, Settings and the approved line-number toggle; remove Open/Close tab there. Preserve terminal/TUI mouse reporting and Shift+right-click override. Keep intentional keyboard shortcuts Ctrl+Shift+T/W and Ctrl+Tab as existing behavior; the user's location restriction is interpreted as applying to mouse context menus. Rename uses a local text prompt with Enter/Save and Esc/Cancel, bounded printable text and no shell/OSC involvement. Renamed labels survive tab switches for the lifetime of that session.
- Keep menu hit-testing and target identity app-owned. Store the stable tab ID, not a mutable index; dismiss actions safely if their session exits while the menu/prompt is open. Close reaps only the selected session, preserving other tabs and their output; closing the last follows existing window shutdown. Opening a tab menu releases tracked terminal mouse buttons and consumes menu input. Verify active/inactive targeting, rename cancellation, shell exit races, strip/body separation, TUI mouse routing, keyboard controls and final-session cleanup with required gates and visual/window evidence. No dependency or VT-engine change.

This revision supersedes the shared terminal-menu tab actions in UI_NEXT_DESIGN.md.
Remaining approved line-number, Junafetch/privacy, updater and CI work stays in scope.

## Approved middle-click extension

The lead selected Require confirmation first on 2026-10-13. Middle-click on a tab opens a
confirmation identifying that tab and warning that its commands will stop, with Cancel selected
by default. Confirming closes that exact session through existing graceful/reaping cleanup.
No prompt-text or process-name heuristic decides whether it is busy. Other tabs continue.
Middle-click in terminal content retains the terminal's own mouse behavior. Explicit Close tab
menu actions and existing keyboard shortcuts retain their already-approved behavior.
Verify inactive/active targeting, child-process teardown and body/header routing.

The lead refined the confirmation to a simple Close tab? title, a green check for Close and
red cross for Cancel. Use the selected theme's green/red and background with contrast correction;
retain text labels and the default Cancel focus. Repeated right-clicks reopen the appropriate
context menu at the new pointer location rather than only closing the old menu.

The lead further refined right-click behavior: only Context/TabContext popups reposition.
Dedicated theme/settings/font/editor/number/rename/confirmation panels ignore right-clicks
and stay open; their explicit controls/Escape remain available.
