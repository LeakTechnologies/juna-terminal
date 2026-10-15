# Testing the repaired terminal

## Dev13 evidence (2026-10-15)

Check and strict all-target Clippy passed; all 231 standard tests and seven Junafetch frontend
tests passed. Explicit GPU tests: six passed, including shaded protected fields and a temporary
reveal overlay. Removing the overlay restores byte-identical masked pixels without changing
engine cells. Captures: target/acceptance/privacy-hidden.png, privacy-revealed.png and
privacy-menu-right.png; these contain only a synthetic RFC documentation IP address.

Input-event tests verify Shift alone does not reveal, movement after Shift does, modifier
release/focus loss hide, and copied selections contain shades. IPC tests cover authentication,
wrong server identity, repeated messages, revoked tab capabilities and cancellation on shutdown.
The extended real ConPTY bundled-fetch test verifies emitted opaque tags and absence of protected
values in captured PTY output. The real-window bundled-fetch test also passed. The cell-storage
regression guard passed at the existing 48-byte ceiling. Physical mouse/keyboard use, other
machines and arbitrary-output privacy recognition remain unverified or unimplemented as stated.

## Dev12 evidence (2026-10-15)

Required check/strict Clippy and all 224 standard tests passed. All five explicit GPU tests
passed, including all bundled fonts, Unicode/row isolation, palette bytes, theme/menu overlays
and footer preservation. Thirty-seven unchanged menu captures matched byte-for-byte across
the renderer optimisation. The update-confirmation fixture was corrected to show Cancel
selected, matching the application's verified default; those two changed captures are excluded
from the unchanged-image comparison.

Real-window testing passed for shell output with panels open and dev11-to-dev12 update/restart.
The latter verifies retained user files, byte-identical settings/custom themes, matching new
notices, native executable identity and first-frame acknowledgement. Explicit forced rollback
and an actual published-release download/verification also passed. Rehearsals use a matching
release executable rather than raising size limits for oversized debug binaries.

PERFORMANCE.md records release-mode CPU timings and a limited desktop Alacritty comparison.
Bulk output and high private memory remain open; laptop performance, battery use, physical
input and stable acceptance are not established by these tests.

The 2026-10-12 audit repair design is approved. This is a working Windows testing milestone,
not a claim of complete developer-grade compatibility. The engine remains Juna's own implementation.

## Launch and controls

Build with `cargo build --release --bin juna`, then run `target\release\juna.exe`.
The default shell is Windows PowerShell 5.1. To test cmd, run
`target\release\juna.exe --shell cmd.exe`. Shell arguments go after `--`, for example
`target\release\juna.exe --shell powershell.exe -- -NoProfile`.
`JUNA_SHELL` remains supported; `JUNA_SHELL_ARGS` now respects quoted argument boundaries.

Space, Ctrl-C, Ctrl-L, Enter, Backspace, Delete, Insert, navigation keys, modifiers, and F1–F12
are encoded explicitly. The shell interprets `exit`; the single terminal window closes after
the actual child exits and its transport threads finish. Nested programs own their own input.

Drag left to select, Ctrl+Shift+C to copy, Ctrl+Shift+V or Shift+Insert to paste.
Right-click opens Copy/Paste/Select All/Themes/Settings. Wheel or Shift+PageUp/PageDown navigates
history. Typing returns to live output.
Hold Shift to use local mouse selection/scrolling while a TUI has enabled mouse reporting.
Shift+right-click opens Juna's menu when a TUI owns mouse input.
Multiline unbracketed paste sends line endings to the shell, so use clipboard content you intend
to execute. Bracketed-paste-aware programs receive the matching start/end delimiters.

Diagnostics are written to `%LOCALAPPDATA%\Juna\logs\juna-<pid>-<nonce>.log`; startup/session failure also displays
a Windows error dialog. `RUST_LOG=info` includes adapter, presentation and lifecycle evidence.
The latest launch replaces that log; copy it before starting another diagnostic run.

## Themes and settings

