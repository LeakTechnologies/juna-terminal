# Juna — Decision Register

## 2026-10-15 — repair ConPTY protected-field markers

Approved PRIVACY_REPAIR_DESIGN.md replaces new OSC span emission with standard SGR colour pairs.
Measured host evidence: zero of five OSC-tagged shade cells retained association through ConPTY,
while five of five truecolour-marked shade cells retained exact attributes. Retract earlier
end-to-end reveal claims: raw-token presence and synthetic engine tests did not prove this path.
Full random 128-bit tokens remain in IPC; display markers occupy a narrower 48-bit space, checked
for collision and never reused during the broker lifetime. Reserve at most 65,536; fail closed
on exhaustion or randomness failure. Retain at most 128 values per tab, with reserved expired
marker metadata for safe normalization. The app normalizes known marker colours in an owned
snapshot, including report-arrival invalidation. Engine/renderer/backend boundaries stay intact.
Masks reveal ordinary display width as requested; explicitly typed passwords use eight asterisks.
Junafetch collects no passwords and this adds no automatic password/terminal-wide detection.
Frontend 0.4.0 is rebuilt and repinned. No dependency change or worker polling was introduced.
Older Windows and other-machine behaviour require separate acceptance.


## 2026-10-15 — command-line updates

Approved CLI_UPDATE_DESIGN.md adds --check-update and --update, optional temporary channel
selection and --help. Reuse the existing verified release worker and transactional installer.
Checks use no GPU/session. External installation requires interactive default-No confirmation;
matching-executable Juna windows must close before handoff. Enumerate fixed-title Juna windows
and verify their query-only process image paths; existing replacement retries/rollback remain.
Current-user authenticated IPC gains an update request carrying a validated channel, without
installation authority. The hosting window verifies/downloads and presents its existing
Cancel-first confirmation. A request never terminates sessions or approves installation.
Verify the host process uses the same executable before sending the request. Channel overrides
are displayed separately and do not alter saved preferences. Add the application directory only
to child-session PATH. Preserve redirected pipe/file standard handles across parent-console
attachment. These use existing Windows platform features; no dependency or VT-engine change.

## 2026-10-12 — application title and Fastfetch

Keep the native window title fixed as Juna Terminal; preserve OSC titles in engine metadata.
Fastfetch is an external user-installed program, with no new Juna dependency or startup hook.
Windows reports Fastfetch 2.69.0 installed. The initial missing-install claim was incorrect:
the restricted environment and running process PATH differed from registered Windows PATH.
Verified PowerShell through Juna's ConPTY/engine ran Fastfetch OS/CPU/GPU/Memory output and
returned the success marker with exit 0 and zero dropped bytes after refreshing the test process PATH.


## 2026-10-12 contrast and theme follow-up

Approved: minimum text contrast, unchanged stored background colors, selected-font diagnostics,
and the three-part THEME_DESIGN.md proposal (Mocha default, menu/settings, 20 presets, custom JSON).

Verified rendering defect before repair: a GPU sRGB texture test read `[110,113,127]` for the
Dracula background `[40,42,54]`. sRGB-coded palette channels were submitted as linear shader
colors, then encoded again by the surface. The earlier non-sRGB acceptance frame missed this.
The renderer now converts palette/clear/emoji color channels for the target encoding. Contrast
changes only presentation foregrounds after reverse-video resolution, targeting 4.5:1 for nominal
opaque text; dim and antialiasing still affect final pixel coverage. Existing readable colors
remain unchanged. `JUNA_MIN_CONTRAST=1` disables correction pending the settings UI.

The GPU test now covers sRGB and non-sRGB surfaces and passes for exact palette background bytes.
IBM Plex Mono selection is logged when info diagnostics are enabled. Theme palette colors can
still be intentionally pastel, and an application's truecolor prompt background is not replaced
by selecting another terminal theme.

Dependency justification for the newly approved theme/settings implementation: `serde` provides
typed, validated serializable theme/settings structures; `serde_json` provides correct versioned
JSON parsing and escaping. Handwriting JSON parsing is not justified. Both are data infrastructure,
not terminal semantics, and no async runtime or terminal-engine dependency is introduced.

Implemented after explicit design approval: Mocha default, 20 immutable starter palettes, a live
right-click menu, searchable selector, font/contrast settings, and custom-theme create/edit/save.
Storage is versioned JSON outside the engine. The palette collection revision and notices are
recorded in licenses/README.md; source URLs remain in the data. This is a curated set of established
themes, not a fabricated cross-terminal popularity ranking. Future export is intentionally deferred.

Locked direct serialization dependencies: existing graph version `serde` 1.0.229 with derive
enabled, and `serde_json` 1.0.151; its added serialization helpers are `itoa` 1.0.18 and `zmij` 1.0.23.
ConPTY and the terminal engine boundary are unchanged. A temporary resolution at serde 1.0.228
was corrected back to the existing 1.0.229 graph version before landing.

Verification in this session: settings/custom theme round trips, invalid/unsupported/duplicate
records, read-only-file preservation, preview cancellation, and engine-cell isolation pass.
GPU evidence covers all 20 default backgrounds, sRGB fidelity, and the actual overlay drawing.
Real-window tests verify output reaches the engine with theme/settings panels open and the shell
still exits. The first menu-output fixture timed out because its delay ran inside every cmd loop
iteration; grouping the loop explicitly corrected the fixture. Logs established output was flowing
during the timeout; it was not treated as a terminal freeze. The corrected window tests pass.

## 2026-10-12 approved readiness repairs

The design lead approved all three stages of AUDIT.md and pushing verified changes.
The following infrastructure requirements justify dependency changes before implementation:

- Add exact `unicode-width` 0.2.2: generated Unicode display-width data, including emoji
  sequences, replaces incomplete hand-maintained ranges behind `vt::width`.
- Add exact `unicode-segmentation` 1.13.3: UAX #29 extended grapheme boundaries let streamed
  scalars form combining/ZWJ clusters without importing terminal semantics.
- Use the already-installed swash shaper for per-cluster OpenType shaping. No additional
  shaper dependency is needed. Typography operates inside fixed terminal cell allocations.
- Extend the existing Windows API features for Unicode clipboard and visible error dialogs;
  these are platform access, not terminal semantics. Clipboard escape requests remain gated.
- Remove unused optional `winres` and the inactive UTF-16 `src/build.rs`. There is no functioning
  resource-build pipeline to preserve; icon embedding is deferred to a reviewed build change.

`conpty-oxide` stays at 0.1.2, and its lockfile resolution must not change.
No async runtime, borrowed terminal engine, Linux backend, panes, editor, or drawer is added.
Historical claims below (no window, zero unsafe, green gates) describe earlier revisions,
not the current code. Current verification will be recorded after the repair gates execute.

Status: **Window, GPU renderer, VT engine, layout library, and live shell integration implemented.
Readiness repairs are implemented under the approved 2026-10-12 design, with acceptance evidence below.**

Last updated: 2026-10-12 — see the readiness repair record above and TERMINAL_TESTING.md.
Build evidence below dated 2026-10-11 is historical and is not reused as current verification.

### Readiness baseline verification — 2026-10-12 (before theme follow-up)

- `cargo check --offline`: passed with no warnings.
- `cargo clippy --all-targets --offline`: passed with no warnings after cleanup.
- `cargo test --offline --quiet`: **165 passed** (152 library, four ConPTY, four elevation,
  five shell integration); two desktop/GPU tests ignored by default and explicitly run separately.
- GPU acceptance: passed; captured `docs/acceptance/render.png`, and compared unchanged row pixels
  while the next row changed. CJK fallback and shaped combining text are visible in the capture.
- Actual-window acceptance: cmd exit 0 and PowerShell exit 7 both present frames and close;
  runtime tests verify tail bytes and joined reader/writer workers. Vulkan on RTX 3060 was selected.
- `cargo build --release --bin juna --offline`: passed. The updated executable is
  `target/release/juna.exe`.

The original audit's lint/test failures are resolved. The first window evidence attempt failed
because the inherited warn-only log filter hid info lines; an explicit test log filter corrected
that test, and the rerun passed. A later repeat exposed a teardown race: joining the reader before
the writer finished left no reader handle for the next tick's readiness check. The shutdown now
joins each completed worker independently and treats an absent handle as already joined;
a deterministic regression covers that state. Earlier successful runs did not establish that
all shutdown interleavings worked. No claim of physical keystroke, clipboard, mixed-DPI, real editor
or pager acceptance is made; these remain the human review matrix in TERMINAL_TESTING.md.

D3 is now implemented with Unicode 17.0.0 in both pinned standards-data crates. D6 uses existing
swash 0.2.10 for cluster shaping; full bidi/contextual run layout is not claimed. The Windows API
features serve Unicode clipboard ownership/memory and failure dialogs. Removing unused winres
also removed its unused toml 0.5.11 lockfile entry. ConPTY remains resolved at 0.1.2;
both manifest entries now use an exact version requirement to enforce the manual-upgrade rule.

The previous status line, *"Phase 0 reconnaissance complete. Engine coding remains unauthorized"*, is
**retracted as stale**. It described the state on 2026-10-10. The engine coding it referred to has
since been authorized and built (§6.10, §6.12, §6.13).

---

## 1. Verified environment facts

Established by direct inspection and direct execution. Not inferred, not assumed.

| Fact | Value | How it was established |
|---|---|---|
| Working directory | `E:\Development\Juna-terminal` | `Get-Location` |
| Repository state | **Not a git repository.** No `.git`, no commits. | `git status` → `fatal: not a git repository` |
| Prior contents | Directory was **completely empty** | `Get-ChildItem -Force` → no output |
| Active toolchain | `stable-x86_64-pc-windows-gnu` 1.97.0 (`2d8144b7`) | `rustc --version --verbose` |
| GNU toolchain links | **YES** — `rustc probe.rs` built and ran, printed `link ok` | direct compile + execute |
| MSVC toolchain links | **NO** — `linker 'link.exe' not found` | `rustc +stable-x86_64-pc-windows-msvc probe.rs` |
| VS installation | `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools` exists, but **no `VC\Tools\MSVC` directory**, and `vswhere -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64` returned nothing | `Get-ChildItem`, `vswhere` |
| Windows SDK | **Absent** — no `Windows Kits\10\bin` | `Get-ChildItem` |
| `pwsh` (PowerShell 7) | **NOT INSTALLED.** Absent from Program Files, WindowsApps, and PATH | `Get-Command`, `Test-Path` on three known paths |
| `powershell.exe` | Present, version **5.1.26100.9444** | `$PSVersionTable` |
| CPU | AMD Ryzen 5 2600, 6 cores | `Win32_Processor` |
| GPU | NVIDIA GeForce RTX 3060 **+ `Parsec Virtual Display Adapter`** | `Win32_VideoController` |
| OS | Windows NT 10.0.26200.0 | `OSVersion.VersionString` |
| Local `AGENTS.md` | **Did not exist.** Only the global file at `~/.config/opencode/AGENTS.md` applied. | `Glob **/AGENTS.md` |
| `CARGO_HOME` | `E:\cargo` (registry sources inspected from here) | `$env:CARGO_HOME` |

### Discrepancies

