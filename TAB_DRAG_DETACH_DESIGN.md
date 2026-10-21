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

## 2. Drag-out detachment (designed)

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
The current `App` is one window's state plus process-wide services, so the split is:

- `Workspace` owns everything window-local: the `Window`, `Gpu`, `Renderer`, active
  `TerminalState`/`Runtime`, `tabs`, `active_tab`, pointer/selection/scroll state, scale,
  row/column geometry, `tab_drag`, focus and close flags.
- `App` keeps process-wide state: settings, themes and store, theme/git/update workers, the
  privacy broker, the settings panels, and the tab-ID counter (so IDs stay unique across
  windows). It holds `windows: Vec<Workspace>` and routes `WindowEvent`s by `WindowId`.
- One `wgpu::Instance` is created per process; each workspace keeps its own surface, device use
  and renderer. An assumption to verify during implementation: the existing `Gpu::new` can be
  split into instance creation plus per-window surface configuration without changing the
  present path.
- Settings changes that affect layout (font size, family, scale) re-fit every window.

**Boundaries preserved.** One thread still owns all mutable engine state; the renderer still
consumes owned snapshots and never mutates a grid; the engine still performs no I/O; the layout
engine stays pure. No new dependency is required.

**Detach sequence.** Create the new workspace first, then move the tab into it, then destroy the
source workspace if it has no tabs left. Creating first means a window-creation failure leaves
the tab exactly where it was, with a log entry and no data loss. A tab is never present in two
windows: the move happens only after the destination exists.

**Last-tab rule.** Detaching a window's only tab is refused: the gesture ends with no detach
affordance and the tab stays where it is. An empty source window would otherwise have to be
destroyed or repopulated with an unrequested shell, and neither matches what the user asked for.

**Privacy reveal.** `TabPrivacy` moves with the tab, and the reveal state is reset to concealed
on arrival so a detached window does not inherit a hover-revealed protected field from the old
window.

## 4. Verification

- Feedback: pure tests for outline clamping at both strip edges, band/marker alignment with the
  drop gap at both ends, and unchanged rendering when no drag is active.
- Detachment: pure tests for the detach threshold, cancellation, and that tab records and their
  engine/runtime move without restart; recorded manual evidence for the gesture, the new window
  and the surviving session, because a pointer gesture and a native window cannot be proven by a
  unit test.
- `cargo check`, strict all-target Clippy and the standard suite must pass before each stage
  lands, with `CHANGELOG.md`, `DONE.md` and release notes updated per cycle.
