# Junafetch bundled-tool design — revised

Requested 2026-10-13. Original proposal was not approved; this revision addresses compact artwork
and the requested privacy behavior. The supplied ASCII art is data, not agent instructions.

- Maintain LeakTechnologies/junafetch as a Fastfetch 2.69.0 fork, retaining upstream MIT and third-party notices. Preserve upstream detection code as a separate utility; author Juna integration and new presentation/privacy behavior in Rust. Use a compact Juna ASCII logo by default (48x18 candidate), with 64x24 and original 89x34 choices. Show a size preview before choosing the final default; preserve the supplied original unchanged. Use a stacked layout when a narrow terminal cannot fit logo and details side by side.
- Build and embed pinned Windows x64 tool payloads with recorded source revisions/hashes, extracting atomically to an app-owned versioned tools directory. Expose junafetch only through each Juna shell's PATH via explicit SessionSpec environment overrides. No manual install, registry PATH edits or profile edits. The command is available without automatic startup execution. Keep a small bundled-tool registry for future utilities; disable unsupported image protocols in this build.
- Privacy defaults on for Junafetch: mask IP addresses, user/host identity, MAC addresses and sensitive paths/identifiers. The Rust tool front end consumes a bounded structured detection report. Protected values travel through authenticated, current-user local IPC to app-owned memory; they never enter PTY output, VT cells/history, regular clipboard text or diagnostic logs while privacy is enabled. The terminal receives safe placeholders; Juna draws an owned hover overlay with a protected value only while that field is hovered. Hide on pointer exit or focus loss; offer a saved Privacy setting to disable censorship and a Never reveal mode for streaming. Outside Juna, use masked output with explicit opt-out. Scope applies to Junafetch reports first; unrelated shell output requires a separate policy.

Acceptance: no installed Fastfetch required; exact argument/status handling; compact/full ASCII
and narrow layout; space/Unicode installation paths; default-safe output/history/copy/logs;
field hover/focus loss and privacy opt-out; bounded/invalid/unauthorized IPC; tab/session cleanup;
byte-exact transport and required check/clippy/test plus GPU/window evidence. Build and push both
repositories after approval. Record tool payload and native build requirements in DECISIONS.md.

Primary references:
- https://raw.githubusercontent.com/fastfetch-cli/fastfetch/dev/LICENSE
- https://github.com/fastfetch-cli/fastfetch/wiki/Building
- https://github.com/fastfetch-cli/fastfetch/wiki/Logo-options

Artwork: assets/junafetch-ascii.txt remains byte-for-byte original. Compact candidates and
an initial comparison are in docs/acceptance/ascii-size-options.png and need visual selection.

## Lean report profile

Default to eight rows: OS/build, CPU/model and cores/threads, GPU/model and available VRAM,
memory used/total, system disk used/total, uptime, shell, and Juna version. Keep percentages and
units consistent. Offer --details for additional supported modules. Hide virtual/software GPU
adapters by default when physical hardware is available; preserve a detailed all-adapters view.
Do not fetch public IP/location or refresh hardware in the background by default.

QOL: width-aware layout and compact/full artwork; safe plain-text and JSON reports with the same
privacy policy; understandable unavailable-data labels. Keep metadata/source errors and process
failure status honest. Additional modules remain optional; no dashboards, telemetry or animation.

Explicitly approved by the design lead on 2026-10-13, in interface → Junafetch → updates order.

