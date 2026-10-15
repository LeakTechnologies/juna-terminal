# Junafetch protected-field implementation

Implementation record under the approved JUNAFETCH_DESIGN.md and
TERMINAL_PRIVACY_DESIGN.md; requested shaded masks confirmed on 2026-10-15.

- A window-owned, current-user-only local named pipe accepts bounded structured values
  from Junafetch. Each tab has a random capability supplied only in its child environment.
  Reject remote clients, wrong capabilities, invalid messages and oversized fields. Use
  overlapped I/O with cancellation and message deadlines; idle IPC does not poll or wake
  periodically. The broker returns unpredictable 128-bit field tokens, never secret values.
- Junafetch emits five `░` cells, tagged with Juna's private OSC 777 `JunaPrivate;<32-hex-token>`
  annotation and a closing zero annotation. Only opaque metadata enters the engine. A
  bounded engine-owned token table gives cells four-byte tags that follow overwrite,
  scrolling and reflow; values remain solely
  in bounded app-owned memory. Plain/JSON output and missing IPC remain masked. Explicit
  privacy Off affects subsequent reports. The renderer never resolves secrets itself.
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
overlapped named-pipe I/O documentation; the private annotation is Juna-owned protocol.
