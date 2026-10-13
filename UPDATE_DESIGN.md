# GitHub releases and updater design

Requested 2026-10-13. Windows x64 is the supported release target for this iteration.

- Publish versioned GitHub Release assets through a reviewed Windows GNU workflow triggered by version tags. Package Juna_<version>_windows_amd64.zip with juna.exe and complete notices; embedded fonts/icons/tools remain self-contained. Run required Windows checks, verify PE/icon/version metadata, and attach SHA-256 checksums/build provenance. Embed application version and source commit; validate tag/version agreement. Produce an initial preview release only after the terminal/interface/tool milestone is verified; stable and preview channels are distinct. Development Actions artifacts are separate from updater assets.
- Add Settings → Updates with a saved Stable/Preview channel and manual Check/Download/Install. Query published releases (not the first tag), reject drafts/unsupported assets, compare semantic versions and source identity, and use browser_download_url from the API. Stream HTTPS downloads on a bounded I/O worker with progress/cancellation, connection/header/idle timeouts, no arbitrary total-transfer deadline, and a maximum payload size. Verify the required SHA-256 digest/manifest before staging; reject missing/mismatched integrity data, wrong PE architecture/version and unsafe archive entries. Use Windows HTTP/crypto infrastructure outside the VT engine; no async runtime. Background checks are optional, with no telemetry.
- Installation follows an explicit user action after the candidate is verified. Shut down/reap all sessions cleanly, then run a hidden Rust helper mode from a temporary copy outside the installed EXE. Retry replacement within a bounded interval after the parent exits, preserve the previous executable as a rollback copy, and commit the executable/notices as one versioned installation transaction. Restart only after verified replacement; on failure restore the old version, persist a result record and surface it after restart (or show a helper failure dialog if restart is impossible). Never silently elevate or weaken TLS. Preserve settings/themes and app-owned versioned tool data.

Acceptance: stable/preview/version selection, missing/no-newer releases, rate limits/offline behavior;
slow/cancelled/mismatched downloads; ZIP traversal and wrong-platform rejection; locked files,
replacement/rollback failures and restart reporting; source/version metadata; actual Release asset
download and isolated update rehearsal; required gates and recorded UI evidence. New dependencies
or Windows features require written justification in DECISIONS.md. No workflow/tag/release mutation
until approval under AGENTS.md §3 and the inherited CI/release review rule.

SHA-256 and HTTPS verify download integrity and GitHub origin; they do not independently protect
against a compromised publisher account. Publisher signature/attestation verification needs a
reviewed key/trust policy before it can be claimed.

Primary API documentation: https://docs.github.com/en/rest/releases/releases

Explicitly approved by the design lead on 2026-10-13, in interface → Junafetch → updates order.

