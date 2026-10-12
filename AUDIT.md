# Terminal readiness audit — 2026-10-12

Audited revision: `73e63c0`. Scope: application input and lifecycle, VT parser/grid/history,
session and transport, GPU and renderer, layout boundaries, compatibility targets, and tests.
This report changes no application behavior. Implementation requires design approval under
`AGENTS.md` §3 and the global three-bullet design rule.

**Conclusion:** Juna has a working shell-to-engine path, but the windowed application is not ready
for dependable interactive testing. Input mapping, lifecycle, grid transitions, resize, and
rendering have concrete defects. More shell throughput tests alone will not close these gaps.

Evidence labels: **reproduced** means executed during this audit; **verified in source** means
the implementation directly establishes the finding; **hypothesis** means a plausible connection
to the reported symptom that still needs a recording. No visual approval is claimed.

## Verification performed

| Check | Observed result |
|---|---|
| `cargo check` | Pass, with three dead-code warnings |
| `cargo clippy --all-targets` | Fail: four `clippy::eq_op` errors in the renderer palette |
| `cargo test` | Fail: 141 library tests and four ConPTY tests pass; elevation suite has three passes and one failure; Cargo stops before shell integration |
| `cargo test --test shell_roundtrip` | Five passes, run separately after the full-suite failure |
| PowerShell 5.1 headless acceptance | Chosen output marker reaches the screen; `exit` produces `Some(0)` |
| Direct engine probes | Thirteen observed failures across targeted cases, listed below |
| Window/input/GPU recording | Not performed; the user's visual report remains an observation requiring reproduction |

Build-cache writes were denied inside the sandbox; check, clippy, and tests were run with approved
normal access. The first PowerShell harness run encountered a sandbox denial of PSReadLine's
history file; a normal-access rerun succeeded without that denial. These are environment effects,
not attributed to Juna. No dependency was changed, and no release binary was rebuilt.

The probes were compiled against the actual source modules using
`rustc --edition=2021 -Awarnings target/audit/probe.rs -o target/audit/probe.exe` and executed.
They are temporary, ignored audit instrumentation, not regression coverage in the project suite.
The important byte sequences and outcomes are recorded here so they can become permanent tests.

## Immediate blockers

### 1. Space and control-key input — P1, verified in source

`src/app/mod.rs:234–263` maps characters and a small list of named keys. Space is missing.
The installed winit Windows mapping explicitly produces `Key::Named(NamedKey::Space)` for
`VK_SPACE`, so that key reaches the catch-all `None` and sends no byte.

The same handler forwards `event.logical_key` and tracks no modifiers. Winit documents that
`logical_key` ignores Ctrl, so the comment claiming Ctrl-C/Ctrl-L arrive as control bytes is
incorrect. Ctrl-C can send an ordinary `c`; interruption, Ctrl-L, Ctrl-D, and modified navigation
need explicit handling. Delete, Insert, function keys, Shift-Tab, application cursor mode,
IME commits, and dead-key text are also missing. `event.text` is unused.

Acceptance: test exact bytes for Space, Enter, Backspace, Delete, arrows, Ctrl-C/Ctrl-L,
modifier combinations, repeats, composed text, and AltGr; then record typing and editing a command
containing spaces in the real window. Terminal-mode-dependent keys must use engine mode values.

