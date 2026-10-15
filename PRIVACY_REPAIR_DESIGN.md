# Repair protected-field reveal and mask sizing

Requested 2026-10-15. Amendment to JUNAFETCH_DESIGN.md and TERMINAL_PRIVACY_DESIGN.md.
Status: approved by the design lead on 2026-10-15, reaffirmed by “finish off the repairs”.
Implemented; acceptance results are recorded in DONE.md and TERMINAL_TESTING.md.

## Reproduction and retraction

The strengthened bundled-fetch integration test fails: a field's opaque annotation exists
in ConPTY output but is not attached to its shaded cells in the resulting engine view/history.
A focused two-line synthetic probe retains 0/5 OSC-tagged shade cells, while preserving all
5/5 shade cells with standard truecolour foreground/background attributes on this host.
Earlier raw-tag checks plus synthetic reveal tests did not prove this association survived
the Windows backend. Retract the earlier claim of verified end-to-end interactive reveal.

## Proposed changes

- Replace the fragile span annotation in new Junafetch output with an opaque per-field marker
  encoded in standard per-cell foreground/background attributes, whose preservation the probe
  verified. Keep authenticated local IPC and the full random 128-bit field token. Its six-byte
  marker uses a random colour pair, with collision checks and no reuse during the window's
  lifetime; cap reservations at 65,536 and fail closed at capacity. The marker is an identifier,
  never part of the sensitive value. Record the narrower 48-bit display-marker space explicitly.
  Only shade/asterisk cells matching a registered marker may reveal. Unknown markers never map
  to a value. The application resolves and normalizes recognized marker colours to the theme
  in its owned committed snapshot before rendering; the renderer never interprets markers.
  Retain bounded per-tab values and non-reused marker metadata so expired fields cannot resolve
  to a newer value. Retire the old emission path; preserve existing VT semantics and byte order.
- Size ordinary masks to the source string's terminal display columns, including Unicode,
  wrapping and narrow reports. Use exactly `********` for explicitly classified password fields,
  irrespective of their length. This deliberately reveals ordinary field width, as requested,
  while concealing password length. Junafetch currently collects no passwords; this establishes
  the mask rule for typed password fields without claiming automatic password detection or
  adding a password detector. Missing IPC, plain and JSON output stay masked under the same
  sizing policy. Do not expose raw-value fallback unless censorship is explicitly Off.
- Preserve saved Shift+hover by default, optional ordinary hover, immediate hiding on exit,
  focus/modifier/tab changes, and censored copies. Refresh the committed cell-to-field mapping
  when IPC reports arrive, output changes or scrolling/reflow changes the viewport. Keep secrets
  outside VT cells, process output, normal copies and diagnostics. No new dependency or periodic
  worker wakeup. No terminal-wide recognition or unrelated settings changes in this repair.

## Acceptance

Use actual bundled Junafetch → ConPTY → engine → app hit-testing, including cursor rewrites,
wraps, scrollback, resize, multiple tabs and scaled cells. A raw token's presence is insufficient:
assert actual displayed masked cells map to their registered values, then exercise reveal/hide
events and verify masked copies. Use only synthetic values in screenshots/diagnostics. Test
ordinary/Unicode lengths, exactly eight password asterisks, expiry, collision rejection,
reservation exhaustion, missing transport and invalid markers. Repeat GPU/real-window evidence,
required check/lint/tests and byte-exact transport; repin the bundled frontend with matching
source fingerprints. Publish the next dev version only after gates pass and verify its assets.
Older Windows/other-machine behaviour remains a separate acceptance check.

References: measured ConPTY probe in src/app/tools.rs; Microsoft's documented SGR 38;2 and 48;2
sequences at https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences.
No terminal-engine implementation from another emulator is used.