1. Global `AGENTS.md` states "Ryzen 5 2600X"; the machine reports **2600**. Register should be corrected.
2. **`Parsec Virtual Display Adapter` is present** alongside the RTX 3060. Virtual display adapters
   are a known source of wgpu adapter-selection and surface-presentation failures in remote
   sessions. Must be enumerated deliberately in Phase 1, not discovered in Phase 3.

---

## 2. Honest build status

**Re-measured 2026-10-11** under `stable-x86_64-pc-windows-gnu` 1.97.0 (`2d8144b7`). Every row was
observed in that session. Nothing here is carried forward from an earlier run.

| Check | Result |
|---|---|
| `cargo check --all-targets` | **PASS** — 2 warnings, the known clippy artifact (§6.13) |
| `cargo clippy --all-targets` | **PASS** — zero warnings beyond that artifact |
| `cargo test` | **PASS — `125 passed; 0 failed`** (121 unit + 4 ConPTY acceptance) |
| `cargo build --release` | **PASS** — `target\release\juna.exe`, 1,176,231 bytes |
| Binary executes | **PASS** — prints, exit code 0 |
| `alacritty_terminal` / `vte` in `Cargo.lock` | **ABSENT — confirmed** |
| Direct `tokio` / `serde` / `serde_json` / `anyhow` deps | **REMOVED — confirmed** |
| GPU adapters enumerated | **YES** — five adapters, §6.4 |
| ConPTY child spawned and process tree reaped | **YES** — `process_tree_is_reaped_on_teardown` |
| Sustained output byte-exact | **YES** — `sustained_output_arrives_byte_exact` |
| Argument vector byte-exact | **YES** — `quoting_preserves_the_argument_vector_exactly` |
| Reader unblocks on shutdown without EOF | **YES** — `shutdown_unblocks_the_reader_within_the_bound` |
| **Window opened** | **NO** |
| **GPU device and surface created** | **NO** |
| **Interactive shell round-trip** | **NO** |

### Stale rows in the previous revision of this table — corrected

This table previously asserted `cargo test` was *"PASS but vacuous — `0 passed; 0 failed`. Zero tests
exist"*, and that GPU enumeration, ConPTY spawn, and process cleanup were all `NO`. Those rows were
true on 2026-10-10 and are false now. Correcting them here rather than rewriting history silently.

**The build is evidence, not a functioning terminal.** `src/main.rs` is three lines: a `println!` and
nothing else. There is no window, no surface, no live shell. A green gate does not narrow that gap.

---

## 3. Correction log — retracted claims

| Retracted claim | Why it was wrong |
|---|---|
| "Test Results: PASSED", rows marked ✅ | No tests had been run. |
| VT "families" named `BEEP`, `BLINK`, `QUIT`, `MULTI`, `MOVE` | Fabricated taxonomy. Real families are C0/C1/ESC/CSI/OSC/DCS. |
| "Secondary Display: separate terminal instance on secondary monitor" | Wrong. Alternate screen is a second buffer inside the same terminal. |
| "Cursor Effects: blink, blink delay, flash duration" | Invented fields, no VT basis. |
| `ConPTYInterface` pseudo-code | Non-compiling pseudo-code presented as an interface. |
| **"Coalesce-and-continue: drop stale bytes when the channel fills"** | **Formally rejected by the design lead.** Dropping bytes corrupts VT stream state permanently. Replaced by the ConPTY-flow-control design in `ARCHITECTURE.md` §3.0. |
| "Row damage is cheaper because glyph rasterization dominates" | Stated as fact; it is a hypothesis. Relabelled as such in §3.5 and made a Phase 3 profiling obligation. |
| **"`conpty` 0.7's `Drop` never terminates the child — High severity"** | **Refuted by spike.** `pid=16984` was gone 1.2 s after `Drop`. `ClosePseudoConsole` does reap the direct child. Downgraded to Medium and re-scoped to grandchildren, which were not tested. |
| **"Prefer direct Win32 bindings for D5"** | **Superseded by evidence.** `conpty-oxide` already provides job objects, owned halves, and close-hang regression tests. Recommendation reversed. See §6. |

---

## 4. D1–D7 status

| # | Decision | Status | Note |
|---|---|---|---|
| **D1** | Toolchain | **BLOCKED** | MSVC is intended, but `link.exe` is absent. See §7 blocker **B3**. Cannot pin until the toolchain can link. |
| **D2** | Async runtime | **DONE** | Tokio removed. std threads + explicit double-buffered handoff. Evidence in §5. |
| **D3** | Unicode data | **RESEARCHED — awaiting decision** | Comparison in §6. |
| **D4** | Scrollback reflow | **SPECIFIED** | `ARCHITECTURE.md` §3.4. |
| **D5** | ConPTY binding | **APPROVED — `conpty-oxide` 0.1.2** | `conpty` 0.7 removed from the manifest. Safeguards and replacement criteria in §6.3. |
| **B6** | wgpu features | **RESOLVED** | See §6.1. | — |
| **D6** | Font rasterization | **CLOSED — zeno primary, DirectWrite deferred** | `fontdb` + `swash`/`zeno` behind a Juna trait. Shaper choice (harfrust vs rustybuzz) still open. See §6.8–6.11. |
| **D7** | Damage model | **DIRECTION SET** | Row damage + full redraw. `ARCHITECTURE.md` §3.5. |

---

## 5. D2 — dependency reduction evidence

`tokio`, `serde`, `serde_json`, and `anyhow` were removed: none had a consumer, which violates
the "no dependencies without justification" rule.

Added, with justification: `log` + `env_logger`. The new mandatory diagnostics requirements
(GPU presentation failure must yield a useful diagnostic, not a blank window; stream overload
must be explicit) cannot be met without a logging facade. `env_logger` keeps it to two small,
widely-used crates.

Current manifest:

```toml
[dependencies]
winit = "0.30"
wgpu = "29"
conpty = "0.7"      # PROVISIONAL — see D5
log = "0.4"
env_logger = "0.11"
```

**Honest caveat:** `Cargo.lock` still shows 282 packages and still contains `tokio` and `serde`
entries. Investigated — these are **not** ours. `cargo tree -i tokio --target all` shows
`tokio` arriving via `wgpu → wasm-bindgen-futures`, a WebAssembly-only path never compiled on
Windows; `serde` resolves to nothing at all. The lock is dominated by wgpu's cross-platform
universe (Metal, GL, WebGPU, Emscripten), not by our manifest. Removing Tokio did not shrink
the dependency graph — it removed a *direct* dependency and a large compile-time cost on the
paths we would have used.

Still unanswered: whether `wgpu` should be `default-features = false` with DX12+Vulkan pinned
explicitly, to cut build time and remove backend ambiguity on a machine that already has a
virtual display adapter.

---

## 6. D3 and D5 — research findings

### D3 — Unicode width and grapheme data

| Option | Provides | Assessment |
|---|---|---|
| `unicode-width` | `UnicodeWidthChar/Str::width()`, East Asian Width based | Small, maintained, already in the tree transitively. No grapheme segmentation. |
| `unicode-segmentation` | `graphemes()`, UAX #29 extended grapheme clusters | Small, maintained, the de-facto standard for exactly the problem in `ARCHITECTURE.md` §2.1. |
| Internal generated tables | Everything | Full control and pinning, but requires owning a generator plus an update process, and grapheme-break rules evolve with each Unicode release. |

**Finding:** the original-engine directive is about *terminal engine* code — parser, state
machine, grid, scrollback, alternate screen. Unicode standards data is reference data, not a
terminal implementation. Reimplementing grapheme segmentation from scratch is out of scope and
would be a correctness liability.

**Recommendation:** adopt `unicode-width` + `unicode-segmentation` **behind a Juna-owned pure
API** (`Juna_term::text::width(c) -> Columns`, `graphemes(s) -> impl Iterator<Item = &str>`),
with versions pinned via `Cargo.lock` and the corresponding Unicode version recorded in the
docs. The wrapper preserves the ability to swap implementations or vendor tables later without
touching call sites. Caveat to record: neither crate exposes its source Unicode version at
runtime, so pinning is by lockfile, not by introspection.

### D5 — ConPTY binding comparison

#### `conpty` 0.7.0 — source inspected

Read `src/lib.rs` and `src/process.rs` directly. Defects found, each of which touches a
non-negotiable requirement:

| # | Finding | Location | Severity |
|---|---|---|---|
| a | `build_commandline` joins program and args with bare spaces and **performs no argument quoting**. Any argument containing a space or quote is corrupted. | `process.rs:336-346` | **High** — breaks launching a shell with arguments |
| b | `Drop` calls `ClosePseudoConsole` but does **not** call `TerminateProcess`, and there is no job object. | `process.rs:165-180` | **Medium** — see spike result: refuted for the direct child, unresolved for grandchildren |
| c | Blanket `unsafe impl Send` **and** `Sync` on a type owning raw `HANDLE`s, while `resize` takes `&mut self`. Defeats the compiler's ownership checking. | `process.rs:193-194`, `125` | **Medium** — fights our ownership rule |
| d | `enableVirtualTerminalSequenceProcessing()` mutates the **host process's** console mode and its error is discarded via `let _ =`. | `process.rs:196-208`, `439` | **Medium** — side effect on the parent, silent failure |
| e | Depends on `windows` **0.54** while `wgpu` pulls `windows-sys` **0.61/0.62**. Two incompatible `HANDLE`/`HRESULT` type universes coexist; handles cannot cross without casts. | `Cargo.toml`, `Cargo.lock` | **Medium** |
| f | `spawn()` convenience wrapper hardcodes `cmd /C` — unusable for an interactive shell. | `lib.rs:52-58` | Low — we would use `Process::spawn` anyway |
| g | The `PROC_THREAD_ATTRIBUTE_LIST` buffer is intentionally leaked and reclaimed in `Drop` via `Box::from_raw`; leaks if `Drop` never runs. | `process.rs:271-272`, `174` | Low |

#### `conpty-oxide` 0.1.2 — metadata from crates.io API

- Created **2026-07-31** — roughly two months old. Three versions, ~1 800 downloads, **single
  maintainer**. Bus factor of one.
- Feature layout is genuinely good and aligns with D2: `blocking` is the **default**;
  `tokio` and `tracing` are opt-in.
- MIT OR Apache-2.0, `rust-version` 1.75, 9 065 lines of Rust.
- Positioned as "correctness-first".

#### Direct `windows` API — not yet implemented

- Reuses the `windows` crate generation already in the tree, eliminating finding (e).
- Full control over argument quoting, cancellation, and job objects, addressing (a), (b), (d).
- Cost: we own every `unsafe` block and must satisfy the `// SAFETY:` rule. Realistic estimate
  300–500 lines including job-object lifecycle and cancellation.

### 6.1 B6 — wgpu 29 backend features, measured

Read verbatim from the vendored `wgpu-29.0.4/Cargo.toml`. Defaults are:

```
default = ["std", "parking_lot", "dx12", "metal", "gles", "vulkan", "wgsl", "webgpu"]
```

Windows-relevant feature names are exactly `dx12`, `vulkan`, `gles`, `wgsl`, plus the
platform-neutral `std`, `parking_lot`, `counters`, `trace`, `serde`, `custom`, `static-dxc`,
and target-specific `metal`, `web`, `webgl`, `naga-ir`, `glsl`, `spirv`, `vulkan-portability`,
`fragile-send-sync-non-atomic-wasm`, `noop`, `strict_asserts`.

