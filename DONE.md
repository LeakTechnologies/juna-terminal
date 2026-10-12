# Completed iterations

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
