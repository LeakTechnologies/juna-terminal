# Tab reordering, directory titles and shell selection

Requested 2026-10-20 under AGENTS.md §3. Approved with live directory tracking enabled by
default.

- **Drag to reorder.** Left-press selects a tab as today; 4 logical pixels of travel starts a
  drag and a themed insertion marker shows the drop gap. Reordering is a remove/insert of tab
  records with `active_tab` re-derived from the active tab's stable ID, so the active slot keeps
  its engine and runtime and the `saved: None` invariant follows the record, not the index.
  Pressing at either strip edge advances the visible window one tab so hidden tabs are
  reachable. Plus, middle-click and right-click behaviour are unchanged.
- **`Directory (Shell)` titles.** `tabs::title` composes the basename of the current OSC 7
  directory with the existing shell display name (`VideoTools (PowerShell)`), falling back to
  the launch directory and then the shell name alone. OSC 7 arrives only from the PowerShell
  prompt wrapper, so `report_shell_directory` now defaults to on and is relabelled "Track
  current folder in tab name"; an explicit stored `false` still wins. `cmd`/`bash` show the
  launch directory until an equivalent per-shell integration exists. Rename pins the label; a
  pinned tab stops following directory changes for its lifetime.
- **Shell selection.** `Settings::shell` is a validated token (`powershell`, `pwsh`, `cmd`,
  `bash`), never a path. Imported settings therefore cannot name an executable to run. The
  executable is resolved from the token at launch; the Terminal panel offers the four choices
  and reports a missing executable instead of opening a broken tab. An explicit `--shell` /
  `JUNA_SHELL` launch outranks the setting for that process. The setting applies to new tabs
  and new windows only; existing sessions are never restarted.

**Tiling boundary.** Tabs and panes stay orthogonal: a tab will own a pane tree, and the title
will follow the focused pane's OSC 7 directory. Reordering only permutes tab records, so no
engine-ownership change is required now or when panes land.

**Verification.** Pure tests cover title formatting, rename pinning, shell validation and
back-compatible defaults, shared strip geometry, drop gaps, edge scrolling and the reorder
permutation, including the active-engine invariant. `cargo check`, strict all-target Clippy and
the standard test suite pass. Drag is a pointer gesture, so visual acceptance requires recorded
manual evidence; the unit tests establish geometry and state, not the feel of the gesture.