Applied configuration:

```toml
wgpu = { version = "29", default-features = false,
         features = ["std", "dx12", "vulkan", "gles", "wgsl"] }
```

**Removed:** `metal` and `wgpu-core-deps-apple` are gone from the resolved graph (confirmed
absent from `Cargo.lock`). `metal` cannot help a Windows build.

**Deliberately kept `gles`.** The lead's instruction was not to assume DX12+Vulkan resolves
every adapter and presentation path. Since this host carries a `Parsec Virtual Display Adapter`
alongside the RTX 3060, the GLES/ANGLE path is retained as a fallback until adapter enumeration
(B7) proves DX12+Vulkan sufficient. Dropping it later is a one-line change; keeping it costs
build time only.

**Kept `wgsl`** — required for shader compilation.

**Dependency consequence, measured:** `webgpu` pulls `wasm-bindgen`, `js-sys`, `web-sys`, and
`wasm-bindgen-futures`; the last of those depends on **Tokio**. Removing wgpu's `webgpu` default
therefore removed wgpu's Tokio path entirely.

**Correction to §5.** I previously attributed the residual Tokio entry to `wgpu`. After the
feature change, `cargo tree -i wasm-bindgen-futures --target all` shows the surviving path is:

```
wasm-bindgen-futures v0.4.79
└── winit v0.30.13
```

So the chain is **`winit` → `wasm-bindgen-futures` → Tokio**, not wgpu. This is winit's
cross-platform support and is outside our control. Critically, `cargo tree -i tokio` **without**
`--target all` prints "nothing to print": Tokio is in the lock graph but is **not compiled for
`x86_64-pc-windows-*`**. The GNU build completes without it.

**Honest caveat on metrics.** `Cargo.lock` package count is a poor measure here — target-gated
dependencies for other platforms persist in the lock regardless of feature selection. The
meaningful figure is the 224 crates actually resolved for `x86_64-pc-windows-gnu`. No claim of a
large build-time reduction is made; that was not measured.

### 6.3 D5 — approved, with supply-risk safeguards

**Approved by the design lead: `conpty-oxide` 0.1.2. `conpty` 0.7 removed from the manifest.**
Juna owns a thin wrapper so the engine/transport boundary stays explicit.

The concern is supply and maintenance risk, not correctness. Three safeguards are recorded in
place of reimplementing several hundred lines of Win32 `unsafe`:

1. **Pin and regress.** Version stays pinned via `Cargo.lock`. Juna's own acceptance tests in
   `tests/conpty_acceptance.rs` cover the four areas that matter and are independent of
   upstream's suite:

   | Test | Guards |
   |---|---|
   | `quoting_preserves_the_argument_vector_exactly` | argument quoting — the defect that disqualified `conpty` 0.7 |
   | `process_tree_is_reaped_on_teardown` | process-tree cleanup — grandchildren must not survive |
   | `shutdown_unblocks_the_reader_within_the_bound` | pseudoconsole shutdown must not rely on pipe EOF |
   | `sustained_output_arrives_byte_exact` | sustained child output is neither lost nor reordered |

   All four pass as of this commit.

2. **No automatic upgrades.** `conpty-oxide` is reviewed manually before any version change.
   Upstream release notes must be read and the four regression tests re-run. Automatic bumps
   are prohibited.

3. **Replacement criteria.** The binding is replaced if any of the following occurs:
   - an unresolved **critical lifecycle defect** — leaked processes, lost output, hung shutdown;
   - **abandonment or unmaintenance** that prevents a necessary fix from landing;
   - a **demonstrated incompatibility** with a supported Windows environment.

   Replacement means either a vetted successor or a Juna-owned Win32 implementation, decided
   per the Design-First gate. It is not grounds to fall back to `conpty` 0.7.

### 6.5 D6 — `crossfont` rejected by doctrine

Proposed as a D6 option and **rejected**. `crossfont` 0.9.0 declares
`repository = github.com/alacritty/crossfont.git`, is published by `chrisduerr` (an Alacritty
maintainer), and has ~1.97 M downloads. It is Alacritty's font-loading and rasterization
component.

`AGENTS.md` §2 forbids adapting "parser, grid, scrollback, or **renderer** code from another
terminal". `crossfont` is part of Alacritty's renderer. Publishing it as a standalone crate does
not change its provenance, and the boundary test in §2 cannot be satisfied by repackaging: the
code belongs to the Alacritty organization and its maintainers.

A narrower reading is available — `crossfont` provides font metrics and rasterization, which §2
classifies as *infrastructure*, and it does not interpret escape sequences or manage a screen
buffer. That reading was considered and rejected, because the *renderer* clause is unconditional
and adopting Alacritty's own rasterization component would put Juna on Alacritty's renderer
stack — precisely the dependency the original-engine doctrine exists to prevent.

**Settled by doctrine, not preference. Not to be revisited without an explicit doctrine change.**

### 6.6 D6 — `cosmic-text` rejected for a terminal grid

Factual correction to the option as proposed: `cosmic-text` 0.16.0 **no longer uses
`rustybuzz`**. Its shaping dependency is `harfrust` ^0.4.1 (HarfBuzz-compatible), and `swash`
has become optional.

Confirmed dependency set: `fontdb`, `skrifa`, `harfrust`, `unicode-linebreak`, `unicode-bidi`,
`unicode-script`, `unicode-segmentation`, `linebender_resource_handle`, `self_cell`, `rangemap`,
`rustc-hash`, `smol_str`, `bitflags`, `log`, plus optional `syntect` and `cosmic_undo_2`.

Rejected on architectural grounds. `unicode-linebreak` and `unicode-bidi` implement paragraph
layout for continuous prose. Juna has a **fixed cell grid** in which every glyph's position is
already known, so that machinery cannot produce information we do not have — it can only add
cache and allocation cost on the render hot path. cosmic-text is a sound text engine and a poor
match for a terminal.

### 6.8 D6 — pure-Rust rasterization spike (zeno/swash), measured

`examples/font_spike.rs`, release build. Host fonts enumerated via `fontdb`.

#### Environment

Nine monospace families installed: **Consolas, Courier New, IBM Plex Mono, Lucida Console,
MS Gothic, NSimSun, SimSun-ExtB, SimSun-ExtG, VCR OSD Mono**.

**Cascadia Mono and Cascadia Code are not installed.** The intended default could not be tested.

Consolas selected as the fallback candidate. Metrics at size 16: `units_per_em = 2048`,
`glyph_count = 3031`, `is_monospace = true`, `ascent = 1521.0`, `descent = 527.0`,
`leading = 350.0`, `cap_height = 1307.0`.

#### Finding 1 — font fallback is mandatory, not optional

| Case | Consolas |
|---|---|
| Latin `A` | gid 4, advance 1126, bitmap 9x11 |
| Latin `g` | gid 137, advance 1126, bitmap 9x12 |
| `e` + U+0301 | gid 135, advance 1126, bitmap 8x9 |
| CJK 漢 | **NO GLYPH** |
| Emoji 😀 | **NO GLYPH** |
| Emoji ZWJ family | **NO GLYPH** |
| Devanagari क | **NO GLYPH** |

Across all nine installed monospace families: **every** family covers Latin and the base of the
combining sequence. Only **MS Gothic** and **NSimSun** cover CJK. **No installed monospace font
covers emoji or Devanagari at all.**

This is the most decision-relevant result of the spike. A terminal that resolves glyphs against
a single font will render tofu for CJK, emoji, and Indic text — which is most non-Latin content
a developer encounters. Cross-font fallback is a **core requirement of the renderer**, not a
polish item, and it must be designed in from the start rather than added after tofu appears.

#### Finding 2 — performance is adequate, provided the API is used correctly

| Measurement | Result |
|---|---|
| cmap lookup, `Charmap` hoisted and reused | **11.6 ns/lookup** |
| cmap lookup, `Charmap` re-materialized per call | 48.0 ns/lookup |
| Rasterization, `Render` + `Image` reused | **2.83 µs/glyph** |
| Rasterization, `Render` rebuilt + `Image` allocated per call | 3.07 µs/glyph |

Allocation overhead is ~1x and therefore **not** worth optimizing; the meaningful lever is
hoisting `Charmap`. At 2.83 µs, rasterizing a full 80x40 grid costs ~9 ms once. With a glyph
cache this is a startup cost, not a per-frame cost. Conclusion: **zeno is fast enough for a
terminal provided glyphs are cached and `Charmap` is held, not rebuilt.**

#### Methodology errors made and corrected

Both were mine, and both would have produced a wrong decision:

1. **First benchmark ran in debug mode.** It reported 1878 ns/lookup and 44.6 µs/glyph — a 100x
   overstatement. Release build: 11.6 ns and 2.83 µs. Any future timing work here must assert
   `--release`.
2. **First benchmark measured API misuse.** `font.charmap()` re-materializes the character map
   on every call, so a per-lookup call pattern measures table re-lookup, not lookup. The naive
   and hoisted figures are both reported above so the gap is visible.

#### Finding 3 — cmap-only lookup is provably insufficient

A ZWJ emoji family reports a present glyph for its first codepoint yet **cannot be rendered**
without shaping; the same is true of the U+0301 mark, which needs mark attachment rather than
independent placement. A cmap-only fast path is therefore correct for Latin, CJK, and symbols,
but shaping is **required** for emoji and Indic, not merely desirable.

#### Status

**D6 is NOT closed.** The DirectWrite half of the comparison — hinting, ClearType subpixel
rendering, glyph metrics agreement, and OS-level fallback behaviour — has **not** been
implemented or measured. The zeno half is complete.

The pure-Rust path is confirmed **viable**, not confirmed **optimal**. The remaining open
question is whether unhinted grayscale output is acceptable for Juna's default rendering, which
is a visual judgement that measurements alone cannot settle.

### 6.10 Gap resolutions — all adopted

The six specification gaps were resolved by the design lead (`ARCHITECTURE.md` §2.8), and the four
consequences raised by the implementation review were subsequently **adopted in full**. This
section is closed.

| ID | Subject | Resolution |
|---|---|---|
| **D10.1** | Cell content and interning | `CellContent::Char(u32)` inline for single scalars; `Arc<str>` reserved for multi-codepoint clusters, drawn from a **mandatory interner**. Interning is required, not optional — without it the choice allocates per cell and reverses §2.1's invariant. |
| **D10.2** | Snapshot boundary | `Snapshot` owns a fresh per-frame copy of **damaged rows only**. No `Arc` or view into engine memory. The renderer cannot outlive the frame's data. |
| **D10.3** | Scrollback budget | Bounded by **bytes** (default 64 MiB), not lines. Line count derived as `budget / (cols * size_of::<Cell>())`, floored at a small minimum. Memory stays flat as windows widen. |
| **D10.4** | Thread ownership | Engine type is **`!Sync` and `Send`**, enforced by a private non-`Sync` marker. §3's single-owner rule becomes a compiler guarantee, not a convention. |

`ARCHITECTURE.md` §2.8.1–§2.8.4 carry the normative detail. §2.3 was updated so the scrollback
section no longer contradicts D10.3.

**Consequence:** the `src/vt/` specification is complete. No known unresolved contract questions
remain for the parser, grid, cursor, scrollback, or engine/renderer boundary. Implementation
still requires explicit design-lead authorization.

