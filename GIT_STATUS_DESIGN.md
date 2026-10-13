# Local Git status indicator

Requested 2026-10-13. Awaiting approval under AGENTS.md §3; a separate cycle after the current
interface/persistence and bundled-tool delivery, rather than accumulating unrelated release work.

Explicitly approved by the design lead on 2026-10-13.

- Add a compact, toggleable status area for normal terminal sessions: branch, distinct changed-file count, conflicts, and ↑ ahead/↓ behind counts against the local tracking ref. Label missing upstream/detached/error states honestly. Explain that counts are based on the last fetch; never auto-fetch, contact a remote or change repository files. Hide the status area in alternate-screen applications so their own UI keeps the space.
- Track session cwd via original OSC 7 metadata handling plus an opt-in, session-local PowerShell prompt wrapper for default interactive launches; preserve existing prompt styling and never edit global profiles. Leave custom launch arguments untouched. Initial cwd is a fallback explicitly identified as such; unsupported shell reporting must not silently show a stale repository as current. Validate local file URIs without performing I/O in the engine.
- Use one bounded worker for Git porcelain-v2 queries of the active cwd, with timeout/output limits, generation-tagged results, disabled optional index locks/fsmonitor hooks/submodule recursion, and no renderer/engine references. Treat failures/oversized output as unavailable rather than fabricated totals. Keep each tab's metadata separate and recheck on cwd/activity/explicit refresh with a modest rate limit. Verify clean/dirty/staged/untracked/conflicted/ahead/behind/detached/no-upstream repositories, worktrees, cwd/tab switches, stale worker results, alternate-screen space, unsupported shells and shutdown. Record required gates and UI/real-repository evidence before releasing.

Repository status is local metadata; it is excluded from portable-settings exports and diagnostic
payload dumps. No terminal implementation or remote credential service is introduced.
