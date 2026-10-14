# Juna Terminal Engine — Architecture & Interface Specification

Session startup selects an explicit --cwd directory or, for ordinary interactive launches,
the validated user home folder. Custom shell-argument launches retain inherited cwd when
no explicit directory is supplied. Selection performs filesystem checks at the session/app
boundary without changing process-wide cwd or performing I/O in the VT engine. New windows
pass their intended directory explicitly and preserve the shell launch-folder fallback.

Ctrl+Shift+N launches the current absolute application executable as an independent process,
with the configured shell and an empty interactive argument vector. Validated reported cwd
is preferred over the launch folder; neither engine nor runtime objects cross this boundary.
Each process reads saved settings and owns its sessions and an exclusive diagnostic log.
Closing one window does not request cleanup of another window's sessions.

Local Git metadata is an app-owned, bounded worker, separate from process transport and engine
state. The original VT engine validates OSC 7 local-file URI metadata without I/O. Active-tab
folder/generation values cross the worker boundary; owned reports return to the UI thread and
stale results are rejected. A read-only footer scene reserves one row outside the terminal grid
and disappears in alternate screens. Opt-in default PowerShell prompt integration is per-session
and never writes profiles. The worker never fetches, mutates the index or invokes active filters;
filtered repositories are reported unavailable. See GIT_STATUS_DESIGN.md for scope and evidence.

Bundled utilities are app-owned console executables, separate from the VT engine. Juna
verifies/extracts the embedded Junafetch frontend into a per-user version directory and
prepends it only to child-session PATH. The frontend verifies its pinned native detector,
uses bounded JSON output and a finite process deadline, and constructs a safe local report.
Neither utility receives engine state. Cache failure reports a warning without preventing
terminal startup. See tools/junafetch/README.md and assets/tools/manifest.json for provenance.
The frontend reads only bounded validated presentation preferences on each invocation;
confirmed app settings choose detail/artwork for the next command in existing sessions.
Owned named sections are wrapped by Unicode display columns before console output. Sensitive
rows remain placeholders until the separate protected-value transport exists. The app's
settings categories and previews do not change engine ownership or byte transport.

Status: **Implemented Windows terminal, with readiness repairs approved 2026-10-12.
The acceptance record distinguishes verified behavior from pending human testing.**

Authority: Work Order 001 (revised) — original VT engine, no borrowed terminal engine.
Provenance: All content below is written from public VT/xterm specifications and observable
terminal behavior. No Alacritty / WezTerm / Kitty source has been copied or adapted.

---

## 0. Scope boundary

### Implemented readiness contract — 2026-10-12

Color clarification: palette and explicit RGB values are sRGB-coded bytes. On an sRGB render
surface they are converted to linear shader/clear values exactly once; non-sRGB surfaces retain
byte encoding. Glyph color imagery follows the same path. A presentation-only minimum-contrast
policy adjusts unreadable foregrounds after reverse-video; it never mutates engine colors or
background values. GPU acceptance covers both surface formats, rather than inferring display
fidelity from a non-sRGB capture alone.

Approved theme/options implementation: `theme` is a pure versioned palette/validation API;
`settings` owns file I/O outside `vt`. Twenty immutable starter palettes use Catppuccin Mocha
as the default. The renderer holds validated presentation settings, consumes the owned terminal
snapshot, and separately consumes owned overlay drawing data; menu/editor state never occupies
terminal cells. Palette changes recolor indexed/default cells; explicit RGB backgrounds remain
application-controlled. Minimum contrast affects presentation foregrounds only.

The application owns a non-modal right-click menu, searchable theme selector, font/contrast
settings, and custom-theme editor. All overlay keyboard/pointer events are consumed before terminal
input routing; mouse releases from a menu click are latched so they cannot leak into a child TUI.
TUI-requested mouse reporting retains right-click ownership unless Shift overrides it. Shell output
continues through the usual bounded drain while overlays are open. Escape/cancel restores the saved
active theme; selection preview does not persist until chosen. Custom edits use a draft until save.

Custom palettes and settings use versioned JSON, bounded reads, validated hex colors/IDs, generated
file names independent of human names, and synchronized temporary files renamed into place. Built-in
IDs cannot be overwritten through custom saves; malformed/unsupported records remain untouched and
produce diagnostics. Theme JSON keeps colors separate from font/session settings so future exporters
can map to other terminals. Export adapters are not implemented in this increment.

This section supersedes conflicting older implementation descriptions below. The design lead
approved AUDIT.md's three-stage repair plan and the practical query/key-mode subset.

- The UI thread is the sole mutable `TerminalState` owner. `RenderSnapshot` owns copied cells,
  dimensions, wrap metadata, cursor visibility, and cursor style. Renderer methods accept snapshots,
  not engine references. `Arc<Window>` safely owns the surface's window lifetime without leaking.
- Reader and writer workers own separate process handles. The reader reserves capacity **before**
  each read, preserves read order, and reports faults. `StreamBuffer` is now a mutex-protected byte
  deque: this permits a bounded 64 KiB engine drain per event-loop tick without repeatedly copying
  an entire backlog. A 64 MiB hard output limit and resume hysteresis propagate backpressure.
- UI input is queued in order to the writer worker. The pending-input budget is 8 MiB. Requests
  exceeding it are explicitly rejected **before acceptance**, reported, and close the failed
  session; accepted bytes are never silently dropped. A 1 MiB clipboard text limit bounds paste.
- `Runtime` polls the child handle independently of pipe EOF. Closing drops the input sender,
  lets queued input finish, allows two seconds for exit, then terminates the session job if needed.
  The UI continues parsing output throughout teardown. Reader and writer threads are joined before
  successful window closure, after the output queue empties. Ten-second failure is reported rather
  than an indefinite silent hang. Exceptional process/OS failure is not described as verified reaping.
- PTY resize must succeed before the engine adopts the same clamped 1–512 by 1–256 dimensions.
  Full-screen margins expand on growth; restricted margins remain bounded. Shrink preserves the
  live cursor by filing displaced top rows into history; the active and alternate screens do not
  reflow. Recorded wrapped history reflows, preserves wide pairs, and remains byte-budgeted.
- Unicode width and extended grapheme boundaries use pinned Unicode 17 data behind `vt::width`.
  Clusters are shaped through swash, inside fixed cell allocations. Glyph backgrounds are emitted
  before foregrounds; glyph geometry is clipped to the owning row/cell allocation. CJK/font fallback
  uses actual font identity; cached blank glyphs and atlas exhaustion have distinct behavior.
  The 2048² RGBA atlas recycles only between frames, rebuilding all visible instances. Per-cluster
  and shape-cache bounds prevent pathological strings from growing indefinitely.
- Surface-compatible hardware adapters are tried in preference order, then software. Lost/outdated
  surfaces reconfigure. Zero-size windows suspend presentation. DPI changes rebuild font resources
  and refit both grid and PTY. Redraw is requested only for changed output/input/view state.
