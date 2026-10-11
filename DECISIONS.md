# Smith — Decision Register

Status: **Phase 0 reconnaissance complete. Engine coding remains unauthorized.**

Last updated: 2026-10-10, following design-lead ruling on D1–D7.

---

## 1. Verified environment facts

Established by direct inspection and direct execution. Not inferred, not assumed.

| Fact | Value | How it was established |
|---|---|---|
| Working directory | `E:\Development\smith-terminal` | `Get-Location` |
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

| Check | Result |
|---|---|
| `cargo check` | **PASS** |
| `cargo clippy --all-targets` | **PASS** — zero warnings |
| `cargo test` | **PASS but vacuous — `0 passed; 0 failed`. Zero tests exist.** |
| `alacritty_terminal` / `vte` in `Cargo.lock` | **ABSENT — confirmed** |
| Direct `tokio` / `serde` / `serde_json` / `anyhow` deps | **REMOVED — confirmed** |
| Window opened | **NO** |
| GPU adapter enumerated | **NO** |
| ConPTY child spawned | **NO** |
| Interactive shell round-trip | **NO** |
| Process cleanup verified | **NO** |

A green build is build evidence, not proof of a functioning terminal. Zero tests exist.

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
| **D6** | Font shaping | **OPEN** | Not started. Must be prototyped before renderer work. |
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

**Recommendation:** adopt `unicode-width` + `unicode-segmentation` **behind a Smith-owned pure
API** (`smith_term::text::width(c) -> Columns`, `graphemes(s) -> impl Iterator<Item = &str>`),
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
Smith owns a thin wrapper so the engine/transport boundary stays explicit.

The concern is supply and maintenance risk, not correctness. Three safeguards are recorded in
place of reimplementing several hundred lines of Win32 `unsafe`:

1. **Pin and regress.** Version stays pinned via `Cargo.lock`. Smith's own acceptance tests in
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

   Replacement means either a vetted successor or a Smith-owned Win32 implementation, decided
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
and adopting Alacritty's own rasterization component would put Smith on Alacritty's renderer
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
layout for continuous prose. Smith has a **fixed cell grid** in which every glyph's position is
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
question is whether unhinted grayscale output is acceptable for Smith's default rendering, which
is a visual judgement that measurements alone cannot settle.

### 6.9 Cascadia Mono is absent from this host

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
[PASS] conpty-oxide    SMITH_RT_9F3A

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
Its `blocking` feature is default, so D2 holds. Smith still owns a thin wrapper so the boundary
between engine and transport stays explicit.

Outstanding risk, unchanged and unmeasured by this spike: **maturity**. Published 2026-07-31,
three versions, ~1 800 downloads, single maintainer. `ConPtyBackend` also supports bundling its
own OpenConsole DLL; we should pin `ConPtyBackend::system()` unless portability is wanted, and
record that choice.

`conpty` 0.7 should be removed from `[dependencies]` once this is approved. It is currently
retained only so the spike can keep comparing against it.

---

## 7. Blockers

| # | Blocker | Impact | Needed |
|---|---|---|---|
| **B1** | **No git repository.** Not yet authorized. | Global CI rule requires commit-per-iteration; nothing can be committed. | Explicit authorization to `git init`. |
| **B2** | ~~No project `AGENTS.md`~~ | — | **RESOLVED** — `AGENTS.md` drafted this session. |
| **B3** | **MSVC toolchain cannot link.** `link.exe` absent; VS 2022 BuildTools present but without the C++ workload; no Windows SDK. | **D1 is blocked.** Pinning MSVC now would break every build immediately. | **Requires elevation.** This session is **not** running as Administrator, so the install could not be attempted. See §7.1 for the exact command. |
| **B4** | ~~`pwsh` not installed~~ | — | **RESOLVED by lead ruling.** PowerShell 5.1 is the primary acceptance target; `cmd.exe` secondary; PowerShell 7 optional later. `pwsh` remains absent from this machine and is **not** a Phase 1 blocker. |
| **B5** | ~~ConPTY spike incomplete~~ | — | **RESOLVED.** Check 1 (quoting) and check 5 (host console mode) both run and pass for `conpty-oxide`. Check 6's MSVC leg remains blocked by B3. See §6.2. |
| **B6** | ~~wgpu feature flags not enumerated~~ | — | **RESOLVED.** Feature names, defaults, and dependency consequences recorded in §6.1; configuration applied and verified building. |
| **B7** | ~~GPU adapter enumeration~~ | — | **PARTIALLY CLOSED.** Five adapters enumerated (`DECISIONS.md` §6.4). Parsec virtual adapter **not** exposed — recorded as a negative result, not as absence. Surface creation and presentation still untested. |

### 7.1 B3 detail — MSVC cannot be installed from this session

```
> rustc +stable-x86_64-pc-windows-msvc probe.rs -o probe.exe
error: linker `link.exe` not found

> IsInRole(Administrator) -> False
> winget -> present at C:\Users\User\AppData\Local\Microsoft\WindowsApps\winget.exe
> free space on C: -> 134.4 GB
```

`rust-toolchain.toml` has **not** been created, per the lead's ruling. The GNU toolchain links
successfully and remains the working default.

Installing the C++ workload requires elevation, which this session cannot obtain. Rather than
fire an unattended UAC prompt, the install is left as a single command for the operator to run
from an elevated shell:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools `
  --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" `
  --accept-package-agreements --accept-source-agreements
```

Validation afterwards, and only then is `rust-toolchain.toml` justified:

```powershell
rustc +stable-x86_64-pc-windows-msvc probe.rs -o probe_msvc.exe   # must print "link ok"
```

### 7.2 Process lifecycle evidence

Git initialized and the first commit made (`63198c9`) after reviewing staged contents: 7 files,
no `target/`, no binaries, no secrets. B1 is resolved.