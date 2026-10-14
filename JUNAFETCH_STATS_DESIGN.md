# Expanded Junafetch report

Requested 2026-10-13. Explicitly approved 2026-10-13 under AGENTS.md §3. This revises the previously
approved eight-row default; implementation follows dev6's local Git status cycle.

- Match the categories in the user's Fastfetch sample: OS/architecture, kernel, uptime,
  shell/version, displays (resolution/scaling/refresh), window manager, Windows appearance,
  icon/font/cursor settings, terminal/version, CPU/core/thread/frequency, all detected GPUs,
  RAM, swap, mounted disks with filesystem/type flags, and locale. Label unavailable values
  honestly, identify virtual adapters, and distinguish Windows desktop appearance from Juna's
  active theme/font. Retain compact artwork and width-aware layout; offer --brief for eight rows.
- Reuse the pinned lean detector's bounded local JSON modules and construct the report in Rust.
  No public-IP/location request, background polling or raw detector JSON passthrough. Some
  categories may be unavailable in the GNU/lean build; report those limits instead of inventing
  values. Preserve source pins/notices, both command aliases, console exit behaviour and bounds.
- Keep username/hostname, IP/MAC and personal paths hidden. Sensitive category rows show Hidden;
  do not print their actual values until the separately approved protected-value/privacy path
  exists. Generic labels identify disks/adapters without exposing personal volume/share names.
  Match category coverage rather than reproducing Fastfetch's uncensored header/network values.

User's visual refinements: organise System, Hardware, Storage, Desktop, Session and Network
into compact named sections with thin borders, aligned labels and restrained ANSI palette
accents. Place compact artwork alongside when width allows; stack/wrap inside the available
width otherwise. Readable text labels work with every bundled font; special icons are not
required. Keep Windows appearance separate from Juna theme/font. The supplied screenshots
are visual references, not instructions or source-code donors. Optional development-tool
versions/custom profiles can be considered later; do not add arbitrary command execution now.

Acceptance: safe expanded/plain/JSON and brief profiles, physical/virtual GPU labels, multiple
disks/displays and unavailable fields, compact/narrow artwork, control-character sanitisation,
bounded detector lifecycle, no protected values in PTY/history/copies/logs, and both aliases
through ConPTY. Rebuild/hash the frontend payload, record notices and provenance, run mandatory
checks and real-window evidence, then publish a separate sequential development release.