- Supported input/query modes: application cursor/keypad, Ctrl/Alt/Shift keys, IME commits,
  DSR/CPR, primary DA, the implemented DEC mode reports, bracketed paste, focus reporting, mouse
  1000/1002/1003 with SGR 1006, REP, and synchronized output with a one-second watchdog.
  DA identifies the basic VT100 subset; it does not claim graphics extensions.
- Local selection/copy uses Ctrl+Shift+C; paste uses Ctrl+Shift+V or Shift+Insert. Right-click opens
  Copy/Paste/Select All/Themes/Settings; Shift+right-click overrides a TUI's mouse reporting.
  Shift overrides mouse reporting for local selection/history. Wheel and Shift+PageUp/PageDown
  navigate retained history; typing returns to live output. Paste removes embedded control codes,
  normalizes line endings, and obeys bracketed-paste mode. OSC 52 remains denied; no program can
  silently read or write the clipboard through escape output.

`TERMINAL_TESTING.md` records actual acceptance evidence and pending human checks. Passing these
repairs does not imply complete Tier 2 conformance, bidi layout, cross-cell contextual shaping,
graphics protocols, or a Linux backend. Those remain explicitly outside this testing milestone.

This document specifies the terminal engine and its interfaces. It does **not** yet commit to
the renderer, BSP layout, editor, or drawer designs; those depend on the engine contract fixed here.

**Out of scope for the first usable release:** Sixel, Kitty graphics protocol, iTerm2 inline
images, ligatures, sixel/kitty bitmap output, arbitrary DCS passthrough payloads.

---

## 1. Parser

### 1.1 State machine

Juna's parser is a byte-driven finite-state machine over the DEC ANSI parser model. States:

| State | Purpose |
|---|---|
| `Ground` | Idle. Executes C0 controls, prints, handles C1 controls. |
| `Escape` | Seen `ESC`. Next byte is intermediate or final. |
| `EscapeIntermediate` | Collecting `0x20..=0x2F` after `ESC`. |
| `CsiEntry` | Seen `CSI`. |
| `CsiParam` | Collecting parameter bytes `0x30..=0x3F`. |
| `CsiIntermediate` | Collecting intermediate bytes `0x20..=0x2F`. |
| `CsiIgnore` | Malformed CSI; swallow until final byte. |
| `OscString` | Inside `OSC`; terminated by `BEL` or `ST`. |
| `DcsEntry` / `DcsParam` / `DcsIntermediate` | DCS header parsing. |
| `DcsPassthrough` | Delivering DCS payload to a registered hook. |
| `DcsIgnore` | Malformed DCS; swallow until `ST`. |
| `SosPmApcString` | `SOS`/`PM`/`APC`; consume until `ST`. |

Byte classification (`0x00..=0x1F`, `0x20..=0x7F`, `0x80..=0x9F` C1, `>= 0xA0` UTF-8) is
data, not a hardcoded match arm, so the tables are testable in isolation.

### 1.2 Control sequence families Juna targets

- **C0 execution**: `BEL BS HT LF VT FF CR SO SI`
- **C1 (7-bit and 8-bit)**: `IND NEL HTS RI SS2 SS3 DCS SOS PM APC OSC`
- **ESC sequences**: charset designation (`ESC ( B` etc.), `DECSC`/`DECRC` (`ESC 7`/`ESC 8`),
  `RIS` (`ESC c`), `IND`/`NEL`/`RI`, `HTS`
- **CSI cursor**: `CUU CUD CUF CUB CNL CPL CHA CUP HVP VPA VPR CBT CHT CNT`
- **CSI erase/edit**: `ED EL ICH DCH ECH IL DL SU SD REP`
- **CSI scrolling region**: `DECSTBM (CSI r)`, `DECSLRM` (deferred)
- **CSI modes**: `SM/RM`, `DECSET/DECRST (CSI ? Ps h/l)`
- **SGR**: attributes, 16/256/truecolor, underline styles (deferred)
- **Tabs**: `HTS TBC`
- **Save/restore**: `DECSC/DECRC`, `CSI s`/`CSI u`
- **OSC**: `0/1/2` title+icon, `4` palette, `8` hyperlinks, `10..19` colors,
  `52` clipboard (gated), `9`/`777` notifications (gated)
- **DCS**: xtgettty/xtsettty, DECRQSS. Passthrough reserved for future graphics protocols.

### 1.3 Error recovery

Contract: the parser must **never** enter an unrecoverable state and must never drop the
stream permanently. Rules:

1. Any byte received in any state is either executed, printed, ignored, or dispatched — no byte is lost silently.
2. Malformed CSI/DCS transitions to the matching `*Ignore` state and resynchronizes at the final byte.
3. Unbounded sequences are bounded: parameter list capped (32 params), OSC string capped
   (configurable, default 8 MiB) with explicit truncation flag rather than unbounded growth.
4. `CAN` (`0x18`) and `SUB` (`0x1A`) abort any sequence in progress from any state.
5. `ESC` inside a string state terminates and restarts the sequence.

---

## 2. Data model

### 2.1 Cell

```
Cell {
  content:  CellContent   // Empty | Char(grapheme) | WideChar | WideCharSpacer
  fg:       Color
  bg:       Color
  attrs:    Attrs         // bold, dim, italic, underline, blink, inverse, invisible,
                          // strikethrough, underline-style, overline
  underline_color: Option<Color>
}
```

Design points:
- `content` stores a **grapheme cluster**, not a `char`. Combining marks and ZWJ emoji
  sequences attach to the base cluster. This is the single most common source of incorrect
  rendering in terminal emulators and is non-negotiable in the data model.
- Wide characters occupy two columns; the second column holds `WideCharSpacer`. This
  makes column arithmetic uniform and keeps cursor/scroll logic free of width special-casing.
- No heap allocation per cell. Cell content borrows from a slab/arena so the grid is
  contiguous and `Send`.
- **Glyph fallback across fonts is a correctness requirement, not polish.** Measured: Consolas
  contains no CJK, emoji, or Indic glyphs, and on the test host no installed monospace font
  covers emoji or Devanagari at all. A cell whose cluster has no glyph in the primary font
  must resolve through an ordered fallback chain, and must render a visible tofu marker if the
  entire chain misses. This must not be deferred to a later milestone.

### 2.2 Grid

```
Grid {
  cols: u16, rows: u16,
  cells: Vec<Cell>,          // row-major, cols*rows
  cursor: Cursor,
  scroll_top: u16, scroll_bottom: u16,   // inclusive; DECSTBM
  scroll_left: u16, scroll_right: u16,   // left/right margins; default = full width
  tabs: Vec<bool>,           // per column
  wrap_pending: bool,        // DECAWM deferred-wrap latch
  origin_mode: bool,         // DECOM
  saved_cursor: Option<SavedCursor>,
}
```

