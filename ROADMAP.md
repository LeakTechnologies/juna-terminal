# Juna roadmap

## Current foundation

Native Windows terminal, original VT engine, ordered ConPTY transport, dark theme choices,
15 bundled fonts, live sizing, a right-click options menu and application icon.

## Completed interface iteration

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

- Theme favourites: yellow stars and a Favourites section, including starter and custom themes.
  Implemented in the 0.1.1-dev1 interface/preferences cycle; see THEME_FAVOURITES_DESIGN.md.

## Languages — after core feature maturity

Canadian English (en-CA) is the primary interface language and spelling convention. Once the
terminal, privacy and distribution features are mature, add a Language tab with US English
(en-US) and British English (en-GB) where interface wording actually differs. Avoid artificial
differences or duplicate translation work for shared wording.

Prioritize Inuktitut for Indigenous-language support, then Cree and Mohawk, followed by other
languages over time. Translation needs community/native-speaker review, an explicit choice of
language variety and writing system, appropriate bundled font coverage, and input/layout testing.
Do not label machine-produced strings as supported translations. Inuktitut syllabics and Latin
orthographies need deliberate coverage; Cree likewise needs variety/orthography decisions.
Localize Juna's interface while preserving shell commands and program output as received.
No translations or Language tab are claimed as implemented at this stage.

## Further terminal work

- Low-priority investigation: Windows versions older than 10 1809, including 1803.
  Assess a separate legacy console backend without replacing the primary ConPTY path
  or borrowing terminal-engine semantics. Review dependency provenance, platform/API
  compatibility and graphics requirements before proposing implementation. Require real
  older-Windows testing of interactive programs, resizing, byte transport and process
  cleanup. Feasibility is unverified; no compatibility guarantee or delivery date is set.
  This stays behind core terminal, local Git status, privacy and updater work.

- Kitty graphics protocol support, after the current terminal, privacy and distribution
  priorities. Implement it within Juna's original engine from the published protocol
  [specification](https://sw.kovidgoyal.net/kitty/graphics-protocol/), with a design proposal
  before implementation. Define bounded image memory,
  decoding limits, placement/scrolling/alternate-screen behaviour and renderer snapshot
  ownership, then test malformed input and compatibility. This is roadmap work only;
  no graphics handling or borrowed Kitty engine code is included in current builds.

- Explore Juna for Android after the primary Windows features mature. Reuse the original pure
  VT engine where feasible, with an Android-specific UI/process backend and explicit assessment
  of local shell capabilities under Android's application sandbox.
- Keep connections local-first and user-controlled: no mandatory Juna accounts, hosted relay,
  transcript storage, telemetry or cloud credential service. An explicitly enabled listener on
  the user's own computer is distinct from a Juna-operated server. Prefer an established secure
  transport with verified host identity and user-owned keys rather than inventing encryption.
- Design remote shell creation separately from attaching to an already running Juna session.
  Shared live sessions need explicit permission, session ownership, reconnect/termination rules,
  paired-device revocation and protection of privacy reveals. Device discovery never authorizes
  access. Do not expose terminals to the network by default. Assess LAN and user-managed private
  network access; internet reachability and relay services are outside the initial exploration.
  This is a product direction, not an approved implementation or a shipped security guarantee.

- Near-term after Junafetch configuration: horizontal and vertical terminal tiles within one
  tab, with keyboard and mouse split/focus/resize controls
  inspired by Terminator's workflow. Use the existing pure BSP layout implementation; declare
  session ownership, output fairness and per-tile shutdown before implementation. The layout
  foundation exists; interactive panes are not implemented. Prepare a separate approved design.
- Export custom themes for other terminals, including VS Code.
- Further privacy controls for outputs beyond Junafetch, with an explicit scope and test plan.
- Mixed-DPI and additional GPU backend acceptance; Unicode/contextual layout improvements.

## Distribution and updates (approved design)

Versioned Windows x64 GitHub Releases, distinct Stable/Preview channels, verified downloads,
explicit installation, rollback and an external helper for replacing the locked executable.
See UPDATE_DESIGN.md. Do not use Actions artifacts as production update assets.

Terminal-wide privacy and intentional Shift+hover are approved in TERMINAL_PRIVACY_DESIGN.md.
