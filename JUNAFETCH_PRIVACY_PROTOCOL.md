# Junafetch protected-field implementation

Implementation record under the approved JUNAFETCH_DESIGN.md and
TERMINAL_PRIVACY_DESIGN.md; requested shaded masks confirmed on 2026-10-15.

- A window-owned, current-user-only local named pipe accepts bounded structured values
  from Junafetch. Each tab has a random capability supplied only in its child environment.
  Reject remote clients, wrong capabilities, invalid messages and oversized fields. Use
  overlapped I/O with cancellation and message deadlines; idle IPC does not poll or wake
  periodically. The broker returns unpredictable 128-bit field tokens, never secret values.
- Junafetch emits one `░` per terminal display column of an ordinary protected value.
  Explicitly typed password fields emit exactly `********`; no passwords are currently collected.
  Standard SGR truecolour foreground/background pairs carry opaque 48-bit markers derived from
  random 128-bit field tokens. The broker collision-checks and never reuses markers during its
  lifetime, including failed requests and expired values; a 65,536-reservation limit fails closed.
  Only registered shade/asterisk cells resolve. The app normalizes those colours in its owned
  committed snapshot and retains at most 128 values per tab. Expired marker metadata remains
  so old cells never identify a different value. The renderer receives no protocol or secrets
  except the deliberate temporary overlay. Plain/JSON and missing IPC remain masked with the
  same width policy. Explicit Off affects subsequent reports.
  Legacy private OSC parsing remains for compatibility; new reports retire OSC emission because
  real ConPTY did not retain its association with the painted cells. Prior raw-token checks
  and direct-engine tests were insufficient evidence of working end-to-end reveal.
- App-owned committed snapshots prevent UI/privacy redraws exposing synchronized partial
  output. Shift+hover is the default deliberate reveal, with no other modifiers/buttons;
  require movement after Shift and hide on release, exit, focus loss or tab change. Ordinary
  copies retain shaded cells even while an owned overlay reveals a value. Keep controls in
  Settings → Privacy. Test real IP/MAC-shaped fixtures, malicious IPC, wraps, overwrites,
  tab isolation, expiry, all hide conditions, copied text and unchanged byte transport.

This first privacy delivery protects Junafetch's structured fields. The separately approved
terminal-wide recognizer remains a subsequent stage; arbitrary Fastfetch/command output is
not claimed protected by this implementation. No raw-secret fallback is permitted.

Dev14 also uses the authenticated broker for `action: "update"` with a validated stable/preview
channel. Its empty-array acknowledgement confirms queueing, not installation. Application-side
handling checks the originating tab is live, then enters the normal verified update/Cancel-first
confirmation flow. Unknown actions and invalid capabilities/channels are rejected. No command
request may confirm installation; the CLI also verifies the host uses the same executable.

Current-user ACLs use read-only token identity queries, without impersonation or privilege
changes. Windows security/pipes/file/overlapped-I/O features serve this concrete requirement;
no new crate or async runtime is needed. References: Microsoft named-pipe security and
overlapped named-pipe I/O documentation; the marker mapping is Juna-owned protocol. Standard SGR 38;2/48;2 is documented at
https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences.