Reference: [winit KeyEvent contract](https://docs.rs/winit/0.30.13/winit/event/struct.KeyEvent.html).

### 2. Shell exit never closes the window — P1, verified in source

The app retains `SessionControl` but never calls `try_wait`. The reader's completion flag is also
unused. The session contract (`src/session/mod.rs:126–136`) explicitly warns that ConPTY can keep
the output pipe open after child exit. Waiting for EOF would therefore be insufficient too.

For the current single-shell window, the proposed default is: let the shell interpret `exit`,
observe its actual process exit, finish session teardown, and close the window. Do not recognize
the literal word inside the input handler: nested shells and programs must own their own input.
Closing on successful shell exit is an assumption proposed for approval, not current behavior.

CloseRequested currently cancels the stream, invokes asynchronous kill, and exits without
confirming process death, draining accepted tail output, or joining the detached reader thread
(`src/app/mod.rs:312–319`). Read errors disappear into `Err(_) => break`; startup thread failure
also needs a cleanup path. Preserve the exit code and distinguish read failure from normal exit.

Acceptance: real PowerShell and cmd `exit`, nonzero child exit, close during heavy output,
blocked reader, immediate-close startup, and process-tree teardown; no surviving child or reader.

### 3. Grid operations corrupt rows and can panic — P1, reproduced

| Input/setup | Actual result | Required repair |
|---|---|---|
| 8×3 screen containing A/B/C; `CSI 1 T` | blank/C/C | Scroll down must yield blank/A/B |
| Same screen; home; `CSI L` | Debug panic; partial row mutation | Fix insertion range and source-row arithmetic |
| Cursor (1,1), zero-based; `CSI 65535 B` | Debug overflow panic | Use bounded/widened arithmetic before clamping |
| `12345678 CR X` on an eight-column screen | Original row remains; X appears on next row | CR must cancel deferred wrap before overwriting column zero |
| A/B/C; `CSI 2 S` | History begins A/A | Capture each removed row, not the same top row twice |
| A; CR/LF; B in a one-row grid | No history recorded | Region height is inclusive; one-row scrolling must work |
| `CSI ;3 H X` | X at row 2, column 0 | Preserve leading omitted parameters; expected row 0, column 2 |
| Save at (1,1); restore; move; restore again | Second restore leaves cursor at (0,0) | Restore must not consume saved state |

Locations: `src/vt/grid.rs:294–332`, `src/vt/grid.rs:345`,
`src/vt/mod.rs:313–320`, `src/vt/mod.rs:404`, `src/vt/mod.rs:736–744`,
`src/vt/parser.rs:482–494`. Also audit analogous addition in CUF/HPR, origin offsets, and ECH.
Release builds can wrap arithmetic instead of panicking, yielding incorrect coordinates.

CNL/CPL currently call line-feed/reverse-index in loops, so they can scroll instead of only moving
the cursor. CSI dispatch mostly ignores private prefixes and intermediates; unsupported sequences
can accidentally invoke an implemented operation with the same final byte. C1 execution is only
partially implemented, and intermediate-byte vectors are unbounded. These are verified source
gaps requiring targeted tests beyond random bytes.

The duplicate-row and pending-wrap failures are **hypotheses for the user's typing symptom**,
not proof of which sequence the user's shell emitted. Record raw shell output and compare the
engine grid with the rendered frame before assigning a root cause.

### 4. Erase semantics are wrong, and an existing test enshrines them — P1, reproduced

`CSI 2;4 H CSI 2 J` moves the cursor to (0,0). ED 2 erases the display; it must preserve the cursor.
ED 3 currently also clears the visible screen and homes, conflating saved-line erasure with ED 2.
`src/vt/tests.rs:121` explicitly expects ED 2 to home: a passing test is asserting the defect.
Repair implementation and expectations against the published control-sequence specification.

Reference for ED, IL, SD, and reports:
[xterm control sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html).

### 5. Resize leaves the shell and grid with inconsistent geometry — P1, reproduced/source

Growing 8×3 to 12×5 leaves the scroll region `(0,2,0,7)` despite a 12×5 grid. Text continues to
wrap and scroll within the old bounds. Shrinking A/B/C to two rows drops C without recording it.
Narrowing can split wide cells; current tests expressly accept discarding columns.

`App::fit` resizes the engine before the PTY, discards PTY resize errors, and stores dimensions
without the engine's 512×256 maximum clamp. Engine and child can disagree on large windows.
It enforces 20×5 even when the physical viewport cannot show those cells. Fixed physical cell sizes
are mixed with logical startup sizing; scale-factor changes are ignored.

Acceptance: grow/shrink, restore/maximize, monitor DPI changes, tiny/minimized windows, constrained
scroll regions, cursor preservation, wide-cell integrity, and PTY failure. Explicitly distinguish
full-screen margins from application-defined margins and preserve the approved no-active-screen-
reflow rule. Scrollback reflow remains a separate required feature.

## Rendering and ownership

### 6. Renderer bypasses the required owned snapshot — P1, verified in source

`Renderer::build(&TerminalState, ...)` at `src/render/mod.rs:284` receives engine state and reads
the grid directly. It does not retain the borrow or mutate the engine, and the engine still has
one mutable owner, but this violates AGENTS §5's **owned, read-only snapshot** boundary. Copying
GPU instances later does not satisfy that input contract. Introduce a Juna-owned render snapshot
containing cells, dimensions, cursor, and the relevant render modes, without parser/session types.

### 7. Unicode and text attributes are incomplete — P1/P2, verified/reproduced

- Wide writes discard the supplied colors and attributes (`src/vt/grid.rs:353–364`). A red-on-blue
  漢 becomes default/default. Overwriting its leading cell with X leaves an orphan spacer.
- The renderer draws only the first scalar in `CellContent::Cluster`, losing accents and joined
  sequences. Width tables are hand-maintained and explicitly incomplete; grapheme segmentation
  and shaping are not implemented. Combining at a wrap boundary or after a wide character also
  needs engine tests. This cannot meet the declared Unicode Tier 1 target yet.
- Font fallback deduplicates by collection index alone (`src/render/atlas.rs:151`). Different font
  files commonly have index 0, so valid fallbacks are rejected as duplicates. The three-face limit
  also counts preferred primary candidates rather than guaranteeing distinct coverage fallbacks.
- Inverse, bold, dim, italic, underline, strike, and overline are stored but not rendered.
  Invisible is handled. Cursor visibility (`?25`) is not tracked, and the renderer always draws
  an underline cursor; focus and requested cursor style are ignored.
- Background and glyph instances are interleaved per cell. A later cell's background can paint
  over an earlier wide/overhanging glyph. Backgrounds need a separate first pass and glyph bounds
  need intentional clipping. Font ascent/descent and fallback placement need measured validation.
- The 1024² atlas has no growth/eviction strategy; exhaustion skips new glyphs. Blank cache entries
  are later treated as absent, and the app's periodic report does not include `atlas.full`.

Acceptance: recorded accented text, CJK, emoji/joined clusters, box drawing, all supported SGR,
cursor hide/show, colored wide glyphs, and fallback/atlas pressure; compare offscreen render output
with expected cell boundaries. Resolve D3/D6 choices with written dependency justification before
introducing any standards-data or shaping crate. No borrowed terminal engine.

### 8. GPU recovery and scheduling are unfinished — P2, verified in source

Adapter selection ranks hardware without checking surface compatibility. An unsupported adapter
can reach empty capabilities and `caps.alpha_modes[0]`. Device request failure has no next-adapter
fallback. Lost/outdated surfaces are merely skipped; no recovery is attempted. Minimized surfaces
are configured as 1×1 instead of suspending presentation.

Every RedrawRequested requests another redraw. Full-screen instance generation continues while
idle, without damage scheduling. `drain` loops until the producer is quiet, allowing sustained
output to monopolize the UI thread. Input writes are synchronous on that thread, so a blocked pipe
can also stall UI responsiveness. These are risks established by control flow; CPU usage and stall
duration were not measured. Bound processing per tick while preserving every byte; measure before
changing thread ownership or introducing a writer queue.

## Remaining usability and conformance work

| Area | Current gap | Testing milestone |
|---|---|---|
| History access | Scrollback is retained but no mouse wheel/history viewport exists | Scroll, return to live output, preserve position while output arrives |
| Clipboard/selection | No selection, copy, or paste integration | Copy chosen text; paste with safe newline and mode handling |
| Tier 1 history | Wrapped history is always recorded as `false`; no reflow implementation | Preserve logical lines across width changes |
| Tier 1 modes | OSC 1 absent; 1049 does not implement a full cursor/rendition save/restore contract; RIS resets only active grid | Exact fixtures for supported reset/title/alternate-screen semantics |
| Query replies | No engine response channel for DSR/CPR/DA | Declare the supported query set; pure engine emits response bytes for app transport |
| Tier 2 | Bracketed paste, focus/mouse reporting, DECRQM, synchronized output, REP, OSC 52 absent | Explicit next milestone, not a developer-grade claim now |
| Session settings | `JUNA_SHELL_ARGS.split_whitespace()` destroys quoted argument boundaries | Structured argument vector with spaces/quotes preserved |
| Diagnostics | OSC title not applied to window; Explorer launch has no parent console to display errors | Visible startup/runtime failure and a persistent useful log |
| Workspace features | Layout module exists, but the window hosts one terminal and does not use it | Defer panes/editor/agent drawer until the terminal passes acceptance |

Query replies and application cursor/keypad handling need an explicit scope decision because
they are practically relevant to TUIs but are not named in the current Tier 1 list. Validate real
editors, pagers, and progress displays as ARCHITECTURE §5.1 requires. Linux implementation remains
deferred; it is not a Windows terminal-readiness blocker.

## Test and documentation defects

The failing elevation test is precisely identified: `src/build.rs` is UTF-16, and
`tests/elevation_boundary.rs:74` unwraps `read_to_string`. That file is also misplaced for Cargo's
automatic build-script discovery: only root `build.rs` is automatic unless the manifest specifies
another path. Its `winres::Windows` call needs review against the actual build-resource API and
build-dependency placement before activation. Do not enable an unreviewed script as an audit fix.

The four lint failures are `255.0/255.0` at renderer lines 70, 74, 78, and 82. The warnings include
unused `luminance` and test helpers compiled outside tests (`mod tests` lacks a cfg gate).

Existing tests miss row identity in scroll-down, the last shifted row in IL, leading omitted CSI
parameters, large structured parameters, repeat restore, renderer output, and real keyboard
events. Random-byte tests rarely generate these structured sequences. Add deterministic fixtures,
chunk-boundary equivalence, and meaningful grid invariants. A broad fuzz target and golden-frame
tests are described in architecture but no corresponding files were found.

DECISIONS still says there is no window/renderer; current source plainly has both. Its historical
green gate must not be treated as today's result. AGENTS §13 and several architecture status rows
also lag the implemented state. ARCHITECTURE §3.3 says to discard reader bytes, conflicting with
§3.0.2's requirement to parse accepted tail bytes. Synchronize these documents explicitly and
record retractions rather than silently presenting past measurements as current.

Transport differs from its normative contract: the app checks capacity after reading, `resume`
is unused, errors are suppressed, and cancellation can abandon an already-read chunk. Stream
statistics exist but the app never reports them. Both Windows writer loops also need a WriteZero
guard to avoid looping forever on a zero-length write. The byte-exact acceptance tests passed;
these lifecycle/overload edge cases are not thereby proven safe.

## Proposed design for approval

1. **Restore basic input and lifecycle.** Add a pure, tested input encoder using produced text,
   modifiers, and explicit terminal modes. Track real process exit, reader status, and ordered,
   bounded cleanup; close the single window after the shell exits. Correct the source-encoding
   and lint failures. Acceptance: real-window typing/editing/interruption and PowerShell/cmd exit,
   plus all required Cargo checks and lifecycle tests.
2. **Make state and presentation reliable.** Repair the reproduced grid/parser/resize defects,
   replace incorrect test expectations, introduce owned render snapshots, and correct layered
   rendering, cursor state, glyph fallback, and GPU recovery. Keep engine I/O-free and one-owner;
   preserve ordered bytes and document transport/shutdown contracts. Acceptance: exact regression
   fixtures, byte-exact pressure tests, recorded typing/wrapping/resize, and GPU frame comparisons.
3. **Complete the declared testing environment.** Add history navigation, selection/copy/paste,
   approved Unicode/shaping and history reflow, and the explicitly approved query/mode subset.
   Refresh status/decision documentation. Acceptance: a written PowerShell/cmd/editor/pager matrix,
   visual evidence, no orphaned processes, and green check/clippy/test before each repair lands.

Assumption: close the current single-shell window on child exit; future multi-pane behavior closes
only the exited pane. No dependency additions, terminal-engine borrowing, platform port, or new
workspace feature is authorized by this proposal alone.