### 6.12 VT engine core — implemented

Authorized and built under `src/vt/`. 89 engine tests, 121 total, green in debug and release.

| Module | Contents |
|---|---|
| `vt::parser` | Byte-driven FSM over the DEC ANSI model, UTF-8 decoding, totality fuzz |
| `vt::grid` | Cells, cursor, margins, tabs, deferred-wrap latch, resize |
| `vt::scrollback` | Byte-bounded history per D10.3 |
| `vt::cell` | `CellContent`, `Color`, `Attrs` (u32 bitfield), `Interner` |
| `vt::width` | Pure width/cluster API |
| `vt::TerminalState` | Binds parser to grid; `!Sync` and `Send` per D10.4 |

#### Bugs the tests caught

1. **`ESC [` was dispatched as an escape final.** `[` is `0x5B`, inside the `0x40..=0x7E` final
   range, so **every CSI sequence was misparsed**. This alone would have broken the terminal.
2. **`ESC 7` / `ESC 8` (DECSC/DECRC) were misread as CSI parameters.** `7` is `0x37`, inside
   the parameter range. The Escape state must treat `0x30..=0x7E` as finals; only `ESC [` is
   a CSI introducer.
3. **`flush_utf8` ran at the end of every `advance()`.** A multi-byte scalar routinely spans two
   chunks, so **every non-ASCII character was corrupted** into replacement characters.
   Flushing is now an explicit `Parser::flush()`, called only at a true end of stream.
4. **`ground()` handled printable ASCII without checking for a pending UTF-8 sequence**, so a
   continuation byte `0x80..=0x9F` was executed as a C1 control instead of being consumed.
5. **`ESC` inside an OSC/DCS string was appended to the string** instead of restarting the
   sequence, so `ESC \` (ST) could never terminate a string.
6. **Wide characters advanced the cursor twice** — once manually, once through
   `advance_cursor` — leaving the cursor one column too far right.
7. The `ZERO` width table was **unsorted**, silently breaking binary search: U+FE0F variation
   selectors returned width 1 instead of 0. A test now asserts the tables are sorted.

#### D3 remains open

`vt::width` uses a hand-maintained range table covering the ranges that occur in terminal
output. `unicode_width_is_exhaustive()` returns `false` and a test asserts it stays that way, so
the gap is visible rather than forgotten. D3 (pinned Unicode version and table source) must
close before release.

### 6.15 Icon asset provenance

`assets/juna-logo.png` is the **sole approved source** for Juna artwork.
`scripts/build-icon.ps1` resizes it into `assets/juna.ico` at seven sizes
(16/24/32/48/64/128/256), all 32bpp ARGB.

The source already carries a fully transparent background (alpha 0 at every corner), so no
background stripping is performed and the master is never modified.

Icon generation is reproducible from that single source. The `.ico` is a build artifact of it
and should be regenerated rather than hand-edited.

Anything else under `assets/` is local reference material, excluded by `.gitignore` and never
committed.

#### Measured legibility (first pass, 2026-10-11)

Display is 2560x1440 at **150% scaling** (`AppliedDPI 144`). A 24-logical-pixel taskbar icon
therefore rasterises at 36 physical pixels, so Windows selects the **48x48** entry — confirmed by
visual inspection.

| Entry | Legible |
|---|---|
| 48x48 | Yes — acceptable |
| 16x16 | No — unusable |

The 16px failure is a **composition** problem, not a resolution gap: at 16px the ears, muzzle,
collar and fur all collapse into one near-black mass. Adding intermediate sizes will not fix it.
The remedy is a separate face-only crop (eye and muzzle) for the small entries, keeping the full
logo for 64 and above. Deliberately deferred — the logo is expected to be revised regardless, and
this pass exists to establish the baseline. Re-measure after any artwork change.

**Decision:** the project is named **Juna**. The Cargo package, binary, and library are `juna`.

Originally established as "Smith" during Phase 0 reconnaissance, then held provisional pending a
working prototype.

**Rationale for rejecting "Juniper":** it collides with Juniper Networks and Junos OS. Network
engineers use terminals daily against Juniper routers and switches, so searching for a terminal
named "Juniper" would be dominated by enterprise vendor results, making the project effectively
undiscoverable. "Juna" keeps the personal tribute while avoiding that collision. "Juniper"
remains the warm inspiration behind the name and is not used as an identifier.

**Scope of this rename:** package/binary/library identifier and project documentation only. The
**working directory is still `E:\Development\smith-terminal` and has not been renamed**, because
moving a git repository out from under an active session carries avoidable risk. Renaming the
directory is a separate, explicit step.

**Not yet renamed:** the git repository has no remote, so no repository slug or package registry
name is affected yet.

### 6.13 Known clippy artifact

`cargo clippy --all-targets` reports `state` and `screen_text` in `src/vt/tests.rs` as unused.
They are used by `#[test]` functions — 121 tests pass. This is clippy's `--all-targets` pass
linting the `#[cfg(test)]` module without the test functions. Reported rather than suppressed.

### 6.14 D6 — closed: rasterization backend

**Resolved.** Primary rasterizer is the pure-Rust stack: **`fontdb`** (system font discovery) +
**`swash`/`zeno`** (rasterization), behind a Juna-owned trait, with a Juna-owned glyph atlas.

**DirectWrite is deferred**, not rejected. It may be added later as a second backend behind the
same trait if small-font-size readability proves inadequate in real use. Rationale as given:
ClearType subpixel AA produces colour fringing on dark backgrounds, which matters for the
Catppuccin Mocha default; grayscale AA is cleaner there; and the pure-Rust path avoids COM
`unsafe` code and matches the project's Rust standard. Cost: without hinting, 9–11 pt sizes may
look softer than grid-fitted DirectWrite.

**Measured basis:** 11.6 ns per cmap lookup, 2.83 µs per glyph rasterization (§6.8).

#### What is NOT deferred — correcting the summary phrasing

The phrase *"deferred fallback/shaping"* would contradict the two findings recorded two sections
earlier and must not be used. The accurate split is:

| Item | Status |
|---|---|
| Rasterization backend (zeno vs DirectWrite) | **Decided — zeno. DirectWrite deferred as an alternative backend.** |
| Cross-font fallback chain | **NOT deferred. Mandatory correctness requirement.** Consolas has no CJK, emoji, or Indic glyphs; no installed monospace font covers emoji or Indic. |
| Shaping (ZWJ, combining marks, Indic) | **NOT deferred. Mandatory.** cmap-only lookup provably cannot render a ZWJ sequence. |
| Glyph atlas and cache | **NOT deferred. Mandatory**, and a precondition for the 2.83 µs figure being irrelevant per frame. |

What is deferred is **DirectWrite as an alternative rasterizer**. Nothing in the fallback,
shaping, or atlas work is deferred. A terminal shipping without them renders tofu for most
non-Latin content.

#### Open sub-decision — which shaper

The shaper is not yet chosen. Candidates: `harfrust` (used by cosmic-text 0.16) or `rustybuzz`.
Both are pure Rust. This should be decided with the renderer implementation, not now, because
shaping is only exercised by emoji and Indic — cmap lookup covers Latin, CJK, and symbols.
Staging is viable: ship cmap + fallback first, add shaping for emoji and Indic once the pipeline
is running. That staging is a **recommended sequencing proposal, not a decision.**

Intended as the default theme font, Cascadia Mono/Code are not installed, so the spike fell back
to Consolas. Any glyph-coverage and metric figures gathered here are **Consolas figures** and
must not be presented as Cascadia results.

`swash` 0.2.0 dependencies: `skrifa` ^0.26.2 (required), `zeno` ^0.3.2 (optional rasterizer),
`yazi` ^0.2.1 (optional), `core_maths` (optional). **No FreeType and no C library**, satisfying
the stated requirement for a C-free text pipeline.

`src/bin/gpu_probe.rs`. Enumeration only; no device request, no surface. Blocked on an executor,
so the binary carries a ~20-line `Box::pin` poller rather than taking a dependency for one
diagnostic. `enumerate_adapters` is async in wgpu 29 and `Backends::ALL` does not exist — it is
`Backends::all()`.

Environment at time of run: all `WGPU_*` overrides unset, **`stdout_is_terminal = false`**.

Five adapters exposed:

| # | Name | Backend | Device type | Driver | max texture 2D | max buffer |
|---|---|---|---|---|---|---|
| 0 | NVIDIA GeForce RTX 3060 | **Vulkan** | DiscreteGpu | NVIDIA 595.79 | 32768 | 1099511627776 |
| 1 | NVIDIA GeForce RTX 3060 | **Dx12** | DiscreteGpu | 32.0.15.9579 | 16384 | 2147483647 |
| 2 | NVIDIA GeForce RTX 3060 | **Dx12** | DiscreteGpu | 32.0.15.9579 | 16384 | 2147483647 |
| 3 | Microsoft Basic Render Driver | **Dx12** | **Cpu** | 10.0.26100.9278 | 16384 | 2147483647 |
| 4 | NVIDIA GeForce RTX 3060/PCIe/SSE2 | **Gl** | Other | ANGLE 4.6.0 (NVIDIA 595.79) | 32768 | 2147483647 |

Per-backend counts: **dx12 = 3, vulkan = 1, gles = 1.**

#### Findings

**The Parsec Virtual Display Adapter is NOT exposed.** This is recorded as a *negative
result*, not as evidence the adapter is unavailable. It may be filtered from adapter lists, may
not present as a wgpu adapter, or may still be selected by the presentation path without being
enumerable. The virtual-adapter risk is therefore **unchanged and untested**.

**Enumeration works despite redirected stdout.** The premise that redirection might prevent
enumeration is disproven — `stdout_is_terminal = false` and enumeration still returned five
adapters. The earlier caution was reasonable but empirically wrong for this API.

**Adapter selection cannot be left to defaults.** Three DX12 entries exist, one of which is a
**CPU software rasteriser** (WARP). A naive "first DX12 adapter" policy could select the Basic
Render Driver and silently deliver software rendering. Selection must be explicit: prefer
`DiscreteGpu`, reject `Cpu`, and treat WARP as a last-resort fallback only.

**Entries [1] and [2] are identical** in name, backend, vendor, and device ID. Whether this is a
duplicate enumeration or genuinely two DX12 devices is unexplained. Adapter selection must not
assume the list is deduplicated.

**Keeping `gles` was justified.** It enumerates via ANGLE and provides a working third backend.
Per the lead's instruction, the feature set is **not** changed on enumeration evidence alone.

#### Status

B7 is **partially closed**. Enumeration is recorded. **Surface creation and presentation remain
untested** and require an interactive desktop session; this harness has no usable console and
redirected output. No claim is made about which adapter presentation would select.

`examples/conpty_spike.rs` ran on this machine, GNU toolchain, Windows build 26200. Every
blocking call is wrapped in an 8-second timeout so a hang becomes a recorded result rather than
a wedged session. Raw output:

