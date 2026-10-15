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
    use windows::Win32::{
        Storage::FileSystem::{GetFileType, FILE_TYPE_UNKNOWN},
        System::Console::{
            AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
            STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        },
    };

    // SAFETY: `AttachConsole` takes no pointers or buffers and imposes no
    // preconditions beyond the process not already owning a console, which is
    // guaranteed here because a GUI-subsystem process has none. Failure is
    // expected and benign when the parent has no console (launched from
    // Explorer), so the result is deliberately discarded.
    unsafe {
        // Preserve caller-owned redirection: attaching a console must not replace valid pipe/file handles.
        let saved = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE].map(|kind| {
            (
                kind,
                GetStdHandle(kind).ok().filter(|handle| {
                    !handle.is_invalid() && GetFileType(*handle) != FILE_TYPE_UNKNOWN
                }),
            )
        });
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        for (kind, handle) in saved {
            if let Some(handle) = handle {
                let _ = SetStdHandle(kind, handle);
            }
        }
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() {}

fn main() {
    attach_parent_console();
    match juna::updates::cli::parse(std::env::args_os().skip(1)) {
        Ok(Some(action)) => {
            let code = match juna::updates::cli::run(action) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("juna: {error}");
                    1
                }
            };
            std::process::exit(code);
        }
        Err(error) => {
            eprintln!("juna: {error}");
            std::process::exit(2);
        }
        Ok(None) => {}
    }
    if let Some(busy) = juna::updates::helper::installation_state() {
        if busy {
            eprintln!("Juna update is in progress; please wait for the automatic restart.");
        } else {
            juna::app::update_error_dialog("A Juna update was interrupted. Previous files are retained in the installation folder's .juna-rollback folder. Restore the complete previous package or extract a verified GitHub release, then remove .juna-installing.");
        }
        std::process::exit(1);
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "--update-helper")
    {
        let result = std::env::args_os()
            .nth(2)
            .ok_or_else(|| "Updater helper plan missing".to_owned())
            .and_then(|path| juna::app::run_update_helper(std::path::Path::new(&path)));
        if result.is_err() {
            std::process::exit(1);
        }
        return;
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--version")
    {
        println!(
            "juna {} ({})",
            env!("CARGO_PKG_VERSION"),
            env!("JUNA_BUILD_COMMIT")
        );
        return;
    }
    if let Err(e) = juna::app::run(100, 30) {
        eprintln!("juna: {e}");
        std::process::exit(1);
    }
}