The default is **Catppuccin Mocha**, with **IBM Plex Mono** as the preferred font. The 20 choices
and their source links are listed in THEME_DESIGN.md. Theme selection changes the display live;
the shell is not restarted. Truecolor backgrounds chosen by a prompt are preserved, while the
contrast setting makes unreadable nominal foregrounds darker or lighter as needed.

Open Themes from right-click, type to search, navigate with arrows, and choose with Enter/click.
Hovering previews a theme; Escape restores the saved selection. Open Settings to edit font family,
font size (8–40), or minimum contrast (1–21). Click a field, type or Ctrl+V a replacement, and press
Enter to save. Missing font families fall back to an installed monospace face and log the actual
selection. A contrast value of 1 disables foreground adjustment.

Create a custom theme from the selector or edit the current theme from Settings. Built-ins are
duplicated; existing custom themes can be edited in place. Click a name/color field, enter a name
or `#RRGGBB`, and use Ctrl+S/Save. Invalid values are reported and are not saved. Escape/Cancel
discards the draft and restores the saved palette. Settings are in `%LOCALAPPDATA%\Juna\settings.json`;
custom palettes are in `%LOCALAPPDATA%\Juna\themes`. Files use versioned JSON with portable palette
fields; VS Code export is not implemented yet.

## Automated acceptance commands

```
cargo check
cargo clippy --all-targets
cargo test
cargo test --test render_acceptance -- --ignored
cargo test --test window_acceptance -- --ignored
```

The last two tests require an interactive Windows desktop, a graphics backend, and system fonts,
so ordinary Cargo test lists them as ignored. They must be explicitly executed on the target host.
They do not use shell prompt matching or inject operating-system keystrokes.

The GPU test compares row pixels before and after editing the next row and captures
`target\acceptance\render.ppm`. The window test opens the actual winit application separately
with cmd and PowerShell, verifies presented frames and real exit codes, and writes isolated logs
under `target\acceptance\<shell>\Juna\logs`. These are synthetic test data, not user sessions.

## Verification matrix

| Area | Evidence | Status |
|---|---|---|
| Space/control/modifier/dead-key encoding | Pure exact-byte tests, including AltGr, keypad and mouse modes | Verified in this session |
| Row corruption, CR/wrap, large parameters, ED, insert/delete, resize, wide pairs | Permanent regression fixtures and chunk-boundary equivalence | Verified in this session |
| Real process lifecycle | cmd and PowerShell tail markers, nonzero exit and joined workers | Verified in this session |
| Byte integrity and process-tree teardown | Existing transport/ConPTY acceptance suite | Verified in this session |
| GPU row isolation and text | Pixel comparison and captured render frame | Verified in this session |
| Window presentation and natural exit | Vulkan on RTX 3060; cmd exit 0 and PowerShell exit 7 | Verified in this session |
| Physical typing, Ctrl-C in an interactive program | Human smoke checklist below | Pending human review |
| Clipboard ownership/paste in the real UI | Platform integration plus pure paste encoding; no OS clipboard test performed | Pending human review |
| Mixed DPI, minimize/restore, surface/device disruption | Code paths present; no multi-monitor/failure injection run | Pending human review |
| Real editor and pager | Input/mode/alternate-screen fixtures; no editor/pager session recorded | Pending human review |

The first window test failed because inherited `RUST_LOG=warn` hid its evidence lines. The test
now explicitly sets `RUST_LOG=info`; both shell cases passed on rerun. This was a test-environment
failure, not a presentation failure. Previous audit failures remain recorded in AUDIT.md.

A later repeat caught the reader-first shutdown race, which is corrected and covered by a
deterministic test. Earlier green runs alone did not prove that interleaving. The final standard
suite has 165 passes, and the two desktop/GPU acceptance tests are run separately.

## Human smoke checklist

1. Type a command containing spaces, edit it with arrows/Home/End/Delete/Backspace, and execute it.
   Repeat near the right edge and after enough output to scroll. Watch the row above the input.
