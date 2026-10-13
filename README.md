<p align="center">
  <img src="assets/juna-logo.png" alt="Juna logo" width="180" />
</p>

<h1 align="center">Juna Terminal</h1>

<p align="center">A native Windows terminal with an original Rust VT engine.</p>

Juna is a terminal first: responsive shell input, ordered output, readable text and a clear
foundation for a future developer workspace. It is currently ready for interactive testing.

## What works today

- Windows PowerShell and Command Prompt sessions through ConPTY, with clean shell-exit handling.
- An original VT parser and screen engine, scrollback, alternate screen, Unicode clusters and font fallback.
- GPU rendering, local selection, copy/paste, keyboard input and terminal mouse reporting.
- Catppuccin Mocha by default, 20 starter themes, and custom themes saved locally.
- Right-click options with searchable theme and font previews, settings, and Save/Cancel.
- 15 bundled monospace fonts; IBM Plex Mono remains the default. Windows font installation is not required.
- An embedded application icon and a dedicated Windows taskbar application identity.

![Font selection](docs/acceptance/font-menu.png)

## Build and run

Requirements: Windows, Rust 1.97 or newer with the Windows GNU toolchain, and a GNU compiler
toolchain that includes `windres` on PATH. Set `WINDRES` to its executable path if needed.
MSVC resource builds remain future work.

```powershell
cargo build --release --bin juna
.\target\release\juna.exe
```

PowerShell is the default shell. To use Command Prompt:

```powershell
.\target\release\juna.exe --shell cmd.exe
```

Pass shell arguments after `--`; they retain their individual argument boundaries.

## Controls

| Action | Control |
|---|---|
| Options | Right-click |
| Options while a terminal app owns the mouse | Shift+right-click |
| Copy selection | Ctrl+Shift+C |
| Paste | Ctrl+Shift+V or Shift+Insert |
| Scroll history | Mouse wheel or Shift+PageUp / Shift+PageDown |
| Increase font size | Ctrl++ or Ctrl+= |
| Decrease font size | Ctrl+- |
| Reset font size to 16 px | Ctrl+0 |
| Resize with the mouse | Ctrl+mouse wheel |
| Adjust the font-size setting | Left/Right arrows on its row |
| Save settings | Save settings or Ctrl+S |
| Cancel a settings/font preview | Esc or Cancel |

Font size updates immediately and is limited to 8–40 px. Shortcuts used outside the settings
preview save the new size automatically. Font and settings previews stay temporary until saved.

## Fonts and themes

Bundled fonts: IBM Plex Mono, JetBrains Mono, Fira Code, Cascadia Code, Cascadia Mono,
Source Code Pro, Iosevka, Inconsolata, Roboto Mono, Ubuntu Sans Mono, Space Mono, PT Mono,
DM Mono, Victor Mono and Monaspace Neon. Installed monospace fonts are also listed.

Original font licenses and pinned sources are retained in [assets/fonts](assets/fonts/manifest.json).
Bundled notices are also written to `%LOCALAPPDATA%\Juna\font-licenses.txt`. Fourteen families
use SIL OFL 1.1; Ubuntu Sans Mono uses Ubuntu Font Licence 1.0.

Theme choices include Catppuccin, Dracula, Tokyo Night, Gruvbox and Nord. Custom themes and
preferences use versioned JSON under `%LOCALAPPDATA%\Juna`. Theme export is planned.

## Testing and project documents

```powershell
cargo check
cargo clippy --all-targets
cargo test
cargo test --test render_acceptance -- --ignored --test-threads=1
cargo test --test window_acceptance -- --ignored --test-threads=1
```

The last two commands require a real GPU and an interactive Windows desktop.
[TERMINAL_TESTING.md](TERMINAL_TESTING.md) records acceptance evidence and remaining human checks.
Full bidi/contextual text layout, image protocols and a Unix process backend remain unsupported.

- [Architecture](ARCHITECTURE.md)
- [Decisions and dependency provenance](DECISIONS.md)
- [Changelog](CHANGELOG.md)
- [Contribution rules](AGENTS.md)

Junafetch and further bundled terminal tools are being designed; they are not included yet.

See the [roadmap](ROADMAP.md) for tabs, tiled terminals, grouped themes and bundled tools.