```
--- CHECK 1: argument quoting ---
[HANG] conpty 0.7 quoting did not finish within 8000ms
[FAIL] conpty 0.7      TIMEOUT after 8000ms (blocking read/close)
[FAIL] conpty-oxide    PowerShell: "Unexpected token 'a' in expression"

--- CHECK 2: output round-trip ---
[HANG] conpty 0.7 rt did not finish within 8000ms
[FAIL] conpty 0.7      TIMEOUT after 8000ms (blocking read/close)
[PASS] conpty-oxide    Juna_RT_9F3A

--- CHECK 3: resize (120x40 then 80x25) ---
[PASS] conpty-oxide    final size 80x25
[PASS] conpty 0.7      resized and exited

--- CHECK 4: terminate + drop-without-kill ---
[PASS] conpty-oxide kill    kill took 18.7us, try_wait=None
[PASS] conpty 0.7 drop      dropped without terminate
[PASS] conpty 0.7 orphan    pid=16984 still running 1.2s after Drop: false
```

#### Findings

**The decisive result is the hang.** `read_to_end` on `conpty` 0.7's output pipe **never
returns**, even after the child has exited, because ConPTY keeps the output pipe handle open
until `ClosePseudoConsole` and the reader holds a cloned handle. Any read loop that waits for
EOF will hang the reader thread forever. This is not a hypothetical — it reproduced on the
first run of the spike, and it is exactly the failure `conpty-oxide` carries an explicit
regression test for (`tests/close_hang.rs`).

**Static finding (b) is retracted.** I predicted that dropping `conpty::Process` would orphan
the child. The empirical test refutes this for the direct child: `pid=16984` was gone 1.2 s
after `Drop`, so `ClosePseudoConsole` does reap the attached client. Two caveats keep this from
being a clean pass: the test observed only the **direct** child PID, not the process tree; and
there is still no job object, so a grandchild spawned by that child is unprotected. The
finding is downgraded from High to Medium and re-scoped to grandchildren only.

**Check 1 is inconclusive and the probe is at fault.** `powershell.exe -Command "$args -join
'|'" a b 'c"d'` does not work as a quoting probe, because PowerShell's `-Command` merges the
trailing arguments into the command string itself. Both libraries therefore produced the same
malformed command line, which tells us nothing about quoting. A correct probe needs a program
that prints `argv` verbatim — `cmd /C echo` collapses spaces and cannot distinguish boundaries.
**This check must be rewritten before D5 is approved.** The quoting defect (finding a) remains
established by source inspection only; it has not been empirically reproduced.

**Check 5 was not implemented.** Host-console-mode mutation requires the `windows` crate, which
is not a dependency. Finding (d) therefore remains static-analysis only.

#### Spike run 2 — B5 closed

`src/bin/argv_probe.rs` is a dedicated probe that prints each received argument as
`ARG[i]=<byte-length>:<bytes>`. It replaces the invalid PowerShell `-Command` probe. The
console-mode check calls `AllocConsole`, opens `CONOUT$`, and **clears**
`ENABLE_VIRTUAL_TERMINAL_PROCESSING` before each spawn, because the initial mode `0x7` already
contained the VT bit `0x4` and therefore could not detect mutation. Raw output:

```
--- CHECK 1: argument preservation (dedicated argv probe) ---
[PASS] conpty-oxide quoting
       COUNT=7 | ARG[0]=5:plain | ARG[1]=9:has space | ARG[2]=9:has"quote
       ARG[3]=10:has'squote | ARG[4]=9:trailing  | ARG[5]=10:back\slash | ARG[6]=0: | END
[FAIL] conpty 0.7 quoting      (empty output)

--- CHECK 4: host console mode must not gain the VT bit ---
       cleared VT bit, mode now 0x3
[PASS] conpty-oxide no VT set   before=0x3 after=0x3
[FAIL] conpty 0.7 no VT set      before=0x3 after=0x7

--- CHECK 5: process-tree teardown (grandchild survival) ---
[PASS] conpty-oxide grandchild  pid=7228  alive 1.5s after teardown: false
[FAIL] conpty 0.7 grandchild    pid=56912 alive 1.5s after teardown: true
```

| Checklist item | conpty 0.7 | conpty-oxide 0.1.2 |
|---|---|---|
| 1 — Command-line quoting | **FAIL** (no output at all) | **PASS** — all 7 args byte-exact, including embedded `"`, `'`, trailing space, backslash, and the empty string |
| 2 — Explicit handle ownership | Blanket `Send`+`Sync`, cloned raw `HANDLE`s | `OwnedReadHalf`/`OwnedWriteHalf`, `into_parts()` |
| 3 — Termination and cleanup | **FAIL** — grandchild survives | **PASS** — grandchild reaped (job object) |
| 4 — No host-console-mode change | **FAIL** — mode mutated `0x3` → `0x7` | **PASS** — mode unchanged |
| 5 — Resize and I/O round-trip | Resize ok, **I/O hangs** | **PASS** |
| 6 — Clear errors, MSVC build | `Error` enum, no structured kinds | Structured `ErrorKind`; **MSVC build impossible** — B3 |

**`conpty` 0.7 is now rejected on five of six measured items.** Finding (d) — host console
mutation — was previously static-analysis only and is now empirically confirmed.
Finding (b) is confirmed in its corrected, narrower form: the *direct* child is reaped, but
the *grandchild leaks*. The earlier retraction was correct.

**Verification of the tested version.** `Cargo.lock` pins `conpty-oxide 0.1.2`, and the vendored
package at that exact version ships 16 integration test files including `close_hang.rs`,
`kill_tree.rs`, `soak.rs`, `owned_halves.rs`, `eof_semantics.rs`, and `managed_drop_order.rs`.
The regression coverage cited above is part of the version under test, not of a different
release.

#### Recommendation for D5

**`conpty-oxide` 0.1.2.** It passes every measured item; `conpty` 0.7 fails five of six.
Its `blocking` feature is default, so D2 holds. Juna still owns a thin wrapper so the boundary
between engine and transport stays explicit.

Outstanding risk, unchanged and unmeasured by this spike: **maturity**. Published 2026-07-31,
three versions, ~1 800 downloads, single maintainer. `ConPtyBackend` also supports bundling its
own OpenConsole DLL; we should pin `ConPtyBackend::system()` unless portability is wanted, and
record that choice.

`conpty` 0.7 should be removed from `[dependencies]` once this is approved. It is currently
retained only so the spike can keep comparing against it.

### 6.16 A directory rename silently broke three acceptance tests

**Date:** 2026-10-11. **Toolchain:** `stable-x86_64-pc-windows-gnu`. **Impact:** three of four
ConPTY acceptance tests failed for a reason unrelated to ConPTY.

#### Symptom

`cargo test` reported `1 passed; 3 failed`. The three failures were
`quoting_preserves_the_argument_vector_exactly`, `sustained_output_arrives_byte_exact`, and
`process_tree_is_reaped_on_teardown`. All three panicked identically, at
`tests/conpty_acceptance.rs:21`, with:

```
Spawn { program: "E:\Development\smith-terminal\target\debug\argv_probe.exe",
        source: Os { code: 3, kind: NotFound } }
```

`shutdown_unblocks_the_reader_within_the_bound` passed, because it spawns `cmd.exe` from `PATH`
rather than the probe.

#### Diagnosis

Not a ConPTY defect and not a code defect. `tests/conpty_acceptance.rs:5` uses
`env!("CARGO_BIN_EXE_argv_probe")`, which Cargo expands **at compile time** to an absolute path
under the package root. The working directory was renamed from `smith-terminal` to `juna-terminal`
after that test binary was last built, and Cargo's fingerprint did not invalidate — the constant was
frozen at the pre-rename path, which no longer exists.

Confirmed directly: `Test-Path target\debug\argv_probe.exe` → `True`;
`Test-Path E:\Development\smith-terminal` → `False`.

Touching the test file forced a rebuild. All four tests then passed, unmodified.

#### Why this is recorded rather than dismissed

The failure presented as *"ConPTY acceptance is broken"*, and the panic text names `conpty-oxide` and
a spawn error — exactly the shape of a D5 regression that would trigger §12's pinned-dependency
review. It was not one. Three of the four tests guarding the `conpty-oxide` decision were failing for
an unrelated reason, and the one that failed *not* to fail was the one that does not touch the probe.

**Standing rule adopted:** a failing acceptance test in a pinned-dependency suite must have its panic
text checked for the *filesystem path it names* before the dependency is suspected. A
`NotFound`/`ERROR_FILE_NOT_FOUND` on a path under the workspace is an environment or stale-artifact
fault, not a library fault.

**Not fixed, deliberately.** The source is correct; `env!("CARGO_BIN_EXE_...")` is the right idiom
and is robust across renames once rebuilt. Adding path-repair code to work around a stale build
artifact would be a defect. The remedy is a rebuild, not a patch.

**Related artifact.** `target\release\smith.exe` remains from before the rename. It is inside the
gitignored `target/` directory and has no effect on the build; noted so it is not mistaken for a
current output.

### 6.17 D10 — process privilege boundary

**Adopted 2026-10-11 (design lead).** Specified normatively in `ARCHITECTURE.md` §3.8.

#### The request

Make Juna runnable as Administrator, with no risk of a catastrophic destructive error.

#### Verified before designing

| Fact | How established |
|---|---|
| UAC enabled: `EnableLUA=1`, `ConsentPromptBehaviorAdmin=5`, `PromptOnSecureDesktop=1` | `Get-ItemProperty HKLM:\...\Policies\System` |
| Current session token is `High Mandatory Level` | `whoami /groups` |
| **A child inherits High integrity** — spawned `cmd.exe`, queried its groups, got `High Mandatory Level` | direct execution |
| **Juna spawns no processes today** | `Grep` for `spawn` — only `std::thread::spawn` in `transport.rs` and `argv_probe.rs` (a test probe). `main.rs` is 3 lines. |
| **`conpty-oxide` exposes no token API** | read vendored `command.rs` — a thin facade over `windows_spawn::Command` |
| **`windows-spawn` reaches only `CreateProcessW` + `creation_flags`** | read vendored `options.rs`, `sys.rs`, `plan.rs` — no `TokenDuplication`, no `CreateProcessWithTokenW`/`AsUserW`, no `logon` |

#### Ruling

Elevation is a **property of the host process**. Juna requests elevation for no child, ever.

1. No `requireAdministrator` in the manifest — casual launch stays unelevated.
2. Elevation is a deliberate user choice at launch: `scripts/run-elevated.ps1` (`-Verb RunAs`).
3. The transport never raises or lowers privilege; a child inherits the host token.
4. No in-UI "run this shell as Administrator" affordance in this increment.

#### Why the alternative was rejected

An **elevated child spawned from an unelevated Juna** is not achievable on the pinned binding. It
requires `CreateProcessAsUserW` plus `UpdateProcThreadAttribute(PROCEDURE_ATTRIBUTE_TOKEN)`,
bypassing `conpty-oxide`'s command path with several hundred lines of `unsafe`. §6.3 permits
replacing `conpty-oxide` only for a critical lifecycle defect, abandonment, or demonstrated
incompatibility — **none of which apply**. The binding stands.

#### On "no catastrophic deletion" — the mechanism was corrected

The lead's concern was a Juna bug deleting files. **Juna's engine cannot delete anything.** By
`AGENTS.md` §5 the VT engine performs no I/O; it is pure state transitions. There is no path from a
keystroke to a filesystem delete inside Juna. This is a design guarantee, not an untested claim.

The real risk is different and was stated plainly:

- **The shell, not a bug.** At High integrity there is no UAC prompt between the user and
  `rm -rf` / `del /f /s` / `git clean -fdx`. Windows' *InstallerDetection* mitigation covers
  installers; it does **not** cover console tools.
