# Changelog

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
