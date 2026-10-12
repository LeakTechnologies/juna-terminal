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
