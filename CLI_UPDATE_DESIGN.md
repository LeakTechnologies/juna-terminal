# Command-line updates

Requested 2026-10-15. Proposed extension to the approved UPDATE_DESIGN.md.
Status: approved by the design lead on 2026-10-15 under AGENTS.md §3.

## Commands

- `juna --check-update`: check published GitHub Releases and print the installed version,
  selected channel and newest compatible version, or that Juna is current. No window, download
  or installation. Successful checks exit 0, including when an update is available.
- `juna --update`: check, download and verify a newer compatible release, then request
  confirmation before installation and restart. If current, print that result and exit.
- Either command accepts `--channel stable` or `--channel preview` for that invocation.
  Otherwise use the saved Settings → Updates channel. Overrides do not change preferences.
- Add `juna --help`, covering these commands and existing shell/cwd/version arguments.
  Reject conflicting actions and unrelated shell arguments before doing network work.
  Failures exit 1; invalid command usage exits 2.

## Installation and in-terminal use

Reuse the existing release selection, HTTPS, bounded staging, checksum/native identity,
notices transaction, hidden helper, rollback and restart acknowledgement. Do not add another
downloader or weaken integrity checks. No new dependency or automatic network polling.

Inside a Juna shell, `--update` sends an authenticated local request to its hosting window
through the existing current-user broker. That window checks and downloads, then opens the
existing Cancel-first Install and Restart confirmation. The command returns before installation;
the host closes/reaps its sessions only after confirmation. This avoids trying to replace the
still-running host from its own child command. Add a matching-executable window preflight
on top of the existing locked-replacement safeguards and close-other-windows instruction.
The request cannot itself confirm installation or terminate sessions. Report unavailable
handoff explicitly; do not silently install through an unrelated Juna window.

Outside Juna, the command provides text progress and a default-No confirmation. Require
interactive input to install; redirected input does not silently consent. Require other Juna
windows using the installation to close, then use the existing helper and restart an interactive
configured shell at home. Never replay the calling shell command. Installation failures retain
the existing helper result/recovery reporting. The command reports helper handoff separately
from successful replacement, which is established only by the existing restart acknowledgement.

Prepend Juna's executable directory to its own child-session PATH so `juna` is available within
Juna without changing the user's system PATH. Outside Juna, use a configured PATH entry or
`./juna.exe` in the portable folder. Check commands never require a GPU or terminal session.

## Acceptance

Verify parsing/help/exit statuses and saved versus temporary channels; current/no-release and
network failure results; parent-console and redirected-output behaviour; authenticated host
handoff and rejection; no closing sessions before confirmation; verified external installation
and host-window installation with locked-file replacement, notices, rollback and restart.
Keep check/lint/tests and byte-exact transport checks passing. Record actual Windows command
and window evidence, then publish the next sequential development release and verify its assets.
