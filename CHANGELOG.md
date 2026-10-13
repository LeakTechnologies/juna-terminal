# Changelog

## 2026-10-12 — application title

- Keep the window named Juna Terminal when shells send title changes.
- Add real-window title regression coverage and record installed Fastfetch smoke evidence.


## 2026-10-12 — themes and options

- Default to Catppuccin Mocha and supply 20 sourced starter palettes.
- Add a live right-click Copy/Paste/Select All/Themes/Settings menu with keyboard navigation.
- Add search/live theme previews, font size/family and contrast settings, and custom theme editing.
- Save validated, versioned theme/settings JSON atomically; retain corrupted files and report errors.
- Preserve TUI mouse reporting with Shift as the local-menu override; consume overlay input separately.
- Record GPU palette/menu captures and real shell output continuing with options overlays open.

## 2026-10-12 — color readability

- Correct double sRGB encoding that brightened the displayed theme, including surface clear colors.
- Adjust unreadable foregrounds to a minimum nominal 4.5:1 contrast while preserving backgrounds.
- Log the selected font and cover both sRGB and non-sRGB render surfaces with exact pixel tests.

## 2026-10-12

- Repair Space, control/modifier keys, composed input, keypad and terminal mouse encoding.
- Detect shell exit and join byte-transport workers before closing the terminal window.
- Correct row scrolling/insertion, deferred wrap, large parameters, erase semantics, saved cursor,
  Unicode clusters, wide-cell rendition and resize margins; add permanent regression coverage.
- Render owned snapshots with layered backgrounds, shaped glyphs, visible text attributes, cursor
  modes, fallback fonts, frame-boundary atlas recycling and recoverable surface presentation.
- Add local history navigation, selection/copy/paste, quoted shell arguments and persistent diagnostics.
- Pin Unicode 17 standards-data crates; remove inactive winres/build inputs; keep ConPTY pinned.
- Add GPU pixel evidence, actual-window lifecycle acceptance, and an explicit human testing matrix.

## 2026-10-13 — application icon

- Embed the approved seven-size icon into the executable and set window/taskbar icons.
- Verify native icon handles in real-window acceptance and source-scan the resource input.

