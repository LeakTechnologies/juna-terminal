## Dev23 taskbar class-icon repair (2026-10-21)

Confirmed the running dev20 window exposed the dog through `WM_GETICON` (ICON_SMALL 32x32,
ICON_BIG 48x48) while its taskbar button showed a generic application icon. The executable's
shell icon was also the dog, and a fresh executable path plus a fresh AppUserModelID still
showed generic, ruling out icon-cache poisoning and a missing resource. Setting the window
class icon on the live process changed the taskbar button to the dog immediately; the same was
then reproduced with a valid AppUserModelID. Explorer uses the class icon for a live button.

The app now registers the winit-owned small/large HICONs with `GCLP_HICON`/`GCLP_HICONSM`
before showing the window and verifies both class words. No dependency, shortcut, file write,
AUMID change or terminal-engine boundary changed. Verified this session: check, strict Clippy,
full standard test suite, and taskbar captures at 66x72 for the generic and repaired states.
The user's already-running dev20 instance keeps its old class icon until restarted.

## Dev22 tab title portability repair (2026-10-20)

The hosted dev21 Rust workflow failed one lib test: Linux `Path::new("C:/").file_name()` is
`C:`, so the drive-root title expected `C:\`. The basename is now derived as text (trim
separators, take the final component, expand a two-character drive component to `C:\`), which
is host-independent and needs no platform branch. Added cases for both separators, a trailing
separator and `/`. The dev21 release binary and Development release workflow were otherwise
successful; its tag remains published and is not moved. This cycle bumps to dev22.

Verified this session: strict all-target Clippy and the standard local suite (234 lib plus all
integration tests; existing desktop/network/GPU markers ignored). Hosted Linux and Windows
confirmation follows the dev22 tag.

## Dev21 tab reordering, directory titles and shell selection (2026-10-20)

Implemented the approved TAB_REORDER_TITLE_SHELL_DESIGN. Left-drag reorders tab records with a
shared insertion-marker geometry and edge scrolling; the active engine stays with its tab because
the active index is re-derived from the stable tab ID. Titles compose the OSC 7 directory basename
with the shell name and fall back to the launch directory; rename pins a label. Directory
tracking now defaults on, while an explicit stored `false` is preserved. `cmd`/`bash` show the
launch directory until an equivalent OSC 7 integration exists.

The shell preference accepts only powershell/pwsh/cmd/bash tokens; the executable is resolved at
launch so imported settings cannot name an arbitrary program. An explicit `--shell`/`JUNA_SHELL`
launch outranks it, and changes apply to new tabs only. Portable settings carry the token with a
back-compatible default.

Verified this session: `cargo check --all-targets`, strict all-target Clippy with `-D warnings`,
and `cargo test` (234 lib tests plus 6 bench, 1 release-verify, 1 CLI, 6 ConPTY, 4 elevation,
5 shell round-trip and 1 update identity passing; 2 lib and 22 desktop/network/GPU tests ignored
by their existing markers). Drag is a pointer gesture: geometry and state are unit-tested, but
recorded manual visual evidence is still required before claiming the gesture feels correct.
No dependency, VT semantic, byte-transport or engine-ownership change was made.

## Dev20 native benchmark utility (2026-10-18)

Completed the approved first owned-Juna measurement slice as a separate source-built binary.
Quick/full profiles measure synthetic output, parent CPU/private memory/working set/regions,
warm fixture startup and targeted input with independently timestamped receipt/unique visible
response. Reports are local JSON and escaped standalone HTML; ordinary Juna sessions do not
run capture or benchmarks. No terminal-engine implementation or new crate was imported.

Verified this session: check, strict all-target Clippy, all 250 executed standard tests,
release builds, native x64/version/seven-icon checks and bundled-tool pins. The restricted
test attempt failed on temp/native fixture permissions; unchanged native rerun passed.
Final full/quick profiles passed in 42 owned windows including warm-ups, with 330 measured
input responses and clean shutdown. Deliberately closing the owned test child rejected the
run, removed staging and wrote no success report. See docs/performance/dev20 for raw evidence.

This is a measurement release, not a memory/performance fix or laptop/stable acceptance.
Cross-terminal equivalence, true Ctrl+C, scrolling and multiple sessions remain unmeasured.
The developer utility is not added to the normal ZIP or menus. Hosted CI and downloadable
assets are verified after tagging; local checks alone do not establish publication.

## Dev19 output allocation repair (2026-10-18)

Use a printable-ASCII pair boundary fast path under the approved performance design; all
Unicode cases retain the existing segmentation path. Exhaustive pair/chunking regression
passed. Three-run isolated desktop medians show parent output CPU 3.52 -> 2.50 seconds for
4 MiB; the instrumented engine probe records 12,084,179 -> 256,979 allocations. See PERFORMANCE.md
and docs/performance/dev19 for exact counters, source, methodology and limitations. Idle
memory is unchanged; laptop and physical Explorer/OpenCode acceptance remain unverified.
Verified this session: check, strict all-target Clippy, all 244 standard tests, all nine
real-window tests and the isolated OpenCode startup-interrupt probe. Native x64/version/seven
icon and bundled-tool pin checks passed. Hosted CI and downloaded assets are verified after
tagging; these local results do not establish human/laptop acceptance.

## Dev18 control-input fixture repair (2026-10-18)

Hosted dev17 diagnostics showed sent=true, child exit 101 and CONTROL_READ count=1 with
values [3,0]. The second ReadFile returned zero for leading Ctrl+Z. This matches the Windows
console behaviour documented in microsoft/terminal issue 4958; it is a fixture API error,
not evidence that Juna dropped the byte or crashed. Correct the child to ReadConsoleW with
one UTF-16 character per call, explicitly testing Ctrl+Z at the start of its own read. Keep
exact values [3,26], successful child exit, existing deadlines and process cleanup assertions.
Raw console mode is read back and verified. Product input/transport are unchanged.

Retraction: the dev16/dev17 raw fixture's local pass was not portable evidence across Windows
console versions. Dev16 and dev17 tags exist, but neither published downloadable release assets.
This cycle carries their file drops, versioned titles, GUI console isolation and diagnostics.
The exact reported OpenCode interaction and physical Explorer drops still need human testing.

References: https://github.com/microsoft/terminal/issues/4958 and
https://learn.microsoft.com/en-us/windows/console/readconsole. Only published behaviour/API
documentation was consulted; no terminal implementation was copied. No dependency changed.


Dev18 local validation: cargo check, strict all-target Clippy and all 243 standard tests
passed, including the corrected control-input fixture and all six ConPTY tests. Hosted
Windows and release-asset evidence is verified separately after tagging.

## Dev17 diagnostic candidate (2026-10-18)

The design lead requested that the existing repairs ship as dev17 with focused diagnostics.
Dev16 local checks passed, but hosted Windows CI failed the new raw-control-byte assertion;
its output was absent from the original assertion. No root cause has been established.
The unchanged retry was cancelled at the lead's request; dev16 was not published.

Dev17 reports fixed probe readiness/input mode/received bytes/exit and bounded fixture output
on failure. Runtime logs record build identity, queued Ctrl+C/Ctrl+Z labels, existing session
exit/fault evidence and final-session closure; panics add source location without payloads.
No command text, arbitrary child output or sensitive values are added to application logs.
Checks retain their deadlines and assertions. Physical Explorer dragging and the original
OpenCode case remain human acceptance; hosted results are reported separately after push.


Dev17 local evidence: cargo check, strict all-target Clippy and all 243 standard tests passed,
including the diagnostic raw-control-byte probe and all six ConPTY tests. Hosted Windows
behaviour and published assets remain separate verification after this push.

## Dev16 input reliability (2026-10-18)

Implemented approved INPUT_RELIABILITY_DESIGN.md: bounded file/folder drops insert quoted
local path text without Enter, versioned native titles retain updater identity checks, and
release GUI startup no longer attaches to the launcher console. CLI redirection remains intact.
No engine/backend semantics or pinned ConPTY dependency changed. Tab detachment is roadmap work.

Verified: cargo check, strict all-target Clippy and all 243 standard tests, including six
ConPTY acceptance tests, exact control bytes, Unicode/smart-quote literal paths and live
input routing. Dropped input immediately hides private reveals. Eight existing real-window
checks passed; the separate release-console regression confirms dev16 stays detached and
survives an isolated launcher interrupt, then exits normally. Baseline dev15 attached and
terminated with 0xC000013A. This establishes that defect, not the exact reported OpenCode cause.
The installed OpenCode 2.0.31 startup-cancellation probe passed with shell recovery and no
transport faults in an isolated profile; its local managed-service port was already occupied,
so this is not full TUI/model-interruption acceptance. Physical Explorer dragging and the
user's original OpenCode interaction remain human acceptance checks.

Fixture corrections: Windows PowerShell's OEM850 ReadLine replaced a Unicode character;
the controlled peer now selects UTF-8 explicitly before input. Its JSON writer also adds a
BOM, which the report reader now accepts. The first OpenCode probe used unsupported v1 flags;
the passing probe invokes the installed v2 wrapper without those flags. These failures did
not establish product defects. Hosted release and downloaded-asset verification follow tagging.

## Dev15 protected-field repair

Implemented approved PRIVACY_REPAIR_DESIGN.md: ordinary masks follow display width, typed
passwords use eight asterisks, and opaque colour-pair markers survive ConPTY before app-owned
mapping/normalization. Actual values remain outside VT output, grid/history and copied text.
Marker reservations never reuse during the window lifetime; expiry removes values, not marker
identity. Late IPC invalidates the committed mapping. No dependency or engine semantics change.

Verified this session: cargo check, strict all-target Clippy, all 237 standard tests and nine
frontend tests; all six explicit GPU tests and all eight real-window tests. The strengthened
bundled-fetch test now follows actual frontend → ConPTY → engine → app input at scaled/gutter
coordinates and through scrollback, exercising Shift+hover, hide and censored selection copies.
Synthetic tests cover wrapping, late registration, surviving cells after resize, marker collision,
capacity, expiry/tab isolation, Unicode width and fixed password masks. GPU captures of the
normalized mask and synthetic reveal were visually reviewed; see docs/acceptance/privacy-hidden.png
and privacy-revealed.png. Byte-exact transport and ConPTY acceptance pass. The new resize test
initially assumed live-screen reflow; corrected it to the engine's documented column clipping,
without changing engine behaviour. Restricted Windows spawning was rerun with process access.

Retraction: the earlier reveal claim relied on raw OSC tokens and direct-engine tests. Real
ConPTY lost the association with shade cells; those tests did not establish end-to-end reveal.
The repair's integration assertion checks actual displayed cells and production input handling.
Junafetch collects no passwords, and arbitrary command output is not automatically recognized.
Other-machine/human acceptance and older supported Windows remain separate from these checks.
Hosted workflows and downloaded release assets are verified after the fresh tag is pushed.

## Dev14 command-line updates

Implemented --check-update, --update, temporary channel overrides and --help under the approved
CLI_UPDATE_DESIGN.md. Checks are window-free; in-terminal installs use authenticated host
handoff and the existing Cancel-first confirmation. External installs require interactive
confirmation. Matching-executable window preflight and preserved redirected standard handles
support portable use without system PATH/profile changes. No dependency or engine change.

Verified this session: check, strict all-target Clippy and all 234 standard tests; six GPU
tests; actual GitHub checks with saved/temporary channels; debug and release help/error output;
and real-window external rejection followed by authenticated handoff, with byte-identical
preferences. The shared installer passed locked-file replacement/restart and preserved settings
and custom themes; actual published-release staging also passed. The new fixture initially
asserted its baseline before saving it; corrected fixture ordering passed without relaxing
application checks. A complete CLI yes-to-newer-release user journey still needs a subsequent
newer published candidate; current checks correctly report no newer release. Stable/human
acceptance remains separate.

## Dev13 protected Junafetch fields

Implemented authenticated current-user local IPC, shaded `░░░░░` cells, per-tab bounded
value storage, deliberate Shift+hover reveal and saved Privacy controls. Copies remain masked;
plain/JSON output is masked unless the user explicitly disables censorship. General terminal
recognition remains open. No additional crate, async runtime or borrowed engine was added.

Verified in this session: check, strict all-target Clippy, 231 standard tests, seven frontend
tests, bundled frontend/source hashes, all six explicit GPU tests and real-window bundled
fetch. The extended ConPTY integration also verifies that protected values never enter its
captured output. GPU captures show five shaded cells and an RFC documentation-address popup;
hiding restores identical pixels. Engine cell storage stays at or below its existing 48 bytes.
Physical input and other-machine acceptance remain outstanding. High memory/bulk-output
performance remains open; this cycle does not claim new laptop performance results.

## Dev12 verified updater and measured redraw preparation

Settings → Updates provides manual Stable/Preview checks, verified GitHub Release downloads
and explicit Install and Restart. A hidden helper handles locked-file replacement, matching
notices, retained rollback files and first-frame startup acknowledgement. No automatic polling
or telemetry. Settings/custom themes remain unchanged through the isolated update rehearsal.
Interrupted-helper/power-loss recovery and backup retention remain manual.

Verified in the 2026-10-15 session: cargo check, strict all-target Clippy and all 224 standard
tests; all five GPU acceptance tests; real-window shell output while menus are open; actual
GitHub asset download/verification; forced partial-install rollback; and dev11-to-dev12 locked
replacement/restart with byte-identical saved font/settings/custom theme files. The initial
sandbox run failed on file permissions; the unsandboxed rerun passed without relaxing tests.
The first rollback rehearsal previously failed because its 285 MB debug executable exceeded
the production limit. Matching release fixtures fixed the rehearsal; the limit was retained.

Measured release-mode redraw preparation improved from 2.119 to 0.951 ms for ASCII, 14.708
to 0.989 ms for light boxes, and 2.086 to 0.960 ms for Unicode (200 timed samples each).
The whole-window output comparison did not show a clear improvement; private memory remains
approximately 435 MiB on this desktop. PERFORMANCE.md and sanitised reports record the limits.
Existing inactive-tab BEL attention work is included; this does not infer command completion.
Laptop, physical-input and stable acceptance remain separate. Hosted CI/assets are verified
after publication; this local record alone does not establish a published release.

## 2026-10-13 — bundled-tool environment preparation

Verified child-only SessionSpec environment overrides with spaces/Unicode and unchanged parent
environment. Check/clippy and 186 standard tests pass, including byte-exact transport coverage.
The separate Junafetch detector fork builds and passes its 18 upstream tests. Bundled command
delivery, Rust report presentation, privacy and updater acceptance remain in progress.

## 2026-10-13 — tabs and theme families complete

Implemented the approved interface stage: independent tabs, keyboard/mouse controls, exact
numeric settings entry, 60 grouped themes, and a Custom submenu with family-folder discovery
and recoverable removal. Verified check/clippy, 185 standard tests, four explicit GPU tests,
three real-window tests, release build, embedded icon bytes and direct release exit 0.
Reviewed captures show right-side panels leave terminal text visible. Cargo emitted incremental
hard-link fallback warnings after a restricted filesystem run; no source Clippy findings remained.
The restricted run failed seven filesystem-dependent tests; the normal-access rerun passed.
Physical mouse/IME/mixed-DPI review remains outstanding. Junafetch and updater are subsequent stages.

## 2026-10-12 — fixed application title

Keep Juna Terminal as the visible window title while preserving shell OSC title metadata.
Verified: check/clippy, 176 standard tests, and two explicit real-window tests pass; both
Command Prompt and PowerShell attempted a title change. Fastfetch 2.69.0 ran through
PowerShell/ConPTY/the engine with success marker, exit 0 and zero dropped bytes.

# Completed iterations

## 2026-10-12 — approved theme/options system

Implemented THEME_DESIGN.md: Catppuccin Mocha default, 20 starter choices, non-modal right-click
options, searchable previews, font/contrast settings, and custom-theme draft/save persistence.
Validation, read-only/corrupt-file preservation, menu geometry/input isolation, all-palette GPU
captures, and real output with overlays open were exercised. Future VS Code export remains deferred.

Verified for this iteration: warning-free check/clippy; 176 standard tests; three explicitly run
GPU acceptance tests; two explicitly run real-window acceptance tests; updated release build.

## 2026-10-12 — approved contrast repair

Confirmed and corrected the sRGB surface brightening defect, added a presentation-only minimum
contrast policy, and logged actual font selection. Both GPU acceptance tests pass; the new
right-click/theme design has separate recorded approval in THEME_DESIGN.md.

## 2026-10-12 — approved terminal readiness repairs

Implemented the three-stage AUDIT.md design. Acceptance evidence and remaining human checks are
recorded in TERMINAL_TESTING.md. Historical audit failures remain in AUDIT.md rather than being
silently rewritten. This milestone supports interactive testing, not a full Tier 2 release claim.

Verified: warning-free check/clippy; 165 standard tests pass; GPU row-isolation/capture and actual
window/exit acceptance run separately; release executable built. Human physical input, clipboard,
mixed-DPI and editor/pager review remain explicit in the testing matrix.

## 2026-10-13 — icon installed

Verified all 176 standard tests, check/clippy, both live-window tests and release build.
Seven embedded icon images match the approved ICO byte for byte; native small/taskbar handles exist.


## 2026-10-13 — fonts and live sizing complete

Verified 180 standard tests, warning-free check/clippy, four real GPU tests, two real-window
tests and a rebuilt release. Fifteen font faces resolve without system fonts; search, Save/Cancel,
zoom/AltGr routing, wheel accumulation, persistence and bounds have focused regression coverage.
GPU captures cover all fifteen, fallback, row isolation and minimum/default/maximum sizes.
README now displays the approved logo and documents current features and controls.
Taskbar API/icon handles are verified; user confirmation of the updated Explorer button is pending.

## 2026-10-13 — preview placement and taskbar verified

Right-side previews and a simplified settings menu pass the pixel/geometry checks. All 180
standard tests, check/clippy, targeted GPU and both real-window tests pass; release rebuilt.
Direct release launch exited 0, native icon resources match the source, and the user confirmed
the visible taskbar logo after the fresh-group property correction. Upcoming work has approved
designs in UI_NEXT_DESIGN.md, JUNAFETCH_DESIGN.md and UPDATE_DESIGN.md.
## 2026-10-13 — 0.1.1-dev1 interface/preferences cycle

Implemented approved separated tab menus, local rename, Cancel-first middle-click confirmation,
theme-coloured check/cross labels, context-menu reanchoring and protected editing panels.
Line numbers live in Settings, have a two-cell gap, and hide/refit in alternate screens.
Favourite stars, a dedicated section, bulk family controls and immediate wheel scrolling are present.
Confirmed preferences persist automatically; JSON transfer validates portable fields/themes, reviews
imports, preserves conflicts and rolls back staged files on failed preference writes.

Verified in this session: required check/strict Clippy and standard tests; four explicit GPU tests,
three actual-window tests, reviewed captures, native x64/version/seven-icon resource verification.
Detailed counts and limitations are recorded in TERMINAL_TESTING.md. Native file-dialog interaction,
physical input/mixed-DPI and other-machine acceptance remain human checks. Publication is not yet
claimed: the tag workflow and asset download/hash verification must finish first.
## 2026-10-13 — dev1 publication verified; dev2 CI correction

Dev1 Windows workflow passed and published a prerelease. Downloaded archive/manifest/checksum
verification confirms its exact source commit, native version and approved icon resources.
The separate Linux run failed the import rollback test because Unix rename replaced a read-only
file. Dev2 adds an explicit portable read-only guard and enables its preservation test on Linux.
This is a fresh version; no published dev1 tag/binary was changed. Hosted dev2 evidence is pending.

Verified locally for dev2: check, strict Clippy and all 200 standard cases pass. The Windows
interface is unchanged; its recorded dev1 GPU/window evidence still applies. Hosted Linux and
new-version artifact checks must verify the correction before publication is reported.

## 2026-10-13 — dev2 publication and bundled-tool acceptance

Verified dev2: hosted Linux CI and Windows release workflow passed. Downloaded ZIP SHA-256
b4c340ca2ea68d448ab9eba987f3a3b346f1325a82ccc81c69fa820e6a2578d7 matches its manifest;
source 7172df647fff607c79ffc079339c62bd1b10646f, native version and all seven icon images match.

Verified bundled utility tests: cached frontend corruption is repaired; both aliases execute
through production ConPTY with only system/tool PATH and a Unicode cache directory, without
installed Fastfetch. The frontend constructs eight safe JSON fields and omits personal/network
identifiers. These checks do not establish terminal-wide masking or hover reveal.

Dev3 validation in this session: cargo check, strict all-target Clippy and all 202 standard
Juna tests passed, including byte-exact ConPTY/shell transport. The separate frontend check,
strict Clippy and one report regression passed. Pinned payload/source verification passed.
The real-window bundled_fetch test passed: system-only PATH, both aliases, eight-field JSON,
rendered acceptance marker, presented GPU frames and normal shell/window cleanup. This is
controlled runtime evidence; artwork aesthetics and other-machine acceptance remain human checks.
## 0.1.1-dev4 — graphics roadmap

Added Kitty graphics protocol support as future work at the user's request. No terminal
semantics, renderer boundary or dependencies changed. Implementation requires its own
approved design and acceptance evidence. This documentation cycle uses a fresh dev tag.
Verified for this documentation/version cycle: cargo check, strict all-target Clippy and all 202 standard tests passed. Graphics runtime acceptance does not apply because no graphics implementation changed.

## 0.1.1-dev5 — legacy Windows roadmap

Recorded the user's low-priority older-Windows investigation. No backend, dependency or runtime behaviour changed; compatibility is not claimed.
Verified in this session: cargo check, strict all-target Clippy and all 202 standard tests passed. Older-Windows runtime support remains unverified and unimplemented.

## 2026-10-13 — local Git status acceptance in progress

Verified targeted tests: real clean/staged/unstaged/untracked/conflicted/detached/worktree and
local ahead/behind fixtures; unchanged index bytes/mtime; fsmonitor suppression and active-filter
refusal; initial-folder invalidation/stale generations; bounded worker shutdown. PowerShell's
controlled prompt text and LASTEXITCODE are preserved, and Unicode OSC 7 metadata survives real
ConPTY. Invalid/remote metadata is rejected without changing cells. Full gates and footer/window
acceptance must pass before this development cycle lands. Physical input/other-machine review
remain separate; older Windows 1803 is unsupported by the current ConPTY backend.

Dev6 final evidence: cargo check, strict all-target Clippy and all 210 standard tests passed
on the settled source. Explicit GPU footer comparison passed, preserving terminal pixels and
engine cells; docs/acceptance/git-status.png was visually reviewed. The actual-window Git test
passed with reported cwd, repository result, visible/hidden footer transitions, presentation
and exit 0. Its controlled stream accepted 967 bytes and presented 14 frames. Physical input,
custom prompt implementations and other-machine acceptance remain human checks. Expanded fetch
profiles/submenus and Ctrl+Shift+N designs received approval but are not shipped in dev6.

## Dev6 hosted failure and dev7 correction

Local dev6 acceptance did not establish hosted release readiness: both hosted runs failed and no
Release was published. Git's conflict fixture lacked a CI committer identity; a Windows test
compared short/long path spellings rather than directory identity; Linux exposed a pre-existing
fake-session scheduling race. Dev7 corrects only those fixtures, with no runtime changes.
The unfinished approved grouped-fetch work is preserved for the following cycle. Hosted and
local dev7 gates must finish before publication is claimed.
Verified locally for dev7: cargo check, strict all-target Clippy and all 210 standard tests passed. Runtime code is unchanged; dev6 footer/window evidence remains applicable. Hosted verification is pending.

## Dev7 hosted timeouts and dev8 scheduling

Dev7 Linux passed. Windows passed the corrected fixtures but timed out in the unchanged
runtime shutdown/tail and PowerShell prompt ConPTY tests (193 passed, two failed).
Both probes passed locally with their original deadlines in this session. The approved
dev8 workflow isolates Windows test scheduling with one test thread; no test is skipped,
deadline increased or process-backend dependency changed. Scheduling pressure is a
hypothesis. An unchanged dev7 job rerun was requested once, separately from this correction.
Expanded Junafetch/grouped settings are preserved outside this release's source.

The single unchanged dev7 Windows rerun subsequently passed; Linux also passed.
This demonstrates intermittent failure, not a confirmed scheduling root cause.

Verified on isolated dev8 source: cargo check, strict all-target Clippy and all 210
standard tests passed with sequential scheduling. No visual/runtime behaviour changed;
prior explicit window evidence remains applicable. Hosted dev8 verification is pending.

## Dev9 grouped Junafetch and settings

Implemented the approved expanded-statistics and grouped-settings designs. The standalone
frontend passed check, strict Clippy and four tests; its rebuilt embedded payload and all
normalized source fingerprints passed verification. On final source, root check, strict
all-target Clippy and all 212 standard tests passed. The ConPTY alias test verified saved
Brief, CLI Standard override, protected placeholders and a next-command Detailed preference
change within the same shell. Explicit GPU panel acceptance passed without engine mutation;
docs/acceptance/grouped-settings.png was visually reviewed, including the shortened folder
reporting label. Actual-window bundled fetch passed with 6,528 accepted bytes, six presented
frames, marker present and exit 0. Human input/other-machine review remain outstanding.
Shell versions may be unavailable; actual IP masking/reveal is not shipped. Dev8 hosted
Windows/Linux checks and independently downloaded checksum/version/seven-icon checks passed.
Dev9 hosted publication still requires verification after push.

## Dev10 independent windows

Implemented the approved Ctrl+Shift+N design: absolute direct launch, configured interactive
shell, reported local folder with launch-folder fallback, preserved argument boundaries and
no replay of one-off/inherited shell arguments. Editing panels, IME, Alt/AltGr and repeat do
not activate the shortcut. Separate create-new diagnostic files prevent cross-window truncation.
Verified locally: cargo check, strict all-target Clippy and all 213 standard tests passed.
The controlled desktop test used the same launch constructor as the shortcut: reported Unicode
cwd and saved Fira Code/18px preferences matched, two independent windows presented frames and
exited cleanly, and the original remained running after the new window closed. The explicit
three-tab concurrent-output/reaping test also passed with the new log paths. Physical shortcut
input and other-machine acceptance remain human checks. No process backend/dependency changed.
Dev9 and dev10 hosted checks/assets must be verified before claiming publication.

## Dev11 home-directory startup

Implemented the approved home-directory design. Ordinary interactive launches resolve a
validated user home; --cwd selects an explicit directory; custom argument launches retain
inherited cwd. New windows explicitly preserve reported or shell launch folders. No process-wide
cwd change or dependency addition. Verified final source: check, strict all-target Clippy and
all 214 standard tests passed. Actual portable-window acceptance verified home, explicit and
custom-command directory identities, GPU frames and shell exit 0. Initial CMD Unicode-text
comparisons failed; Unicode-safe measurement confirmed home selection and exposed a real CMD
fallback for canonical device-prefixed explicit paths. Normal drive syntax fixes that defect.
This retracts the interim interpretation that the failing acceptance was only a text fixture
issue: the explicit-folder path also needed a production correction. Independent-window
acceptance is recorded separately after completion. Human portable-folder testing remains pending.
Verified final independent-window acceptance also passed: reported Unicode folder, saved appearance, separate logs and peer-session survival after closing the new window.
