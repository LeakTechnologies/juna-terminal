# Next interface iteration

Requested 2026-10-13. The terminal stays responsive and live previews remain on the right.

- Add tabs with New Tab in the right-click menu and Ctrl+Shift+T, plus Ctrl+Tab/Ctrl+Shift+Tab switching and Ctrl+Shift+W closing. Each tab owns its terminal state, history and process runtime; the single UI thread fairly drains every tab's ordered output, including inactive tabs. Close/reap one session without interrupting others; resize each affected PTY before adopting its dimensions. Tiled terminals within a tab stay on the roadmap.
- Replace implicit numeric-row editing with explicit click/Space/Enter prompts for font size and contrast, visible focus/caret, Apply/Cancel and Left/Right adjustment. Font size accepts px and pt (1 pt = 96/72 logical px), retaining the current 16 px default. Keep the existing searchable font picker, live preview and transaction-wide Save/Cancel.
- Expand to 60 verified starter palettes grouped under collapsible theme families, including all four Catppuccin variants. Put user themes under Custom with Create, Manage, Edit/Duplicate and reversible removal. Discover supported Juna JSON and Windows Terminal scheme JSON under %LOCALAPPDATA%/Juna/themes/<Family>/, retaining compatibility with existing files at the folder root. An app-owned I/O worker scans bounded files without blocking engine output; list updates automatically while the menu is open. Invalid/duplicate files are preserved and reported; no theme executes code or downloads content. Include Open themes folder and native-format template instructions.

Acceptance: isolated tab screen/history/input/exit and fair sustained output; mouse and keyboard
activate the same settings controls; point/px parsing and cancellation; family search/expansion;
custom file discovery/import/collision/corruption behavior; original byte-exact transport; required
check/clippy/test gates; recorded GPU/window evidence; release build and push.

Explicitly approved by the design lead on 2026-10-13, in interface → Junafetch → updates order.