2. Run a long-lived process, interrupt with Ctrl-C, and confirm a usable prompt returns.
3. Print accented text, CJK, emoji, colored text, and box drawing; compare the captured sample.
4. Drag/select/copy and paste one line, then multiple lines in a bracketed-paste-aware program.
5. Scroll back while new output arrives. Return to live output by typing. Resize small/large,
   minimize/restore, and move across monitors with different scaling.
6. Run an installed editor and pager; check alternate-screen restoration, cursor hide/show,
   navigation, mouse reporting and exit. Record application/version, observed bytes if available,
   screenshots and the diagnostic log for a failure.
7. Type `exit` in each shell, and close during heavy output. Confirm no orphan process survives.

## Explicit remaining scope

OSC 52 clipboard is denied. Graphics protocols, panes/editor/drawer features, and the Unix PTY
backend are deferred. Full bidi layout and cross-cell contextual shaping are not implemented;
current shaping handles each stored grapheme cluster. Fonts can still lack a requested glyph,
which is recorded as a miss. The atlas has a finite per-frame capacity and reports exhaustion;
it no longer permanently blacklists a glyph because an earlier frame filled it.

This evidence supports further interactive testing. It does not replace the human checklist or
claim all existing Tier 2 requirements have shipped. DX12/GLES presentation is not verified by the
Vulkan result. No benchmark or universal compatibility claim is made.

## Recorded render sample

![GPU acceptance frame](docs/acceptance/render.png)

## Theme follow-up evidence — 2026-10-12

The GPU tests now verify palette bytes on both sRGB and non-sRGB surfaces; the previous test
covered only the latter and missed double encoding. Before the correction, Dracula's `[40,42,54]`
background rendered as `[110,113,127]` on the sRGB path. Both target formats now preserve it.
Twenty theme backgrounds and actual menu-overlay drawing were captured and checked. The real
window received an end marker after 300 output rows with theme/settings panels open, then closed
when the shell exited. An initially incorrect test command delayed every loop iteration; explicit
grouping fixed that fixture, and both menu cases passed.

Final theme iteration: 176 standard tests pass, plus three explicitly run GPU tests and two
explicitly run window tests. Check/clippy are warning-free, and the release executable was rebuilt.

Custom storage tests cover round trips, replacement saves, corrupt/unsupported/duplicate records,
read-only settings preservation, and cancellation without changing engine cells. These are automated
code/GPU/window checks. Hands-on mouse/clipboard/IME/mixed-DPI review remains the human checklist.

![Catppuccin Mocha and readable prompt box](docs/acceptance/catppuccin-mocha.png)
![Theme selector preview](docs/acceptance/theme-menu.png)
![Right-click options](docs/acceptance/context-menu.png)
![Settings preview](docs/acceptance/settings-menu.png)
![Custom theme editor preview](docs/acceptance/theme-editor.png)

## Application title and Fastfetch — 2026-10-12

Verified: installed Fastfetch 2.69.0 ran in PowerShell through the production ConPTY/engine
path with OS/CPU/GPU/Memory output, a success marker, exit 0 and zero dropped bytes.
The test refreshed its process PATH from registered Windows machine/user values because
the parent session had an older PATH. Launch Juna from a fresh Windows session if
fastfetch is not found; Fastfetch is installed on this host, not bundled with Juna.
Verified title regression: both real shell windows presented frames, retained Juna Terminal after explicit shell title changes, and closed on shell exit. Check/clippy, all 176 standard tests and both explicit real-window tests passed in this iteration.

## Bundled fonts and sizing — 2026-10-13

Verified: 180 standard tests, check/clippy, four explicit GPU tests and two explicit real-window
tests pass. All fifteen faces resolve in an empty font database; the GPU samples exercise regular
weight, ASCII disambiguation, box drawing/CJK fallback and row clipping. Sizes 8/16/40 render.
Font/setting cancellation, saved selection, zoom bounds, AltGr exclusion, pixel-wheel accumulation
and stored sizing are covered. The real window keeps output flowing with a font picker open.

![Fifteen bundled font samples](docs/acceptance/font-samples.png)
![Font picker](docs/acceptance/font-menu.png)

