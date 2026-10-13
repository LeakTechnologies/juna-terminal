# Persistent settings and portable JSON

Requested 2026-10-13. Supersedes the Settings-wide Save/Cancel appearance policy under AGENTS.md §3.
Explicit font autosave also received approval.

Explicitly approved by the design lead on 2026-10-13.

- Save confirmed preference changes automatically: explicit font/theme choices, font-size arrows/zoom, contrast entries, line-number toggles and favourites. Hover/navigation remains temporary; numeric editing previews until Enter/Apply, and Cancel discards only that unfinished edit. Maintain a committed preference baseline so a hover preview never leaks into saved JSON. Replace the Settings Save/Cancel rows with Close, Import settings and Export settings; Escape closes while preserving already committed changes. Report failed writes and restore the prior committed setting.
- Export a versioned, bounded JSON bundle containing an explicit portable-settings allowlist and validated custom themes needed by the active/favourite choices. Exclude credentials, machine identity, private literals, process environment, history and tool-cache paths. Use native file selection on a bounded worker so the engine thread keeps consuming output. Import parses/validates the entire file before an explicit review/Apply step; support existing bare settings JSON too. Show changed preferences and theme/conflict counts. Do not execute data or fetch URLs. Reject unknown schemas/invalid values/oversized bundles, preserve original files, and never silently overwrite custom theme conflicts.
- Stage imported themes under new app-owned IDs when necessary and remap their preference references; commit settings last with rollback of newly staged files on failure. Restore the committed baseline on preview cancellation without losing independently saved changes. Keep renderer/engine/backend boundaries unchanged, no new runtime/dependency; justify native file-dialog platform features. Verify restart persistence for every current preference, hover/unfinished entry cancellation, failed writes, cross-machine export/import, conflicting/missing themes and invalid/hostile JSON. Run required gates and record UI/file-dialog evidence before publishing.

Future preferences must register persistence and export sensitivity explicitly rather than
automatically exporting arbitrary new fields. Tabs, history and credentials are not settings backups.
