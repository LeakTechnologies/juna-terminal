## 2026-10-13 — bundled-tool environment preparation

Verified child-only SessionSpec environment overrides with spaces/Unicode and unchanged parent
environment. Check/clippy and 186 standard tests pass, including byte-exact transport coverage.
The separate Junafetch detector fork builds and passes its 18 upstream tests. Bundled command
delivery, Rust report presentation, privacy and updater acceptance remain in progress.

## 2026-10-13 — tabs and theme families complete

Implemented the approved interface stage: independent tabs, keyboard/mouse controls, exact
numeric settings entry, 60 grouped themes, and a Custom submenu with family-folder discovery
and recoverable removal. Verified check/clippy, 185 standard tests, four explicit GPU tests,
three real-window tests, release build, embedded icon bytes and direct release exit 0.
Reviewed captures show right-side panels leave terminal text visible. Cargo emitted incremental
hard-link fallback warnings after a restricted filesystem run; no source Clippy findings remained.
The restricted run failed seven filesystem-dependent tests; the normal-access rerun passed.
Physical mouse/IME/mixed-DPI review remains outstanding. Junafetch and updater are subsequent stages.

## 2026-10-12 — fixed application title

Keep Juna Terminal as the visible window title while preserving shell OSC title metadata.
Verified: check/clippy, 176 standard tests, and two explicit real-window tests pass; both
Command Prompt and PowerShell attempted a title change. Fastfetch 2.69.0 ran through
PowerShell/ConPTY/the engine with success marker, exit 0 and zero dropped bytes.

# Completed iterations

## 2026-10-12 — approved theme/options system

Implemented THEME_DESIGN.md: Catppuccin Mocha default, 20 starter choices, non-modal right-click
options, searchable previews, font/contrast settings, and custom-theme draft/save persistence.
Validation, read-only/corrupt-file preservation, menu geometry/input isolation, all-palette GPU
captures, and real output with overlays open were exercised. Future VS Code export remains deferred.

Verified for this iteration: warning-free check/clippy; 176 standard tests; three explicitly run
GPU acceptance tests; two explicitly run real-window acceptance tests; updated release build.

## 2026-10-12 — approved contrast repair

Confirmed and corrected the sRGB surface brightening defect, added a presentation-only minimum
contrast policy, and logged actual font selection. Both GPU acceptance tests pass; the new
right-click/theme design has separate recorded approval in THEME_DESIGN.md.

## 2026-10-12 — approved terminal readiness repairs

Implemented the three-stage AUDIT.md design. Acceptance evidence and remaining human checks are
recorded in TERMINAL_TESTING.md. Historical audit failures remain in AUDIT.md rather than being
silently rewritten. This milestone supports interactive testing, not a full Tier 2 release claim.

Verified: warning-free check/clippy; 165 standard tests pass; GPU row-isolation/capture and actual
window/exit acceptance run separately; release executable built. Human physical input, clipboard,
mixed-DPI and editor/pager review remain explicit in the testing matrix.

## 2026-10-13 — icon installed

Verified all 176 standard tests, check/clippy, both live-window tests and release build.
Seven embedded icon images match the approved ICO byte for byte; native small/taskbar handles exist.


## 2026-10-13 — fonts and live sizing complete

Verified 180 standard tests, warning-free check/clippy, four real GPU tests, two real-window
tests and a rebuilt release. Fifteen font faces resolve without system fonts; search, Save/Cancel,
zoom/AltGr routing, wheel accumulation, persistence and bounds have focused regression coverage.
GPU captures cover all fifteen, fallback, row isolation and minimum/default/maximum sizes.
README now displays the approved logo and documents current features and controls.
Taskbar API/icon handles are verified; user confirmation of the updated Explorer button is pending.

## 2026-10-13 — preview placement and taskbar verified

Right-side previews and a simplified settings menu pass the pixel/geometry checks. All 180
standard tests, check/clippy, targeted GPU and both real-window tests pass; release rebuilt.
Direct release launch exited 0, native icon resources match the source, and the user confirmed
the visible taskbar logo after the fresh-group property correction. Upcoming work has approved
designs in UI_NEXT_DESIGN.md, JUNAFETCH_DESIGN.md and UPDATE_DESIGN.md.
## 2026-10-13 — 0.1.1-dev1 interface/preferences cycle

Implemented approved separated tab menus, local rename, Cancel-first middle-click confirmation,
theme-coloured check/cross labels, context-menu reanchoring and protected editing panels.
Line numbers live in Settings, have a two-cell gap, and hide/refit in alternate screens.
Favourite stars, a dedicated section, bulk family controls and immediate wheel scrolling are present.
Confirmed preferences persist automatically; JSON transfer validates portable fields/themes, reviews
imports, preserves conflicts and rolls back staged files on failed preference writes.

