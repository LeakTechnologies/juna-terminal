# File drops, versioned title and interrupt investigation

Requested 2026-10-18. Approved by the design lead on 2026-10-18.

- Set the native title to `Juna - v{CARGO_PKG_VERSION}`. This produces the actual build
  version rather than hard-coding dev15 into subsequent releases. Keep child OSC titles
  as terminal metadata. Update the updater's window recognition to accept both the legacy
  title and the validated versioned form, retaining executable-path/PID verification.
  Update title and real-window updater acceptance together; preserve the application ID/icon.
- Handle native file drops into the terminal body by inserting a local filesystem path
  as text at the active shell's existing input caret. Preserve Unicode and spaces, quote
  paths appropriately for supported configured Windows shells, and separate multiple paths.
  Never insert Enter, launch/open the file, change directories, modify clipboard contents,
  or log dropped paths. Respect bracketed paste. Reject control characters and excessive
  input before accepting bytes; ignore drops into settings, the tab strip or closing sessions.
  Document literal-path behaviour for unknown/nested/remote shell contexts rather than
  claiming to infer their quoting rules or translate Windows paths to remote paths.
- Reproduce Ctrl+C/Ctrl+Z against a controlled foreground child and the user's identified
  command. Preserve normal control-byte semantics: Ctrl+Z is not universally suspension on
  Windows. Separate application panic/backend fault, child exit and normal last-tab closure
  using exit/lifecycle evidence without logging command input or output. Repair only the
  demonstrated failure and add regression coverage; do not swallow interrupts, restart shells
  silently or change normal `exit` behaviour. An architectural backend replacement, dependency
  upgrade or changed last-tab policy requires its own proposal if evidence calls for it.

Acceptance: pure path/quoting/control-byte tests, active-tab/drop-region routing, Unicode and
spaces, multiple paths, bracketed paste and rejection without partial input. Record controlled
ConPTY interrupt/recovery and real-window lifecycle evidence, retaining byte-exact transport
and process cleanup. Run required check/lint/tests and updater/title acceptance; record native
mouse-drop testing or explicitly leave physical drag/drop acceptance to human review. No new
dependency or terminal-engine borrowing. Keep unrelated completed cycles separately versioned;
each shipped cycle receives the next unused dev tag, matching notes and verified release assets.
