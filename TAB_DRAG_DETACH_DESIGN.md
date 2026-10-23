# Tab drag feedback and drag-out detachment

Requested 2026-10-21 under AGENTS.md §3. Approved 2026-10-21 as one design covering both the
drag feedback and detachment into a new window. Delivery is staged: the feedback lands first
because it is self-contained and independently useful, and detachment needs the window-ownership
work described in §3.

## 1. Drag feedback (implemented)

- **Dragged-tab outline.** While a tab is being dragged, `ui::tab_scene` draws the outline of the
  tab's slot as four thin filled rectangles (the overlay renderer supports fills only), sized
  `step - 2` by `TAB_HEIGHT * scale`, centred on the pointer and clamped to the strip so it never
  covers the new-tab button. `ui::ghost_rect` computes it as pure geometry.
- **Destination highlight.** `ui::drop_rects` derives both a translucent accent band over the
  slot the tab will occupy and the crisp insertion marker from the same resolved gap, so the
  highlight cannot disagree with the reorder. At the far end of the strip the slot is off-strip,
  so the band moves left of the marker.
- Both use the existing pre-removal `drop_gap` indexing, so the highlight matches where
  `move_tab` inserts. The accent is fixed rather than themed, so the gesture reads the same in
  low-contrast light themes as in dark ones.
- The renderer contract is unchanged: `ui.rs` stays pure geometry and the renderer still receives
  an owned, read-only `Overlay`.

## 2. Drag-out detachment (implemented)

**Gesture.** A press selects the tab as today. Travel beyond `TAB_DRAG_THRESHOLD` in either axis
starts a drag. Releasing inside the strip reorders. Releasing below the strip, past
`TAB_HEIGHT * scale + 24 * scale`, detaches the tab into a new window. While the pointer is in
the detach zone the strip shows the detach affordance and no destination band, so the two
outcomes are never ambiguous. Escape and losing the pointer cancel the gesture without moving
the tab. A window whose strip holds a single tab shows no detach affordance, because detaching
it is refused (see the last-tab rule).

**Session transfer is a move, not a restart.** `Tab` already owns everything a suspended session
needs (`TerminalState`, `Runtime`, viewport offset, selection, failure, closing) except for the
active slot, whose state lives in the window's active fields. Detaching therefore moves the tab
record into a new window as a value: no shell restart, no parser replay, no scrollback loss and
no re-derivation of the working directory.

## 3. Window ownership (the load-bearing change)

Detachment needs a second native window owned by the same process, because a ConPTY session
cannot be moved to another process without duplicating pipe handles and serialising parser state.
The split that landed:

- `Workspace` (`src/app/workspace.rs`) owns everything window-local: the `Window`, `Gpu`,
  `Renderer`, active `TerminalState`/`Runtime`, `tabs`, `active_tab`, pointer/selection/scroll
  state, scale, row/column geometry, `tab_drag`, focus and close flags.
- `App` keeps process-wide state: settings, themes and store, theme/git/update workers, the
  privacy broker, the settings panels, and the tab-ID counter (so IDs stay unique across
  windows).
- **One workspace is mounted at a time.** `App::ws` is the mounted window and
  `App::background` holds the others; `App::mount(id)` swaps by `WindowId` so `window_event`
  always reaches the right workspace. This was chosen over indexing every field access because
  every existing engine path already operates on "the current window", and the swap is a few
  hundred bytes of pointer moves rather than a re-derivation of state. It also keeps the
  single-owner rule literal: exactly one workspace's engine is reachable at a time.
- `App::pump()` drains, ticks and requests redraws for the mounted workspace. `about_to_wait`
  mounts each background workspace in turn and pumps it, so a detached window keeps consuming
  shell output and closes when its session ends. `Pump::{Keep, Close, Exit}` distinguishes a
  window that must be destroyed from a process that cannot continue.
- Window creation is one function (`App::create_window`) shared by the first window and by a
  detached tab, so icons, GPU selection, renderer setup and font metrics cannot diverge.
- Font/scale changes call `rebuild_renderer`, which now re-applies to every workspace.
- A detached window inherits the source window's **physical** inner size. Its columns and rows
  were measured in those pixels; passing them as a logical size inflated the new window at
  non-100% DPI (observed at 150%: 2019 px wide instead of 1524 px).

**Boundaries preserved.** One thread still owns all mutable engine state; the renderer still
consumes owned snapshots and never mutates a grid; the engine still performs no I/O; the layout
engine stays pure. No new dependency is required.

**Known gaps, recorded rather than hidden:**

- Theme previews and theme picks that arrive after detachment apply to the mounted window; a
  detached window keeps the theme it was created with until it is re-created. Font and scale do
  propagate.
- `WindowEvent::ScaleFactorChanged` is still unhandled. That predates this change; dragging any
  window between monitors with different DPI does not rebuild its renderer until the next font
  change or restart.
- Each window currently creates its own `wgpu::Instance` (as the single window already did)
  rather than sharing one. Sharing is an optimisation, not a correctness requirement, and is
  deferred.

**Detach sequence.** Create the new workspace first, then move the tab into it, then destroy the
source workspace when its remaining sessions subsequently close. Creating first means a window-creation failure leaves
the tab exactly where it was, with a log entry and no data loss. A tab is never present in two
windows: the move happens only after the destination exists.

**Last-tab rule.** Detaching a window's only tab is refused: the gesture ends with no detach
affordance and the tab stays where it is. An empty source window would otherwise have to be
destroyed or repopulated with an unrequested shell, and neither matches what the user asked for.

**Privacy reveal.** `TabPrivacy` moves with the tab, and the reveal state is reset to concealed
on arrival so a detached window does not inherit a hover-revealed protected field from the old
window.

## 4. Verification

- 2026-10-23 completion: required check, strict all-target Clippy and full standard suite
  passed; explicit `detached_windows_drain_both_sessions_and_exit` desktop acceptance passed
  with two tail markers and two clean window closures. The initial missing-INFO fixture
  failure and the successful rerun are recorded in DONE.md. Physical input remains unverified.

- Feedback: pure tests for outline clamping at both strip edges, band/marker alignment with the
  drop gap at both ends, and unchanged rendering when no drag is active.
- Detachment: pure tests for the detach threshold and last-tab refusal, the detach scene (no
  destination band, release hint present), and `split_tab` moving the active or inactive record
  with its engine attached while leaving the source window with a valid active slot.
- Live run (debug build, `JUNA_STARTUP_TABS=2 JUNA_ACCEPT_DETACH=1`): the process created a
  second native window, logged `Detached tab 2; windows=2` / `source tabs=1`, both windows
  reported the same physical size (1524×1062) and both stayed responsive. Log:
  `juna-46864-custom-…-0.log`.
- Not verified: the pointer gesture itself and typing into the detached window. A pointer drag
  and interactive shell input cannot be driven from this harness, so the gesture and the
  surviving session still need recorded human review. The session is the same `Runtime` value
  moved between workspaces (proven by the unit test), not a restarted shell.
- `cargo check`, strict all-target Clippy and the standard suite pass before landing, with
  `CHANGELOG.md`, `DONE.md` and release notes updated per cycle.
