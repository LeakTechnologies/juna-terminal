# Changelog

## 0.1.1-dev19 — reduce ordinary output allocation overhead

- Avoid grapheme-text allocation for adjacent printable ASCII scalars, preserving the existing Unicode path.
- Add exhaustive printable-ASCII pairs and Unicode prefix/combining/emoji/chunk-boundary regression coverage.
- Retain measured desktop evidence: about 29% lower parent CPU in the fixed 4 MiB output probe.
- High idle memory, laptop/battery acceptance and the exact OpenCode interaction remain open.

## 0.1.1-dev18 — correct Windows control-input acceptance

- Use ReadConsoleW in the controlled raw-input child instead of ReadFile, which can treat
  leading Ctrl+Z as EOF on Windows even when processed input is disabled.
- Read one control character per call and retain exact Ctrl+C/Ctrl+Z, exit and lifecycle assertions.
- Include all dev16/dev17 repairs and diagnostics; neither previous tag produced a release.
- Keep the application's actual input/transport implementation unchanged.

## 0.1.1-dev17 — interrupt diagnostics

- Carry forward dev16 file drops, versioned titles and launcher-console isolation.
- Record queued Ctrl+C/Ctrl+Z events, session exit and final-session window closure without command text.
- Record build identity and panic source location without logging panic payloads.
- Expand the controlled CI fixture failure report with child exit, input mode and bounded fixture output.
- Dev16 Windows CI failed in the new raw-control-byte probe despite local acceptance passing;
  its cause remains unverified. The superseded retry was cancelled; no dev16 release was published.

## 0.1.1-dev16 — file drops, visible version and console isolation

- Insert dropped file/folder paths as text into the active terminal, without Enter or execution.
- Show `Juna - v<actual build version>` and preserve updater recognition of legacy windows.
- Keep release GUI sessions independent of launcher-console interrupts; retain CLI output.
- Add controlled Ctrl+C/Ctrl+Z byte, drop-routing and isolated console-event acceptance.
- Record drag-out tab detachment and Move to new window as future roadmap work.


## 0.1.1-dev15 — repair protected-field reveal

- Replace fragile OSC spans with per-cell markers preserved through ConPTY.
- Restore Shift+hover resolution while keeping output and copies censored.
- Match ordinary mask width to the value's display columns; typed passwords use eight asterisks.
- Prevent marker reuse and refresh mappings when protected reports arrive.


## 0.1.1-dev14 — command-line updates

- Add --check-update, --update, temporary --channel stable/preview, and --help.
- Reuse verified GitHub downloads and the existing installer, rollback and restart.
- Hand in-terminal requests to the authenticated hosting window for Cancel-first confirmation.
- Require interactive confirmation externally and check for other windows using the executable.
- Add Juna to its own child-session PATH and preserve redirected command output.

## 0.1.1-dev13 — protected Junafetch fields

- Replace sensitive-field placeholders with five shaded cells and deliberate Shift+hover reveal.
- Keep protected values outside terminal output, scrollback and copied text using authenticated local IPC.
- Add saved Privacy settings for deliberate reveal, ordinary hover and explicit censorship opt-out.
- Preserve committed views during synchronized output and hide reveal on focus loss or tab changes.
- Bound field storage and protocol messages; retain the existing terminal-cell storage size.

## 0.1.1-dev12 — verified updates and faster redraw preparation

- Add Settings → Updates with saved Stable/Preview selection and manual checks/downloads.
- Verify GitHub asset digests, SHA-256 manifests, restricted ZIP members and native identity.
- Install through a hidden Rust helper with locked-file retries, rollback and startup acknowledgement.
- Preserve settings/themes and report failure/recovery status without exposing private paths.
- Reuse contrast results and render storage; avoid per-cell text/glyph allocation on redraw.
- Add reproducible release-mode timing probes and document the Alacritty comparison limits.
- Show an attention indication when an inactive tab receives a terminal bell.

## 0.1.1-dev11 — start interactive sessions at home

- Normal interactive launches start in the user home directory instead of the portable folder.
- Add --cwd DIRECTORY for deliberate folder launches and preserve it in new windows.
- Keep custom command launches in their inherited folder unless --cwd is supplied.

## 0.1.1-dev10 — independent terminal windows

- Add Ctrl+Shift+N for a new independent window in the reported folder or launch folder.
- Start the configured shell interactively without repeating one-off commands.
- Keep logs separate per window under the Juna logs directory.

## 0.1.1-dev9 — grouped Junafetch and settings

- Expand Junafetch into named sections covering the demonstrated Fastfetch categories.
- Save Brief/Standard/Detailed and compact/full/no-art choices; CLI flags override them.
- Group Settings into Appearance, Terminal, Tools and Import/Export with keyboard/mouse Back.
- Keep identifiers/network values Hidden pending protected-value transport and reveal.

## 0.1.1-dev8 — Windows test isolation

- Run Windows release tests sequentially while retaining lifecycle/output deadlines.
- Record dev7's two hosted ConPTY timeouts without claiming a confirmed runtime cause.
- Keep expanded Junafetch and grouped settings in the following development cycle.

## 0.1.1-dev7 — CI fixture portability

- Give Git merge fixtures their own identity instead of relying on machine-wide settings.
- Compare Windows directory identities after resolving short/long names.
- Synchronise the fake-session fixture so reads begin after its output is queued.
- Preserve dev6's local Git feature; expanded fetch/settings remain the next cycle.

## 0.1.1-dev6 — local Git status