- Row-major `Vec<Cell>` with row/col accessors keeps scrolling, damage computation, and
  snapshotting simple and cache-friendly.
- `wrap_pending` implements deferred wrapping: printing in the last column sets the latch and
  writes the cell *without* advancing; the next print wraps. Without this, applications that
  write to the bottom-right cell (progress bars, `tput`) misbehave.

### 2.3 Scrollback

Scrollback is **not** part of the VT specification — it is an emulator feature, and this spec
treats it as an explicit, bounded extension.

- Bounded ring buffer, **bounded by bytes, not by lines** (see §2.8, D10.3). Default budget
  64 MiB, configurable, `0` allowed. The effective line count is derived as
  `budget / (cols * size_of::<Cell>())`, floored at a small minimum, so memory stays flat as
  the window widens instead of growing with column count.
- Primary screen pushes scrolled-off lines into scrollback; alternate screen does not.
- Scrollback stores `Vec<Cell>` rows plus wrapped-line continuation flags so reflow can
  reconstruct logical lines.
- Truncation is oldest-first and is a documented, observable event.

### 2.4 Alternate screen

Three distinct buffers, because conflating them is a common bug:

| Mode | Cursor | Scrollback |
|---|---|---|
| Primary (normal) | visible | collects |
| Alternate (`?1049`) | visible | does not collect |
| Saved-cursor slot | hidden | n/a |

- `?1049` = save cursor, switch to alternate, clear alternate.
- `?1047` = switch to alternate (no implicit clear).
- `?1048` = save/restore cursor only, buffer unchanged.
- Restoring primary must restore cursor **and** `wrap_pending`.

### 2.5 Cursor

```
Cursor { row, col, shape, visible, blink, style: Steady|Blink|SteadyHollow }
```
`shape` and `blink` are terminal preferences overridden by DECSCUSR (`CSI Ps SP q`).

### 2.6 Colors

`Color = Default | Indexed(u8) | Rgb(u8,u8,u8)`.
- Default fg/bg resolve against the active theme at render time, not stored per cell — this is
  what makes theme switching correct without rewriting the grid.
- 256-color palette and the 16 ANSI colors resolved from theme.
- **BCE (background color erase)**: cells written by ED/EL/IL/DL/scroll inherit the current
  SGR background. Many real-world tools depend on this.

### 2.7 Unicode

- **Width**: derived from East Asian Width `W`/`F` → 2, combining/zero-width → 0,
  control → 0. Width lookup is a separate pure function with a version-pinned Unicode table.
- **Clustering**: grapheme cluster segmentation over the decoded stream, with explicit
  handling of combining marks, variation selectors (including emoji presentation
  selectors), ZWJ sequences, and regional indicator pairs.
- **Ambiguous width** is a user setting, default narrow — this is a real cross-platform
  divergence and must not be silently hardcoded.

### 2.8 Resolved data-model decisions

Approved by the design lead, closing the six gaps raised during the specification review. **Not
yet implemented** — engine coding remains unauthorized.

| # | Gap | Resolution |
|---|---|---|
| 1 | Cell content lifetimes | Frame-scoped arenas rejected. `CellContent` stores a single scalar **inline** as `Char(u32)`; true multi-code-point graphemes use **`Arc<str>`** drawn from a string interner. See §2.8.1. |
| 2 | `Attrs` layout | Packed **`u32` bitfield** via `bitflags!`. Bits 0–3 underline style, bits 4–11 font-modifier flags (bold, italic, dim, inverse, blink, invisible, strikethrough, overline). Bits 12–31 reserved. Keeps `Cell` memory-dense. |
| 3 | Engine→renderer boundary | Immutable value types. `Snapshot` owns a **fresh copy of the damaged rows only**, never a view into engine memory. See §3.1 and §2.8.2. |
| 4 | Grid dimensions | `cols` and `rows` clamped to **512 × 256** at init and on resize, so malformed escape sequences cannot trigger unbounded allocation. Scrollback is bounded by **bytes** (§2.3), not by lines. |
| 5 | Wide-character invariant | Raw mutable slice exposure on the grid is forbidden. Writing a wide character at column *N* automatically places `CellContent::WideCharSpacer` at *N+1*, via safe validated write methods only. |
| 6 | Single-owner enforcement | The state machine is wrapped in a **`TerminalEngine` trait** taking `&mut self`. The type is additionally **`!Sync` while remaining `Send`**, making thread confinement a compiler-enforced invariant. See §2.8.4. |

#### 2.8.1 Cell content representation (D10.1, adopted)

```
enum CellContent {
    Empty,
    Char(u32),                    // single scalar, inline, no allocation
    Cluster(Arc<str>),            // multi-codepoint grapheme, interned
    WideChar(u32),
    WideCharSpacer,
}
```

- `Char(u32)` covers the overwhelming majority of terminal content — ASCII, Latin, CJK — and
  performs **no heap allocation**, preserving §2.1's "no heap allocation per cell" for the hot path.
- `Cluster(Arc<str>)` is reserved for true multi-code-point clusters: combining sequences, emoji
  with ZWJ or variation selectors, and regional indicator pairs.
- **Interning is mandatory**, not optional. A per-configuration interner bounds the number of
  live `Arc<str>` by the number of *distinct clusters observed*, not by the number of cells
  written. Without interning, `Arc<str>` would allocate once per cell and silently reverse the
  invariant it was chosen to satisfy.
- Rows copied into scrollback bump refcounts only; no reallocation and no deep clone.

#### 2.8.2 Snapshot ownership (D10.2, adopted)

`Snapshot` owns a per-frame copy of **only the damaged rows**. It never exposes a view or `Arc`
into engine-owned buffers. Consequences:

- The renderer holds owned, stable data whose lifetime is bounded to a single frame.
- §3.1's rule — the renderer "may not store references into engine buffers that outlive a frame"
  — is upheld by construction rather than by convention.
- Cost is negligible because row-level damage (D7) already limits the set of rows copied.

#### 2.8.3 Scrollback budget (D10.3, adopted)

Scrollback is bounded by bytes. See §2.3. Rationale: a line-count ceiling is a memory ceiling
only for a fixed width. With the 512-column clamp, a 10 000-line budget reaches ~5.12 M cells —
roughly 200 MB or more — so users would pay for width they never used.

#### 2.8.4 Thread ownership (D10.4, adopted)

The engine type is **`!Sync` and `Send`**, enforced by a private non-`Sync` marker field
(`PhantomData<*mut ()>` or equivalent).

- `Send` permits the engine to be constructed on and moved with the UI thread.
- `!Sync` makes it impossible to share references to engine state across threads at all.

This converts §3's "exactly one thread owns mutable engine state" from a human convention into
a compiler guarantee. A background thread attempting to touch engine state fails to compile
rather than corrupting state at runtime.

