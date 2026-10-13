# Juna release cadence

Each completed landed cycle gets a fresh sequential development version, matching Cargo
version/lockfile, a release note in releases/<version>.md, and its own version tag. Push the
commit and tag, verify CI/release workflows, download and validate assets, then report publication.
Never replace changed binaries under an existing published tag. Workflow repair for a failed,
unpublished tag is recorded explicitly; do not reuse a published tag to hide a repair.

The first current interface preview is 0.1.1-dev1. Later completed cycles increment dev2, dev3,
and so forth, including CI fixes. Stable builds omit the dev suffix and require explicit stable
acceptance. Juna's planned 0.2.0 milestone is a tested terminal with bundled Junafetch, verified
privacy controls and a rehearsed verified updater; it is independent of VideoTools milestones.

Release assets target Windows x64 GNU: Juna_<version>_windows_amd64.zip, SHA256SUMS.txt and
build-info.json. The ZIP contains juna.exe, original font/palette notices and release notes.
Build metadata includes version, source commit and target. Version/icon checks are performed on
the actual PE. The release workflow checks tag/version agreement, runs standard gates, creates
a draft, uploads assets, verifies their names/sizes/digests, then publishes the prerelease.
The local follow-up downloads and checks the archive before reporting it available.

Checksums/HTTPS verify integrity and origin, not independent publisher authenticity. There is
no signature/attestation trust policy yet. GPU and interactive desktop acceptance is recorded
locally; hosted CI passing does not replace the human input/mixed-DPI checklist.

Junafetch/fetch, terminal-wide privacy, the in-app updater, translations and Android remain
unshipped in the first interface preview. Each gets its own completed cycle and evidence.
