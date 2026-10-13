# Juna roadmap

## Current foundation

Native Windows terminal, original VT engine, ordered ConPTY transport, dark theme choices,
15 bundled fonts, live sizing, a right-click options menu and application icon.

## Next interface iteration (approved design)

- Tabs: right-click New Tab, Ctrl+Shift+T, keyboard switching and per-session shutdown.
- Explicit mouse/keyboard settings prompts and font sizes entered in px or pt.
- Sixty starter themes organized by family, with collapsible variants.
- Custom submenu for creating/managing themes and automatic discovery from family folders.

See UI_NEXT_DESIGN.md for architecture and acceptance criteria.

## Bundled tools (approved revised design)

Junafetch: a maintained Fastfetch fork with a compact Juna ASCII logo, bundled execution,
and privacy controls. Protected values are masked by default, can be revealed on hover,
and have a user preference to turn censorship off. Sensitive values should stay outside
terminal history, regular clipboard copies and diagnostic logs.

See JUNAFETCH_DESIGN.md. Additional terminal tools can use the same packaging registry later.

## Later

- Multiple terminal tiles within one tab, with keyboard and mouse split/focus/resize controls
  inspired by Terminator's workflow. Use the existing pure BSP layout implementation; declare
  session ownership, output fairness and per-tile shutdown before implementation.
- Export custom themes for other terminals, including VS Code.
- Further privacy controls for outputs beyond Junafetch, with an explicit scope and test plan.
- Mixed-DPI and additional GPU backend acceptance; Unicode/contextual layout improvements.

## Distribution and updates (approved design)

Versioned Windows x64 GitHub Releases, distinct Stable/Preview channels, verified downloads,
explicit installation, rollback and an external helper for replacing the locked executable.
See UPDATE_DESIGN.md. Do not use Actions artifacts as production update assets.
