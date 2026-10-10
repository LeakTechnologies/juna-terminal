# Smith Terminal Engine — Architecture & Interface Specification

Status: **PROPOSED — architecture direction approved; not approved as a frozen implementation
contract. Engine coding remains unauthorized.**

Authority: Work Order 001 (revised) — original VT engine, no borrowed terminal engine.
Provenance: All content below is written from public VT/xterm specifications and observable
terminal behavior. No Alacritty / WezTerm / Kitty source has been copied or adapted.

---

## 0. Scope boundary

This document specifies the terminal engine and its interfaces. It does **not** yet commit to
the renderer, BSP layout, editor, or drawer designs; those depend on the engine contract fixed here.

**Out of scope for the first usable release:** Sixel, Kitty graphics protocol, iTerm2 inline
images, ligatures, sixel/kitty bitmap output, arbitrary DCS passthrough payloads.

---

## 1. Parser

### 1.1 State machine

Smith's parser is a byte-driven finite-state machine over the DEC ANSI parser model. States:

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

### 1.2 Control sequence families Smith targets

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

- Bounded ring buffer, default 10 000 lines, configurable, `0` allowed.
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

Implemented as `smith::transport::StreamBuffer` with six passing unit tests:

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
5. Drain and discard remaining reader bytes.
6. Join the reader thread.
7. Drop GPU resources, then the window.

Child processes must not outlive the app. A leaked `powershell.exe` after closing Smith is a
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
| **D1** | Toolchain | **BLOCKED** | MSVC is the intended toolchain. Project-local pinning via `rust-toolchain.toml`; the global rustup default must not be changed. | Cannot be executed: MSVC `link.exe` is absent. See `DECISIONS.md` §7. Requires VS Build Tools "Desktop development with C++" + Windows SDK. |
| **D2** | Async runtime | **DIRECTION SET — implemented** | Remove Tokio. Standard threads + explicit buffer handoff. | None. Async may return only with a demonstrated requirement. |
| **D3** | Unicode data | **UNRESOLVED** | Pinned Unicode version behind a pure API. | Compare internal tables against `unicode-width`, `unicode-segmentation`, `unicode-properties` before committing. See `DECISIONS.md` §6. |
| **D4** | Scrollback reflow | **DIRECTION SET — specified** | Reflow logical lines. | Specified in §3.4. Wide cells, cursor, scroll-region resize, and tab rules are now explicit. |
| **D5** | ConPTY binding | **APPROVED — `conpty-oxide` 0.1.2** | Measured against six criteria; `conpty` 0.7 failed five and was removed. Smith wraps it in its own transport boundary. Replacement criteria in `DECISIONS.md` §6.3. |
| **D6** | Font shaping | **UNRESOLVED** | Decide before renderer implementation. | Prototype shaping, fallback, rasterization, and atlas as one pipeline. Must cover combining marks, wide glyphs, emoji, and missing-glyph fallback. |
| **D7** | Damage model | **DIRECTION SET** | Row-level damage plus full-redraw support; retain cell-level option. | Specified in §3.5. Rationale is a hypothesis pending Phase 3 profiling. |
| **D8** | wgpu backends | **APPROVED PROVISIONALLY** | `default-features = false`, features `std, dx12, vulkan, gles, wgsl`. Enumeration confirms dx12=3, vulkan=1, gles=1. | `gles` retained. Feature set is **not** to be changed on enumeration evidence alone; presentation paths are untested. See `DECISIONS.md` §6.4. |
| **D9** | Adapter selection policy | **DIRECTION SET — must be explicit** | Never take the first adapter by default. | Reject `DeviceType::Cpu` (a WARP software rasteriser is exposed and could be selected silently). Prefer `DiscreteGpu`; treat software as last-resort fallback. The DX12 list contains apparent duplicates, so do not assume deduplication. |

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