//! Juna Terminal — entry point.
//!
//! # Why the subsystem attribute exists
//!
//! A Rust binary defaults to the **console** subsystem, which makes Windows
//! allocate a console window for the process. That produced two windows on
//! launch: the console box, plus the real terminal window. `windows_subsystem`
//! stops the first one being created.
//!
//! It is applied in **release only**. A debug build keeps its console so
//! diagnostics are visible without a parent terminal, which is worth more during
//! development than a tidy desktop.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// Re-attach to the console we were launched from, if there is one.
///
/// Without this a GUI-subsystem binary has no stdout/stderr at all, so a GPU
/// failure would show as a blank window with no explanation — which the
/// diagnostics requirements in `DECISIONS.md` 5 explicitly reject.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};

    // SAFETY: `AttachConsole` takes no pointers or buffers and imposes no
    // preconditions beyond the process not already owning a console, which is
    // guaranteed here because a GUI-subsystem process has none. Failure is
    // expected and benign when the parent has no console (launched from
    // Explorer), so the result is deliberately discarded.
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() {}

fn main() {
    attach_parent_console();
    if let Err(e) = juna::app::run(100, 30) {
        eprintln!("juna: {e}");
        std::process::exit(1);
    }
}