- **One genuinely Juna-specific elevated risk.** A mangled byte stream does not merely garble the
  display — it can deliver a **corrupted argument to the child process**. Unelevated, that is
  usually a permission error. Elevated, the same corruption can reach a destructive path with no
  prompt. This is the concrete reason §3.0's byte-exactness is upgraded from a correctness rule to
  a **release gate for elevated builds**.
- **UAC is consent, not a security boundary.** It does not defend against malware already running
  as the user. The marginal risk of an always-elevated terminal is therefore higher than it may
  appear.

#### Enforcement

`tests/elevation_boundary.rs` — 4 tests. **Verified non-vacuous by fault injection**, not assumed:
injecting `// CreateProcessWithTokenW elevated runas token` into `src/transport.rs` failed
`no_source_reaches_for_a_token_api` and `the_transport_layer_is_free_of_elevation_intent`, both
naming the offending file and symbol. The injection was reverted and the file confirmed byte-clean.

**Scope limit, stated in the test's own header:** this is a *source-level* guard. It does not parse
the PE resource section, so it cannot detect a manifest injected by the linker or an external `.rc`
file. It prevents the mistake in the code we own; it is not a proof about the shipped `.exe`.

### 6.18 D11 — cross-platform direction, and the one hard blocker removed

**Raised by the design lead, 2026-10-11:** *"We want to be able to eventually port this app to other
platforms, including Linux."* Recorded as a **direction**. Not a commitment to ship Linux now.

#### Survey result — the engine is already portable, by design

All 16 `.rs` files under `src/` were scanned recursively for `cfg(windows)`, `cfg(target_os`,
`windows::`, `Win32_`, `conpty`, `winit`, and `wgpu`. Results:

| Area | Files | Platform coupling |
|---|---|---|
| `src/vt/` — parser, grid, scrollback, cell, width | 8 | **NONE** |
| `src/layout/` — geometry, BSP tree | 4 | **NONE** |
| `src/transport.rs` — byte stream, backpressure | 1 | **NONE** — pure `std` (`Mutex`, `Condvar`, `Vec<u8>`) |
| `src/lib.rs`, `src/main.rs` | 2 | **NONE** |
| `src/bin/gpu_probe.rs` | 1 | `wgpu` + DX12/Vulkan/GL **backend names** |

This is not luck. `AGENTS.md` §5 already mandates it: the engine performs no I/O, the layout engine
is pure geometry, and the transport moves bytes. **14 of 16 files are platform-neutral by
construction.**

#### The single hard blocker — found and removed

`conpty-oxide` 0.1.2 carries, at `src/lib.rs:126`:

```rust
#[cfg(not(windows))]
compile_error!("conpty-oxide only supports Windows targets; ...");
```

It was an **unconditional** `[dependencies]` entry. Therefore `cargo build` on any non-Windows
target failed outright — not degraded, failed. That was the entirety of the hard blocker.

**Fixed.** `conpty-oxide` and `windows` moved to `[target.'cfg(windows)'.dev-dependencies]`.

Both are **dev**-dependencies, not runtime ones: nothing under `src/` references either. The only
consumer of `conpty-oxide` is `tests/conpty_acceptance.rs`, which is already `#![cfg(windows)]`.

**Verified.** `cargo metadata --filter-platform x86_64-unknown-linux-gnu` resolves **158 packages
with zero Windows-only crates** — `conpty-oxide`, `windows-spawn`, `windows-sys`, and `windows` are
all absent from the Linux graph. Windows behaviour is unchanged: all **129 tests** pass, clippy
clean, release builds.

#### Honest limit on that verification

**This is a resolved dependency graph, not a successful compile.** Only
`x86_64-pc-windows-gnu` is installed on this host, and a Linux build additionally needs a Linux
linker, neither of which exists here. A Linux port may still surface compile errors — most likely
in `src/bin/gpu_probe.rs`, the one file with real backend coupling. **No claim of Linux
compatibility is made.**

#### Not decided here

A Unix port needs a PTY backend (`forkpty`/`openpty` + `TIOCSWINSZ`). PTY syscalls are *platform
access*, which §2's boundary test classifies as infrastructure, so an off-the-shelf binding is
permitted — unlike a terminal engine, which is not.

### 6.19 D11 — session seam implemented

**Approved and built 2026-10-11.** Specified normatively in `ARCHITECTURE.md` §3.9.

`src/session/` is now the only module permitted to name a platform API for process I/O. Callers hold
`Box<dyn Session>` and never learn the backend.

| Item | Detail |
|---|---|
| `Session` | object-safe; `write` / `read` / `resize` / `try_wait` / `kill` / `pid` |
| `SessionSpec` | program, args, cwd — args kept as **separate** elements, never joined |
| `ConptySession` | `cfg(windows)`, wraps `conpty-oxide`'s owned halves, **zero `unsafe`** |
| `FakeSession` | pure `std`, compiles and behaves identically on every target |
| Tests | 15, of which 4 drive the real ConPTY backend through `dyn Session` |

#### Verified after the change

| Claim | How established |
|---|---|
| 144 tests pass; clippy clean; release builds | `cargo test` / `cargo clippy --all-targets` / `cargo build --release` |
| **Zero `unsafe` in the whole codebase** | recursive scan of `src/` — the single `unsafe` hit is the word inside a doc comment |
| `conpty_oxide` referenced from exactly one file | recursive scan; no matches outside `src/session/` |
| Linux graph still Windows-free | `cargo metadata --filter-platform x86_64-unknown-linux-gnu` → 158 packages, zero Windows-only crates |

#### Two contracts the tests discovered, not the docs

1. **`read` blocks and `Ok(0)` means EOF, not "nothing yet".** Already predicted in §6.5 from the
   `conpty` 0.7 hang; now it is a documented property of our own trait, with a test.
2. **`kill` is asynchronous.** The first version asserted the child was gone on the very next
   `try_wait` and **failed**. The backend tears down the session job object and termination proceeds
   asynchronously. The test now polls, and the trait documents that a successful `kill` is not
   evidence of death.

Both are recorded in `ARCHITECTURE.md` §3.9 as contracts, not implementation notes, because a caller
that gets either wrong hangs or leaks.

#### Honest limits

- **The Unix backend is not implemented.** `spawn` returns `ErrorKind::Unsupported` there. This is a
  seam, not a port.
- **No Linux compile has been performed.** Only `x86_64-pc-windows-gnu` is installed, and a Linux
  build needs a Linux linker. The dependency graph resolves; the code is unverified there.
- **`FakeSession` is compiled into release builds.** It is small, kept public deliberately so a
  future Linux CI can test the wiring, and the linker will drop it if unreferenced.

### 6.20 Slice 1 — headless conformance harness, and the scrollback defect it found

**Built 2026-10-11.** `src/bin/juna_headless.rs` drives a real shell through the whole production
data path and renders the screen as text:

```
session ─read─► reader thread ─► StreamBuffer ─► TerminalState ─► screen text
     └──────────write──────────────────────────────────────────────▲
```

No window, no GPU, no winit. Every component is production code. This answers *"is the engine
correct against a real shell?"* before a renderer exists to confuse the answer.

#### Defect found: scrollback was never populated

**Severity: high. Tier 1 requirement, silently non-functional.**

`Scrollback::push` had **no call sites outside its own tests.** `TerminalState` only ever called
`len`, `get`, `set_cols`, and `clear`. The `Scrollback` type passed all of its unit tests and was
entirely inert: no matter what a program printed, `scrollback_len()` stayed `0` forever.

Unit tests missed this completely. They tested `Scrollback` as a type and the engine's grid and
parser separately, and nothing ever asserted the two were connected. The harness found it on its
first real run: 60 lines into a 24-row window produced `scrollback=0`.

**Fixed.** `Grid` now records rows pushed off the top into `pending_history`; `TerminalState` drains
it into `Scrollback` after each action, per `ARCHITECTURE.md` 2.3. Eligibility rules:

| Rule | Reason |
|---|---|
| primary screen only | §2.3: "alternate screen does not" |
| full-screen scroll region only | lines leaving a *restricted* region are still on screen; filing them would duplicate them on region reset |
| rows are **drained** even when ineligible | buffering ineligible rows would leak a line per scroll |

Rows move via `Vec::append`, not `clone`, so recording history costs no extra allocation beyond the
row the grid already builds for the scroll.

`Grid::scroll_up`/`scroll_down`/`scroll_region_impl` changed from returning `Vec<Vec<Cell>>` to
returning `()`. Only one call site used the value, and after the change it would always have been
empty — a return type that silently lies. `take_pending_history()` replaces it.

#### Two harness bugs found in my own test, not the engine

Both were recorded because the instinct to blame the engine was wrong twice:

1. **`\e[5;10H` appeared to land at column 1.** Not an engine bug: PowerShell's `Write-Host` host
   repositions the cursor before writing. Proven by driving the identical escape through
   `[Console]::Out.Write`, which bypasses the host — the cursor landed exactly at column 11.
2. **A truecolor line went missing.** The engine's SGR handling is correct; proven by a new test,
   `text_after_extended_colour_sgr_is_printed_not_consumed`, and by raw-byte output through
   `[Console]::Out.Write`. The pre-existing `sgr_truecolor_and_256_color` test asserted only the
   *colour*, never that following text printed — a gap this exposed.

**Lesson recorded:** when a conformance failure appears, isolate the byte sequence before blaming
the engine. `Write-Host` applies host-side formatting and repositioning that raw writes do not.

#### Verified results

| Target | Result |
|---|---|
| `cmd.exe` | Banner, prompt, echo, `cls`, 400-line bulk output — all correct |
| **Windows PowerShell 5.1** (primary target, §5.1) | Full ANSI prompt renders, including posh-git powerline segments; truecolor, 256-colour, bold/underline/reverse, box drawing, CJK 漢 字, combining `é`, CUP all correct; OSC title captured |
| Scrollback | 48 lines retained across a 60-line scroll, first/middle/last all found |
| Reaping | Child exits cleanly, nothing orphaned |

New coverage: `tests/shell_roundtrip.rs` — **5** tests against real `cmd.exe`, all shell-independent
(assertions are only on text the test itself chose, never on prompt strings). `split_survives...`
and three more join the session suite for **19** session tests.

**Total: 154 tests** (141 lib + 4 conpty + 4 elevation + 5 shell round-trip).

#### Still not true

There is **no window and no renderer.** `src/main.rs` still prints a banner. A user cannot type
into Juna. What exists is a proven data path from a real child process to a correct screen.

---

## 7. Blockers

