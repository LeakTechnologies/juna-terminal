# Junafetch console frontend

This small Rust console program presents the separately maintained Fastfetch 2.69.0 detector.
It is built independently to avoid GUI-process wait/exit behavior and circular binary embedding.
Juna embeds the resulting executable and registers both junafetch.exe and fetch.exe per session.

Default report: OS/build, CPU cores/threads, physical GPU/VRAM where known, memory, system disk,
uptime, launch-session shell and Juna version. Values use binary units and consistent percentages.
Unknown data is labelled Unavailable. A generated JSON profile scopes default disk detection to
the system drive and ignores installed Fastfetch configs. --details adds safe hardware rows;
--plain and --json avoid artwork. Artwork uses the compact candidate, full original or none.
Width comes from the native console; narrow output stacks artwork and rows. Pipes default to plain.

Personal/network identifiers and raw paths are omitted rather than passed through the report.
There is no public-IP request or background polling. Interactive protected-value reveal and
terminal-wide detection remain unshipped privacy work; this frontend does not claim those features.

The detector is fixed by source/hash manifest, extracted into the current-user cache, verified
with Windows SHA-256, and run with bounded output/lifecycle. It needs no installed Fastfetch.
Original MIT/compiled-library notices are retained in assets and notices.

```powershell
cargo check --manifest-path tools/junafetch/Cargo.toml
cargo clippy --manifest-path tools/junafetch/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tools/junafetch/Cargo.toml
cargo build --manifest-path tools/junafetch/Cargo.toml --release
```

The release workflow verifies this source and checks that the pinned frontend manifest matches
the embedded asset. A payload update is a reviewed change with a fresh source/hash record.