---

## 3. State ownership & concurrency

**Core rule: exactly one thread owns mutable engine state. Ever.**

```
┌──────────────────────────── UI thread (winit + wgpu) ────────────────────────────┐
│  owns: Window, Renderer, InputRouter, SessionManager, and all TerminalState      │
└───────────────┬──────────────────────────────────────────┬─────────────────────┘
                │ read-only snapshot                        │ write bytes
                ▼                                           ▼
        ┌───────────────┐                          ┌──────────────────┐
        │  Render pass  │                          │  ConPTY writer   │
        └───────────────┘                          └──────────────────┘

        ┌──────────────────────────── ConPTY reader thread ───────────────────────┐
        │  reads stdout pipe → channel-send raw bytes → no engine state access    │
        └─────────────────────────────────────────────────────────────────────────┘
```

- The reader thread performs **no** parsing and holds **no** engine state. It only moves
  bytes into a staging buffer. This keeps the ConPTY crate off the engine's back.
- `TerminalState::advance(bytes) -> SnapshotDelta` runs only on the UI thread.
- No interior mutability, no `Arc<Mutex<Terminal>>`. If a second consumer of engine state
  ever appears, the answer is a `Snapshot` value type handed over, not shared access.

### 3.0 Output stream integrity — normative

Terminal output is an **ordered byte stream**. Losing bytes corrupts parser state
permanently: a dropped `ESC` turns the remainder of the stream into printed garbage, and a
partial UTF-8 sequence becomes a replacement character. **Byte-dropping overload policies
are therefore prohibited**, including the earlier "coalesce-and-continue" proposal, which
was formally rejected by the design lead.

The correct resolution is to use ConPTY's own flow control rather than to discard data.

**Mechanism.** The pseudoconsole maintains a finite internal output buffer. When our reader
stops draining the output pipe, that buffer fills, and the *child's* writes to the console
block. Backpressure therefore propagates inward from the child, not outward from us. This
gives us integrity for free, provided we never drop.

**Handoff.** Double-buffered swap, not a bounded channel:

```
reader thread                          UI thread
-------------                          ---------
loop {                                 frame {
  n = ReadFile(pipe)                     let mut batch = staging.take()   // swap
  lock(staging)                           engine.advance(batch)
  staging.extend(buf[..n])               request_render()
  unlock + notify                        unlock
}                                      }
```

- The reader appends in read order under a short-lived mutex. It never waits for the UI
  thread's permission to store bytes; it contends only for the swap, which is sub-microsecond.
- Exactly two buffers exist. Each is bounded by "bytes the child produced since the last UI
  frame", which is finite because the child's production rate is bounded by the pty buffer
  size and the UI drains it every frame.
- Memory is therefore bounded **structurally**, not by a chosen constant. There is no
  unbounded-growth path and no discard path.

#### 3.0.1 Backpressure is applied *before* the read, never after

The reader must never consume bytes from the pipe that it cannot store. Therefore capacity is
reserved **before** each `ReadFile`, not checked afterwards:

```
loop {
    match buffer.wait_for_capacity(CHUNK) {
        Cancelled => break,                       // session is tearing down
        Open      => { let n = ReadFile(pipe); buffer.push(&buf[..n]); }
    }
}
```

`wait_for_capacity` returns immediately while `depth + CHUNK <= hard`, and otherwise waits on a
condition variable with a bounded poll interval, re-checking cancellation on every wake.
Because `hard` is never exceeded by more than one chunk, peak depth is bounded by
`hard + CHUNK`. A post-read rejection path must not exist: it would require discarding bytes
already removed from the pipe.

#### 3.0.2 Failure policy — three termination paths, no deadlock

The reader may never stop *permanently*. If it did, a child blocked writing to a full pty buffer
would deadlock the session forever. The policy has exactly three exits, and all of them
terminate:

| # | Condition | Action | Bytes |
|---|---|---|---|
| **1 — Resume** | UI drains below `resume` | Reader wakes and continues reading normally | All preserved |
| **2 — Cancel** | UI loop exited, window closed, or child handle lost | `cancel()` sets the flag and notifies; `wait_for_capacity` returns `Cancelled`; reader stops reading and exits | All accepted bytes remain in staging and are handed to the parser before teardown |
| **3 — Teardown** | Window closed with a blocked child | Shutdown sequence in §3.3 runs: close PTY input, grace period, terminate, drain, parse tail | All accepted bytes parsed before the grid is torn down |

Additional rules:

- **Never discard to recover.** There is no code path that drops bytes to unblock either side.
  If bytes are lost due to a defect, that is a bug to be reported, not a policy to be applied.
- **Escalation on sustained failure.** If the reader pauses and the session has already been
  paused more than `N` times (default 64) without the child draining, this indicates the UI
  cannot keep up structurally. The session is terminated with an explicit diagnostic rather
  than left to spin. The counter is surfaced, never reset silently.
- **Diagnostics.** `soft` threshold crossed → warn with current depth, rate-limited to once per
  second. `hard` threshold reached → warn that backpressure is engaged and the UI is the
  bottleneck. Counters exposed as `accepted`, `max_depth`, `pauses`, `paused_total`.

**Structural conclusion:** the only durable fix for a stalled UI is to make the UI faster.
Backpressure is a symptom detector, not a solution. Every pause is a defect report.

#### 3.0.3 Verification status

Implemented as `Juna::transport::StreamBuffer` with six passing unit tests:

| Test | Property |
|---|---|
| `byte_exact_round_trip` | Invalid UTF-8, split multi-byte sequences, and truncated escape sequences emerge byte-identical |
| `ordering_preserved_under_pressure` | 512 KiB across 1024-byte chunks, concurrent producer/consumer, output equals input exactly |
| `depth_stays_bounded_under_pressure` | Peak depth never exceeds `hard + CHUNK`; pauses are counted; accepted count is exact |
| `cancel_unblocks_a_full_reader` | `cancel()` releases a reader blocked at capacity with `Cancelled` |
| `accepted_bytes_are_never_discarded` | `accepted` is cumulative across drains and never decreases |
| `empty_stream_is_clean` | Clean EOF with no bytes produces no spurious data |

These prove the buffer's contract in isolation. They do **not** prove end-to-end ConPTY
integrity; that requires the Phase 1 spike to run a real child through the real pipe and
compare.

**Shutdown ordering.** On close, the reader stops reading the pipe, appends whatever
remains, signals end-of-stream, and exits. The UI parses the tail **before** tearing down, so
the last line a shell printed before exit is never lost.