- Show branch, changed files, conflicts and local ahead/behind counts in a saved, toggleable footer.
- Follow validated OSC 7 folders; offer opt-in PowerShell reporting for new default sessions.
- Reject stale tab/folder results and hide/refit the footer in alternate-screen applications.
- Keep queries local/read-only; report active-filter repositories and failed checks unavailable.
- Queue approved grouped Junafetch profiles/settings and near-term horizontal/vertical panes.

## 0.1.1-dev5 — legacy Windows roadmap

- Record low-priority investigation of a legacy console backend for pre-1809 Windows.
- No Windows compatibility or terminal behaviour changes are included.

## 0.1.1-dev4 — graphics roadmap

- Record future Kitty graphics protocol support, with original implementation, bounded
  resource handling and compatibility acceptance to be designed before implementation.
- No graphics protocol support is added in this development build.

## 0.1.1-dev3 — bundled Junafetch

- Embed the pinned console utility and provide `junafetch` and `fetch` in Juna sessions.
- Present eight local statistics with compact, width-aware artwork and plain/JSON options.
- Verify cached payloads, repair corruption and retain native-library notices/source pins.
- Omit personal/network identifiers; terminal-wide masking and hover reveal remain pending.

## 0.1.1-dev2 — read-only JSON preservation

- Refuse replacing read-only preference/export files consistently across platforms.
- Enable the preservation regression on Linux and retain import staging rollback coverage.
- Record the verified dev1 Windows release and publish this CI correction under a fresh tag.

## 2026-10-13 — context menus, row numbers and theme favourites

- Separate tab actions from terminal options; rename tabs and reanchor repeated right-clicks.
- Confirm middle-click closes using theme-coloured check/cross labels with Cancel selected.
- Add optional left-side terminal row numbers without changing output or clipboard text.
- Add saved yellow favourite stars, a Favourites section and Expand/Collapse all family controls.
- Fix the Linux unsupported-backend test compilation failure reported by CI.
- Record Canadian-English/Indigenous-language priorities and local-first Android exploration.

## 2026-10-13 — bundled-tool preparation

- Prepare the pinned lean Junafetch detector fork and verify its 18 upstream tests.
- Add child-only session environment overrides with a Unicode/space regression probe.
- Record approval of terminal-wide privacy. Bundled execution and privacy UI remain in progress.

## 2026-10-13 — tabs and clearer theme/settings controls

- Add independent tabs, right-click New/Close Tab, tab-strip mouse controls and keyboard switching.
- Keep inactive sessions consuming output and reap each session before closing the last window.
- Accept exact font sizes in px or pt through mouse/Space/Enter prompts with live preview.
- Group 60 starter themes into families and add a dedicated Custom create/manage submenu.
- Discover downloaded JSON in custom family folders; preserve invalid/imported files and recover removals.

## 2026-10-12 — application title

- Keep the window named Juna Terminal when shells send title changes.
- Add real-window title regression coverage and record installed Fastfetch smoke evidence.


## 2026-10-12 — themes and options

- Default to Catppuccin Mocha and supply 20 sourced starter palettes.
- Add a live right-click Copy/Paste/Select All/Themes/Settings menu with keyboard navigation.
- Add search/live theme previews, font size/family and contrast settings, and custom theme editing.
- Save validated, versioned theme/settings JSON atomically; retain corrupted files and report errors.
- Preserve TUI mouse reporting with Shift as the local-menu override; consume overlay input separately.
- Record GPU palette/menu captures and real shell output continuing with options overlays open.

## 2026-10-12 — color readability

- Correct double sRGB encoding that brightened the displayed theme, including surface clear colors.
- Adjust unreadable foregrounds to a minimum nominal 4.5:1 contrast while preserving backgrounds.
- Log the selected font and cover both sRGB and non-sRGB render surfaces with exact pixel tests.

## 2026-10-12

- Repair Space, control/modifier keys, composed input, keypad and terminal mouse encoding.
- Detect shell exit and join byte-transport workers before closing the terminal window.
- Correct row scrolling/insertion, deferred wrap, large parameters, erase semantics, saved cursor,
  Unicode clusters, wide-cell rendition and resize margins; add permanent regression coverage.
- Render owned snapshots with layered backgrounds, shaped glyphs, visible text attributes, cursor
  modes, fallback fonts, frame-boundary atlas recycling and recoverable surface presentation.
- Add local history navigation, selection/copy/paste, quoted shell arguments and persistent diagnostics.
- Pin Unicode 17 standards-data crates; remove inactive winres/build inputs; keep ConPTY pinned.
- Add GPU pixel evidence, actual-window lifecycle acceptance, and an explicit human testing matrix.

## 2026-10-13 — application icon

- Embed the approved seven-size icon into the executable and set window/taskbar icons.
- Verify native icon handles in real-window acceptance and source-scan the resource input.


## 2026-10-13 — fonts, sizing and README

- Bundle 15 licensed monospace font choices, original notices, pinned sources and hashes.
- Add searchable live font previews, installed monospace choices and a license viewer.
- Add Left/Right size adjustment, Ctrl+wheel, Ctrl++/Ctrl+- and Ctrl+0 with persisted sizing.
- Give settings previews Save/Cancel behavior and render variable fonts at regular weight.
- Set a dedicated Windows taskbar application identity before window creation.
- Add a GitHub README with the approved Juna logo, features, build instructions and controls.

## 2026-10-13 — preview placement and taskbar

- Place live-preview panels on the right and keep terminal text visible on the left.
- Remove the license selector; retain embedded notices in the app-owned notices file.
- Configure explicit taskbar relaunch/icon properties before showing the window.
- Add the approved tabs/themes, compact/private Junafetch and update roadmap.