| # | Blocker | Impact | Needed |
|---|---|---|---|
| **B1** | **No git repository.** Not yet authorized. | Global CI rule requires commit-per-iteration; nothing can be committed. | Explicit authorization to `git init`. |
| **B2** | ~~No project `AGENTS.md`~~ | — | **RESOLVED** — `AGENTS.md` drafted this session. |
| **B3** | ~~MSVC toolchain cannot link.~~ | **DEFERRED by design-lead ruling, 2026-10-11.** Elevation was made available and deliberately not spent on this. | The GNU toolchain links, builds, and passes all 125 tests. MSVC is not required by any current acceptance criterion. See §7.1 for the exact command should the ruling be revisited. |
| **B4** | ~~`pwsh` not installed~~ | — | **RESOLVED by lead ruling.** PowerShell 5.1 is the primary acceptance target; `cmd.exe` secondary; PowerShell 7 optional later. `pwsh` remains absent from this machine and is **not** a Phase 1 blocker. |
| **B5** | ~~ConPTY spike incomplete~~ | — | **RESOLVED.** Check 1 (quoting) and check 5 (host console mode) both run and pass for `conpty-oxide`. Check 6's MSVC leg remains blocked by B3. See §6.2. |
| **B6** | ~~wgpu feature flags not enumerated~~ | — | **RESOLVED.** Feature names, defaults, and dependency consequences recorded in §6.1; configuration applied and verified building. |
| **B7** | ~~GPU adapter enumeration~~ | — | **PARTIALLY CLOSED.** Five adapters enumerated (`DECISIONS.md` §6.4). Parsec virtual adapter **not** exposed — recorded as a negative result, not as absence. Surface creation and presentation still untested. |

### 7.1 B3 detail — the fix is a modify, not an install

**Ruling, 2026-10-11 (design lead).** An elevated session was made available specifically so this
could be closed. **The lead's decision was to leave MSVC alone and continue on GNU.** B3 is
therefore deferred, not blocked.

**Why this is a sound outcome, stated plainly rather than assumed.** The GNU toolchain was
re-verified in an elevated session and produces `cargo check`, `cargo clippy --all-targets`,
`cargo test` (125 passing), and `cargo build --release`, plus a binary that runs. D1's stated
concern — *"pinning MSVC now would break every build immediately"* — does not arise, because nothing
is being pinned. `rust-toolchain.toml` **is not created.** The project is fully buildable and fully
tested in its current state.

**D1 remains OPEN.** The toolchain decision is undecided, not made. If a future requirement genuinely
needs MSVC — for example C-compatible FFI, or an acceptance target on a machine without the GNU
runtime — this section becomes live again and the command below applies unchanged.

**Re-verified 2026-10-11:** `link.exe` still absent, MSVC toolchain still fails to link, VS 2022
BuildTools still present but without the C++ workload, Windows SDK still absent. The machine state
described below is unchanged.

**Correction.** An earlier revision of this section recorded a `winget install` command. That was
wrong: it would reinstall or repair, not add a workload to an installation that already exists.

**Verified state of the machine:**

```
> vswhere -products * -format json
  instanceId          e9f03673
  installationPath    C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools
  installationVersion 17.14.37614.0
  isComplete          true
  isLaunchable        true
  isRebootRequired    false

> vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64
  (empty - component absent)

> Get-ChildItem "...\BuildTools\VC"          -> not found
> Get-ChildItem "...\Windows Kits\10"        -> not found
```

**Visual Studio Build Tools 2022 is already installed, complete, and launchable.** What is missing
is only the C++ workload and the Windows SDK. This is therefore a fast **modify**, not a fresh
multi-gigabyte install.

The session is not elevated, so the modify cannot be run from here. Rather than fire an unattended
UAC prompt, it is left to the operator.

**GUI route:** open Visual Studio Installer → *Modify* on the Build Tools 2022 instance → tick
**Desktop development with C++** → *Modify/Install*.

**CLI route, from an elevated PowerShell:**

```powershell
& "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vs_installer.exe" `
  modify `
  --installPath "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools" `
  --add Microsoft.VisualStudio.Workload.VCTools `
  --includeRecommended --quiet --wait --norestart
```

To avoid the recommended Windows SDK download, pin a specific one and drop `--includeRecommended`:

```powershell
  --add Microsoft.VisualStudio.Workload.VCTools `
  --add Microsoft.VisualStudio.Component.Windows11SDK.22621 `
  --quiet --wait --norestart
```

#### Validation, and only then is `rust-toolchain.toml` justified

```powershell
scripts\verify-toolchain.ps1
```

That script probes MSVC linkage, builds and tests the whole workspace under MSVC, and reports
pass/fail per step. `rust-toolchain.toml` is created only after it reports success.

The GNU toolchain links successfully today and remains the working default, so the project stays
buildable throughout.

### 7.2 Process lifecycle evidence

Git initialized and the first commit made (`63198c9`) after reviewing staged contents: 7 files,
no `target/`, no binaries, no secrets. B1 is resolved.

## 2026-10-13 — approved application icon wiring

The approved ICO is embedded as Windows resource 1 by build.rs using GNU windres.
No Rust dependency was added. WINDRES can select the resource compiler explicitly.
The native window loads small/taskbar icons from that resource and verifies both handles.
GNU remains the working toolchain; Windows MSVC resource compilation needs a future reviewed path.
Verified: required check/clippy/test gates, both real-window tests, and release build passed.
All seven release resource images match assets/juna.ico byte for byte (16–256 pixels).

## 2026-10-13 — approved bundled fonts and live sizing

Implemented FONT_DESIGN.md and its explicitly approved live-sizing extension. Fifteen original
font assets are embedded and resolve without system installation; source URLs and SHA-256 hashes
are recorded in assets/fonts/manifest.json. No Rust dependency was added. Fourteen assets use
SIL OFL 1.1; Ubuntu Sans Mono uses Ubuntu Font Licence 1.0 (correction to the initial all-OFL
assumption). Original notices ship in the assets and an embedded Settings license view.

The font picker includes installed monospace fonts, search and live preview. Settings previews
are transactional: Save/Ctrl+S persists and Cancel/Esc restores the original preferences.
Ctrl+Plus/Equals, Ctrl+Minus, Ctrl+0 and Ctrl+wheel reserve local resizing before shell/TUI input;
Left/Right adjusts the size row. Sizes clamp to 8–40 px and successful external shortcuts persist.
AltGr is excluded. Pixel wheel deltas accumulate, and PTY/grid resize uses the existing checked path.
Measured variable defaults included Fira Code 300, Source Code Pro 200 and Victor Mono 100;
measurement/shaping/rasterization explicitly select regular weight 400 instead.

Windows taskbar grouping now receives LeakTechnologies.Juna.Terminal before UI creation.
The existing windows infrastructure enables Win32_UI_Shell for that concrete requirement.
Native icon handles and API success are verified; actual Explorer taskbar appearance remains human
review. A user-reported missing taskbar logo was traced to a window started before this build,
not proof that the new identity has fixed the displayed taskbar button.

Verified: warning-free check/clippy, 180 standard tests, four GPU acceptance tests (15 fonts,
clipping/fallback and size limits), two live-window tests (output with fonts/settings/themes open),
release build, and byte-for-byte approved icon resources. Recorded captures were visually reviewed.
## 2026-10-13 — right-side previews and verified taskbar display

Settings/fonts/themes/editor geometry now stays against the right edge as the window changes
size; the context menu retains its pointer anchor. GPU comparisons verify the left 480 pixels
are unchanged by a panel in a 1024-pixel frame. The standalone font-license selector was removed;
original notices remain embedded and are written to the app-owned font-licenses.txt file.

Retraction of the taskbar hypothesis: process identity and valid ICON_SMALL/ICON_BIG handles did
not fix the existing Explorer group. Executable extraction and a fresh file both returned the
correct logo, but Explorer retained a generic group icon. Setting complete relaunch properties
and a fresh group ID displayed the dog logo; the user confirmed both taskbar and title-bar icons.
The permanent setup declares LeakTechnologies.Juna.Terminal.Desktop, configures relaunch command,
display name and resource -1 before showing the window, and uses a 48px large window icon.
Windows PropertiesSystem/StructuredStorage/Variant features serve only this platform metadata.

The first Rust property integration incorrectly treated PROPVARIANT as a non-owning wrapper.
Its Drop implementation freed borrowed storage, and the real-window tests caught heap corruption.
That uncommitted attempt was replaced by the library's safe owned string conversion; the repeated
real-window tests and a direct release-executable launch now exit successfully.

Verified: warning-free check/clippy, 180 standard tests, right-panel GPU comparison, both actual
window tests, updated release build, direct release exit 0, all seven embedded source-icon images
matching byte-for-byte, and visual taskbar inspection. No terminal-engine boundary changed.

The next UI, Junafetch and update proposals received explicit approval on 2026-10-13, in that order.

## 2026-10-13 — approved tabs and grouped custom themes

Implemented UI_NEXT_DESIGN.md with at most 16 independent sessions. Active/inactive engines move
between App and owned tab records on the same thread; no shared mutable engine or dependency was added.
Inactive output continues with bounded fair drains. New/close/switch controls reserve Ctrl+Shift+T,
Ctrl+Shift+W and Ctrl+Tab; header and right-click actions provide mouse access.
Numeric settings accept explicit px/pt entry with live preview and transactional cancellation.

Sixty starter palettes use the previously pinned published color-data source; no terminal-engine
source was used. Families collapse by default and search exposes matches. Custom JSON discovery
uses a bounded worker, family subfolders, native Juna records or single Windows Terminal schemes.
Imported files are preserved; edits duplicate them, and removal moves files to recoverable trash.
Read-only, oversized, malformed and duplicate sources are reported. No new dependency was added.

## 2026-10-13 — Junafetch detector and child environment boundary

Created the approved LeakTechnologies/junafetch fork; detector iteration 6ff6424 on
codex/juna-detector starts at upstream 2.69.0 (0c3b852bf7bad2837a814c7a31bf332092048a2b).
The lean GNU build disables image protocols, Lua/QuickJS, zlib and optional native video-codec
detection. Missing DirectX AV1 declarations blocked that unused module; the fork's build stub
returns an explicit unavailable error. No terminal implementation is borrowed. All 18 upstream
tests pass and PE imports require only Windows system libraries. DXCore is absent; uncertain
GPU classification and upstream compiler warnings are retained as limitations.

SessionSpec now carries explicit child-only environment overrides to support the approved tool
PATH. ConPTY receives owned keys/values without changing parent/global environment or profiles.
The terminal-wide privacy extension received explicit approval; it is not implemented by this
preparatory detector/environment milestone. No dependency version changed.

## 2026-10-13 — refined tab menus, line numbers and theme favourites

Implemented the approved TAB_CONTEXT_DESIGN, LINE_NUMBERS_DESIGN and THEME_FAVOURITES_DESIGN.
Right-click menus depend on tab-strip versus terminal location; repeated clicks reanchor them.
Tab actions use stable IDs, renaming is local, and middle-click requires a themed check/cross
confirmation with Cancel selected. Explicit menu/keyboard closes retain their approved behavior.
No busy-command heuristic or automatic command execution was added.

Optional line numbers count retained physical buffer rows, with alternate-screen row numbering.
Gutter width is stable for history capacity; its origin is shared with rendering and mouse input.
Labels never enter copied text or terminal state. Favourite IDs are bounded/deduplicated, with
defaulted JSON fields for older preferences; missing sources retain IDs for later restoration.
Yellow stars are distinct from the current-theme marker. Saved stars survive preview cancellation.
Expand/Collapse all reduce family-list navigation; collapsing clears a search that expands matches.
No dependency or VT semantics changed. Future Android and language directions are roadmap only.

The red Linux CI run was a test compilation error: Result::unwrap_err required Debug on the
successful Box<dyn Session>. The unsupported-backend test now extracts its error without that
unnecessary bound. No workflow modification was needed; the next hosted run must verify Linux.

