# Junafetch console frontend

This small Rust console program presents the separately maintained Fastfetch 2.69.0 detector.
It is built independently to avoid GUI-process wait/exit behavior and circular binary embedding.
Juna embeds the resulting executable and registers both junafetch.exe and fetch.exe per session.

Standard groups System, Hardware, Storage, Desktop, Session and Network statistics. It includes
kernel, locale, all detected GPUs/displays/disks, memory/swap and Windows appearance. Detailed
adds board, BIOS, battery and driver fields; Brief retains eight compact rows and the system disk.
Unknown data, including shell versions the detector cannot identify, is labelled Unavailable.
Values use binary units and consistent percentages. Generated profiles ignore installed configs.
Settings → Tools → Junafetch saves profile/artwork choices for the next invocation in any tab.
Explicit --brief, --details, --profile and --logo flags override saved choices. --plain and --json
avoid artwork; JSON schema 2 includes profile and section on each field. Artwork is compact, full
or none. Unicode-aware wrapping fits console width; narrow output stacks or omits oversized art.
Pipes default to plain headings. Section borders use Juna's ANSI palette accent.

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