**EOF is not a reliable terminator — measured.** The ConPTY output pipe does **not** report EOF
when the child exits. It stays open until `ClosePseudoConsole` is called, so a read loop that
waits for EOF hangs forever. This was reproduced empirically: `read_to_end` on a finished
`cmd /C echo` child never returned within 8 seconds (`DECISIONS.md` §6, CHECK 2). The reader
must therefore be driven by **explicit lifetime**, not by pipe EOF: the session's close
sequence is the only thing that ends the read, and the reader must treat "no bytes available"
as ordinary, not as end-of-stream.

**Verifiability.** The guarantee is testable as a property: a byte-exact round-trip test
asserts that an arbitrary byte stream (including invalid UTF-8, split multi-byte sequences,
and truncated escape sequences) emerges from the handoff identical to what was written. This
test must exist before Phase 1 is considered complete.

### 3.1 Renderer contract

The renderer consumes a **read-only, owned** view. It may not:
- parse VT, hold engine state, or mutate the grid;
- store references into engine buffers that outlive a frame.

```
enum Damage { Full, Rows(Range<u16>), Cells(Rect) }
SnapshotDelta { damage, resize: Option<Size>, cursor, scroll_offset, alt_active, ... }
```

Damage is produced by the engine, not the renderer: the engine knows which rows it touched.
This is the boundary that keeps "replace the renderer" a local change.

### 3.2 Resize protocol

1. Window resized → UI thread computes new pixel size.
2. UI thread **coalesces** resizes (one pending request; latest wins) — resizes during a drag
   must not spawn one `ResizePseudoConsole` per frame.
3. UI thread computes the new integer cell grid from cell metrics, guaranteeing
   `cols >= 1`, `rows >= 1`, and honoring minimum pane size later.
4. UI thread sends PTY resize with the cell grid.
5. UI thread applies grid resize to `TerminalState`; the PTY may echo reflowed content
   afterwards, which flows through the normal parse path.

Order matters: PTY resize before grid resize, so shell-side reflow output is parsed by the
already-correct grid.

### 3.3 Shutdown

Explicit, ordered, and non-blocking:
1. UI loop observes `CloseRequested` or `TerminalExited`.
2. Close PTY input (signals EOF to the shell).
3. Bounded grace period (default 2 s) for the child to exit.
4. Terminate, then wait-with-timeout on the process handle.
5. Drain and parse every accepted reader byte; flush incomplete UTF-8 only at true stream end.
6. Join the reader thread.
7. Drop GPU resources, then the window.

Child processes must not outlive the app. A leaked `powershell.exe` after closing Juna is a
release-blocking defect, not a cosmetic issue.

### 3.4 Scrollback reflow on resize (D4)

Reflow of **logical lines** is the approved target. This section specifies the behaviour that
was previously left undefined. It is normative.

#### 3.4.1 Storage model

Scrollback stores logical lines, not physical rows:

```
LogicalLine {
  rows:    Vec<Vec<Cell>>,   // >= 1 physical rows
  wrapped: bool,             // occupies more than one row
}
```

A line is *wrapped* iff it was produced by an automatic wrap (deferred-wrap latch) rather than
by an explicit newline or control sequence. This distinction must be recorded at write time,
because it cannot be reliably reconstructed later.

#### 3.4.2 What reflows and what does not

| Buffer | On width change |
|---|---|
| Scrollback (primary) | **Reflows** |
| Active primary screen | **Does not reflow** — plain grid resize (see 3.4.5) |
| Alternate screen | **Does not reflow** — plain grid resize; it has no scrollback |

Reflowing the *active* screen is deliberately excluded. A full-screen application owns that
content and will redraw it; reflowing underneath it corrupts its layout model. Scrollback is
already-dead content, so reflowing it is safe and is what users actually expect when narrowing
a window over a long log.

#### 3.4.3 Cell-level rules during reflow

A logical line is flattened to its cell sequence, re-flowed at the new width, then re-cut into
rows.

1. **Wide cells (width 2).** A wide cell is never split across a row boundary. If only one
   column remains at the end of a row, the wide cell moves intact to the next row and the
   trailing column is filled with an empty default cell. The cell's `WideCharSpacer` moves
   with it.
2. **Zero-width cells (combining marks).** These attach to their base cell and travel with it
   as part of the same grapheme cluster. They never occupy a column and never trigger a wrap.
   A zero-width cell encountered with no preceding base in the logical line is rendered as
   width-1 replacement rather than being dropped.
3. **Empty cells.** Trailing empty cells beyond the last non-empty cell are trimmed before
   re-flow and re-padded afterwards, so a resize does not manufacture thousands of empty rows.
4. **Line length below new width.** A line that fits is emitted as a single row with
   `wrapped = false`.
5. **Empty logical line.** Produces exactly one empty row; never zero rows.
6. **Zero-cell edge case.** A logical line with no cells yields one empty row.

#### 3.4.4 Cursor under reflow

The cursor is **not** stored as an absolute `(row, col)`. It is stored as
`(logical_line_index, column_offset_in_logical_line)`, and the absolute position is recomputed
after reflow. Consequences:

- Cursor column is preserved with respect to the logical line, so the caret stays on the same
  logical line even if it moves to a different physical row.
- If the cursor is inside the scrollback viewport, it is expressed as an offset from the
  bottom of the screen, and that offset survives reflow unchanged.
- Cursor row must never be recomputed from a stale absolute value.

#### 3.4.5 Active screen resize rules

- **Narrowing (cols decrease):** truncate each row at the new width. A wide cell straddling the
  new boundary is replaced by a single empty cell at the boundary column, and the truncated
  column is not carried into scrollback.
- **Row count change** must respect the scroll region. Rows are removed from the bottom edge
  **of the scroll region**, not the bottom edge of the screen. If `scroll_top != 0` and rows
  are removed, content above `scroll_top` must not move. Getting this wrong is a classic and
  highly visible defect.
- **Widening (rows increase):** new rows are inserted at the bottom edge of the scroll region,
  filled with the current default cell. Inserting at the screen bottom while a scroll region is
  active is incorrect.
- **Cursor clamping:** after any resize the cursor is clamped into the grid. If the cursor row
  falls outside the new bounds it is moved to the last row, preserving its column when possible.
- **Tabs** are re-evaluated against the new width; tab stops beyond `cols` are dropped and new
  stops follow the standard 8-column cadence within the new width.

#### 3.4.6 Ordering and cost

Reflow runs on the UI thread inside the same frame that applies the resize, before rendering.
It is O(total cells in scrollback) per resize. If profiling shows this is unacceptable during
window drags, the mitigation is to reflow lazily on scroll-into-view rather than eagerly — this
must be measured, not assumed.

### 3.7 Workspace layout engine (BSP)

The layout engine is **pure**: it consumes a container rectangle and a pane tree, and returns
rectangles. It has no knowledge of sessions, processes, glyphs, cells, or GPU state. It performs
no I/O and is fully unit-testable without a window, a shell, or an adapter.

Implemented in `src/layout`.

