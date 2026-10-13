# Optional terminal line numbers

Requested 2026-10-13 under AGENTS.md §3.

Explicitly approved by the design lead on 2026-10-13.

Visual reference supplied by the lead: 4318.png. Match the compact left gutter, right-aligned
numbers, shared text baseline and small gap before terminal content. Use the terminal's selected
font and a readable theme accent; avoid a boxed sidebar or prominent divider. The reference shows
an editor's source-line numbers; Juna's proposed gutter labels terminal rows and scrollback instead.
The screenshot establishes the requested appearance; the complete design received explicit approval.

- Add a saved Show line numbers toggle directly in the right-click Options menu, reachable by mouse or keyboard. Default off; apply immediately to every tab. Use right-aligned, readable theme-aware numbers in a narrow gutter on the left, leaving the tab strip and right-side panels unchanged.
- Number physical rows in the current retained primary buffer, including scrollback, from 1. Wrapped continuations receive their own row numbers; these are terminal rows, not commands or source-file lines. Numbers follow scrolling and each tab's own buffer. Evicted history resets the retained-buffer numbering; clearing history resets it too. Full-screen alternate-buffer applications use screen rows 1 through the visible height. The gutter never enters shell output, selection text or clipboard copies.
- Keep geometry app-owned and rendering read-only: derive labels from snapshot/history metadata, reserve a stable digit width based on the history capacity, and use one shared terminal origin for glyphs, cursor, selection, mouse reporting and privacy hit-testing. Toggle/font/DPI/window changes resize the PTY before the grid through the existing checked path; suppress the gutter if there is not enough room for a terminal column. No dependency or VT parsing change. Verify scrolling, wraps, tabs, alternate screen, selection/copy, mouse coordinates, sizing and privacy reveal alignment; run required gates plus GPU/window checks and record evidence before committing.

This is an additional interface feature within the ongoing pass; Junafetch, privacy, updater
and CI repair remain outstanding work.

The lead refined placement on 2026-10-13: Show line numbers belongs inside Settings, not the
main context menu. It previews immediately and follows the same Save/Cancel appearance transaction.

The lead's TUI feedback supersedes alternate-buffer numbering: automatically suppress Juna's
row labels in alternate screens used by full-screen clients such as OpenCode. Do not inspect
process names or alter the client's own colours. Settings persistence follows the later
PERSISTENT_SETTINGS_DESIGN.md, not the original Settings-wide Save/Cancel policy.

Alternate-screen transitions and tab activation refit the existing checked PTY/grid path so
the hidden gutter does not reserve TUI width. Reproject mouse coordinates when the inset changes.
Use a two-cell gap between right-aligned labels and terminal content for clear separation.
