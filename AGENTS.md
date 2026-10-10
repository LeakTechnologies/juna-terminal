# AGENTS.md — Smith

Project-specific engineering rules for Smith. Binding on every agent session in this
repository. Where this file and the global `~/.config/opencode/AGENTS.md` conflict, this file
wins on project-specific matters; the global file still governs cost, commit protocol, and
model selection.

## 1. What Smith is

A native Windows terminal emulator and developer workspace, written in Rust.

Smith implements its **own** VT engine: parser, state machine, screen model, cursor,
scrollback, alternate screen, and control-sequence semantics.

## 2. Provenance — absolute constraint

**No borrowed terminal-engine implementation.** Alacritty, WezTerm, Kitty, Contour, and any
other terminal emulator are legitimate references for *observable behaviour* and *published
specifications*. Their source code is **not** a donor for our implementation.

This means:

- Do not copy or adapt parser, grid, scrollback, or renderer code from another terminal.
- Do not add `alacritty_terminal`, `wezterm-term`, or an equivalent engine as a dependency.
- Infrastructure crates — `winit`, `wgpu`, `windows`, `log`, Unicode data crates — are
  permitted. See §4 for the boundary test.
- Every significant design choice should be traceable to a public specification (DEC ANSI
  parser model, ECMA-48, xterm ctlseqs) or to measured behaviour of a reference terminal.

**Boundary test:** if a crate provides *standards data* or *platform access* (Unicode tables,
font metrics, Win32 calls, GPU APIs), it is infrastructure. If it provides *terminal
semantics* (interpreting escape sequences, managing a screen buffer), it is a borrowed engine
and is forbidden.

## 3. Design-First gate

Before writing or modifying code for any architectural or multi-file change, produce a short
Markdown design proposal and **wait for the design lead's approval**. Trivial, obviously-correct
changes (typos, a test that must pass, formatting) do not require a proposal.

A task is not complete because it compiles. It is complete when its acceptance criteria pass
**and** the evidence is reported.

## 4. Dependencies

- No dependency without a written justification naming the concrete requirement it serves.
- Prefer standard library over a crate when the standard library is adequate.
- Existing crates used only for standards data may be wrapped behind a Smith-owned pure API so
  the implementation can be swapped without touching call sites.
- Record every addition and removal in `DECISIONS.md`.
- Do not add an async runtime without a demonstrated requirement.

## 5. Architecture boundaries

These are load-bearing. Violating one is a defect even if the code works.

- **The renderer never parses VT, never holds engine state, and never mutates the grid.** It
  consumes an owned, read-only snapshot.
- **The Windows process backend never owns terminal screen state.** It moves bytes.
- **The engine never performs I/O.** It consumes bytes and produces state transitions.
- **Exactly one thread owns mutable engine state.** No `Arc<Mutex<Engine>>`, no interior
  mutability for engine state. Pass values.
- **The layout engine is pure.** BSP computation takes geometry and returns rectangles; it
  knows nothing about processes or pixels.

## 6. Byte-stream integrity

Terminal output is an ordered byte stream. **Dropping bytes is forbidden** — it corrupts
parser state permanently. Overload is handled with backpressure and explicit instrumentation,
never by discarding output. See `ARCHITECTURE.md` §3.0.

Any change to the transport path must keep the byte-exact round-trip test passing.

## 7. Unsafe code

Minimise. Where unavoidable (Win32/ConPTY interop, GPU surface access), every `unsafe` block
carries a `// SAFETY:` comment stating the invariant that makes it sound. An `unsafe` block
without that comment will be rejected.

## 8. Testing

- `cargo check && cargo clippy --all-targets && cargo test` must pass before any change lands.
- The parser must be **total**: no byte sequence may panic or loop forever.
- Engine transitions must be **deterministic**: identical input and initial state must yield an
  identical snapshot.
- Conformance tests are **shell-independent**. Never assert on one shell's output.
- A green build is build evidence, not proof of a functioning terminal. Visual, input, and
  state-synchronisation changes require recorded evidence or human review.

## 9. Honest reporting

- Never report a test as passing without having run it in this session.
- Never invent benchmarks, timings, adapter capabilities, or completed work.
- Record retractions explicitly when earlier claims turn out to be wrong — do not quietly
  rewrite history.
- Distinguish **verified**, **hypothesis**, and **assumption** in every report.
- Report blockers rather than working around them silently.

## 10. Scope discipline

Smith is a terminal first. Editor, agent drawer, transparency, and notifications must not
compromise terminal correctness. If a feature cannot be built without weakening the terminal,
it does not get built yet.

Conformance targets are declared explicitly in `ARCHITECTURE.md` §4.3. Anything outside them is
a scope change requiring approval, not a silent omission.

## 11. Commit protocol

Inherit the global rule: commit after every successful iteration. Additionally, every change
that affects behaviour or a structural boundary must update `ARCHITECTURE.md` (if the boundary
changed) and `DECISIONS.md` (if a decision changed) **in the same commit**.

No AI attribution of any kind in code, comments, or commit messages.

## 12. Pinned dependencies requiring manual review

`conpty-oxide` is pinned and must be upgraded **manually**. Before any version change:

1. Read the upstream release notes.
2. Re-run `cargo test --test conpty_acceptance` — quoting, process-tree reaping, shutdown
   without EOF, and sustained output.
3. Record the change in `DECISIONS.md`.

Automatic version bumps are prohibited. Replacement criteria are in `DECISIONS.md` §6.3:
an unresolved critical lifecycle defect, abandonment preventing a necessary fix, or a
demonstrated incompatibility with a supported Windows environment.

## 13. Unresolved

Read `DECISIONS.md` before starting work. D1 (toolchain) and D6 (font shaping) are open.
B3 (MSVC elevation) blocks D1. B7 is partially closed — adapter enumeration is done, but
surface creation and presentation are untested.