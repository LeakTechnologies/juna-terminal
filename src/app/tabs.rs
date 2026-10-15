//! Inactive sessions are moved values; all screen state stays on the UI thread.
use super::runtime::Runtime;
use crate::vt::TerminalState;
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
    pub label: String,
    /// Exactly the active slot is None; its state resides in App's active fields.
    pub saved: Option<SavedSession>,
    pub git: super::git_status::View,
    /// `Some(since)` when this (inactive) tab rang BEL and has not been viewed
    /// since. The timestamp drives the flash animation; the flag itself drives
    /// the settled accent colour, cleared when the tab becomes active.
    pub attention: Option<std::time::Instant>,
    pub privacy: crate::privacy::view::TabPrivacy,
}
pub fn label(spec: &crate::session::SessionSpec) -> String {
    let name = std::path::Path::new(&spec.program)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    match name.to_ascii_lowercase().as_str() {
        "powershell" => "PowerShell".into(),
        "pwsh" => "PowerShell 7".into(),
        "cmd" => "Command Prompt".into(),
        _ => name.into_owned(),
    }
}
