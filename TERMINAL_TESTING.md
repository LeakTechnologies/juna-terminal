# Testing the repaired terminal

The 2026-10-12 audit repair design is approved. This is a working Windows testing milestone,
not a claim of complete developer-grade compatibility. The engine remains Juna's own implementation.

## Launch and controls

Build with `cargo build --release --bin juna`, then run `target\release\juna.exe`.
The default shell is Windows PowerShell 5.1. To test cmd, run
`target\release\juna.exe --shell cmd.exe`. Shell arguments go after `--`, for example
`target\release\juna.exe --shell powershell.exe -- -NoProfile`.
`JUNA_SHELL` remains supported; `JUNA_SHELL_ARGS` now respects quoted argument boundaries.

Space, Ctrl-C, Ctrl-L, Enter, Backspace, Delete, Insert, navigation keys, modifiers, and F1–F12
are encoded explicitly. The shell interprets `exit`; the single terminal window closes after
the actual child exits and its transport threads finish. Nested programs own their own input.

Drag left to select, Ctrl+Shift+C to copy, Ctrl+Shift+V or Shift+Insert to paste.
Right-click pastes. Wheel or Shift+PageUp/PageDown navigates history. Typing returns to live output.
Hold Shift to use local mouse selection/scrolling while a TUI has enabled mouse reporting.
Multiline unbracketed paste sends line endings to the shell, so use clipboard content you intend
to execute. Bracketed-paste-aware programs receive the matching start/end delimiters.

Diagnostics are written to `%LOCALAPPDATA%\Juna\juna.log`; startup/session failure also displays
a Windows error dialog. `RUST_LOG=info` includes adapter, presentation and lifecycle evidence.
The latest launch replaces that log; copy it before starting another diagnostic run.

## Automated acceptance commands

```
cargo check
cargo clippy --all-targets
cargo test
cargo test --test render_acceptance -- --ignored
cargo test --test window_acceptance -- --ignored
```

The last two tests require an interactive Windows desktop, a graphics backend, and system fonts,
so ordinary Cargo test lists them as ignored. They must be explicitly executed on the target host.
They do not use shell prompt matching or inject operating-system keystrokes.

The GPU test compares row pixels before and after editing the next row and captures
`target\acceptance\render.ppm`. The window test opens the actual winit application separately
with cmd and PowerShell, verifies presented frames and real exit codes, and writes isolated logs
under `target\acceptance\<shell>\Juna\juna.log`. These are synthetic test data, not user sessions.

## Verification matrix

| Area | Evidence | Status |
|---|---|---|
| Space/control/modifier/dead-key encoding | Pure exact-byte tests, including AltGr, keypad and mouse modes | Verified in this session |
| Row corruption, CR/wrap, large parameters, ED, insert/delete, resize, wide pairs | Permanent regression fixtures and chunk-boundary equivalence | Verified in this session |
| Real process lifecycle | cmd and PowerShell tail markers, nonzero exit and joined workers | Verified in this session |
| Byte integrity and process-tree teardown | Existing transport/ConPTY acceptance suite | Verified in this session |
| GPU row isolation and text | Pixel comparison and captured render frame | Verified in this session |
| Window presentation and natural exit | Vulkan on RTX 3060; cmd exit 0 and PowerShell exit 7 | Verified in this session |
| Physical typing, Ctrl-C in an interactive program | Human smoke checklist below | Pending human review |
| Clipboard ownership/paste in the real UI | Platform integration plus pure paste encoding; no OS clipboard test performed | Pending human review |
| Mixed DPI, minimize/restore, surface/device disruption | Code paths present; no multi-monitor/failure injection run | Pending human review |
| Real editor and pager | Input/mode/alternate-screen fixtures; no editor/pager session recorded | Pending human review |

The first window test failed because inherited `RUST_LOG=warn` hid its evidence lines. The test
now explicitly sets `RUST_LOG=info`; both shell cases passed on rerun. This was a test-environment
failure, not a presentation failure. Previous audit failures remain recorded in AUDIT.md.

A later repeat caught the reader-first shutdown race, which is corrected and covered by a
deterministic test. Earlier green runs alone did not prove that interleaving. The final standard
suite has 165 passes, and the two desktop/GPU acceptance tests are run separately.

## Human smoke checklist

1. Type a command containing spaces, edit it with arrows/Home/End/Delete/Backspace, and execute it.
   Repeat near the right edge and after enough output to scroll. Watch the row above the input.
2. Run a long-lived process, interrupt with Ctrl-C, and confirm a usable prompt returns.
3. Print accented text, CJK, emoji, colored text, and box drawing; compare the captured sample.
4. Drag/select/copy and paste one line, then multiple lines in a bracketed-paste-aware program.
5. Scroll back while new output arrives. Return to live output by typing. Resize small/large,
   minimize/restore, and move across monitors with different scaling.
6. Run an installed editor and pager; check alternate-screen restoration, cursor hide/show,
   navigation, mouse reporting and exit. Record application/version, observed bytes if available,
   screenshots and the diagnostic log for a failure.
7. Type `exit` in each shell, and close during heavy output. Confirm no orphan process survives.

## Explicit remaining scope

OSC 52 clipboard is denied. Graphics protocols, panes/editor/drawer features, and the Unix PTY
backend are deferred. Full bidi layout and cross-cell contextual shaping are not implemented;
current shaping handles each stored grapheme cluster. Fonts can still lack a requested glyph,
which is recorded as a miss. The atlas has a finite per-frame capacity and reports exhaustion;
it no longer permanently blacklists a glyph because an earlier frame filled it.

This evidence supports further interactive testing. It does not replace the human checklist or
claim all existing Tier 2 requirements have shipped. DX12/GLES presentation is not verified by the
Vulkan result. No benchmark or universal compatibility claim is made.

## Recorded render sample

![GPU acceptance frame](docs/acceptance/render.png)