Verified taskbar evidence: embedded ICO images match the approved source; native small/big icon
handles exist; Windows accepts the dedicated application ID. The observed user window started
before that identity build. Explorer's displayed icon after relaunch still needs human confirmation.

## Right-side preview and taskbar — 2026-10-13

Verified: right-edge geometry follows viewport resize; GPU comparison leaves the left 480 pixels
unchanged in a 1024-pixel frame. Required gates pass (180 tests) and both real-window cases pass.
A direct release EXE starts, presents, configures relaunch icon properties and exits 0. Native
small/large icons and all seven embedded resource images are verified. Explorer initially retained
a generic group icon despite valid resources; a fresh explicit group with relaunch properties
shows the Juna dog, confirmed by the user. The permanent release setup was visually inspected.
The unsuccessful Rust borrowed-variant attempt is recorded in DECISIONS.md, including its repair.

![Right-side settings leave terminal text visible](docs/acceptance/right-side-settings.png)

## Tabs, numeric prompts and theme families — 2026-10-13

Verified: check/clippy, 185 standard tests, four GPU cases, three real-window cases and release
build pass. The byte-exact ConPTY coverage remains passing. Concurrent startup tabs consume their
independent output markers and reap without closing surviving sessions. Overlays leave shell
output flowing, and the final session closes the window. The actual release presents and exits 0;
all seven embedded icon images match the approved source. Reviewed captures cover grouped families,
Custom management, numeric entry and tab presentation. UI tests cover Space, Escape, caret editing,
point conversion and saved/cancelled previews. Theme worker tests cover discovery, mapping,
bounded input, generation results and byte-preserving trash/restore.

Retraction/limits: the first restricted filesystem run failed Clippy configuration access and seven
filesystem-dependent tests. Normal-access rerun passed. Incremental hard-link fallback warnings
were emitted; they are build-cache warnings, not source lints. These automated UI/window checks
do not replace hands-on keyboard/mouse/IME and mixed-DPI review. No Junafetch privacy or updater
acceptance is claimed at this milestone.

![Custom family menu](docs/acceptance/custom-themes.png)
![Exact font-size prompt](docs/acceptance/numeric-prompt.png)
![Terminal tabs](docs/acceptance/tabs.png)
## 0.1.1-dev1 interface/preferences acceptance — 2026-10-13

Verified: standard suite passed with 199 distinct cases (184 library, one release-verifier and
14 integration cases); the added import-failure/private-source regression passed separately with
the transfer round-trip test. The final transfer rerun passes both cases, check and strict Clippy
with captured original permissions. Four explicit GPU cases and three real-window cases passed,
including concurrent sessions with line numbers enabled.
Reviewed captures show the saved-settings controls, favourite stars, themed close confirmation,
tab-only context and separated gutter. The release PE is x64, carries native version/source
metadata and preserves all seven approved icon images byte for byte.

The first favourites fixture loaded the user's saved 15px size while assuming 16px; the fixture
now explicitly uses defaults. This was a test assumption failure, not evidence of a font reset.
The import rollback test initially triggered a portability lint while restoring Windows permissions;
it now restores the captured original permissions. Invalid/oversized JSON and conflicts preserve
existing data. Native file-dialog interaction and all physical input/mixed-DPI/other-adapter checks
remain human acceptance; no full desktop/TUI maturity claim is made.

Git log diagnosis: the referenced eternal repository has exactly five commits, so the supplied
five-entry output was complete. The tool environment had GIT_PAGER=cat and TERM=dumb; an initial
pager smoke emitted a terminal warning. A repeated child-only less/xterm smoke completed after q
was sent and returned a controlled marker with exit 0. This is a smoke test, not complete pager
navigation evidence. No user Git configuration was changed.

![Persistent Settings](docs/acceptance/persistent-settings.png)
![Theme favourites](docs/acceptance/theme-favourites.png)
![Close tab confirmation](docs/acceptance/close-tab-confirmation.png)
![Terminal row numbers](docs/acceptance/line-numbers.png)

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
Verified locally for dev7: required check, strict Clippy and all 210 standard cases passed; no runtime changes or new graphics claims.