Approved persistence refinement: confirmed appearance choices save immediately, while hover and
unfinished numeric edits retain a restorable committed baseline. Export uses a portable field
allowlist plus only referenced custom themes. Import validates bounded JSON, preserves conflicting
sources via fresh IDs, reviews before applying, and writes preferences last with staged-file rollback.
Native file selection and file I/O run on one bounded worker; no engine references cross threads.
The existing windows infrastructure enables Win32_UI_Controls_Dialogs solely for these native dialogs;
no Rust dependency/runtime was added. File-dialog interaction still needs recorded acceptance.

Development release cadence was explicitly adopted: this cohesive interface/preferences cycle
is 0.1.1-dev1; subsequent completed cycles get fresh sequential versions/tags. The approved release
design now permits early dev previews without claiming unshipped tool/privacy/updater milestones.
GNU windres embeds native version/source metadata alongside the unchanged approved icon. A small
original Rust verifier reads Microsoft PE resource structures to check architecture, version
presence and every icon image without executing an untrusted image. Publishing uses pinned
checkout/MSYS2 actions, Rust 1.97 GNU, original notices, checksums and source/build provenance.
No dependency was added for packaging or verification. Checksums are not publisher signatures.

Dev1 Windows release workflow succeeded and its three assets were downloaded independently;
archive SHA-256, source f4f8bfbe26f97074146cccdea80e01482bed3999, native version and all seven icons
match. Linux CI then exposed Unix rename semantics: replacing an existing read-only file can
succeed despite the file's write bits. write_json now refuses that replacement before staging
on every platform. Dev2 records this small correction; published dev1 assets are not replaced.

Junafetch delivery uses an independently compiled console Rust frontend so PowerShell waits for
its exit/status and GUI-subsystem launching cannot return the prompt early. Its serde_json 1.0.151
dependency serves bounded detector-report parsing; windows 0.61.3 serves native SHA-256 cache
verification and console-width detection. These reuse the project's existing pinned versions;
no async runtime or terminal engine is introduced. Root windows enables Security_Cryptography
for the same pinned launcher-payload hash check. Native Fastfetch 2.69.0 detector source remains
the separately maintained fork; script/image/codec capabilities excluded by the lean build are
not claimed. This cycle delivers eight safe local stats, width-aware art and JSON/plain output;
interactive protected-value reveal and terminal-wide privacy remain a separate unshipped cycle.
## 2026-10-15 — protected Junafetch fields

Implement the previously approved Junafetch/terminal privacy designs' structured-field stage,
with five shaded cells and deliberate Shift+hover reveal. JUNAFETCH_PRIVACY_PROTOCOL.md records
the private annotation, tab capabilities and bounded current-user local IPC. Add Windows
Security_Authorization, Storage_FileSystem, System_Pipes and System_IO features for a current-user
DACL, remote-rejecting named pipe and cancellable overlapped operations. Identity queries are
read-only; there is no impersonation, privilege adjustment, new crate or async runtime.
Values remain outside PTY output/VT/history/normal copies. General terminal-wide recognition
remains a subsequent approved stage and is not claimed by this structured-field delivery.
Confirmed by the cell-storage regression guard: the new metadata fits within the existing
48-byte cell ceiling. Broker I/O waits on completion/stop events; it adds no periodic idle
polling. An application-owned cached viewport supports immediate hiding during synchronized
output; values are never put in engine snapshots. The enlarged settings import message is
boxed rather than increasing the worker-result enum for every empty/export result.

## 2026-10-15 — measured redraw preparation

Existing inactive-tab attention work is included in this validation pass: the pure engine
counts BEL bytes, while application-owned timestamps drive the tab indication and viewing
the tab clears it. No audio or command-completion inference is introduced.

The lead approved PERFORMANCE_DESIGN.md and retained the GitHub updater in this cycle.
Release-mode measurement found repeated contrast correction dominated the light-box workload.
Reuse its exact result for adjacent identical colour pairs, borrow cached glyph slices until
the next mutable atlas access, encode scalar UTF-8 on the stack, and retain foreground-instance
capacity between frames. No terminal semantics, accessibility policy or ownership boundary
changes. No dependency addition. PERFORMANCE.md records desktop timing and memory evidence,
comparison limitations and outstanding laptop/private-memory work. Timing probes contain no
terminal contents and do not claim end-to-end latency or a general terminal ranking.

## 2026-10-14 — verified manual updates and restart

Implemented the approved update design with manual Stable/Preview checks and explicit
Download/Install-and-Restart actions in Settings. Development builds default to Preview;
stable builds default to Stable. Compare Juna's strict numeric versions, inspect published
releases independently of API order, and require the exact x64 ZIP plus checksum/provenance
assets and API digests. Stream HTTPS through WinHTTP on one owned I/O worker with DNS/connect,
header and idle timeouts, cancellation and size limits; do not set an overall transfer deadline
or weaken TLS. Disable automatic HTTP credentials and restrict initial/final origins.

Add pinned flate2 1.1.10 with only its Rust backend because the standard library cannot decode
the release ZIP's Deflate members. Its resolved compression/checksum dependencies are
miniz_oxide 0.9.1, crc32fast 1.5.2, adler2 2.0.1 and simd-adler32 0.3.10; original notices ship
under notices/updater. They provide nonterminal compression infrastructure. No async runtime
or terminal engine was added. Windows WinHTTP and Threading features provide HTTPS and parent
process synchronization. The original read-only PE/resource reader is shared by packaging and
update verification; it never executes a downloaded file.

The restricted ZIP subset admits only stored/Deflate entries for juna.exe, notices and release
notes; reject traversal, duplicates, device names (including superscript aliases), links,
encryption, ZIP64 and oversized/overlapping records. Verify archive SHA-256, member CRCs and
native x64/version/source identity before installation. A temporary copy of Juna runs a hidden
Rust helper. Wait for parent exit, copy onto the installation volume, retry locked replacement,
retain the old executable/notices, and roll back handled failures. New startup acknowledges its
first presented frame and launched session; missing acknowledgement restores the old version.
An installation marker prevents starting a mixed file set. A helper crash/power loss is reported
as interrupted and requires manual recovery from retained files; automatic crash recovery is
not claimed. Rollback/staging retention is currently manual after completed installations.
Other Juna windows must be closed; they are never forcibly terminated to unlock an update.

Persist only the channel in portable settings. Runtime schema-1 settings ignore unknown future
fields so a rollback preserves known preferences; portable imports remain a strict allowlist.
Paths, update plans and results stay in local app-owned staging and never enter exports or
terminal output. Checksums/TLS are integrity/origin checks, not publisher signatures.

References: [GitHub Releases API](https://docs.github.com/en/rest/releases/releases),
[PKWARE APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT),
[Windows filenames/namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file),
and the existing Microsoft PE resource format reference. No terminal implementation was copied.

## 2026-10-14 — interactive startup directory

The approved startup-directory change resolves USERPROFILE (HOME on Unix) at the session
boundary for launches without custom shell arguments. Validate an absolute existing home
folder; do not change the application working directory. Explicit --cwd DIRECTORY resolves
and validates a selected folder, including relative CLI paths. Custom argument/command launches
retain inherited cwd unless explicitly overridden, preserving scripts. Ctrl+Shift+N passes
its intended folder as --cwd, using reported cwd or the shell's launch folder as fallback.
Missing folders produce generic actionable errors without private path contents. No dependency
was added; backend byte transport and pure engine ownership are unchanged.
Validated canonical Windows paths are converted from device-prefix syntax to ordinary
drive/share syntax before shell launch. Actual acceptance exposed CMD falling back to the
Windows directory for a canonical device-prefixed drive path; Unicode filesystem identity
checks verify the correction without relying on CMD's legacy text encoding.

## 2026-10-13 — independent windows

The approved Ctrl+Shift+N shortcut uses a direct absolute current-executable launch and
separate argument values, without invoking a shell to launch Juna. Pass --shell and an empty
-- argument list to avoid replaying one-off arguments or inherited JUNA_SHELL_ARGS. Prefer
validated reported local cwd, fall back to the application launch folder, and reject nonlocal
or relative launch folders. Alt/AltGr, Super, IME composition, repeat and editing-panel ownership
prevent accidental activation. Processes load the normal saved preferences independently.
Logs move to Juna/logs/juna-<pid>-<nonce>.log with create-new semantics; a new window cannot
truncate another's diagnostic file. The startup record identifies its filename. No dependency
was added and no mutable engine/runtime state is shared. Log retention remains manual.

## 2026-10-13 — grouped Junafetch presentation

The approved expanded report constructs safe fields from bounded native detector modules;
it never passes raw JSON through. Standard/Detailed cover all detected GPUs, displays and
mounted disks using generic drive labels, Windows desktop appearance and Juna preferences.
Unavailable values are explicit. Sensitive identifiers/network data are Hidden placeholders,
not evidence of privacy detection or hover reveal. Saved profiles/artwork are read on each
invocation from a bounded validated settings file; CLI overrides them. Settings use short
Appearance/Terminal/Tools/Import-Export categories and retain immediate confirmed persistence.
The standalone frontend adds pinned unicode-width 0.2.2 and unicode-segmentation 1.13.3 for
display-column wrapping without splitting grapheme clusters. These existing project standards
data dependencies provide no terminal semantics; their original notices accompany the payload.
No engine, process transport or native detector dependency changed.

## 2026-10-13 — Windows CI scheduling

Dev7 Linux checks passed, but Windows timed out in two existing ConPTY lifecycle probes.
The approved release workflow runs Windows tests sequentially with RUST_TEST_THREADS=1,
retaining all tests and their existing deadlines. Scheduling pressure is a hypothesis,
not an established root cause. Explicit multi-session and sustained-output acceptance
remain required; a repeated failure requires lifecycle investigation rather than relaxed
deadlines. No process-backend or dependency change is made by this scheduling decision.

## 2026-10-13 — local Git status

Implemented the approved local Git status design using the installed Git CLI and standard
library worker/channels; no dependency was added. OSC 7 local-file URI validation is original
pure engine metadata. Reject remote/invalid URI updates and clear stale folder data. Optional
PowerShell prompt reporting applies only to default interactive launches, with saved opt-in
and new-tab scope; custom launch arguments are untouched. Preserve original prompt text and
LASTEXITCODE. Unsupported shells show folder-reporting unavailable after initial input.

Git queries disable optional index writes, fsmonitor, submodule recursion, auto maintenance
and lazy object fetching. Check tracked attributes before status, refuse active external
filters and blank discovered filter commands as a further safeguard. Inactive global LFS
configuration does not block ordinary repositories. Counts use local tracking refs from the
last fetch; oversized/failing reports are unavailable. Checks refresh on folder/activity/tab
changes or explicit request with a two-second rate limit. Concurrent repository edits can
change the view between reads; this is a status indicator, not an atomic repository snapshot.
Initial-directory fallback is labelled and retired on input or reported metadata. Status data
is not stored/exported/logged. Only display/reporting preferences enter portable settings.

Protocol references: [Git porcelain v2](https://git-scm.com/docs/git-status), [Git filters](https://git-scm.com/docs/gitattributes), and [published OSC 7 behaviour](https://wezterm.org/config/lua/pane/get_current_working_dir.html). These are specification/behaviour references; no terminal-engine implementation was copied.
