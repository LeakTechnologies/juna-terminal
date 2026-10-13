# Bundled monospace font selection

Requested 2026-10-13. This is a curated shortlist of widely used coding/terminal fonts,
not a claim of a definitive popularity ranking. IBM Plex Mono remains the default.

- Bundle 15 unmodified font families with their original copyright/license files and a pinned source/hash manifest: IBM Plex Mono, JetBrains Mono, Fira Code, Cascadia Code, Cascadia Mono, Source Code Pro, Iosevka, Inconsolata, Roboto Mono, Ubuntu Sans Mono, Space Mono, PT Mono, DM Mono, Victor Mono, and Monaspace Neon. Verify the exact downloaded assets' SIL OFL 1.1 licenses before inclusion; retain reserved names and notices. No Rust dependencies or system font installation.
- Add a searchable font picker under right-click Settings with live preview, Save/Cancel, persistent selection, and optional existing installed monospace fonts. Load bundled faces through the existing fontdb/swash infrastructure so every starter choice works on a clean machine. Preserve fixed terminal cells and fallback behavior; do not promise cross-cell ligatures.
- Acceptance: every bundled face loads and resolves to the selected family; GPU samples cover ASCII disambiguation, box drawing, Unicode fallback and clipping; selection persists and cancel restores the previous font; output continues during font selection. Run required checks, real-window tests, release build and push.

## Primary source and license references

| Family | Official source/license |
|---|---|
| IBM Plex Mono | https://github.com/IBM/plex |
| JetBrains Mono | https://github.com/JetBrains/JetBrainsMono |
| Fira Code | https://github.com/tonsky/FiraCode |
| Cascadia Code / Mono | https://github.com/microsoft/cascadia-code |
| Source Code Pro | https://github.com/adobe-fonts/source-code-pro |
| Iosevka | https://github.com/be5invis/Iosevka |
| Inconsolata | https://github.com/google/fonts/tree/main/ofl/inconsolata |
| Roboto Mono | https://github.com/google/fonts/tree/main/ofl/robotomono |
| Ubuntu Sans Mono | https://github.com/canonical/Ubuntu-Sans-Mono-fonts |
| Space Mono | https://github.com/google/fonts/tree/main/ofl/spacemono |
| PT Mono | https://github.com/google/fonts/tree/main/ofl/ptmono |
| DM Mono | https://github.com/google/fonts/tree/main/ofl/dmmono |
| Victor Mono | https://github.com/rubjo/victor-mono |
| Monaspace Neon | https://github.com/githubnext/monaspace |

SIL OFL permits embedding and redistribution with the license and copyright notices:
https://openfontlicense.org/open-font-license-official-text/

Approved font selection design on 2026-10-13.

## Requested live sizing extension

- Left/Right on the Settings font-size row changes size by 1 px with immediate preview.
- Ctrl+mouse wheel and Ctrl+Plus/Equals / Ctrl+Minus resize live; Ctrl+0 restores 16 px. Clamp to existing 8–40 px limits, preserve ordered shell output and refit the PTY after successful resize.
- Save shortcut/wheel changes to settings; settings-panel changes retain Save/Cancel semantics. Add focused input/state tests and GPU/window verification.

Approved live sizing extension on 2026-10-13.
License correction after reading upstream assets: Ubuntu Sans Mono uses Ubuntu Font Licence 1.0,
which permits embedding/redistribution with notices. The other fourteen use SIL OFL 1.1.
All original notices are retained, and the Settings license view makes embedded notices readable.
## Proposed explicit font-selection persistence refinement

Requested during testing on 2026-10-13; awaiting approval. Hover/navigation remains temporary,
but explicitly clicking a font or pressing Enter saves that font family immediately. Save only
the chosen family against the committed preference baseline, preserving saved favourites and
without committing unrelated size/contrast previews. Update the rollback baseline so Cancel
does not undo the saved font. Numeric settings retain Save/Cancel; external zoom retains its
existing immediate persistence. Show an honest Saved font status and report failed writes.
## Current persistence policy

The lead approved explicit font autosave and then PERSISTENT_SETTINGS_DESIGN.md on 2026-10-13.
Those refinements supersede the original Settings-wide Save/Cancel policy: confirmed choices
persist automatically; only hovering and unfinished numeric entries remain temporary.
