# Themes, custom palettes, and the right-click menu

Status: approved by the design lead 2026-10-12. The contrast fix is already approved
and is implemented separately. This proposal covers the new theme/settings UI and persistence.

## Three-part design

1. **Theme model and darker default.** Use Catppuccin Mocha by default, keep IBM Plex Mono,
   and introduce one Juna-owned theme value containing the default foreground/background,
   cursor, selection, and 16 ANSI colors. Supply the 20 starter choices below. The renderer
   receives resolved theme values alongside its owned screen snapshot; the VT engine continues
   to store logical color indices and explicit RGB values. Changing a theme redraws existing
   content without restarting the shell. Truecolor backgrounds from prompt programs remain
   application-controlled; minimum contrast corrects unreadable foregrounds.
2. **Live right-click options and basic mouse routing.** Replace unconditional right-click paste
   with a compact Juna-rendered context menu: Copy, Paste, Select All, Themes, Settings.
   Themes opens a searchable selector with a preview, active-theme checkmark, and Custom themes.
   Settings initially covers font family/size, contrast, and theme editing. Menu and editor overlays
   have their own UI state and geometry, never live in the terminal grid, and keep the event loop
   draining shell output. Escape/outside-click dismiss; keyboard arrows/Enter work. If a TUI has
   requested mouse reporting, its right-click still reaches the child; Shift+right-click opens
   Juna's menu. Shift remains the override for local selection/history. Selection uses drag and
   clipboard shortcuts; overlay clicks and keystrokes never leak to the shell.
3. **Custom themes and portable storage.** Duplicate a preset, name it, edit colors with hex fields
   and live preview, save or cancel. Store a complete versioned JSON theme under
   `%LOCALAPPDATA%/Juna/themes/<generated-id>.json`; store the active ID and settings separately.
   IDs never derive paths from user-provided names. Validate colors, required keys, unique IDs,
   supported schema version and file size; write a sibling temporary file and atomically replace
   the saved file, preserving the previous file if writing fails. A corrupt custom file reports
   its error and falls back to Mocha without overwriting the source. Presets remain immutable.
   Keep storage separate from the settings view so future export adapters can emit VS Code
   terminal color settings and Windows Terminal schemes. Export is explicitly future work.

## Starter selection

There is no reliable cross-terminal usage ranking establishing the exact most-used 20 themes.
This is a curated starter set: established theme families with public projects and terminal
palette availability. Family variants are chosen for a dark-first menu; they do not inflate the
count with three Catppuccin entries. Exact palette/terminal-port provenance and attribution will
be recorded with bundled data. Include only freely redistributable palette data; classic Monokai
is distinct from commercial Monokai Pro, and Material data must come from a documented public
palette rather than the now-premium redirected VS Code project.

| Starter choice | Primary project / palette reference |
|---|---|
| Catppuccin Mocha — default | [Official palette](https://catppuccin.com/palette/), [project](https://github.com/catppuccin/catppuccin) |
| Dracula | [Official palette](https://draculatheme.com/contribute) |
| Tokyo Night | [Project](https://github.com/folke/tokyonight.nvim) |
| Gruvbox Dark | [Project](https://github.com/morhetz/gruvbox) |
| Nord | [Palette](https://github.com/nordtheme/nord) |
| One Dark | [Original Atom project](https://github.com/atom/one-dark-syntax) |
| Solarized Dark | [Original palette](https://github.com/altercation/solarized) |
| Rosé Pine | [Project](https://github.com/rose-pine/rose-pine-theme) |
| Kanagawa Wave | [Project](https://github.com/rebelot/kanagawa.nvim) |
| Everforest Dark | [Project](https://github.com/sainnhe/everforest) |
| Nightfox | [Project](https://github.com/EdenEast/nightfox.nvim) |
| Ayu Dark | [Palette](https://github.com/ayu-theme/ayu-colors) |
| Monokai Classic | [Public terminal palette collection](https://github.com/mbadolato/iTerm2-Color-Schemes) |
| Material Dark | [Public terminal palette collection](https://github.com/mbadolato/iTerm2-Color-Schemes) |
| GitHub Dark | [Official theme project](https://github.com/primer/github-vscode-theme) |
| Tomorrow Night | [Original project](https://github.com/chriskempson/tomorrow-theme) |
| Sonokai | [Project](https://github.com/sainnhe/sonokai) |
| Oxocarbon Dark | [Project](https://github.com/nyoom-engineering/oxocarbon.nvim) |
| Night Owl | [Project](https://github.com/sdras/night-owl-vscode-theme) |
| Oceanic Next | [Project](https://github.com/voronianski/oceanic-next-color-scheme) |

The [terminal palette collection](https://github.com/mbadolato/iTerm2-Color-Schemes) provides data
ports across many terminals. Only palette data is relevant; terminal engines are not code donors.
Catppuccin describes Mocha as its darkest variant, but its accent colors are deliberately pastel.
A darker background does not automatically darken prompt boxes painted with explicit RGB colors.

## Storage shape and implementation boundaries

```
ThemeV1 { schema_version, id, name, colors: {
  foreground, background, cursor, selection_background, selection_foreground,
  ansi: [16 #RRGGBB values]
}}
SettingsV1 { schema_version, active_theme_id, font_family, font_size, minimum_contrast }
```

The portable theme contains colors, not a running session, fonts, executable commands, or a
VS Code editor grammar. A future VS Code export maps foreground/background/cursor/selection
and ANSI black through bright white to `terminal.*` color customizations documented in
[VS Code's terminal appearance reference](https://code.visualstudio.com/docs/terminal/appearance#_terminal-colors); it does not promise
an entire editor/syntax theme. Unsupported fields are reported by the exporter rather than lost
from the saved Juna source theme.

Proposed modules: pure `theme` model/validation/builtin data; settings file I/O outside `vt`;
application-owned context-menu/theme-editor state; renderer-owned overlay drawing. Layout for
these overlays is pure geometry. Terminal mouse encoders stay pure and separately tested.

Dependency justification for this increment: `serde` derives typed theme/settings structures;
`serde_json` reads/writes versioned JSON, handles escaping and rejects malformed data. A handwritten
JSON parser is not justified. Neither supplies terminal semantics. No async runtime or GUI framework
is needed. Additions and locked versions must be recorded in DECISIONS.md before implementation.

## Acceptance evidence required

- Select all 20 presets; verify checked selection, live redraw, contrast, and restart persistence.
- Create/duplicate/save/reload/cancel custom themes; test malformed files, unsupported versions,
  duplicate names, path-like names, interrupted writes, and read-only storage without data loss.
- Open/close menus with mouse and keyboard; verify copy/paste/selection, TUI right-click forwarding,
  Shift override, and no accidental command input while an overlay has focus.
- Continue sustained shell output while the menu/editor is open; retain byte-exact transport tests.
- GPU pixel tests on sRGB and linear surfaces, including palette backgrounds and unreadable pairs;
  record dark Mocha and custom-theme previews. Keep font selection diagnostics explicit.
- Run check/clippy/test, build the release binary, synchronize architecture/decisions/changelog,
  and commit each successful implementation iteration. Human context-menu/clipboard/DPI review
  remains explicitly recorded where it cannot be automated.
