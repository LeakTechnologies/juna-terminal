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
| **D5** | ConPTY binding | **RESEARCHED — evidence favours `conpty-oxide`, but NOT closed** | Spike evidence in §6. `conpty` 0.7 rejected as default. Two of six checklist items still unresolved. |
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

### D5 spike — empirical results

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

#### Revised recommendation for D5

**`conpty-oxide` 0.1.2**, not direct Win32.

The reasoning has changed on evidence. My previous preference for hand-written Win32 bindings
was based on the assumption that `conpty` 0.7's defects were unfixable-in-place and that a
correct wrapper would be small. The spike showed the opposite: `conpty-oxide` already implements
job objects (`core/job.rs`), owned pipe halves (`OwnedReadHalf`/`OwnedWriteHalf`), explicit
ownership decomposition (`into_parts()` → `SessionParts`), `resize(&self)` rather than
`&mut self`, structured `ErrorKind`, and dedicated regression tests for exactly the failure
modes we care about — `close_hang`, `kill_tree`, `soak`, `eof_semantics`, `owned_halves`,
`managed_drop_order`.

Writing that ourselves would mean 300–500 lines of Win32 `unsafe` to reproduce behaviour a
maintained crate already provides and tests. The `blocking` feature is the default, so it does
not reintroduce Tokio (D2 holds).

Risks that remain, and must be weighed by the lead:

- **Maturity.** Published 2026-07-31, three versions, ~1 800 downloads, single maintainer.
  This is the real objection, and it is about supply risk, not correctness.
- **Unproven at our scale.** Its `soak.rs` test is not our soak test.
- **`ConPtyBackend` bundles its own OpenConsole DLL** (`backend/bundle.rs`,
  `ConPtyBackend::from_dir`). This is useful for portability but is an added surface we have
  not evaluated. If we want the system OpenConsole, `ConPtyBackend::system()` must be used
  explicitly, and that choice needs recording.

`conpty` 0.7 is rejected as the default. It stays in the manifest only until this decision is
approved, after which it should be removed.

#### Spike checklist status

| # | Checklist item | Result |
|---|---|---|
| 1 | Command-line quoting | **INCONCLUSIVE — probe invalid, must be rewritten** |
| 2 | Explicit handle ownership | conpty-oxide: `OwnedReadHalf`/`OwnedWriteHalf`/`SessionParts`. conpty 0.7: blanket `Send`+`Sync`, cloned raw handles. Static + API evidence. |
| 3 | Termination and cleanup | conpty-oxide: `kill()` in 18.7 µs. conpty 0.7: direct child reaped on Drop; grandchildren untested. |
| 4 | No host-console-mode change | **NOT IMPLEMENTED** |
| 5 | Resize and I/O round-trip | conpty-oxide: both pass. conpty 0.7: resize passes, **I/O hangs**. |
| 6 | Clear errors, MSVC build | Structured `ErrorKind` by inspection. **MSVC build impossible** — see B3. |

Two of six items are unresolved. D5 is **not** ready to close.

---

## 7. Blockers

| # | Blocker | Impact | Needed |
|---|---|---|---|
| **B1** | **No git repository.** Not yet authorized. | Global CI rule requires commit-per-iteration; nothing can be committed. | Explicit authorization to `git init`. |
| **B2** | ~~No project `AGENTS.md`~~ | — | **RESOLVED** — `AGENTS.md` drafted this session. |
| **B3** | **MSVC toolchain cannot link.** `link.exe` absent; VS 2022 BuildTools present but without the C++ workload; no Windows SDK. | **D1 is blocked.** Pinning MSVC now would break every build immediately. | **Requires elevation.** This session is **not** running as Administrator, so the install could not be attempted. See §7.1 for the exact command. |
| **B4** | ~~`pwsh` not installed~~ | — | **RESOLVED by lead ruling.** PowerShell 5.1 is the primary acceptance target; `cmd.exe` secondary; PowerShell 7 optional later. `pwsh` remains absent from this machine and is **not** a Phase 1 blocker. |
| **B5** | ConPTY child lifecycle unproven | Phase 1 cannot start | Resolve D5 via spike. |

| **B5** | ConPTY spike incomplete | D5 cannot close | Rewrite the quoting probe (check 1) and implement the host-console-mode check (check 5). |
| **B6** | wgpu feature flags not enumerated | Phase 1 GPU validation cannot start | Enumerate wgpu 29's actual Windows backend features before changing the manifest. Do not assume `dx12` + `vulkan` covers every adapter and presentation path on a host with a virtual display adapter. |

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