Verified in this session: required check/strict Clippy and standard tests; four explicit GPU tests,
three actual-window tests, reviewed captures, native x64/version/seven-icon resource verification.
Detailed counts and limitations are recorded in TERMINAL_TESTING.md. Native file-dialog interaction,
physical input/mixed-DPI and other-machine acceptance remain human checks. Publication is not yet
claimed: the tag workflow and asset download/hash verification must finish first.
## 2026-10-13 — dev1 publication verified; dev2 CI correction

Dev1 Windows workflow passed and published a prerelease. Downloaded archive/manifest/checksum
verification confirms its exact source commit, native version and approved icon resources.
The separate Linux run failed the import rollback test because Unix rename replaced a read-only
file. Dev2 adds an explicit portable read-only guard and enables its preservation test on Linux.
This is a fresh version; no published dev1 tag/binary was changed. Hosted dev2 evidence is pending.

Verified locally for dev2: check, strict Clippy and all 200 standard cases pass. The Windows
interface is unchanged; its recorded dev1 GPU/window evidence still applies. Hosted Linux and
new-version artifact checks must verify the correction before publication is reported.

## 2026-10-13 — dev2 publication and bundled-tool acceptance

Verified dev2: hosted Linux CI and Windows release workflow passed. Downloaded ZIP SHA-256
b4c340ca2ea68d448ab9eba987f3a3b346f1325a82ccc81c69fa820e6a2578d7 matches its manifest;
source 7172df647fff607c79ffc079339c62bd1b10646f, native version and all seven icon images match.

Verified bundled utility tests: cached frontend corruption is repaired; both aliases execute
through production ConPTY with only system/tool PATH and a Unicode cache directory, without
installed Fastfetch. The frontend constructs eight safe JSON fields and omits personal/network
identifiers. These checks do not establish terminal-wide masking or hover reveal.

Dev3 validation in this session: cargo check, strict all-target Clippy and all 202 standard
Juna tests passed, including byte-exact ConPTY/shell transport. The separate frontend check,
strict Clippy and one report regression passed. Pinned payload/source verification passed.
The real-window bundled_fetch test passed: system-only PATH, both aliases, eight-field JSON,
rendered acceptance marker, presented GPU frames and normal shell/window cleanup. This is
controlled runtime evidence; artwork aesthetics and other-machine acceptance remain human checks.
## 0.1.1-dev4 — graphics roadmap

Added Kitty graphics protocol support as future work at the user's request. No terminal
semantics, renderer boundary or dependencies changed. Implementation requires its own
approved design and acceptance evidence. This documentation cycle uses a fresh dev tag.
Verified for this documentation/version cycle: cargo check, strict all-target Clippy and all 202 standard tests passed. Graphics runtime acceptance does not apply because no graphics implementation changed.

## 0.1.1-dev5 — legacy Windows roadmap

Recorded the user's low-priority older-Windows investigation. No backend, dependency or runtime behaviour changed; compatibility is not claimed.
Verified in this session: cargo check, strict all-target Clippy and all 202 standard tests passed. Older-Windows runtime support remains unverified and unimplemented.

## 2026-10-13 — local Git status acceptance in progress

Verified targeted tests: real clean/staged/unstaged/untracked/conflicted/detached/worktree and
local ahead/behind fixtures; unchanged index bytes/mtime; fsmonitor suppression and active-filter
refusal; initial-folder invalidation/stale generations; bounded worker shutdown. PowerShell's
controlled prompt text and LASTEXITCODE are preserved, and Unicode OSC 7 metadata survives real
ConPTY. Invalid/remote metadata is rejected without changing cells. Full gates and footer/window
acceptance must pass before this development cycle lands. Physical input/other-machine review
remain separate; older Windows 1803 is unsupported by the current ConPTY backend.

Dev6 final evidence: cargo check, strict all-target Clippy and all 210 standard tests passed
on the settled source. Explicit GPU footer comparison passed, preserving terminal pixels and
engine cells; docs/acceptance/git-status.png was visually reviewed. The actual-window Git test
passed with reported cwd, repository result, visible/hidden footer transitions, presentation
and exit 0. Its controlled stream accepted 967 bytes and presented 14 frames. Physical input,
custom prompt implementations and other-machine acceptance remain human checks. Expanded fetch
profiles/submenus and Ctrl+Shift+N designs received approval but are not shipped in dev6.
