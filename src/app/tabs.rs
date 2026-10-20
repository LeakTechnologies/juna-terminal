//! Inactive sessions are moved values; all screen state stays on the UI thread.
use super::runtime::Runtime;
use crate::vt::TerminalState;
use std::path::Path;
pub type Selection = Option<((usize, u16), (usize, u16))>;
pub struct SavedSession {
    pub terminal: TerminalState,
    pub runtime: Option<Runtime>,
    pub offset: usize,
    pub selection: Selection,
    pub failure: Option<String>,
    pub closing: bool,
}
pub struct Tab {
    pub id: u64,
    /// Effective display name: the manual rename when pinned, otherwise the
    /// directory/shell title recomputed from OSC 7 metadata.
    pub label: String,
    /// Set by Rename tab. A pinned name no longer follows directory changes.
    pub pinned: bool,
    /// Shell display name captured at spawn, e.g. `PowerShell`.
    pub shell: String,
    /// Exactly the active slot is None; its state resides in App's active fields.
    pub saved: Option<SavedSession>,
    pub git: super::git_status::View,
    /// `Some(since)` when this (inactive) tab rang BEL and has not been viewed
    /// since. The timestamp drives the flash animation; the flag itself drives
    /// the settled accent colour, cleared when the tab becomes active.
    pub attention: Option<std::time::Instant>,
    pub privacy: crate::privacy::view::TabPrivacy,
}
/// Human-readable shell name for the tab suffix.
pub fn shell_name(spec: &crate::session::SessionSpec) -> String {
    let name = Path::new(&spec.program)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    match name.to_ascii_lowercase().as_str() {
        "powershell" => "PowerShell".into(),
        "pwsh" => "PowerShell 7".into(),
        "cmd" => "Command Prompt".into(),
        "bash" => "Bash".into(),
        _ => name.into_owned(),
    }
}
/// Directory component only; no I/O. Drive and share roots keep their path text.
pub fn directory_name(path: &Path) -> Option<String> {
    if let Some(name) = path.file_name() {
        let name = name.to_string_lossy();
        if !name.is_empty() {
            return Some(name.into_owned());
        }
    }
    let text = path.to_string_lossy();
    if text.is_empty() {
        return None;
    }
    #[cfg(windows)]
    let text = text.replace('/', "\\");
    Some(text.to_string())
}
/// `Directory (Shell)`. Without a directory the shell name stands alone.
pub fn title(directory: Option<&Path>, shell: &str) -> String {
    match directory.and_then(directory_name) {
        Some(directory) if !shell.is_empty() => format!("{directory} ({shell})"),
        Some(directory) => directory,
        None => shell.to_owned(),
    }
}
/// Initial title from the launch specification.
pub fn label(spec: &crate::session::SessionSpec) -> String {
    title(spec.cwd.as_deref(), &shell_name(spec))
}
/// Final index after removing `from` and inserting at `gap`, where `gap` is a
/// position in the pre-removal vector. `None` means the order would not change.
pub fn reorder_target(from: usize, gap: usize, len: usize) -> Option<usize> {
    if from >= len {
        return None;
    }
    let gap = gap.min(len);
    let to = if gap > from { gap - 1 } else { gap };
    (to != from).then_some(to)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn spec(program: &str, cwd: Option<&str>) -> crate::session::SessionSpec {
        crate::session::SessionSpec {
            program: program.into(),
            cwd: cwd.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn titles_put_the_directory_before_the_shell() {
        assert_eq!(
            title(Some(Path::new("E:/development/VideoTools")), "PowerShell"),
            "VideoTools (PowerShell)"
        );
        assert_eq!(title(Some(Path::new("C:/")), "PowerShell"), "C:\\ (PowerShell)");
        assert_eq!(title(None, "Command Prompt"), "Command Prompt");
        assert_eq!(title(Some(Path::new("")), "Bash"), "Bash");
        assert_eq!(title(Some(Path::new("E:/日本語")), "Bash"), "日本語 (Bash)");
    }

    #[test]
    fn launch_labels_use_the_launch_directory_and_shell() {
        assert_eq!(
            label(&spec("powershell.exe", Some("E:/development/Juna"))),
            "Juna (PowerShell)"
        );
        assert_eq!(label(&spec("cmd.exe", None)), "Command Prompt");
        assert_eq!(label(&spec("pwsh.exe", None)), "PowerShell 7");
        assert_eq!(label(&spec("bash.exe", None)), "Bash");
        assert_eq!(label(&spec("nu.exe", None)), "nu");
    }

    #[test]
    fn reorder_target_accounts_for_the_removed_element() {
        // Moving right: insert after the item that followed the target gap.
        assert_eq!(reorder_target(0, 3, 4), Some(2));
        // Moving left: the gap index is already the destination.
        assert_eq!(reorder_target(3, 1, 4), Some(1));
        // Dropping into either adjacent gap is a no-op.
        assert_eq!(reorder_target(1, 1, 4), None);
        assert_eq!(reorder_target(1, 2, 4), None);
        // Ends are inclusive gaps.
        assert_eq!(reorder_target(1, 4, 4), Some(3));
        assert_eq!(reorder_target(2, 0, 4), Some(0));
        assert_eq!(reorder_target(4, 0, 4), None);
        assert_eq!(reorder_target(0, 99, 4), Some(3));
    }
}
