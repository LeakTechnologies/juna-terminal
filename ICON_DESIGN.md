# Application icon wiring

- Use the approved `assets/juna-logo.png` and its generated `assets/juna.ico` unchanged.
- Embed the ICO as a Windows executable resource using the GNU toolchain resource compiler; add no Rust dependency. Supply the window icon through Windows resource access and winit's platform icon API, without touching terminal state.
- Acceptance: validate executable icon resource entries, verify the live window has its icon, run required checks, and rebuild/push. Small-size artwork refinement stays separately reviewable.

Approved by the design lead on 2026-10-13: implement the existing approved icon.