**Determinism.** Split ratios are fixed-point per-mille (`Ratio(u16)`, range 1..=999) and all
arithmetic is integer in `i64`/`i128`. No floating point is used anywhere in the layout path, so
identical inputs yield bit-identical output on every platform. This is a hard requirement, not a
preference: the renderer must be able to cache geometry without re-deriving it.

**Invariants asserted by the test suite, on every tree it constructs:**

1. The number of emitted rects equals the pane count.
2. **Leaf areas sum exactly to the container area** — no gaps, no double-counting.
3. Every leaf rect is contained by the container.
4. No two leaf rects overlap.
5. `below_minimum` is truthful — it agrees with the rect it describes.
6. Repeated layout of the same tree is byte-identical.

**Minimum size** is enforced pairwise at each split: when a parent's extent can satisfy two
minimums, both children are clamped and the ratio yields to the minimum. **Known limitation:** a
deeply nested tree can still produce a pane below minimum even when the container itself is
large enough, because a sibling subtree may consume the space first. This is reported through
`LayoutReport::unsatisfied` rather than hidden. A globally-optimal minimum assignment is a
separate constraint-satisfaction problem and is **not** attempted.

**Closing a pane** replaces the pane's parent split with the surviving sibling, so the survivor
inherits the collapsed subtree's rectangle. Closing the root empties the tree. **Focus** follows a
survivor rather than being lost. The layout engine has no opinion about what happens to the
*session* behind a pane — that is the session manager's responsibility, and closing one pane must
never terminate an unrelated session.

**Focus movement** is structural: walking up from the focused leaf, the first ancestor whose
split axis matches the requested direction supplies the target subtree, whose extreme leaf is
selected. Movement at an edge returns `None` and leaves focus unchanged.

| API | Purpose |
|---|---|
| `Tree::split_focused(axis, ratio)` | Replace the focused leaf with a split; focus the new pane |
| `Tree::close(pane)` | Remove a leaf and collapse its parent |
| `Tree::move_focus(dir)` / `pane_in_direction(from, dir)` | Directional focus |
| `Tree::layout(area, min) -> LayoutReport` | Geometry pass; rects plus unsatisfied minima |

**Not yet specified:** ratio adjustment by dragging, pane minimums derived from cell metrics,
tab-to-tree mapping, and resize coalescing. These depend on the renderer and session manager.

### 3.5 Damage model (D7)

Row-level damage is the provisional renderer strategy, with full-redraw always available.

- The engine emits `Damage::Rows(Range<u16>)` for ordinary screen mutation and
  `Damage::Full` after a resize, buffer switch, or scrollback discontinuity.
- `Damage::Cells(Rect)` is retained in the type but not emitted yet. It exists so that
  cell-granular damage can be added later without an API break if profiling justifies it.
- The renderer must honour whatever damage it is given; it may widen a frame to `Full` at any
  time, but the engine must never widen on the renderer's behalf.
- Empirical basis for choosing row-level first: glyph rasterization and glyph-cache lookups
  dominate frame cost, so shrinking the *upload* region while re-running the same per-cell work
  yields little without a bindless/instanced glyph path. Row damage is the cheap, low-risk
  starting point. **This reasoning is a hypothesis, not a measurement** — it must be confirmed
  or refuted by profiling in Phase 3 before cell damage is considered.

### 3.8 Process privilege boundary (D10) — normative

**Adopted 2026-10-11.** Elevation is a property of the **host process**, never something Juna
requests on behalf of a child.

#### Rules

1. **The manifest must not request elevation.** `juna` ships without `requireAdministrator`,
   `highestAvailable`, or `uiAccess`. Casual launch is unelevated.
2. **Elevation is chosen at launch, by the user.** `scripts/run-elevated.ps1` launches Juna via
   the `runas` shell verb, which raises a UAC prompt. This is the *only* supported elevation path.
3. **The transport layer never raises or lowers privilege.** No ConPTY session may request
   elevation for its child, and none may impersonate. A child inherits the host's token, full stop.
4. **The engine is privilege-agnostic.** It performs no I/O and has no access to a token, so the
   integrity level cannot reach it. There is no code path from a byte in the stream to a privilege
   change.
5. **No in-UI elevation affordance in this increment.** "Run this shell as Administrator" is out of
   scope. See *Rejected* below.

#### Why the boundary is drawn here

`conpty-oxide` 0.1.2 exposes no token API — it wraps `windows-spawn`, which reaches only
`CreateProcessW` with `creation_flags` (verified by reading the vendored source). A per-pane
elevated child spawned from an unelevated Juna would therefore require `CreateProcessAsUserW` plus
`UpdateProcThreadAttribute(PROCEDURE_ATTRIBUTE_TOKEN)`, bypassing the pinned binding with several
hundred lines of `unsafe`. `DECISIONS.md` §6.3 permits replacing `conpty-oxide` only for a critical
lifecycle defect, abandonment, or demonstrated incompatibility — **none of which apply**. The
binding stands.

#### Why §3.0 becomes a safety property under elevation

§3.0 is written as a correctness rule: dropping bytes corrupts parser state. At high integrity it
is also a **safety** rule, and the reason is specific rather than atmospheric.

A mangled byte stream does not merely garble the display — it can deliver a **corrupted argument to
the child process**. At unelevated integrity that is usually a permission error. At high integrity
the same corruption can reach a destructive path with no UAC prompt between it and the filesystem.
Windows' *InstallerDetection* mitigation covers installers; it does **not** cover console tools.

Therefore: **byte-exact round-trip (§3.0.3) is a release gate for any elevated build**, not only a
correctness gate. An elevated release that has not demonstrated byte-exact transport on a real shell
session is not releasable.

#### Threat note

Windows' UAC is a consent prompt, not a security boundary; it does not defend against malware that
is already running as the user. That does not weaken the rules above — it means the *marginal* risk
of an always-elevated terminal is higher than a user may assume, because every command typed at the
prompt inherits full administrative authority with no prompt of its own.

#### Rejected in this increment

- **`requireAdministrator` in the binary manifest.** Rejected: it makes *every* launch elevated,
  including casual use. Elevation is opt-in per launch instead.
- **Per-pane elevated child from an unelevated host.** Rejected: contradicts §6.3, as above. It
  would also make the elevated surface reachable at all times, which is the opposite of the intent.
- **Silent auto-elevation on detection of a privileged command.** Rejected: it removes the consent
  boundary the user relies on to know when they are at elevated authority.

### 3.9 Session seam (D11) — normative

**Adopted 2026-10-11.** `src/session/` is the **only** place in the project permitted to name a
platform API for process I/O.

