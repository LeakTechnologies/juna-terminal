# Grouped settings and Junafetch profiles

Requested 2026-10-13. Explicitly approved 2026-10-13 under AGENTS.md §3. Extends the approved expanded
Junafetch report and tidies settings without adding options to the main terminal context menu.

- Settings opens a short category list: Appearance, Terminal, Tools, Import/Export, Close.
  Appearance contains themes, fonts, size and contrast; Terminal contains line numbers, local
  Git status and opt-in folder reporting. Tools contains Junafetch. Each submenu supports
  mouse/keyboard navigation, wheel scrolling and a Back row; retain live appearance changes,
  immediate persistence and the right-side panel. Dedicated panels continue ignoring right-click.
- Tools → Junafetch offers saved Brief, Standard and Detailed profiles plus compact/full/no
  artwork. Standard defaults to the approved grouped Fastfetch-equivalent category coverage;
  Brief keeps the existing eight stats; Detailed adds supported local hardware/diagnostic
  fields, labelled unavailable where necessary. Thin theme-coloured section borders and aligned
  labels follow the supplied visual references. Wrap/stack in narrow terminals. No mandatory
  special-font icons, custom-command execution, development-tool scanning or background polling.
- Store confirmed choices in versioned settings, including portable import/export. The console
  frontend reads only its bounded validated preference fields on each invocation so existing
  tabs see the next-command changes; explicit CLI flags override saved choices. Preserve safe
  Hidden placeholders for personal/network information until protected-value transport/reveal
  is implemented. Preference errors use safe defaults and generic messages, never file contents.

Acceptance: all menu paths by mouse/keyboard, Back/Escape/right-click behaviour, no accidental
shell input, font numeric/live sizing and theme preview persistence, saved profile restart and
existing-tab next-command behaviour, CLI precedence, old/malformed/oversized settings, portable
transfer and no private metadata export. Run mandatory gates and GPU/real-window evidence.

Order: finish dev6 local Git status first; implement this together with the expanded Junafetch
cycle after approval. Privacy remains a distinct cycle with its own acceptance.
