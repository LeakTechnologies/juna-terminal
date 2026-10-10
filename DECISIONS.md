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

---

## 4. D1–D7 status

| # | Decision | Status | Note |
|---|---|---|---|
| **D1** | Toolchain | **BLOCKED** | MSVC is intended, but `link.exe` is absent. See §7 blocker **B3**. Cannot pin until the toolchain can link. |
| **D2** | Async runtime | **DONE** | Tokio removed. std threads + explicit double-buffered handoff. Evidence in §5. |
| **D3** | Unicode data | **RESEARCHED — awaiting decision** | Comparison in §6. |
| **D4** | Scrollback reflow | **SPECIFIED** | `ARCHITECTURE.md` §3.4. |
| **D5** | ConPTY binding | **RESEARCHED — awaiting decision** | Defects found. Comparison in §6. |
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
| b | `Drop` calls `ClosePseudoConsole` but **never terminates the child**, and there is no job object. Orphaned children are possible. | `process.rs:165-180` | **High** — violates "no leaked child processes" |
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

#### Recommendation (D5, for approval)

**Direct `windows` API**, wrapped in a Smith-owned `PtySession` newtype that enforces single
ownership, explicit shutdown, and job-object-based child cleanup.

Rationale: the requirements that matter most here — explicit handle ownership, no leaked
children, correct argument quoting, clean cancellation — are exactly where `conpty` 0.7 is
weakest, and they are non-negotiable under the project doctrine. `conpty-oxide` has a better
shape but is too young and too thinly maintained to accept as the foundation of process
lifecycle. The honest counter-argument is that we would be writing and owning Win32 unsafe code
that a mature crate already encapsulates.

This remains **unresolved** pending a spike that actually spawns a shell under each candidate
and records exit, resize, and kill behaviour.

---

## 7. Blockers

| # | Blocker | Impact | Needed |
|---|---|---|---|
| **B1** | **No git repository.** Not yet authorized. | Global CI rule requires commit-per-iteration; nothing can be committed. | Explicit authorization to `git init`. |
| **B2** | ~~No project `AGENTS.md`~~ | — | **RESOLVED** — `AGENTS.md` drafted this session. |
| **B3** | **MSVC toolchain cannot link.** `link.exe` absent; VS 2022 BuildTools present but without the C++ workload; no Windows SDK. | **D1 is blocked.** Pinning MSVC now would break every build immediately. | Install "Desktop development with C++" workload + Windows SDK, or ratify MinGW as the target. |
| **B4** | **`pwsh` is not installed.** | The designated **primary** acceptance target does not exist on this machine. | Install PowerShell 7, or re-designate the primary target. |
| **B5** | ConPTY child lifecycle unproven | Phase 1 cannot start | Resolve D5 via spike. |

### B3 detail — why D1 is blocked rather than merely deferred

D1 was answered "MSVC as the intended toolchain, subject to a clean build." There is no clean
MSVC build to be had: the toolchain cannot link anything at all.

```
> rustc +stable-x86_64-pc-windows-msvc probe.rs -o probe.exe
error: linker `link.exe` not found
note: the msvc targets depend on the msvc linker but `link.exe` was not found
note: please ensure that Visual Studio 2017 or later, or Build Tools for Visual Studio
      were installed with the Visual C++ option
```

The GNU toolchain links successfully today (`link ok`), so the project is currently buildable
on MinGW only. `rust-toolchain.toml` has **not** been created, because writing it would pin
every subsequent build to a broken toolchain.

### B4 detail — acceptance target

Only Windows PowerShell 5.1 is present. `cmd.exe` is available. The primary target `pwsh` is
absent from Program Files, Program Files (x86), WindowsApps, and PATH. Note that the ruling
also requires conformance tests to be shell-independent, so this blocks *acceptance testing of
the transport layer*, not the engine's test suite.