```
        ┌────────────── application / renderer / engine ──────────────┐
        │   never sees a platform type; holds Box<dyn Session>         │
        └──────────────────────────┬───────────────────────────────────┘
                                   │
        ┌──────────────────────────┴───────────────────────────┐
        │  SessionSpec → spawn() → Box<dyn Session>            │
        │  write / read / resize / try_wait / kill / pid       │
        └───────┬──────────────────────────────┬───────────────┘
                │                              │
   FakeSession (pure std,            ConptySession (cfg(windows))
   runs on every target)            conpty-oxide, zero unsafe
```

#### Rules

1. **The trait signature carries no platform type.** `Session`, `SessionSpec`, `Dimensions`, and
   `ExitStatus` are defined in Juna and mention no crate, handle, or Win32 type. If one appears, a
   backend stops being additive.
2. **`spawn` is the only entry point.** Callers never name a backend.
3. **Backends are additive.** A Unix PTY backend is a new file in `src/session/` implementing
   `Session`. It must not require edits anywhere else.
4. **A missing backend is an error, never a stub.** `spawn` returns `ErrorKind::Unsupported` where no
   backend exists. It must not fall back to a fake or a degraded pipe.
5. **`write` is all-or-error.** Backends loop internally; a short write is never surfaced. §3.0
   forbids dropping bytes.

#### Two contracts callers must know

These are not implementation details. Both were found by tests, not by reading the docs.

- **`read` blocks, and `Ok(0)` is EOF — not "nothing yet".** ConPTY holds its output pipe open after
  the child exits and never delivers EOF. Waiting for `Ok(0)` to notice exit hangs forever. **Detect
  exit with `try_wait`; run reads on their own thread**, per §3.
- **`kill` returns before the process has died.** The Windows backend tears down the session job
  object and termination proceeds asynchronously, so `try_wait` can still report `None` immediately
  after a successful `kill`. Callers must poll. A successful `kill` is **not** evidence of death.

#### Why `FakeSession` is not a test-only convenience

It is the only way to exercise the wiring on a target with no backend. Being pure `std`, it compiles
and behaves identically everywhere, which is what keeps the seam honest on a machine that has no
ConPTY. A fake that could pass on Windows and fail elsewhere would not be testing the seam.

#### Enforcement

`src/session/tests.rs` — 15 tests, 4 of which drive the **real** ConPTY backend through `dyn Session`.
The Linux dependency graph is verified to contain zero Windows-only crates
(`DECISIONS.md` §6.18). `conpty-oxide` is referenced from `src/session/windows.rs` and nowhere else.

#### Why `split` is required rather than optional

A terminal **cannot** drive a session from one thread. `read` blocks and ConPTY never delivers EOF,
so a thread blocked reading cannot also answer input — if the child waits for input before producing
output, a single-threaded loop deadlocks. §3 already prescribes the fix (reads on their own thread)
but that requires ownership to be *divided*, which is what `split` does. `reader` goes to the reader
thread; `writer` and `control` stay on the UI thread. No handle is shared, so no locking.

---

## 3.10 Conformance harness — normative

`src/bin/juna_headless.rs` drives a real shell through the entire production path and prints the
rendered screen. It is the primary evidence that the engine is correct against real programs.

**Every component is production code.** Real shell, real ConPTY backend, real `StreamBuffer`, real
`TerminalState`. Nothing is stubbed. A stubbed path would prove only that the stub works.

| Rule | Reason |
|---|---|
| Input is sent only after the shell goes **quiet**, never matched against a prompt string | matching a prompt couples the harness to one shell, which §5.1 forbids. PowerShell discards input written before its console host attaches (~1.5s of profile loading), so a fixed delay is not sufficient and a prompt match is not permitted. |
| Markers are asserted on **screen or scrollback** | content that scrolled off is still history; reporting that as a miss would be wrong |
| Erased content is expected to be **gone** | xterm does not push lines erased by ED 2 into scrollback, and neither does Juna |
| Teardown always reaps | §3.9 `kill` is asynchronous; the harness polls `try_wait` before exiting |

`tests/shell_roundtrip.rs` is the automated form of the same harness: real `cmd.exe`, five tests, all
shell-independent because every assertion is on text the test itself chose.

---

## 4. Conformance & test strategy

### 4.1 Layers

| Layer | Scope | Runs in CI | Determinism |
|---|---|---|---|
| Parser unit | byte → action stream, state transitions | yes | total |
| Engine unit | action stream → grid/cursor/scrollback diff | yes | total |
| VT fixtures | recorded byte sequences → expected screen | yes | total |
| Unicode | width + clustering tables | yes | total |
| Stress | burst output, resize storms, malformed floods | yes | bounded-time |
| Windows integration | real ConPTY child, real window | manual/CI runner | non-deterministic |

### 4.2 Required test properties

- Parser is **total**: no input byte sequence may panic or loop forever (fuzz target).
- Engine transitions are **deterministic**: same byte stream + same initial state ⇒
  identical snapshot. Golden-file assertions make this enforceable.
- Every `unsafe` block carries a `// SAFETY:` rationale (per global rule; ConPTY/Win32
  interop is the expected place for them).

### 4.3 Initial compatibility scope (explicit target)

**Tier 1 — required for first usable release**
C0/C1 execution, SGR (16/256/truecolor), cursor motion, ED/EL/ICH/DCH/IL/DL/SU/SD,
DECSTBM, DECAWM with deferred wrap, tabs, DECSC/DECRC, `?1049`/`?1047`/`?1048`,
OSC 0/1/2, scrollback, resize + reflow, Unicode width/clustering, BCE.

**Tier 2 — required before "developer-grade" claim**
bracketed paste (`?2004`), focus events (`?1004`), mouse (`?1000`/`?1002`/`?1003`),
DECRQM, alt-wrap/word-wrap modes, synchronized output (`?2026`), REP, OSC 52 clipboard.

**Tier 3 — explicitly deferred**
Sixel, Kitty graphics, iTerm2 inline images, sixel scroll region control, OSC 133 shell
integration marks (desirable, not blocking).

Anything in Tier 3 that a real application needs is a **scope change**, not a silent omission.

---

## 5. Decision status D1–D7

Status values: **DIRECTION SET** (lead has given a direction; not yet proven).
**UNRESOLVED** (requires evidence or a decision before the dependent phase starts).
**BLOCKED** (a direction exists but cannot be executed in this environment).

| # | Decision | Status | Direction / requirement | Outstanding |
|---|---|---|---|---|
| **D1** | Toolchain | **DEFERRED — GNU in use** | MSVC remains the intended toolchain. Project-local pinning via `rust-toolchain.toml`; the global rustup default must not be changed. | **Deferred by design-lead ruling 2026-10-11.** GNU produces `check`, `clippy`, 129 passing tests, and a release binary. `rust-toolchain.toml` is deliberately **not** created. Re-probed: MSVC `link.exe` still absent. See `DECISIONS.md` §7.1. |
| **D2** | Async runtime | **DIRECTION SET — implemented** | Remove Tokio. Standard threads + explicit buffer handoff. | None. Async may return only with a demonstrated requirement. |
| **D3** | Unicode data | **UNRESOLVED** | Pinned Unicode version behind a pure API. | Compare internal tables against `unicode-width`, `unicode-segmentation`, `unicode-properties` before committing. See `DECISIONS.md` §6. |
| **D4** | Scrollback reflow | **DIRECTION SET — specified** | Reflow logical lines. | Specified in §3.4. Wide cells, cursor, scroll-region resize, and tab rules are now explicit. |
| **D5** | ConPTY binding | **APPROVED — `conpty-oxide` 0.1.2** | Measured against six criteria; `conpty` 0.7 failed five and was removed. Juna wraps it in its own transport boundary. Replacement criteria in `DECISIONS.md` §6.3. |
| **D6** | Font rasterization | **CLOSED — pure-Rust primary, DirectWrite deferred** | `fontdb` + `swash`/`zeno` behind a Juna-owned trait, with a Juna-owned glyph atlas. Measured 11.6 ns/lookup, 2.83 µs/glyph. | DirectWrite may return as a second backend if small-size readability proves poor. **Fallback, shaping, and atlas are mandatory, not deferred.** Shaper choice (`harfrust` vs `rustybuzz`) still open. See `DECISIONS.md` §6.11. |
| **D7** | Damage model | **DIRECTION SET** | Row-level damage plus full-redraw support; retain cell-level option. | Specified in §3.5. Rationale is a hypothesis pending Phase 3 profiling. |
| **D8** | wgpu backends | **APPROVED PROVISIONALLY** | `default-features = false`, features `std, dx12, vulkan, gles, wgsl`. Enumeration confirms dx12=3, vulkan=1, gles=1. | `gles` retained. Feature set is **not** to be changed on enumeration evidence alone; presentation paths are untested. See `DECISIONS.md` §6.4. |
| **D9** | Adapter selection policy | **DIRECTION SET — must be explicit** | Never take the first adapter by default. | Reject `DeviceType::Cpu` (a WARP software rasteriser is exposed and could be selected silently). Prefer `DiscreteGpu`; treat software as last-resort fallback. The DX12 list contains apparent duplicates, so do not assume deduplication. |
| **D10** | Process privilege | **DIRECTION SET — specified** | Elevation is a property of the **host process**, never requested by Juna for a child. No manifest elevation; no in-UI elevation affordance; token APIs forbidden in `src/`. | Specified in §3.8. Enforced by `tests/elevation_boundary.rs`, verified non-vacuous by fault injection. §3.0 byte-exactness becomes a **release gate** for any elevated build, not only a correctness gate. |
| **D11** | Cross-platform | **DIRECTION SET — seam built** | Cross-platform is a **direction**, not a shipping commitment. Windows is first and must be mature; the seam exists so Linux is not foreclosed. `src/session/` is the sole place permitted to name a platform process API; backends are additive. | Seam specified in §3.9 and implemented: `ConptySession` (Windows) + `FakeSession` (portable). **The Unix PTY backend is not implemented** — `spawn` returns `Unsupported` there. No Linux compile has been performed; only the dependency graph is verified. |

### 5.1 Shell acceptance targets

Conformance testing is **shell-independent**. The engine must never be designed around one
shell's output. Shells are used only as acceptance targets for the process/transport layer.

| Target | Role | Present on this machine |
|---|---|---|
| Windows PowerShell 5.1 | **Primary** interactive target (per lead ruling) | Yes — 5.1.26100.9444 |
| `cmd.exe` | Secondary compatibility target | Yes |
| PowerShell 7 (`pwsh`) | Optional later; **not** a Phase 1 blocker | No — not installed |

Phase 1 must additionally verify behaviour for real applications, not only shells: editors,
pagers, progress displays, and full-screen TUIs exercise substantially more of the engine than
a shell prompt does.

---

## 6. Explicitly rejected

- **Borrowing a terminal engine** (`alacritty_terminal`, `wezterm-term`): rejected by directive.
  Public *behavior* is a valid reference; *implementation* is not a donor.
- **`vte` crate as the parser**: rejected. It is an infrastructure-quality parser, but the
  directive is an original engine with our own state machine, and a third-party parser would
  make parser behavior an external dependency.
- **Pre-allocating a crate per subsystem**: deferred until a real boundary needs it. A
  workspace split is justified by build-graph or ownership pressure, not by diagram symmetry.

The native application window remains titled Juna Terminal. OSC title sequences continue
to update engine metadata without changing the application window title.

Application artwork is presentation infrastructure: GNU windres embeds assets/juna.ico into
the Juna executable; winit loads that owned native icon for the window and taskbar.


Bundled font assets are renderer infrastructure, loaded into fontdb before system discovery.
The selected embedded face takes precedence over installed versions. Shaping, rasterization and
metrics use regular weight 400 for variable fonts. The engine never sees font bytes or settings I/O.
Settings/font previews hold a restorable app-owned preference snapshot; committing writes through
Store and cancellation restores it. Local zoom is intercepted before shell input and TUI mouse
reporting, then uses the existing checked PTY/grid resize path. Each Juna process declares its
Windows taskbar application identity before presenting a window.

Live-preview panel geometry is derived from the current viewport's right edge, independent of
terminal cells; pointer-anchored context menus keep their existing behavior. Font notices are
app-owned file I/O. Native Windows relaunch properties are set before the window becomes visible;
they do not enter terminal state or the byte transport.

Tabs keep active engine/session values in App and park inactive values in owned tab records.
The UI thread alone mutates every engine. Each output stream receives a bounded 64 KiB drain
per event-loop turn, including inactive tabs; bytes remain ordered and backpressure remains intact.
Each tab reaps its process independently, and the window closes only after its final session ends.
The tab header is a presentation inset; the renderer still consumes owned snapshots and UI scenes.

Theme discovery uses a bounded standard-library worker with generation-tagged owned results.
It reads validated JSON only, skips linked folders, limits depth/files/bytes, and never sees engines.
Family folders group custom themes; edits preserve imported sources and removal uses recoverable trash.

SessionSpec carries owned child-only environment overrides. Only the process backend passes them
to ConPTY; app/tool packaging may prepend a tools directory for each session without global PATH
or profile mutation. Environment plumbing does not change engine ownership or byte transport.

The line-number gutter is app-owned presentation geometry. Its reserved left inset feeds PTY/grid
width, glyph/cursor origin and mouse hit-testing together; labels come from retained-buffer row
indices, not extra VT cells. The renderer consumes only snapshots and owned label overlays.
Tab menus hold stable session IDs so inactive actions and exit races cannot target another tab.
Middle-click close uses a Cancel-first confirmation; process shutdown follows the existing path.
Theme favourites are independent saved preferences and update the appearance rollback snapshot
without committing previewed fonts/themes. The confirmation UI shares the pure contrast policy.
