//! Single-owner window, input, engine and rendering integration.
mod clipboard;
mod file_drop;
mod git_status;
pub mod gutter;
mod input;
#[cfg(all(test, windows))]
mod interrupt_acceptance;
#[doc(hidden)]
pub mod new_window;
mod privacy_input;
mod runtime;
mod settings_files;
mod shell_reporting;
mod shells;
mod tabs;
#[cfg(windows)]
mod tools;
pub mod ui;
use crate::{
    gpu::{FrameSkip, Gpu},
    session::{Dimensions, SessionSpec},
    vt::{CellContent, TerminalState, MAX_COLS, MAX_ROWS},
};
use runtime::Runtime;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};
const FRAME: Duration = Duration::from_millis(16);
const PAD: f32 = 8.;
const CELL_W: f32 = 10.;
const CELL_H: f32 = 22.;
const FONT_PX: f32 = 16.;
#[cfg(windows)]
const APP_ID: &str = "LeakTechnologies.Juna.Terminal.Desktop";
#[cfg(windows)]
fn taskbar_properties(hwnd: windows::Win32::Foundation::HWND) -> Result<(), String> {
    use windows::{
        core::GUID,
        Win32::{
            Foundation::PROPERTYKEY,
            System::Com::StructuredStorage::PROPVARIANT,
            UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow},
        },
    };
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let executable = executable
        .to_str()
        .ok_or("Executable path is not valid Unicode")?;
    // SAFETY: winit owns this live HWND on the UI thread and initializes its OLE apartment during creation.
    let store: IPropertyStore = unsafe { SHGetPropertyStoreForWindow(hwnd) }
        .map_err(|e| format!("Taskbar property store: {e}"))?;
    for (pid, text) in [
        (2, format!("\"{executable}\"")),
        (4, "Juna Terminal".into()),
        (3, format!("{executable},-1")),
        (5, APP_ID.into()),
    ] {
        let key = PROPERTYKEY {
            fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
            pid,
        };
        let value = PROPVARIANT::from(text.as_str());
        // SAFETY: this owned string variant stays live for the synchronous copying call.
        unsafe { store.SetValue(&key, &value) }
            .map_err(|e| format!("Taskbar property {pid}: {e}"))?;
    }
    // SAFETY: the property store is live and belongs to this window's UI-thread apartment.
    unsafe { store.Commit() }.map_err(|e| format!("Taskbar property commit: {e}"))?;
    log::info!("Taskbar relaunch icon configured");
    Ok(())
}
/// In-progress tab drag. Index and window origin are captured at press so the
/// permutation is resolved against a stable order; only the geometry scrolls.
#[derive(Clone, Copy)]
struct TabDrag {
    from: usize,
    press_x: f32,
    /// Strip window origin, advanced when the pointer reaches a strip edge.
    start: usize,
    moved: bool,
}
pub struct App {
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    renderer: Option<crate::render::Renderer>,
    terminal: TerminalState,
    runtime: Option<Runtime>,
    modifiers: ModifiersState,
    dirty: bool,
    focused: bool,
    minimized: bool,
    ime: bool,
    scale: f32,
    offset: usize,
    selection: Option<((usize, u16), (usize, u16))>,
    selecting: bool,
    mouse_button: Option<u8>,
    frames: u64,
    pointer: (u16, u16),
    cols: u16,
    rows: u16,
    last_report: Instant,
    failure: Option<String>,
    last_sync: Instant,
    left_inset: f32,
    settings: crate::settings::Settings,
    themes: Vec<crate::theme::Theme>,
    store: crate::settings::Store,
    ui: Option<ui::Ui>,
    pointer_px: [f32; 2],
    ui_mouse_latch: Vec<MouseButton>,
    settings_before: Option<crate::settings::Settings>,
    settings_file_task: Option<settings_files::ResultChannel>,
    pending_import: Option<crate::settings_transfer::ImportPlan>,
    zoom_wheel: f32,
    tabs: Vec<tabs::Tab>,
    active_tab: usize,
    next_tab_id: u64,
    tab_drag: Option<TabDrag>,
    shell_spec: Option<SessionSpec>,
    shell_explicit: bool,
    active_closing: bool,
    closing_all: bool,
    theme_worker: Option<crate::theme_library::Worker>,
    theme_sources: std::collections::BTreeMap<String, std::path::PathBuf>,
    theme_generation: u64,
    last_theme_scan: Instant,
    theme_errors: Vec<String>,
    removed_theme: Option<(std::path::PathBuf, std::path::PathBuf)>,
    git_worker: Option<git_status::Worker>,
    git_activity: Instant,
    update_worker: Option<crate::updates::Worker>,
    update_candidate: Option<crate::updates::Candidate>,
    update_stage: Option<crate::updates::Staged>,
    update_message: String,
    update_cancelled: bool,
    update_progress: u64,
    update_receipt_until: Option<Instant>,
    privacy_broker: Option<crate::privacy::ipc::Broker>,
    privacy_reveal: crate::privacy::view::Reveal,
    privacy_dirty: bool,
    command_update_pending: bool,
    command_update_channel: Option<String>,
    last_update_receipt: Instant,
}
impl App {
    fn accept_downloaded_update(&mut self, stage: crate::updates::Staged) {
        self.update_stage = Some(stage);
        self.update_message = "Checksum, version and source verified".into();
        if std::mem::take(&mut self.command_update_pending) {
            self.open_ui(ui::Kind::ConfirmUpdate);
        }
    }
    fn command_update(&mut self, channel: String) {
        if self.update_worker.is_some() || self.closing_all {
            return;
        }
        log::info!("Authenticated command-line update request accepted; channel={channel}");
        self.command_update_channel = Some(channel.clone());
        self.open_ui(ui::Kind::Updates);
        let discard = self.update_stage.take().map(|stage| stage.root);
        self.update_candidate = None;
        self.command_update_pending = true;
        if let Err(error) = self.start_update_task(
            crate::updates::Task::Check {
                channel,
                current: env!("CARGO_PKG_VERSION").into(),
                directory: self.store.directory.clone(),
                discard,
            },
            "Command-line update: checking GitHub...",
        ) {
            self.command_update_pending = false;
            self.update_message = error;
            self.refresh_update_panel();
        }
    }
    fn start_update_task(
        &mut self,
        task: crate::updates::Task,
        message: &str,
    ) -> Result<(), String> {
        if self.update_worker.is_some() || self.closing_all {
            return Err("Wait for the current operation or cancel it first".into());
        }
        self.update_worker = Some(crate::updates::Worker::start(task)?);
        self.update_cancelled = false;
        self.update_progress = 0;
        self.update_message = message.into();
        self.refresh_update_panel();
        Ok(())
    }
    fn refresh_update_panel(&mut self) {
        if let Some(ui) = self.ui.as_mut().filter(|ui| ui.kind == ui::Kind::Updates) {
            ui.summary = vec![format!("Installed: v{}", env!("CARGO_PKG_VERSION"))];
            if let Some(channel) = self.command_update_channel.as_ref() {
                ui.summary.push(format!("Command channel: {channel}"));
            }
            if let Some(candidate) = self.update_candidate.as_ref() {
                ui.summary
                    .push(format!("Available: v{}", candidate.version));
            }
            if let Some(stage) = self.update_stage.as_ref() {
                ui.summary.push(format!("Verified: v{}", stage.version));
            }
            let mut line = String::new();
            for word in self.update_message.split_whitespace() {
                if !line.is_empty() && line.len() + word.len() + 1 > 32 {
                    ui.summary.push(std::mem::take(&mut line));
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
            if !line.is_empty() {
                ui.summary.push(line);
            }
            ui.message = "Manual checks; verified downloads".into();
            self.dirty = true;
        }
    }
    fn poll_updates(&mut self) {
        if self
            .update_receipt_until
            .is_some_and(|deadline| Instant::now() < deadline)
            && self.last_update_receipt.elapsed() > Duration::from_secs(1)
        {
            self.last_update_receipt = Instant::now();
            if let Some(receipt) = crate::updates::helper::take_receipt(&self.store.directory) {
                self.update_receipt_until = None;
                self.update_message=match receipt.status {
                    crate::updates::ReceiptStatus::Updated=>format!("Updated to v{}",receipt.version),
                    crate::updates::ReceiptStatus::RolledBack=>"Update failed; previous version kept. Close other windows and retry.".into(),
                    crate::updates::ReceiptStatus::Failed=>"Update failed; recover the .juna-rollback folder in the installation directory.".into(),
                };
                if !matches!(receipt.status, crate::updates::ReceiptStatus::Updated) {
                    self.open_ui(ui::Kind::Updates);
                }
                self.refresh_update_panel();
            }
        }
        let Some(worker) = self.update_worker.as_mut() else {
            return;
        };
        let progress = worker.progress.load(std::sync::atomic::Ordering::Relaxed);
        let result = worker.poll();
        if let Some(result) = result {
            self.update_worker.take();
            if self.update_cancelled {
                self.command_update_pending = false;
                let root = match result {
                    Ok(crate::updates::Outcome::Downloaded(stage)) => Some(stage.root),
                    Ok(crate::updates::Outcome::Prepared(plan)) => {
                        plan.parent().map(std::path::Path::to_path_buf)
                    }
                    _ => None,
                };
                if let Some(root) = root {
                    let _ = self.start_update_task(
                        crate::updates::Task::Discard {
                            root,
                            directory: self.store.directory.clone(),
                        },
                        "Discarding cancelled staging...",
                    );
                }
                self.update_stage = None;
                self.update_message = "Update cancelled".into();
            } else {
                match result {
                    Ok(crate::updates::Outcome::Discarded) => {
                        self.update_message = "Staged update discarded".into()
                    }
                    Ok(crate::updates::Outcome::Checked(candidate)) => {
                        self.update_message = if candidate.is_some() {
                            "New release available; download to verify"
                        } else {
                            "No newer compatible release in this channel"
                        }
                        .into();
                        self.update_candidate = candidate;
                        if std::mem::take(&mut self.command_update_pending)
                            && self.update_candidate.is_some()
                        {
                            self.command_update_pending = true;
                            self.ui_action(ui::Action::DownloadUpdate);
                        }
                    }
                    Ok(crate::updates::Outcome::Downloaded(stage)) => {
                        self.accept_downloaded_update(stage);
                    }
                    Ok(crate::updates::Outcome::Prepared(path)) => {
                        match crate::updates::helper::launch(&path) {
                            Ok(()) => {
                                self.update_stage = None;
                                self.close_ui();
                                self.close_all_tabs();
                                return;
                            }
                            Err(error) => self.update_message = error,
                        }
                    }
                    Err(error) => {
                        self.command_update_pending = false;
                        self.update_message = error;
                    }
                }
            }
            self.refresh_update_panel();
        } else if progress != self.update_progress {
            self.update_progress = progress;
            if !self.update_cancelled {
                self.update_message =
                    format!("Transferred {:.1} MiB", progress as f64 / (1024. * 1024.));
            }
            self.refresh_update_panel();
        }
    }
    fn poll_git_status(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let (revision, cwd) = self.terminal.working_directory();
        let active = self.active_tab;
        let tab = &mut self.tabs[active];
        let old = tab.git.generation;
        tab.git.sync(revision, cwd);
        let mut changed = old != tab.git.generation;
        if !tab.pinned {
            let title = tabs::title(cwd.map(std::path::Path::new), &tab.shell);
            if title != tab.label {
                tab.label = title;
                changed = true;
            }
        }
        if changed {
            self.dirty = true;
        }
        let Some(worker) = self.git_worker.as_ref() else {
            return;
        };
        while let Ok(result) = worker.results.try_recv() {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == result.tab) {
                if let Some(saved) = tab.saved.as_ref() {
                    let (revision, cwd) = saved.terminal.working_directory();
                    tab.git.sync(revision, cwd);
                }
                if result.generation == tab.git.generation {
                    #[cfg(debug_assertions)]
                    if std::env::var_os("JUNA_ACCEPT_GIT_STATUS").is_some() {
                        log::info!(
                            "Git acceptance repository={}",
                            matches!(result.report, git_status::Report::Repo(_))
                        );
                    }
                    tab.git.report = Some(result.report);
                    tab.git.pending = false;
                    self.dirty = true;
                }
            }
        }
        if !self.settings.show_git_status
            || self.terminal.alternate_active()
            || !self.focused
            || self.closing_all
        {
            return;
        }
        let tab = &mut self.tabs[self.active_tab];
        if tab.git.pending
            && tab
                .git
                .last_check
                .is_some_and(|time| time.elapsed() > Duration::from_secs(20))
        {
            tab.git.pending = false;
            tab.git.generation = tab.git.generation.wrapping_add(1);
        }
        if tab.git.pending
            || tab.git.last_check.is_some_and(|time| {
                time.elapsed() < Duration::from_secs(2) || time >= self.git_activity
            })
        {
            return;
        }
        let Some(cwd) = tab.git.cwd.clone() else {
            return;
        };
        if worker
            .requests
            .try_send(git_status::Request {
                tab: tab.id,
                generation: tab.git.generation,
                cwd,
            })
            .is_ok()
        {
            tab.git.pending = true;
            tab.git.last_check = Some(Instant::now());
        }
    }
    pub fn new(cols: u16, rows: u16) -> Self {
        let store = crate::settings::Store::new(crate::settings::application_directory());
        let (settings, themes, errors) = store.load();
        for error in errors {
            log::warn!("Settings/theme: {error}");
        }
        Self {
            window: None,
            gpu: None,
            renderer: None,
            terminal: TerminalState::new(cols, rows),
            runtime: None,
            modifiers: ModifiersState::empty(),
            dirty: true,
            focused: true,
            minimized: false,
            ime: false,
            scale: 1.,
            offset: 0,
            selection: None,
            selecting: false,
            mouse_button: None,
            frames: 0,
            pointer: (0, 0),
            cols,
            rows,
            last_report: Instant::now(),
            failure: None,
            last_sync: Instant::now(),
            left_inset: 0.,
            settings,
            themes,
            store,
            ui: None,
            pointer_px: [0., 0.],
            ui_mouse_latch: Vec::new(),
            settings_before: None,
            settings_file_task: None,
            pending_import: None,
            zoom_wheel: 0.,
            tabs: vec![tabs::Tab {
                id: 1,
                label: "Terminal".into(),
                pinned: false,
                shell: String::new(),
                saved: None,
                git: git_status::View::new(None),
                attention: None,
                privacy: Default::default(),
            }],
            active_tab: 0,
            next_tab_id: 2,
            tab_drag: None,
            shell_spec: None,
            shell_explicit: false,
            active_closing: false,
            closing_all: false,
            theme_worker: None,
            theme_sources: std::collections::BTreeMap::new(),
            theme_generation: 0,
            last_theme_scan: Instant::now(),
            theme_errors: Vec::new(),
            removed_theme: None,
            git_worker: None,
            git_activity: Instant::now(),
            update_worker: None,
            update_candidate: None,
            update_stage: None,
            update_message: "Check GitHub Releases manually".into(),
            update_cancelled: false,
            update_progress: 0,
            update_receipt_until: Some(Instant::now() + Duration::from_secs(40)),
            privacy_broker: None,
            privacy_reveal: Default::default(),
            privacy_dirty: false,
            command_update_pending: false,
            command_update_channel: None,
            last_update_receipt: Instant::now(),
        }
    }
    fn take_session(&mut self) -> tabs::SavedSession {
        tabs::SavedSession {
            terminal: std::mem::replace(&mut self.terminal, TerminalState::new(1, 1)),
            runtime: self.runtime.take(),
            offset: std::mem::take(&mut self.offset),
            selection: self.selection.take(),
            failure: self.failure.take(),
            closing: self.active_closing,
        }
    }
    fn restore_session(&mut self, index: usize) {
        self.privacy_reveal.hide();
        self.privacy_dirty = true;
        let state = self.tabs[index]
            .saved
            .take()
            .expect("inactive tab owns state");
        self.terminal = state.terminal;
        self.runtime = state.runtime;
        self.offset = state.offset;
        self.selection = state.selection;
        self.failure = state.failure;
        self.active_closing = state.closing;
        self.active_tab = index;
        self.tabs[index].attention = None;
        let (revision, cwd) = self.terminal.working_directory();
        self.tabs[index].git.sync(revision, cwd);
        self.tabs[index].git.last_check = None;
        self.selecting = false;
        self.mouse_button = None;
        self.dirty = true;
        if let Some(window) = self.window.as_ref() {
            let size = window.inner_size();
            self.fit(size.width, size.height);
        }
    }
    fn window_width(&self) -> f32 {
        self.window
            .as_ref()
            .map(|window| window.inner_size().width as f32)
            .unwrap_or(1024.)
    }
    /// Reorder tab records only. The active record keeps its engine/session
    /// fields, so the `saved: None` invariant follows the tab, not the index.
    fn move_tab(&mut self, from: usize, gap: usize) {
        let Some(to) = tabs::reorder_target(from, gap, self.tabs.len()) else {
            return;
        };
        let active_id = self.tabs[self.active_tab].id;
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active_tab = self
            .tabs
            .iter()
            .position(|tab| tab.id == active_id)
            .unwrap_or(to);
        log::info!("Moved tab {active_id}; index={}; count={}", self.active_tab, self.tabs.len());
        self.dirty = true;
    }
    fn switch_tab(&mut self, index: usize) {
        if index == self.active_tab || index >= self.tabs.len() {
            return;
        }
        self.close_ui();
        if let Some(button) = self.mouse_button.take() {
            let modes = self.terminal.input_modes();
            self.send(&input::mouse(
                button,
                self.pointer.0,
                self.pointer.1,
                true,
                self.modifiers,
                modes.mouse_sgr,
            ));
        }
        if self.focused && self.terminal.input_modes().focus_events {
            self.send(b"\x1b[O");
        }
        let state = self.take_session();
        self.tabs[self.active_tab].saved = Some(state);
        self.restore_session(index);
        if self.focused && self.terminal.input_modes().focus_events {
            self.send(b"\x1b[I");
        }
    }
    fn add_tab(&mut self, spec: SessionSpec) -> Result<(), String> {
        if self.closing_all {
            return Err("The window is closing".into());
        }
        if self.tabs.len() >= 16 {
            return Err("Close a tab before opening more (limit 16)".into());
        }
        let spec = self.privacy_environment(spec, self.next_tab_id);
        let spec = shell_reporting::prepare(spec, self.settings.report_shell_directory);
        let runtime = Runtime::spawn(
            &spec,
            Dimensions {
                cols: self.cols,
                rows: self.rows,
            },
        )
        .map_err(|e| e.to_string())?;
        self.close_ui();
        let state = self.take_session();
        self.tabs[self.active_tab].saved = Some(state);
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(tabs::Tab {
            id,
            label: tabs::label(&spec),
            pinned: false,
            shell: tabs::shell_name(&spec),
            saved: None,
            git: git_status::View::new(spec.cwd.clone().or_else(|| std::env::current_dir().ok())),
            attention: None,
            privacy: Default::default(),
        });
        self.active_tab = self.tabs.len() - 1;
        self.terminal = TerminalState::new(self.cols, self.rows);
        self.runtime = Some(runtime);
        self.active_closing = false;
        self.selecting = false;
        self.mouse_button = None;
        self.dirty = true;
        log::info!("Opened tab {id}; count={}", self.tabs.len());
        if let Some(window) = self.window.as_ref() {
            let size = window.inner_size();
            self.fit(size.width, size.height);
        }
        Ok(())
    }
    fn close_tab(&mut self) {
        self.privacy_reveal.hide();
        self.privacy_dirty = true;
        if let Some(broker) = self.privacy_broker.as_ref() {
            broker.unregister(self.tabs[self.active_tab].id);
        }
        self.close_ui();
        self.active_closing = true;
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.request_close();
        }
        self.dirty = true;
    }
    fn close_all_tabs(&mut self) {
        self.privacy_reveal.hide();
        self.privacy_dirty = true;
        if let Some(broker) = self.privacy_broker.as_ref() {
            for tab in &self.tabs {
                broker.unregister(tab.id);
            }
        }
        self.closing_all = true;
        self.active_closing = true;
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.request_close();
        }
        for tab in &mut self.tabs {
            if let Some(state) = tab.saved.as_mut() {
                state.closing = true;
                if let Some(r) = state.runtime.as_mut() {
                    r.request_close();
                }
            }
        }
    }
    fn drain_inactive(&mut self) {
        let mut titles_changed = false;
        for tab in &mut self.tabs {
            let Some(state) = tab.saved.as_mut() else {
                continue;
            };
            let Some(runtime) = state.runtime.as_mut() else {
                continue;
            };
            let batch = runtime.stream.take_up_to(64 * 1024);
            let history = state.terminal.scrollback_len();
            let dropped = state.terminal.scrollback_dropped();
            let bell = state.terminal.bell_count();
            state.terminal.advance(&batch);
            if state.terminal.bell_count() != bell {
                tab.attention = Some(Instant::now());
            }
            if state.offset > 0 {
                state.offset = (state.offset
                    + state.terminal.scrollback_len().saturating_sub(history)
                    + state.terminal.scrollback_dropped().saturating_sub(dropped) as usize)
                    .min(state.terminal.scrollback_len());
            }
            if state.terminal.scrollback_dropped() != dropped {
                state.selection = None;
            }
            let replies = state.terminal.take_responses();
            if !state.closing && !runtime.is_closing() && !replies.is_empty() {
                if let Err(error) = runtime.write(&replies) {
                    state.failure = Some(error.to_string());
                    state.closing = true;
                    runtime.request_close();
                }
            }
            if !tab.pinned {
                let cwd = state.terminal.working_directory().1.map(str::to_owned);
                let title = tabs::title(cwd.as_deref().map(std::path::Path::new), &tab.shell);
                if title != tab.label {
                    tab.label = title;
                    titles_changed = true;
                }
            }
        }
        if titles_changed {
            self.dirty = true;
        }
    }
    fn tab_controls(&mut self, event: &WindowEvent) -> bool {
        // Finish a drag before the panel guard: releasing over a panel or outside
        // the strip must not leave the gesture latched forever.
        if let WindowEvent::MouseInput {
            state: ElementState::Released,
            button: MouseButton::Left,
            ..
        } = event
        {
            if let Some(drag) = self.tab_drag.take() {
                self.ui_mouse_latch.retain(|button| *button != MouseButton::Left);
                if drag.moved {
                    let strip = ui::strip(
                        self.window_width(),
                        self.scale,
                        self.tabs.len(),
                        self.active_tab,
                        Some(drag.start),
                    );
                    let gap = ui::drop_gap(self.pointer_px[0], &strip, self.tabs.len());
                    self.move_tab(drag.from, gap);
                }
                self.dirty = true;
                return true;
            }
        }
        if self.ui.is_some() {
            return false;
        }
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if event.state == ElementState::Pressed
                && !self.ime
                && new_window::shortcut(&event.logical_key, self.modifiers)
            {
                if !event.repeat && !self.closing_all {
                    let result = (|| -> Result<(), std::io::Error> {
                        let shell = self
                            .shell_spec
                            .as_ref()
                            .ok_or_else(|| std::io::Error::other("Shell unavailable"))?;
                        let executable = std::env::current_exe()?;
                        let launch = shell
                            .cwd
                            .clone()
                            .map(Ok)
                            .unwrap_or_else(std::env::current_dir)?;
                        let cwd = self.terminal.working_directory().1;
                        new_window::command(&executable, shell, cwd, &launch)?.spawn()?;
                        Ok(())
                    })();
                    if result.is_err() {
                        log::warn!("New window launch failed");
                        clipboard::error_dialog("Could not open a new Juna window. Check that the application and launch folder are still available.");
                    } else {
                        log::info!("Opened independent Juna window");
                    }
                }
                return true;
            }
            if event.state == ElementState::Pressed
                && !self.ime
                && self.modifiers.control_key()
                && !self.modifiers.alt_key()
            {
                let action = match &event.logical_key {
                    Key::Character(key)
                        if self.modifiers.shift_key() && key.eq_ignore_ascii_case("t") =>
                    {
                        Some(ui::Action::NewTab)
                    }
                    Key::Character(key)
                        if self.modifiers.shift_key() && key.eq_ignore_ascii_case("w") =>
                    {
                        Some(ui::Action::CloseTab)
                    }
                    Key::Named(NamedKey::Tab) if !self.tabs.is_empty() => {
                        let n = self.tabs.len();
                        let next = if self.modifiers.shift_key() {
                            (self.active_tab + n - 1) % n
                        } else {
                            (self.active_tab + 1) % n
                        };
                        self.switch_tab(next);
                        return true;
                    }
                    _ => None,
                };
                if let Some(action) = action {
                    self.ui_action(action);
                    return true;
                }
            }
        }
        if let WindowEvent::CursorMoved { position, .. } = event {
            self.pointer_px = [position.x as f32, position.y as f32];
        }
        if matches!(event, WindowEvent::CursorMoved { .. }) {
            let width = self.window_width();
            if let Some(drag) = self.tab_drag.as_mut() {
                if !drag.moved
                    && (self.pointer_px[0] - drag.press_x).abs()
                        > ui::TAB_DRAG_THRESHOLD * self.scale
                {
                    drag.moved = true;
                }
                if drag.moved {
                    let strip = ui::strip(
                        width,
                        self.scale,
                        self.tabs.len(),
                        self.active_tab,
                        Some(drag.start),
                    );
                    drag.start =
                        ui::drag_start(self.pointer_px[0], width, self.scale, &strip, self.tabs.len());
                    self.dirty = true;
                }
                return true;
            }
        }
        if self.pointer_px[1] < ui::TAB_HEIGHT * self.scale {
            if let WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } = event
            {
                let width = self
                    .window
                    .as_ref()
                    .map(|w| w.inner_size().width as f32)
                    .unwrap_or(1024.);
                if let Some(ui::TabHit::Select(index)) = ui::tab_hit(
                    self.pointer_px,
                    width,
                    self.scale,
                    self.tabs.len(),
                    self.active_tab,
                ) {
                    let id = self.tabs[index].id;
                    let label = self.tabs[index].label.clone();
                    if let Some(button) = self.mouse_button.take() {
                        let modes = self.terminal.input_modes();
                        self.send(&input::mouse(
                            button,
                            self.pointer.0,
                            self.pointer.1,
                            true,
                            self.modifiers,
                            modes.mouse_sgr,
                        ));
                    }
                    self.selecting = false;
                    self.ui_mouse_latch.push(MouseButton::Middle);
                    self.open_ui(ui::Kind::ConfirmClose);
                    if let Some(panel) = self.ui.as_mut() {
                        panel.target_tab = Some(id);
                        panel.message = format!("Commands will stop. Tab: {label}");
                        let rows = panel.rows(&self.themes, &self.settings);
                        panel.focus_row(1, &rows);
                    }
                }
                return true;
            }
            if let WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } = event
            {
                self.open_context_menu();
                return true;
            }
            if let WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } = event
            {
                if !self.ui_mouse_latch.contains(&MouseButton::Left) {
                    self.ui_mouse_latch.push(MouseButton::Left);
                }
                let width = self
                    .window
                    .as_ref()
                    .map(|w| w.inner_size().width as f32)
                    .unwrap_or(1024.);
                match ui::tab_hit(
                    self.pointer_px,
                    width,
                    self.scale,
                    self.tabs.len(),
                    self.active_tab,
                ) {
                    Some(ui::TabHit::New) => self.ui_action(ui::Action::NewTab),
                    Some(ui::TabHit::Select(index)) => {
                        self.switch_tab(index);
                        let strip = ui::strip(
                            width,
                            self.scale,
                            self.tabs.len(),
                            self.active_tab,
                            None,
                        );
                        self.tab_drag = Some(TabDrag {
                            from: index,
                            press_x: self.pointer_px[0],
                            start: strip.start,
                            moved: false,
                        });
                    }
                    _ => {}
                }
                return true;
            }
            if matches!(event, WindowEvent::CursorMoved { .. }) {
                return true;
            }
        }
        false
    }
    fn fail(&mut self, message: String) {
        log::error!("{message}");
        self.failure = Some(message);
        if let Some(r) = self.runtime.as_mut() {
            r.request_close();
        }
    }
    fn send(&mut self, bytes: &[u8]) {
        if bytes.is_empty() || self.active_closing {
            return;
        }
        if let Some(r) = self.runtime.as_mut() {
            if r.is_closing() {
                self.active_closing = true;
                return;
            }
            if let Err(e) = r.write(bytes) {
                self.fail(format!("Session write failed: {e}"));
            } else if matches!(bytes, [3] | [26]) {
                log::info!(
                    "Control input queued: key={}; tab={}; alternate={}",
                    if bytes[0] == 3 { "Ctrl+C" } else { "Ctrl+Z" },
                    self.active_tab,
                    self.terminal.alternate_active()
                );
            }
        }
    }
    fn drain(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let batch = runtime.stream.take_up_to(64 * 1024);
        if batch.is_empty() {
            return;
        }
        self.git_activity = Instant::now();
        let history = self.terminal.scrollback_len();
        let dropped = self.terminal.scrollback_dropped();
        let alternate = self.terminal.alternate_active();
        self.terminal.advance(&batch);
        if alternate != self.terminal.alternate_active() {
            if let Some(window) = self.window.as_ref() {
                let size = window.inner_size();
                self.fit(size.width, size.height);
            }
        }
        if self.offset > 0 {
            self.offset = (self.offset
                + self.terminal.scrollback_len().saturating_sub(history)
                + self.terminal.scrollback_dropped().saturating_sub(dropped) as usize)
                .min(self.terminal.scrollback_len());
        }
        if self.terminal.scrollback_dropped() != dropped {
            self.selection = None;
        }
        let replies = self.terminal.take_responses();
        self.send(&replies);
        self.dirty = true;
        // Shell OSC titles remain engine metadata; the application title stays fixed.
    }
    fn fit(&mut self, width: u32, height: u32) {
        self.minimized = width == 0 || height == 0;
        if self.minimized {
            return;
        }
        let inset = gutter::width(
            width as f32,
            self.scale,
            self.settings.show_line_numbers && !self.terminal.alternate_active(),
        );
        let cols =
            (((width as f32 - 2. * PAD * self.scale - inset) / (CELL_W * self.scale)).max(1.)
                as u16)
                .clamp(1, MAX_COLS);
        let status_height = if self.settings.show_git_status && !self.terminal.alternate_active() {
            ui::STATUS_HEIGHT
        } else {
            0.
        };
        #[cfg(debug_assertions)]
        if std::env::var_os("JUNA_ACCEPT_GIT_STATUS").is_some() {
            log::info!("Git acceptance footer={}", status_height > 0.);
        }
        let rows = (((height as f32 - (2. * PAD + ui::TAB_HEIGHT + status_height) * self.scale)
            / (CELL_H * self.scale))
            .max(1.) as u16)
            .clamp(1, MAX_ROWS);
        if (cols, rows) != (self.cols, self.rows) {
            if let Some(r) = self.runtime.as_mut() {
                if let Err(e) = r.control.resize(Dimensions { cols, rows }) {
                    self.fail(format!("PTY resize failed: {e}"));
                    return;
                }
            }
            for tab in &mut self.tabs {
                if let Some(state) = tab.saved.as_mut() {
                    if let Some(r) = state.runtime.as_mut() {
                        if let Err(error) = r.control.resize(Dimensions { cols, rows }) {
                            state.failure = Some(error.to_string());
                            state.closing = true;
                            r.request_close();
                            continue;
                        }
                    }
                    state.terminal.resize(cols, rows);
                    state.selection = None;
                    state.offset = state.offset.min(state.terminal.scrollback_len());
                }
            }
            self.terminal.resize(cols, rows);
            self.cols = cols;
            self.rows = rows;
            self.selection = None;
            self.offset = self.offset.min(self.terminal.scrollback_len());
        }
        self.left_inset = inset;
        let col = ((self.pointer_px[0] - PAD * self.scale - inset) / (CELL_W * self.scale)).max(0.)
            as u16;
        let row = ((self.pointer_px[1] - (PAD + ui::TAB_HEIGHT) * self.scale)
            / (CELL_H * self.scale))
            .max(0.) as u16;
        self.pointer = (row.min(self.rows - 1), col.min(self.cols - 1));
        if let Some(g) = self.gpu.as_mut() {
            g.resize(width, height);
        }
        self.dirty = true;
    }
    fn draw(&mut self) {
        if self.minimized {
            return;
        }
        let selection_theme = self.active_theme();
        self.tabs[self.active_tab]
            .privacy
            .update(&self.terminal, self.offset);
        let mut snapshot = self.tabs[self.active_tab]
            .privacy
            .snapshot()
            .unwrap()
            .clone();
        let selection_top = self.tabs[self.active_tab].privacy.top();
        let show_numbers = !self.terminal.alternate_active();
        let Some(g) = self.gpu.as_ref() else { return };
        let (w, h) = (g.config.width, g.config.height);
        let privacy_scene = self.privacy_overlay(w, h);
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if !self.focused {
            snapshot.cursor_style = 4;
        }
        if let Some((a, b)) = self.selection {
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            let top = selection_top;
            for row in 0..snapshot.rows {
                for col in 0..snapshot.cols {
                    let point = (top + row as usize, col);
                    if point >= a && point <= b {
                        let cell = &mut snapshot.cells
                            [row as usize * snapshot.cols as usize + col as usize];
                        let theme = &selection_theme;
                        let to_color = |value: &str| {
                            let c = crate::theme::rgb(value).unwrap();
                            crate::vt::Color::Rgb(
                                (c[0] * 255.).round() as u8,
                                (c[1] * 255.).round() as u8,
                                (c[2] * 255.).round() as u8,
                            )
                        };
                        cell.fg = to_color(&theme.colors.selection_foreground);
                        cell.bg = to_color(&theme.colors.selection_background);
                        cell.attrs.set(crate::vt::Attrs::INVERSE, false);
                    }
                }
            }
        }
        renderer.set_top_inset(ui::TAB_HEIGHT * self.scale);
        renderer.set_left_inset(self.left_inset);
        renderer.build(&snapshot, w, h);
        let color = crate::theme::rgb(&selection_theme.colors.foreground).unwrap();
        renderer.overlay(&gutter::scene(
            if show_numbers { self.left_inset } else { 0. },
            self.scale,
            selection_top,
            snapshot.rows,
            [color[0], color[1], color[2], 1.],
        ));
        let labels: Vec<String> = self.tabs.iter().map(|t| t.label.clone()).collect();
        let attention: Vec<bool> = self.tabs.iter().map(|t| t.attention.is_some()).collect();
        let now = Instant::now();
        let flash_on = self
            .tabs
            .iter()
            .filter_map(|t| t.attention)
            .min()
            .map(|at| {
                let elapsed = now.duration_since(at).as_millis() as u64;
                elapsed < ui::ATTENTION_FLASH_MS
                    && (elapsed / ui::ATTENTION_FLASH_PERIOD_MS).is_multiple_of(2)
            })
            .unwrap_or(false);
        let drag = self.tab_drag.filter(|drag| drag.moved).map(|drag| {
            let strip = ui::strip(
                w as f32,
                self.scale,
                self.tabs.len(),
                self.active_tab,
                Some(drag.start),
            );
            ui::DragView {
                start: drag.start,
                gap: ui::drop_gap(self.pointer_px[0], &strip, self.tabs.len()),
                pointer_x: self.pointer_px[0],
            }
        });
        renderer.overlay(&ui::tab_scene(
            &labels,
            self.active_tab,
            &attention,
            flash_on,
            w as f32,
            self.scale,
            drag,
        ));
        if self.settings.show_git_status && show_numbers {
            renderer.overlay(&ui::status_scene(
                &self.tabs[self.active_tab].git.label(),
                w as f32,
                h as f32,
                self.scale,
                &selection_theme,
            ));
        }
        if let Some(ui) = self.ui.as_mut() {
            let rows = ui.rows(&self.themes, &self.settings);
            let (_, visible, _) = ui.geometry(w as f32, h as f32, self.scale, rows.len());
            ui.ensure_visible(visible, rows.len());
            let scene = ui.scene(&rows, w as f32, h as f32, self.scale);
            renderer.overlay(&scene);
        }
        if let Some(scene) = privacy_scene.as_ref() {
            renderer.overlay(scene);
        }
        match g.render(renderer.clear_color(), |device, queue, pass| {
            renderer.draw(device, queue, w, h, pass)
        }) {
            Ok(true) => {
                self.dirty = false;
                self.privacy_dirty = false;
                self.frames += 1;
                if self.frames == 1
                    && self.update_receipt_until.is_some()
                    && crate::updates::helper::acknowledge_restart().is_err()
                {
                    self.fail("Updated application could not acknowledge startup".into());
                }
            }
            Ok(false) => {}
            Err(FrameSkip::Lost | FrameSkip::Outdated) => {
                self.gpu.as_mut().unwrap().resize(w, h);
            }
            Err(FrameSkip::Validation) => self.fail("GPU presentation validation failed".into()),
            Err(_) => {}
        }
    }
    fn selection_text(&self) -> String {
        let Some((a, b)) = self.selection else {
            return String::new();
        };
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let history = if self.terminal.alternate_active() {
            0
        } else {
            self.terminal.scrollback_len()
        };
        let mut out = String::new();
        for row in a.0..=b.0 {
            let cells = if row < history {
                self.terminal.scrollback_line(row)
            } else {
                self.terminal.grid().row_slice((row - history) as u16)
            };
            let Some(cells) = cells else { continue };
            let start = if row == a.0 { a.1 as usize } else { 0 };
            let end = if row == b.0 {
                b.1 as usize + 1
            } else {
                cells.len()
            };
            let mut line = String::new();
            for cell in cells.iter().take(end).skip(start) {
                match &cell.content {
                    CellContent::Char(cp) | CellContent::WideChar(cp) => {
                        line.push(char::from_u32(*cp).unwrap_or('�'))
                    }
                    CellContent::Cluster(s) => line.push_str(s),
                    CellContent::Empty => line.push(' '),
                    CellContent::WideCharSpacer => {}
                }
            }
            out.push_str(line.trim_end());
            if row < b.0 {
                let wrapped = if row < history {
                    self.terminal.history_wrapped(row)
                } else {
                    self.terminal.grid().row_wrapped((row - history) as u16)
                };
                if !wrapped {
                    out.push('\n');
                }
            }
        }
        out
    }
    fn paste(&mut self) {
        match clipboard::read() {
            Ok(text) => {
                if text.contains(['\r', '\n']) {
                    self.tabs[self.active_tab].git.invalidate_initial();
                }
                let bytes = input::paste(&text, self.terminal.input_modes().bracketed_paste);
                self.offset = 0;
                self.selection = None;
                self.send(&bytes);
            }
            Err(e) => log::warn!("Paste: {e}"),
        }
    }
    fn active_theme(&self) -> crate::theme::Theme {
        self.themes
            .iter()
            .find(|t| t.id == self.settings.active_theme_id)
            .cloned()
            .unwrap_or_else(crate::theme::default_theme)
    }
    fn history_start(&self) -> usize {
        if self.terminal.alternate_active() {
            0
        } else {
            self.terminal.scrollback_len().saturating_sub(self.offset)
        }
    }
    fn rebuild_renderer(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        self.scale = window.scale_factor() as f32 * self.settings.font_size / FONT_PX;
        if let Some(gpu) = self.gpu.as_ref() {
            self.renderer = crate::render::Renderer::new_with_family(
                &gpu.device,
                &gpu.queue,
                gpu.format,
                2048,
                (CELL_W * self.scale, CELL_H * self.scale),
                PAD * self.scale,
                FONT_PX * self.scale,
                &self.settings.font_family,
            );
        }
        let theme = self.active_theme();
        if let Some(r) = self.renderer.as_mut() {
            let _ = r.apply_theme(&theme, self.settings.minimum_contrast);
        }
        let size = window.inner_size();
        self.fit(size.width, size.height);
        self.dirty = true;
    }
    fn resize_font(&mut self, zoom: input::Zoom) {
        let size = match zoom {
            input::Zoom::Step(step) => (self.settings.font_size + f32::from(step)).clamp(8., 40.),
            input::Zoom::Reset => FONT_PX,
        };
        if size == self.settings.font_size && size == self.committed_preferences().font_size {
            return;
        }
        let mut settings = self.committed_preferences();
        settings.font_size = size;
        if let Err(error) = self.commit_preferences(settings) {
            log::warn!("Font size: {error}");
            if let Some(ui) = self.ui.as_mut() {
                ui.message = error;
            }
            return;
        }
        self.rebuild_renderer();
        log::info!("Font size: {size} px");
        if let Some(ui) = self.ui.as_mut() {
            if ui.kind == ui::Kind::Number && ui.number == Some(1) {
                ui.set_number_value(size.to_string());
            }
            if let Some(before) = ui.prior_settings.as_mut() {
                before.font_size = size;
            }
        }
    }
    fn handle_zoom(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !self.ime =>
            {
                if let Some(zoom) =
                    input::zoom(&event.logical_key, event.physical_key, self.modifiers)
                {
                    self.resize_font(zoom);
                    return true;
                }
            }
            WindowEvent::MouseWheel { delta, .. }
                if self.modifiers.control_key()
                    && !self.modifiers.alt_key()
                    && !self.modifiers.super_key() =>
            {
                self.zoom_wheel += match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.,
                };
                let steps = self.zoom_wheel.trunc().clamp(-32., 32.) as i8;
                if steps != 0 {
                    self.zoom_wheel -= f32::from(steps);
                    self.resize_font(input::Zoom::Step(steps));
                }
                return true;
            }
            WindowEvent::ModifiersChanged(m) if !m.state().control_key() => self.zoom_wheel = 0.,
            _ => {}
        }
        false
    }
    fn open_ui(&mut self, kind: ui::Kind) {
        self.privacy_reveal.hide();
        self.privacy_dirty = true;
        if kind == ui::Kind::Appearance
            && self
                .ui
                .as_ref()
                .is_some_and(|ui| ui.kind == ui::Kind::Fonts)
        {
            self.settings = self.committed_preferences();
            self.rebuild_renderer();
        }
        if matches!(
            kind,
            ui::Kind::Settings | ui::Kind::Appearance | ui::Kind::Fonts
        ) {
            if self.settings_before.is_none() {
                self.settings_before = Some(self.settings.clone());
            }
        } else if let Some(before) = self.settings_before.take() {
            self.settings = before;
            self.rebuild_renderer();
        }
        log::info!("Options panel: {kind:?}");
        let anchor = if matches!(kind, ui::Kind::Context | ui::Kind::TabContext) {
            self.pointer_px
        } else {
            [16. * self.scale, 16. * self.scale]
        };
        let mut ui = ui::Ui::new(kind, anchor);
        if kind == ui::Kind::ConfirmUpdate {
            ui.focus = 1;
            ui.message = "All tabs in this window will close".into();
        }
        ui.theme = Some(self.active_theme());
        if matches!(kind, ui::Kind::Themes | ui::Kind::Custom) {
            if let Some(theme) = self
                .themes
                .iter()
                .find(|t| t.id == self.settings.active_theme_id)
            {
                ui.expanded.insert(theme.family_name());
            }
            let rows = ui.rows(&self.themes, &self.settings);
            ui.focus = rows
                .iter()
                .position(|row| {
                    row.action == ui::Action::Pick(self.settings.active_theme_id.clone())
                })
                .unwrap_or(0);
            if let Some(worker) = self.theme_worker.as_ref() {
                worker.request(self.theme_generation);
            }
        }
        if kind == ui::Kind::Fonts {
            ui.focus = crate::fonts::choices()
                .iter()
                .position(|name| name == &self.settings.font_family)
                .unwrap_or(0)
                + 1;
        }
        let rows = ui.rows(&self.themes, &self.settings);
        ui.focus_row(ui.focus, &rows);
        self.ui = Some(ui);
        if kind == ui::Kind::Updates {
            self.refresh_update_panel();
        }
        self.selecting = false;
        self.dirty = true;
    }
    fn open_context_menu(&mut self) {
        self.close_ui();
        if let Some(button) = self.mouse_button.take() {
            let modes = self.terminal.input_modes();
            self.send(&input::mouse(
                button,
                self.pointer.0,
                self.pointer.1,
                true,
                self.modifiers,
                modes.mouse_sgr,
            ));
        }
        self.selecting = false;
        if !self.ui_mouse_latch.contains(&MouseButton::Right) {
            self.ui_mouse_latch.push(MouseButton::Right);
        }
        if self.pointer_px[1] < ui::TAB_HEIGHT * self.scale {
            let width = self
                .window
                .as_ref()
                .map(|w| w.inner_size().width as f32)
                .unwrap_or(1024.);
            let target = match ui::tab_hit(
                self.pointer_px,
                width,
                self.scale,
                self.tabs.len(),
                self.active_tab,
            ) {
                Some(ui::TabHit::Select(index)) => Some(self.tabs[index].id),
                _ => None,
            };
            self.open_ui(ui::Kind::TabContext);
            if let Some(panel) = self.ui.as_mut() {
                panel.target_tab = target;
            }
        } else {
            self.open_ui(ui::Kind::Context);
        }
    }
    fn refresh_themes(&mut self) {
        let watching = self
            .ui
            .as_ref()
            .is_some_and(|ui| matches!(ui.kind, ui::Kind::Themes | ui::Kind::Custom));
        if watching && self.last_theme_scan.elapsed() > Duration::from_secs(2) {
            self.last_theme_scan = Instant::now();
            if let Some(worker) = &self.theme_worker {
                worker.request(self.theme_generation);
            }
        }
        let result = self
            .theme_worker
            .as_ref()
            .and_then(|worker| worker.result());
        let Some((generation, mut library)) = result else {
            return;
        };
        if generation < self.theme_generation {
            return;
        }
        if library.errors != self.theme_errors {
            for error in &library.errors {
                log::warn!("Theme: {error}");
            }
            self.theme_errors = library.errors;
            if let Some(ui) = self.ui.as_mut().filter(|ui| {
                matches!(
                    ui.kind,
                    ui::Kind::Themes | ui::Kind::Custom | ui::Kind::Manage
                )
            }) {
                ui.message = self.theme_errors.first().cloned().unwrap_or_default();
            }
        }
        let active = self.active_theme();
        if !library.themes.iter().any(|t| t.id == active.id) && active.id.starts_with("custom-") {
            library.themes.push(active);
        }
        let focus = self.ui.as_ref().and_then(|ui| {
            ui.rows(&self.themes, &self.settings)
                .get(ui.focus)
                .map(|row| row.action.clone())
        });
        let changed = self.themes != library.themes;
        self.themes = library.themes;
        self.theme_sources = library.sources;
        if changed {
            if let Some(ui) = self.ui.as_mut() {
                let rows = ui.rows(&self.themes, &self.settings);
                if let Some(index) = focus.and_then(|a| rows.iter().position(|row| row.action == a))
                {
                    ui.focus = index;
                }
                if let Some(error) = self.theme_errors.first() {
                    ui.message = error.clone();
                }
            }
            let theme = self.active_theme();
            if let Some(renderer) = self.renderer.as_mut() {
                let _ = renderer.apply_theme(&theme, self.settings.minimum_contrast);
            }
            self.preview_ui();
            self.dirty = true;
        }
    }
    fn preview_ui(&mut self) {
        let mut preview = None;
        let mut font = None;
        let mut number = None;
        if let Some(ui) = self.ui.as_mut() {
            let rows = ui.rows(&self.themes, &self.settings);
            if ui.kind == ui::Kind::Number {
                number = ui.number.map(|index| (index, ui.draft_value.clone()));
            }
            if ui.kind == ui::Kind::Fonts {
                if let Some(ui::Action::PickFont(name)) = rows.get(ui.focus).map(|r| &r.action) {
                    font = Some(name.clone());
                }
            }
            if matches!(
                ui.kind,
                ui::Kind::Themes | ui::Kind::Custom | ui::Kind::Manage
            ) {
                if let Some(ui::Action::Pick(id) | ui::Action::ManageTheme(id)) =
                    rows.get(ui.focus).map(|r| &r.action)
                {
                    ui.preview_id = Some(id.clone());
                    preview = self.themes.iter().find(|t| &t.id == id).cloned();
                }
            }
            if ui.kind == ui::Kind::Editor {
                if let Some(row) = rows.get(ui.focus) {
                    let draft = ui.draft.as_mut().unwrap();
                    let result = match row.action {
                        ui::Action::Name => {
                            draft.name = ui.buffer.clone();
                            Ok(())
                        }
                        ui::Action::Family => {
                            draft.family = Some(ui.buffer.clone());
                            draft.validate()
                        }
                        ui::Action::Color(i) => draft.set_color(i, ui.buffer.clone()),
                        _ => Ok(()),
                    };
                    ui.message = result.err().unwrap_or_default();
                    if draft.validate().is_ok() {
                        preview = Some(draft.clone());
                    }
                }
            }
        }
        if let Some((index, value)) = number {
            match ui::numeric_settings(&self.settings, index, &value) {
                Ok(settings) => {
                    let changed = settings.font_size != self.settings.font_size
                        || settings.minimum_contrast != self.settings.minimum_contrast;
                    self.settings = settings;
                    if changed {
                        self.rebuild_renderer();
                    }
                    if let Some(ui) = self.ui.as_mut() {
                        ui.message.clear();
                    }
                }
                Err(error) => {
                    if let Some(ui) = self.ui.as_mut() {
                        ui.message = error;
                    }
                }
            }
        }
        if let Some(font) = font {
            if self.settings.font_family != font {
                self.settings.font_family = font;
                self.rebuild_renderer();
            }
        }
        if let (Some(theme), Some(renderer)) = (preview, self.renderer.as_mut()) {
            let _ = renderer.apply_theme(&theme, self.settings.minimum_contrast);
        }
        self.dirty = true;
    }
    fn close_ui(&mut self) {
        self.pending_import = None;
        self.ui = None;
        if let Some(before) = self.settings_before.take() {
            self.settings = before;
            self.rebuild_renderer();
        }
        let theme = self.active_theme();
        if let Some(r) = self.renderer.as_mut() {
            let _ = r.apply_theme(&theme, self.settings.minimum_contrast);
        }
        self.dirty = true;
    }
    fn committed_preferences(&self) -> crate::settings::Settings {
        self.settings_before
            .clone()
            .unwrap_or_else(|| self.settings.clone())
    }
    fn poll_settings_files(&mut self) {
        let result = match self.settings_file_task.as_ref().map(|task| task.try_recv()) {
            Some(Ok(result)) => result,
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                Err("Settings file worker stopped".into())
            }
            _ => return,
        };
        self.settings_file_task = None;
        let outcome = (|| -> Result<(), String> {
            match result? {
                Some(settings_files::Outcome::Imported(bundle)) => {
                    let plan = bundle.prepare(&self.themes)?;
                    let summary = vec![
                        format!(
                            "Theme: {}",
                            plan.themes
                                .iter()
                                .chain(self.themes.iter())
                                .find(|t| t.id == plan.settings.active_theme_id)
                                .map(|t| t.name.as_str())
                                .unwrap_or("Unavailable")
                        ),
                        format!("Font: {}", plan.settings.font_family),
                        format!(
                            "Size: {} px; contrast: {}:1",
                            plan.settings.font_size, plan.settings.minimum_contrast
                        ),
                        format!(
                            "Line numbers: {}; favourites: {}",
                            plan.settings.show_line_numbers,
                            plan.settings.favourite_theme_ids.len()
                        ),
                        format!(
                            "Custom theme copies: {}; conflicts preserved: {}",
                            plan.themes.len(),
                            plan.conflicts
                        ),
                    ];
                    self.open_ui(ui::Kind::ImportReview);
                    self.pending_import = Some(plan);
                    if let Some(panel) = self.ui.as_mut() {
                        panel.summary = summary;
                        let rows = panel.rows(&self.themes, &self.settings);
                        panel.focus_row(rows.len() - 1, &rows);
                    }
                }
                Some(settings_files::Outcome::Exported) => {
                    if let Some(panel) = self.ui.as_mut() {
                        panel.message = "Settings exported".into();
                    }
                }
                None => {}
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            if let Some(panel) = self.ui.as_mut() {
                panel.message = error;
            }
        }
        self.dirty = true;
    }
    fn commit_preferences(&mut self, settings: crate::settings::Settings) -> Result<(), String> {
        self.store.save_settings(&settings)?;
        self.settings = settings.clone();
        if self.settings_before.is_some() {
            self.settings_before = Some(settings);
        }
        Ok(())
    }
    fn ui_action(&mut self, action: ui::Action) {
        use ui::{Action, Kind};
        let result = (|| -> Result<(), String> {
            match action {
                Action::ImportSettings | Action::ExportSettings => {
                    if self.settings_file_task.is_some() {
                        return Err("A settings file operation is already open".into());
                    }
                    let export = if action == Action::ExportSettings {
                        Some(crate::settings_transfer::Bundle::capture(
                            &self.committed_preferences(),
                            &self.themes,
                        ))
                    } else {
                        None
                    };
                    let window = self
                        .window
                        .as_ref()
                        .ok_or("File selection requires a window")?
                        .clone();
                    self.close_ui();
                    self.open_ui(Kind::Settings);
                    self.settings_file_task = Some(settings_files::start(window, export)?);
                }
                Action::ApplyImport => {
                    let plan = self
                        .pending_import
                        .take()
                        .ok_or("No settings import to apply")?;
                    if let Err(error) = plan.apply(&self.store) {
                        self.pending_import = Some(plan);
                        return Err(error);
                    }
                    self.settings = plan.settings;
                    self.settings_before = None;
                    let (_, themes, errors) = self.store.load();
                    self.themes = themes;
                    self.theme_sources = crate::theme_library::scan(&self.store.directory).sources;
                    self.theme_generation += 1;
                    self.rebuild_renderer();
                    self.open_ui(Kind::Settings);
                    if let Some(panel) = self.ui.as_mut() {
                        panel.message = if errors.is_empty() {
                            "Settings imported".into()
                        } else {
                            format!(
                                "Settings imported; {} existing theme warnings",
                                errors.len()
                            )
                        };
                    }
                }
                Action::ExpandAll => {
                    if let Some(panel) = self.ui.as_mut() {
                        let custom = panel.kind == ui::Kind::Custom;
                        panel.expanded.extend(
                            self.themes
                                .iter()
                                .filter(|t| t.id.starts_with("custom-") == custom)
                                .map(|t| t.family_name()),
                        );
                        panel.expanded.insert("\0favourites".into());
                    }
                    self.dirty = true;
                }
                Action::CollapseAll => {
                    if let Some(panel) = self.ui.as_mut() {
                        panel.expanded.clear();
                        panel.query.clear();
                        let rows = panel.rows(&self.themes, &self.settings);
                        let index = rows
                            .iter()
                            .position(|row| row.action == Action::CollapseAll)
                            .unwrap_or(0);
                        panel.focus_row(index, &rows);
                        panel.scroll = 0;
                    }
                    self.dirty = true;
                }
                Action::ToggleFavourite(id) => {
                    let previous = self.ui.as_ref().and_then(|panel| {
                        panel
                            .rows(&self.themes, &self.settings)
                            .get(panel.focus)
                            .map(|row| row.action.clone())
                    });
                    if !self.themes.iter().any(|t| t.id == id) {
                        return Ok(());
                    }
                    let mut saved = self
                        .settings_before
                        .as_ref()
                        .unwrap_or(&self.settings)
                        .clone();
                    if saved.favourite_theme_ids.contains(&id) {
                        saved.favourite_theme_ids.retain(|value| value != &id);
                    } else {
                        saved.favourite_theme_ids.push(id);
                    }
                    self.store.save_settings(&saved)?;
                    self.settings.favourite_theme_ids = saved.favourite_theme_ids.clone();
                    if let Some(before) = self.settings_before.as_mut() {
                        before.favourite_theme_ids = saved.favourite_theme_ids;
                    }
                    if let Some(panel) = self.ui.as_mut() {
                        let star = panel.star_focused;
                        let rows = panel.rows(&self.themes, &self.settings);
                        if let Some(index) = rows
                            .iter()
                            .position(|row| Some(&row.action) == previous.as_ref())
                        {
                            panel.focus_row(index, &rows);
                        } else {
                            panel.focus_row(panel.focus.min(rows.len().saturating_sub(1)), &rows);
                        }
                        panel.star_focused =
                            star && rows.get(panel.focus).and_then(ui::theme_id).is_some();
                    }
                    self.dirty = true;
                }
                Action::ToggleLineNumbers => {
                    let mut settings = self.committed_preferences();
                    settings.show_line_numbers = !settings.show_line_numbers;
                    self.commit_preferences(settings)?;
                    self.rebuild_renderer();
                    if let Some(window) = self.window.as_ref() {
                        let size = window.inner_size();
                        self.fit(size.width, size.height);
                    }
                    self.dirty = true;
                }
                Action::ToggleGitStatus | Action::ToggleDirectoryReporting => {
                    let mut settings = self.committed_preferences();
                    if matches!(action, Action::ToggleGitStatus) {
                        settings.show_git_status = !settings.show_git_status;
                    } else {
                        settings.report_shell_directory = !settings.report_shell_directory;
                    }
                    self.commit_preferences(settings)?;
                    self.tabs[self.active_tab].git.last_check = None;
                    if let Some(window) = self.window.as_ref() {
                        let size = window.inner_size();
                        self.fit(size.width, size.height);
                    }
                    self.dirty = true;
                }
                Action::RefreshGitStatus => {
                    self.tabs[self.active_tab].git.last_check = None;
                    self.dirty = true;
                }
                Action::CloseTargetTab => {
                    let target = self.ui.as_ref().and_then(|u| u.target_tab);
                    self.close_ui();
                    if let Some(index) =
                        target.and_then(|id| self.tabs.iter().position(|t| t.id == id))
                    {
                        if index == self.active_tab {
                            self.close_tab();
                        } else if let Some(state) = self.tabs[index].saved.as_mut() {
                            state.closing = true;
                            if let Some(runtime) = state.runtime.as_mut() {
                                runtime.request_close();
                            }
                            self.dirty = true;
                        }
                    }
                }
                Action::RenameTab => {
                    let target = self.ui.as_ref().and_then(|u| u.target_tab);
                    if let Some(index) =
                        target.and_then(|id| self.tabs.iter().position(|t| t.id == id))
                    {
                        let label = self.tabs[index].label.clone();
                        self.open_ui(Kind::Rename);
                        if let Some(panel) = self.ui.as_mut() {
                            panel.target_tab = target;
                            panel.draft_value = label;
                            let rows = panel.rows(&self.themes, &self.settings);
                            panel.focus_row(0, &rows);
                        }
                    } else {
                        self.close_ui();
                    }
                }
                Action::SaveTabName => {
                    let panel = self.ui.as_ref().ok_or("Rename prompt is closed")?;
                    let target = panel.target_tab;
                    let label = panel.draft_value.trim().to_string();
                    if label.is_empty()
                        || label.chars().count() > 64
                        || label.chars().any(char::is_control)
                    {
                        return Err("Use 1–64 printable characters for the tab name".into());
                    }
                    if let Some(tab) =
                        target.and_then(|id| self.tabs.iter_mut().find(|t| t.id == id))
                    {
                        tab.label = label;
                        tab.pinned = true;
                    }
                    self.close_ui();
                }
                Action::NewTab => {
                    let mut spec = self.shell_spec.clone().ok_or("Shell is not initialized")?;
                    if !self.shell_explicit {
                        shells::apply(&mut spec, &self.settings.shell);
                    }
                    self.add_tab(spec)?;
                }
                Action::CloseTab => self.close_tab(),
                Action::Copy => {
                    let text = self.selection_text();
                    if !text.is_empty() {
                        if let Some(window) = self.window.as_ref() {
                            clipboard::write(window, &text)?;
                        }
                    }
                    self.close_ui();
                }
                Action::Paste => {
                    self.close_ui();
                    self.paste();
                }
                Action::SelectAll => {
                    let history = if self.terminal.alternate_active() {
                        0
                    } else {
                        self.terminal.scrollback_len()
                    };
                    self.selection =
                        Some(((0, 0), (history + self.rows as usize - 1, self.cols - 1)));
                    self.close_ui();
                }
                Action::Themes => self.open_ui(Kind::Themes),
                Action::Custom => self.open_ui(Kind::Custom),
                Action::ManageTheme(id) => {
                    let mut ui = ui::Ui::new(Kind::Manage, [16. * self.scale, 16. * self.scale]);
                    ui.preview_id = Some(id);
                    self.ui = Some(ui);
                }
                Action::ToggleFamily(family) => {
                    if let Some(ui) = self.ui.as_mut() {
                        if !ui.expanded.remove(&family) {
                            ui.expanded.insert(family.clone());
                        }
                        let rows = ui.rows(&self.themes, &self.settings);
                        let index = rows
                            .iter()
                            .position(
                                |r| matches!(&r.action,Action::ToggleFamily(name) if name==&family),
                            )
                            .unwrap_or(0);
                        ui.focus_row(index, &rows);
                    }
                }
                Action::OpenThemesFolder => {
                    self.store.initialize_theme_folder()?;
                    std::process::Command::new("explorer.exe")
                        .arg(self.store.directory.join("themes"))
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                Action::RemoveTheme => {
                    let id = self
                        .ui
                        .as_ref()
                        .and_then(|ui| ui.preview_id.clone())
                        .ok_or("Preview a custom theme first")?;
                    let path = self
                        .theme_sources
                        .get(&id)
                        .ok_or("This theme has no removable source file")?
                        .clone();
                    let pair = self.store.trash_theme(&path)?;
                    if self.settings.active_theme_id == id {
                        let mut settings = self.settings.clone();
                        settings.active_theme_id = "catppuccin-mocha".into();
                        if let Err(error) = self.store.save_settings(&settings) {
                            self.store.restore_theme(&pair)?;
                            return Err(error);
                        }
                        self.settings = settings;
                    }
                    self.removed_theme = Some(pair);
                    self.themes.retain(|t| t.id != id);
                    self.theme_sources.remove(&id);
                    self.theme_generation += 1;
                    self.rebuild_renderer();
                    self.open_ui(Kind::Custom);
                }
                Action::RestoreTheme => {
                    let pair = self
                        .removed_theme
                        .as_ref()
                        .ok_or("No removed theme to restore")?;
                    self.store.restore_theme(pair)?;
                    self.removed_theme = None;
                    self.theme_generation += 1;
                    self.open_ui(Kind::Custom);
                }
                Action::Settings => self.open_ui(Kind::Settings),
                Action::Appearance => self.open_ui(Kind::Appearance),
                Action::Privacy => self.open_ui(Kind::Privacy),
                Action::PrivacyMode(mode) => {
                    let mut settings = self.committed_preferences();
                    settings.privacy_mode = mode;
                    settings.validate()?;
                    self.commit_preferences(settings)?;
                    self.privacy_reveal.hide();
                    self.privacy_dirty = true;
                }
                Action::Updates => self.open_ui(Kind::Updates),
                Action::UpdateChannel(channel) => {
                    self.command_update_channel = None;
                    self.command_update_pending = false;
                    if self.update_worker.is_some() {
                        return Err("Cancel or finish the current operation first".into());
                    }
                    let mut settings = self.committed_preferences();
                    settings.update_channel = channel;
                    self.commit_preferences(settings)?;
                    self.update_candidate = None;
                    let old = self.update_stage.take();
                    self.open_ui(Kind::Updates);
                    self.update_message = "Channel saved; check for a release".into();
                    if let Some(stage) = old {
                        self.start_update_task(
                            crate::updates::Task::Discard {
                                root: stage.root,
                                directory: self.store.directory.clone(),
                            },
                            "Discarding previous staging...",
                        )?;
                    }
                    self.refresh_update_panel();
                }
                Action::CheckUpdate => {
                    self.command_update_channel = None;
                    self.command_update_pending = false;
                    if self.update_worker.is_some() {
                        return Err("Wait for the current operation or cancel it first".into());
                    }
                    let discard = self.update_stage.take().map(|stage| stage.root);
                    self.update_candidate = None;
                    self.start_update_task(
                        crate::updates::Task::Check {
                            channel: self.settings.update_channel.clone(),
                            current: env!("CARGO_PKG_VERSION").into(),
                            directory: self.store.directory.clone(),
                            discard,
                        },
                        "Checking GitHub Releases...",
                    )?;
                }
                Action::DownloadUpdate => {
                    if self.update_worker.is_some() {
                        return Err("Wait for the current operation or cancel it first".into());
                    }
                    let candidate = self
                        .update_candidate
                        .clone()
                        .ok_or("Check for an update first")?;
                    let discard = self.update_stage.take().map(|stage| stage.root);
                    self.start_update_task(
                        crate::updates::Task::Download {
                            candidate,
                            directory: self.store.directory.clone(),
                            discard,
                        },
                        "Downloading and verifying...",
                    )?;
                }
                Action::InstallUpdate => {
                    if self.update_worker.is_some() {
                        return Err("Wait for download verification to finish".into());
                    }
                    self.update_stage
                        .as_ref()
                        .ok_or("Download and verify an update first")?;
                    self.open_ui(Kind::ConfirmUpdate);
                }
                Action::ConfirmUpdate => {
                    let executable =
                        std::env::current_exe().map_err(|_| "Installed executable unavailable")?;
                    if crate::updates::cli::installation_in_use(&executable)? {
                        return Err("Close other Juna windows before installing this update".into());
                    }
                    let stage = self
                        .update_stage
                        .clone()
                        .ok_or("Download and verify an update first")?;
                    let shell = self.shell_spec.clone().ok_or("Shell is not initialised")?;
                    let cwd = self
                        .terminal
                        .working_directory()
                        .1
                        .map(std::path::PathBuf::from)
                        .or_else(|| shell.cwd.clone())
                        .or_else(|| std::env::current_dir().ok())
                        .ok_or("Restart folder unavailable")?;
                    let target =
                        std::env::current_exe().map_err(|_| "Installed executable unavailable")?;
                    self.open_ui(Kind::Updates);
                    self.start_update_task(
                        crate::updates::Task::Prepare {
                            stage,
                            target,
                            shell,
                            cwd,
                            directory: self.store.directory.clone(),
                        },
                        "Preparing safe restart...",
                    )?;
                }
                Action::CancelUpdate => {
                    self.command_update_pending = false;
                    if let Some(worker) = self.update_worker.as_ref() {
                        worker
                            .cancel
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        self.update_cancelled = true;
                        self.update_message =
                            "Cancelling after the current HTTPS operation...".into();
                    } else {
                        self.update_message = "No check or download is running".into();
                    }
                    self.refresh_update_panel();
                }
                Action::TerminalSettings => self.open_ui(Kind::TerminalSettings),
                Action::Shells => self.open_ui(Kind::Shells),
                Action::PickShell(name) => {
                    if !crate::settings::valid_shell(&name) {
                        return Err("Unknown shell".into());
                    }
                    if !shells::available(&name) {
                        return Err(format!(
                            "{} was not found on PATH",
                            shells::display(&name)
                        ));
                    }
                    let mut settings = self.committed_preferences();
                    settings.shell = name;
                    self.commit_preferences(settings)?;
                    self.open_ui(Kind::Shells);
                    if let Some(ui) = self.ui.as_mut() {
                        ui.message = if self.shell_explicit {
                            "Saved; this launch's --shell still wins".into()
                        } else {
                            "Applies to new tabs".into()
                        };
                    }
                }
                Action::Tools => self.open_ui(Kind::Tools),
                Action::ImportExport => self.open_ui(Kind::ImportExport),
                Action::Junafetch => self.open_ui(Kind::Junafetch),
                Action::FetchProfiles => self.open_ui(Kind::FetchProfiles),
                Action::FetchArtwork => self.open_ui(Kind::FetchArtwork),
                Action::FetchProfile(profile) => {
                    let mut settings = self.committed_preferences();
                    settings.junafetch_profile = profile;
                    self.commit_preferences(settings)?;
                    self.open_ui(Kind::Junafetch);
                    if let Some(ui) = self.ui.as_mut() {
                        ui.message = "Saved — applies to next fetch".into();
                    }
                }
                Action::FetchLogo(logo) => {
                    let mut settings = self.committed_preferences();
                    settings.junafetch_logo = logo;
                    self.commit_preferences(settings)?;
                    self.open_ui(Kind::Junafetch);
                    if let Some(ui) = self.ui.as_mut() {
                        ui.message = "Saved — applies to next fetch".into();
                    }
                }
                Action::Fonts => self.open_ui(Kind::Fonts),
                Action::PickFont(name) => {
                    let mut settings = self.committed_preferences();
                    settings.font_family = name;
                    self.commit_preferences(settings)?;
                    self.rebuild_renderer();
                    self.open_ui(Kind::Appearance);
                    if let Some(panel) = self.ui.as_mut() {
                        panel.message = "Font saved".into();
                    }
                }
                Action::SaveSettings => {
                    self.store.save_settings(&self.settings)?;
                    self.settings_before = None;
                    self.close_ui();
                }
                Action::Resize(step) => self.resize_font(input::Zoom::Step(step)),
                Action::Cancel => self.close_ui(),
                Action::Pick(id) => {
                    let mut settings = self.settings.clone();
                    settings.active_theme_id = id;
                    self.store.save_settings(&settings)?;
                    self.settings = settings;
                    self.close_ui();
                }
                Action::Duplicate | Action::Edit => {
                    if let Some(before) = self.settings_before.take() {
                        self.settings = before;
                        self.rebuild_renderer();
                    }
                    let chosen = self
                        .ui
                        .as_ref()
                        .and_then(|ui| ui.preview_id.as_ref())
                        .and_then(|id| self.themes.iter().find(|t| &t.id == id));
                    let mut theme = chosen.cloned().unwrap_or_else(|| self.active_theme());
                    if action == Action::Duplicate
                        || !theme.id.starts_with("custom-")
                        || theme.id.starts_with("custom-import-")
                    {
                        theme.id = crate::settings::new_id();
                        theme.name = format!("My {}", theme.name);
                        theme.source = None;
                    }
                    let mut ui = ui::Ui::new(Kind::Editor, [16. * self.scale, 16. * self.scale]);
                    ui.draft = Some(theme);
                    let rows = ui.rows(&self.themes, &self.settings);
                    ui.focus_row(0, &rows);
                    self.ui = Some(ui);
                }
                Action::Save => {
                    let ui = self.ui.as_ref().ok_or("No theme editor")?;
                    if !ui.message.is_empty() {
                        return Err(ui.message.clone());
                    }
                    let theme = ui.draft.as_ref().ok_or("No theme draft")?.clone();
                    if let Some(path) = self.theme_sources.get(&theme.id).cloned() {
                        let path = self.store.save_theme_at(&theme, &path)?;
                        self.theme_sources.insert(theme.id.clone(), path);
                    } else {
                        self.store.save_theme(&theme)?;
                        let path = self
                            .store
                            .directory
                            .join("themes")
                            .join(crate::theme_library::folder_name(&theme.family_name()))
                            .join(format!("{}.json", theme.id));
                        self.theme_sources.insert(theme.id.clone(), path);
                    }
                    self.theme_generation += 1;
                    let mut settings = self.settings.clone();
                    settings.active_theme_id = theme.id.clone();
                    self.store.save_settings(&settings)?;
                    if let Some(existing) = self.themes.iter_mut().find(|t| t.id == theme.id) {
                        *existing = theme;
                    } else {
                        self.themes.push(theme);
                    }
                    self.settings = settings;
                    self.close_ui();
                }
                Action::Setting(index) => {
                    if self.settings_before.is_none() {
                        self.settings_before = Some(self.settings.clone());
                    }
                    self.ui = Some(ui::Ui::number(
                        index,
                        &self.settings,
                        [16. * self.scale, 16. * self.scale],
                    ));
                }
                Action::ApplySetting => {
                    let ui = self.ui.as_ref().ok_or("No numeric prompt")?;
                    let index = ui.number.ok_or("No field selected")?;
                    let settings = ui::numeric_settings(
                        &self.committed_preferences(),
                        index,
                        &ui.draft_value,
                    )?;
                    self.commit_preferences(settings)?;
                    self.rebuild_renderer();
                    self.open_ui(Kind::Appearance);
                    if let Some(ui) = self.ui.as_mut() {
                        let rows = ui.rows(&self.themes, &self.settings);
                        ui.focus_row(index, &rows);
                    }
                }
                Action::CancelSetting => {
                    let index = self.ui.as_ref().and_then(|ui| ui.number).unwrap_or(0);
                    if let Some(before) = self.ui.as_ref().and_then(|ui| ui.prior_settings.clone())
                    {
                        self.settings = before;
                        self.rebuild_renderer();
                    }
                    self.open_ui(Kind::Appearance);
                    if let Some(ui) = self.ui.as_mut() {
                        let rows = ui.rows(&self.themes, &self.settings);
                        ui.focus_row(index, &rows);
                    }
                }
                Action::Search | Action::Color(_) | Action::Name | Action::Family => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            log::warn!("Options: {error}");
            if let Some(ui) = self.ui.as_mut() {
                ui.message = error;
            }
        }
        self.dirty = true;
    }
    fn handle_ui(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::CursorMoved { position, .. } = event {
            self.pointer_px = [position.x as f32, position.y as f32];
        }
        if let WindowEvent::MouseInput {
            state: ElementState::Released,
            button,
            ..
        } = event
        {
            if self.ui_mouse_latch.contains(button) {
                self.ui_mouse_latch.retain(|b| b != button);
                return true;
            }
        }
        let Some(mut ui) = self.ui.take() else {
            return false;
        };
        if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            }
        ) {
            self.ui = Some(ui);
            if self
                .ui
                .as_ref()
                .is_some_and(|panel| matches!(panel.kind, ui::Kind::Context | ui::Kind::TabContext))
            {
                self.open_context_menu();
            } else if !self.ui_mouse_latch.contains(&MouseButton::Right) {
                self.ui_mouse_latch.push(MouseButton::Right);
            }
            return true;
        }
        let rows = ui.rows(&self.themes, &self.settings);
        let size = self
            .window
            .as_ref()
            .map(|w| w.inner_size())
            .unwrap_or(winit::dpi::PhysicalSize::new(1024, 768));
        let width = size.width as f32;
        let height = size.height as f32;
        let mut action = None;
        let consumed = match event {
            WindowEvent::CursorMoved { .. } => {
                if ui
                    .star_hit(self.pointer_px, width, height, self.scale, &rows)
                    .is_some()
                {
                    self.ui = Some(ui);
                    return true;
                }
                if let Some(index) = ui.hit(self.pointer_px, width, height, self.scale, rows.len())
                {
                    if index != ui.focus {
                        ui.focus_row(index, &rows);
                    }
                }
                true
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *state == ElementState::Pressed {
                    if !self.ui_mouse_latch.contains(button) {
                        self.ui_mouse_latch.push(*button);
                    }
                    if *button == MouseButton::Left {
                        if let Some(id) =
                            ui.star_hit(self.pointer_px, width, height, self.scale, &rows)
                        {
                            action = Some(ui::Action::ToggleFavourite(id));
                        } else if let Some(index) =
                            ui.hit(self.pointer_px, width, height, self.scale, rows.len())
                        {
                            ui.focus_row(index, &rows);
                            if rows[index].value.is_none() {
                                action = Some(rows[index].action.clone());
                            }
                        } else {
                            let (r, _, _) = ui.geometry(width, height, self.scale, rows.len());
                            if self.pointer_px[0] < r[0]
                                || self.pointer_px[0] > r[0] + r[2]
                                || self.pointer_px[1] < r[1]
                                || self.pointer_px[1] > r[1] + r[3]
                            {
                                action = Some(if ui.kind == ui::Kind::Number {
                                    ui::Action::CancelSetting
                                } else {
                                    ui::Action::Cancel
                                });
                            }
                        }
                    } else if *button == MouseButton::Right {
                        action = Some(if ui.kind == ui::Kind::Number {
                            ui::Action::CancelSetting
                        } else {
                            ui::Action::Cancel
                        });
                    }
                }
                true
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !self.ime {
                    if self.modifiers.control_key()
                        && matches!(&event.logical_key,Key::Character(s)if s.eq_ignore_ascii_case("v"))
                    {
                        if matches!(ui.kind, ui::Kind::Themes | ui::Kind::Fonts)
                            || rows.get(ui.focus).is_some_and(|r| r.value.is_some())
                        {
                            match clipboard::read() {
                                Ok(text) => ui.paste_text(&text),
                                Err(e) => ui.message = e,
                            }
                        }
                    } else {
                        action = ui.key(
                            &event.logical_key,
                            event.text.as_deref(),
                            self.modifiers.control_key(),
                            &rows,
                        );
                    }
                }
                true
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.ime = false;
                ui.key(
                    &Key::Character(text.clone().into()),
                    Some(text),
                    false,
                    &rows,
                );
                true
            }
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                self.ime = !text.is_empty();
                true
            }
            WindowEvent::Ime(Ime::Disabled) => {
                self.ime = false;
                true
            }
            WindowEvent::Ime(_) => true,
            WindowEvent::MouseWheel { delta, .. } => {
                let (_, visible, step) = ui.geometry(width, height, self.scale, rows.len());
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -*y * 3.,
                    MouseScrollDelta::PixelDelta(p) => -p.y as f32 / step,
                };
                ui.scroll_rows(amount, visible, &rows);
                self.ui = Some(ui);
                self.dirty = true;
                return true;
            }
            _ => false,
        };
        self.ui = Some(ui);
        if consumed {
            self.preview_ui();
            if let Some(action) = action {
                self.ui_action(action);
            }
        }
        consumed
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            let attributes = Window::default_attributes()
                .with_title(crate::window_identity::title())
                .with_visible(false)
                .with_inner_size(winit::dpi::LogicalSize::new(
                    self.cols as f32 * CELL_W + PAD * 2.,
                    self.rows as f32 * CELL_H + PAD * 2. + ui::TAB_HEIGHT,
                ));
            #[cfg(windows)]
            let attributes = {
                use winit::platform::windows::{IconExtWindows, WindowAttributesExtWindows};
                let small = winit::window::Icon::from_resource(
                    1,
                    Some(winit::dpi::PhysicalSize::new(32, 32)),
                )
                .map_err(|e| format!("Juna window icon: {e}"))?;
                let large = winit::window::Icon::from_resource(
                    1,
                    Some(winit::dpi::PhysicalSize::new(48, 48)),
                )
                .map_err(|e| format!("Juna taskbar icon: {e}"))?;
                attributes
                    .with_window_icon(Some(small))
                    .with_taskbar_icon(Some(large))
            };
            let window = Arc::new(
                event_loop
                    .create_window(attributes)
                    .map_err(|e| e.to_string())?,
            );
            #[cfg(windows)]
            {
                use windows::Win32::{
                    Foundation::{HWND, LPARAM, WPARAM},
                    UI::WindowsAndMessaging::{
                        GetClassLongPtrW, SendMessageW, SetClassLongPtrW, GCLP_HICON, GCLP_HICONSM,
                        WM_GETICON,
                    },
                };
                use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
                let RawWindowHandle::Win32(handle) =
                    window.window_handle().map_err(|e| e.to_string())?.as_raw()
                else {
                    return Err("Expected a Windows window handle".into());
                };
                let hwnd = HWND(handle.hwnd.get() as *mut _);
                // SAFETY: winit owns this live HWND on the current UI thread; WM_GETICON has no pointer arguments.
                let (small, large) = unsafe {
                    (
                        SendMessageW(hwnd, WM_GETICON, Some(WPARAM(0)), Some(LPARAM(0))),
                        SendMessageW(hwnd, WM_GETICON, Some(WPARAM(1)), Some(LPARAM(0))),
                    )
                };
                if small.0 == 0 || large.0 == 0 {
                    return Err("Juna native window icon was not installed".into());
                }
                // Explorer draws the live taskbar button from the window class icon,
                // not the per-window WM_SETICON handles winit installs. The handles
                // belong to the window and outlive this class registration.
                // SAFETY: the class and HICONs are owned by winit for this window's
                // lifetime on this UI thread; Set/GetClassLongPtr only swap a class word.
                unsafe {
                    SetClassLongPtrW(hwnd, GCLP_HICON, large.0);
                    SetClassLongPtrW(hwnd, GCLP_HICONSM, small.0);
                }
                // SAFETY: reads back the class words set immediately above on the same live window.
                if unsafe {
                    GetClassLongPtrW(hwnd, GCLP_HICON) != large.0 as usize
                        || GetClassLongPtrW(hwnd, GCLP_HICONSM) != small.0 as usize
                } {
                    return Err("Juna window class icon was not installed".into());
                }
                log::info!("Window and class icons verified: small and taskbar");
                taskbar_properties(hwnd)?;
            }
            window.set_visible(true);
            window.set_ime_allowed(true);
            self.scale = window.scale_factor() as f32 * self.settings.font_size / FONT_PX;
            let size = window.inner_size();
            let display = event_loop.owned_display_handle();
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
                Box::new(display),
            ));
            let target = window.clone();
            let gpu = Gpu::new(
                &instance,
                |instance| instance.create_surface(target).map_err(|e| e.to_string()),
                size.width,
                size.height,
            )
            .map_err(|e| e.to_string())?;
            log::info!("GPU: {:?}", gpu.choice);
            self.renderer = Some(
                crate::render::Renderer::new_with_family(
                    &gpu.device,
                    &gpu.queue,
                    gpu.format,
                    2048,
                    (CELL_W * self.scale, CELL_H * self.scale),
                    PAD * self.scale,
                    FONT_PX * self.scale,
                    &self.settings.font_family,
                )
                .ok_or("No usable font")?,
            );
            self.gpu = Some(gpu);
            self.window = Some(window);
            let theme = self.active_theme();
            if let Some(r) = self.renderer.as_mut() {
                r.apply_theme(&theme, self.settings.minimum_contrast)?;
            }
            self.fit(size.width, size.height);
            if let Err(error) = self.store.initialize_theme_folder() {
                log::warn!("Theme folder: {error}");
            }
            let library = crate::theme_library::scan(&self.store.directory);
            self.theme_sources = library.sources;
            self.theme_worker = crate::theme_library::Worker::new(self.store.directory.clone())
                .map(Some)
                .unwrap_or_else(|error| {
                    log::warn!("Theme discovery: {error}");
                    None
                });
            let mut spec = SessionSpec::from_environment().map_err(|e| e.to_string())?;
            self.shell_explicit = SessionSpec::shell_is_explicit();
            if !self.shell_explicit {
                shells::apply(&mut spec, &self.settings.shell);
            }
            #[cfg(windows)]
            let spec = {
                let mut spec = spec;
                match tools::prepare(&self.store.directory, &spec.program) {
                    Ok(environment) => spec.env.extend(environment),
                    Err(error) => log::warn!("Bundled tools unavailable: {error}"),
                }
                spec
            };
            self.tabs[self.active_tab].label = tabs::label(&spec);
            self.tabs[self.active_tab].shell = tabs::shell_name(&spec);
            self.shell_spec = Some(spec.clone());
            self.tabs[self.active_tab].git =
                git_status::View::new(spec.cwd.clone().or_else(|| std::env::current_dir().ok()));
            self.git_worker = match git_status::Worker::new() {
                Ok(worker) => Some(worker),
                Err(_) => {
                    log::warn!("Local Git worker unavailable");
                    self.tabs[self.active_tab].git.report =
                        Some(git_status::Report::Unavailable("worker unavailable"));
                    None
                }
            };
            self.privacy_broker = crate::privacy::ipc::Broker::new().ok();
            if self.privacy_broker.is_none() {
                log::warn!("Protected fetch transport unavailable; values remain masked");
            }
            let spec = self.privacy_environment(spec, self.tabs[self.active_tab].id);
            let spec = shell_reporting::prepare(spec, self.settings.report_shell_directory);
            self.runtime = Some(
                Runtime::spawn(
                    &spec,
                    Dimensions {
                        cols: self.cols,
                        rows: self.rows,
                    },
                )
                .map_err(|e| e.to_string())?,
            );
            #[cfg(debug_assertions)]
            if let Ok(command) = std::env::var("JUNA_ACCEPT_WINDOW_COMMAND") {
                let cwd_ok = std::env::var_os("JUNA_ACCEPT_WINDOW_CWD")
                    .map(std::path::PathBuf::from)
                    .is_some_and(|expected| {
                        std::env::current_dir().ok().as_ref() == Some(&expected)
                    });
                let preferences_ok = std::env::var("JUNA_ACCEPT_WINDOW_FONT").ok().as_ref()
                    == Some(&self.settings.font_family)
                    && std::env::var("JUNA_ACCEPT_WINDOW_SIZE")
                        .ok()
                        .and_then(|v| v.parse::<f32>().ok())
                        == Some(self.settings.font_size);
                log::info!("Window acceptance cwd={cwd_ok} preferences={preferences_ok}");
                if command.len() <= 8192 {
                    self.send(command.as_bytes());
                }
            }
            #[cfg(debug_assertions)]
            if let Ok(count) = std::env::var("JUNA_STARTUP_TABS") {
                let count = count.parse::<usize>().unwrap_or(1).clamp(1, 16);
                for _ in 1..count {
                    self.add_tab(spec.clone())?;
                }
                self.switch_tab(0);
            }
            #[cfg(debug_assertions)]
            if let Ok(panel) = std::env::var("JUNA_STARTUP_PANEL") {
                match panel.as_str() {
                    "themes" => self.open_ui(ui::Kind::Themes),
                    "settings" => self.open_ui(ui::Kind::Settings),
                    "fonts" => self.open_ui(ui::Kind::Fonts),
                    _ => {}
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            log::error!("Startup: {e}");
            self.failure = Some(e);
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }
    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::DroppedFile(path) = &event {
            self.dropped_file(path);
            return;
        }
        self.privacy_event(&event);
        if self.tab_controls(&event) {
            return;
        }
        if self.handle_zoom(&event) {
            return;
        }
        if self.handle_ui(&event) {
            return;
        }
        if self.settings.show_git_status
            && !self.terminal.alternate_active()
            && self.window.as_ref().is_some_and(|w| {
                self.pointer_px[1] >= w.inner_size().height as f32 - ui::STATUS_HEIGHT * self.scale
            })
        {
            if matches!(event, WindowEvent::CursorMoved { .. }) {
                if let Some(button) = self.mouse_button.take() {
                    let modes = self.terminal.input_modes();
                    self.send(&input::mouse(
                        button,
                        self.pointer.0,
                        self.pointer.1,
                        true,
                        self.modifiers,
                        modes.mouse_sgr,
                    ));
                }
                self.selecting = false;
                return;
            }
            if matches!(
                event,
                WindowEvent::MouseInput {
                    button: MouseButton::Left | MouseButton::Middle,
                    ..
                } | WindowEvent::MouseWheel { .. }
            ) {
                return;
            }
        }
        if self.left_inset > 0.
            && self.pointer_px[0] < PAD * self.scale + self.left_inset
            && self.pointer_px[1] >= ui::TAB_HEIGHT * self.scale
        {
            if matches!(event, WindowEvent::CursorMoved { .. }) {
                if let Some(button) = self.mouse_button.take() {
                    let modes = self.terminal.input_modes();
                    self.send(&input::mouse(
                        button,
                        self.pointer.0,
                        self.pointer.1,
                        true,
                        self.modifiers,
                        modes.mouse_sgr,
                    ));
                }
                self.selecting = false;
                return;
            }
            if matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left | MouseButton::Middle,
                    ..
                }
            ) {
                return;
            }
        }
        match event {
            WindowEvent::CloseRequested => self.close_all_tabs(),
            WindowEvent::Resized(size) => self.fit(size.width, size.height),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32 * self.settings.font_size / FONT_PX;
                self.rebuild_renderer();
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::Focused(f) => {
                self.focused = f;
                self.dirty = true;
                if self.terminal.input_modes().focus_events {
                    self.send(if f { b"\x1b[I" } else { b"\x1b[O" });
                }
            }
            WindowEvent::Ime(Ime::Enabled) => {}
            WindowEvent::Ime(Ime::Disabled) => self.ime = false,
            WindowEvent::Ime(Ime::Preedit(text, _)) => self.ime = !text.is_empty(),
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.ime = false;
                self.send(text.as_bytes());
                self.offset = 0;
                self.dirty = true;
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.git_activity = Instant::now();
                if matches!(event.logical_key, Key::Named(NamedKey::Enter)) {
                    self.tabs[self.active_tab].git.invalidate_initial();
                }
                let shortcut = self.modifiers.control_key() && self.modifiers.shift_key();
                if shortcut
                    && matches!(&event.logical_key,Key::Character(s)if s.eq_ignore_ascii_case("v"))
                {
                    self.paste();
                } else if shortcut
                    && matches!(&event.logical_key,Key::Character(s)if s.eq_ignore_ascii_case("c"))
                {
                    let text = self.selection_text();
                    if !text.is_empty() {
                        if let Some(w) = self.window.as_ref() {
                            if let Err(e) = clipboard::write(w, &text) {
                                log::warn!("Copy: {e}");
                            }
                        }
                    }
                } else if self.modifiers.shift_key()
                    && matches!(event.logical_key, Key::Named(NamedKey::Insert))
                {
                    self.paste();
                } else if self.modifiers.shift_key()
                    && matches!(
                        event.logical_key,
                        Key::Named(NamedKey::PageUp | NamedKey::PageDown)
                    )
                {
                    if matches!(event.logical_key, Key::Named(NamedKey::PageUp)) {
                        self.offset =
                            (self.offset + self.rows as usize).min(self.terminal.scrollback_len());
                    } else {
                        self.offset = self.offset.saturating_sub(self.rows as usize);
                    }
                    self.dirty = true;
                } else if !self.ime {
                    if let Some(bytes) =
                        input::keypad(event.physical_key, self.terminal.input_modes()).or_else(
                            || {
                                input::encode(
                                    &event.logical_key,
                                    event.text.as_deref(),
                                    self.modifiers,
                                    self.terminal.input_modes(),
                                )
                            },
                        )
                    {
                        self.offset = 0;
                        self.selection = None;
                        self.send(&bytes);
                        self.dirty = true;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let col = ((position.x as f32 - PAD * self.scale - self.left_inset)
                    / (CELL_W * self.scale))
                    .max(0.) as u16;
                let row = ((position.y as f32 - (PAD + ui::TAB_HEIGHT) * self.scale)
                    / (CELL_H * self.scale))
                    .max(0.) as u16;
                self.pointer = (row.min(self.rows - 1), col.min(self.cols - 1));
                let modes = self.terminal.input_modes();
                if !self.modifiers.shift_key()
                    && (modes.mouse_tracking == 1003
                        || (modes.mouse_tracking == 1002 && self.mouse_button.is_some()))
                {
                    self.send(&input::mouse(
                        32 + self.mouse_button.unwrap_or(3),
                        self.pointer.0,
                        self.pointer.1,
                        false,
                        self.modifiers,
                        modes.mouse_sgr,
                    ));
                }
                if self.selecting {
                    let top = self.history_start();
                    if let Some((_, b)) = self.selection.as_mut() {
                        *b = (top + self.pointer.0 as usize, self.pointer.1);
                    }
                    self.dirty = true;
                }
            }
            WindowEvent::MouseInput { state, button, .. }
                if self.terminal.input_modes().mouse_tracking > 0
                    && !self.modifiers.shift_key() =>
            {
                let code = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                let released = state == ElementState::Released;
                self.mouse_button = if released { None } else { Some(code) };
                self.send(&input::mouse(
                    code,
                    self.pointer.0,
                    self.pointer.1,
                    released,
                    self.modifiers,
                    self.terminal.input_modes().mouse_sgr,
                ));
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.selecting = state == ElementState::Pressed;
                if self.selecting {
                    let p = (
                        self.history_start() + self.pointer.0 as usize,
                        self.pointer.1,
                    );
                    self.selection = Some((p, p));
                }
                self.dirty = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if !self.ui_mouse_latch.contains(&MouseButton::Right) {
                    self.ui_mouse_latch.push(MouseButton::Right);
                }
                self.open_context_menu();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 3.,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / (CELL_H * self.scale),
                };
                if self.terminal.input_modes().mouse_tracking > 0 && !self.modifiers.shift_key() {
                    for _ in 0..lines.abs().ceil().min(64.) as u8 {
                        self.send(&input::mouse(
                            if lines > 0. { 64 } else { 65 },
                            self.pointer.0,
                            self.pointer.1,
                            false,
                            self.modifiers,
                            self.terminal.input_modes().mouse_sgr,
                        ));
                    }
                    return;
                }
                if lines > 0. {
                    self.offset =
                        (self.offset + lines.ceil() as usize).min(self.terminal.scrollback_len());
                } else {
                    self.offset = self.offset.saturating_sub((-lines).ceil() as usize);
                }
                self.dirty = true;
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.poll_privacy();
        self.poll_updates();
        self.poll_settings_files();
        self.refresh_themes();
        self.drain();
        self.drain_inactive();
        self.poll_git_status();
        let mut inactive_done = Vec::new();
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            let Some(state) = tab.saved.as_mut() else {
                continue;
            };
            let Some(runtime) = state.runtime.as_mut() else {
                if state.closing {
                    inactive_done.push(i);
                }
                continue;
            };
            match runtime.tick() {
                Ok(true) => {
                    state.terminal.finish();
                    #[cfg(debug_assertions)]
                    if let Ok(marker) = std::env::var("JUNA_ACCEPT_MARKER") {
                        let content: String = state
                            .terminal
                            .snapshot(0)
                            .cells
                            .iter()
                            .map(|c| match &c.content {
                                CellContent::Char(cp) | CellContent::WideChar(cp) => {
                                    char::from_u32(*cp).unwrap_or('\u{fffd}').to_string()
                                }
                                CellContent::Cluster(s) => s.to_string(),
                                _ => " ".into(),
                            })
                            .collect();
                        log::info!("Inactive marker present={}", content.contains(&marker));
                    }
                    log::info!(
                        "Inactive tab {} exited; stream={:?}",
                        tab.id,
                        runtime.stream.stats()
                    );
                    inactive_done.push(i);
                }
                Ok(false) => {}
                Err(error) => {
                    log::error!("Tab {}: {error}", tab.id);
                    state.failure = Some(error.to_string());
                    state.closing = true;
                    runtime.request_close();
                    if error.to_string().contains("exceeded") {
                        self.failure = Some(error.to_string());
                        event_loop.exit();
                        return;
                    }
                }
            }
        }
        for index in inactive_done.into_iter().rev() {
            if let Some(broker) = self.privacy_broker.as_ref() {
                broker.unregister(self.tabs[index].id);
            }
            self.tabs.remove(index);
            if index < self.active_tab {
                self.active_tab -= 1;
            }
            self.dirty = true;
        }
        let mut active_done = self.runtime.is_none() && self.active_closing;
        if let Some(runtime) = self.runtime.as_mut() {
            match runtime.tick() {
                Ok(true) => {
                    self.terminal.finish();
                    if let Some(window) = self.window.as_ref() {
                        log::info!("Window title: {:?}", window.title());
                    }
                    #[cfg(debug_assertions)]
                    if let Ok(marker) = std::env::var("JUNA_ACCEPT_MARKER") {
                        let content: String = self
                            .terminal
                            .snapshot(0)
                            .cells
                            .iter()
                            .map(|c| match &c.content {
                                CellContent::Char(cp) | CellContent::WideChar(cp) => {
                                    char::from_u32(*cp).unwrap_or('\u{fffd}').to_string()
                                }
                                CellContent::Cluster(s) => s.to_string(),
                                _ => " ".into(),
                            })
                            .collect();
                        log::info!("Acceptance marker present={}", content.contains(&marker));
                    }
                    log::info!(
                        "Shell exited: {:?}; stream: {:?}; frames={}",
                        runtime.exit,
                        runtime.stream.stats(),
                        self.frames
                    );
                    active_done = true;
                }
                Ok(false) => {}
                Err(error) => {
                    let timed_out = error.to_string().contains("exceeded");
                    self.fail(error.to_string());
                    self.active_closing = true;
                    if timed_out {
                        event_loop.exit();
                        return;
                    }
                }
            }
        }
        if active_done && !self.tabs.is_empty() {
            if let Some(broker) = self.privacy_broker.as_ref() {
                broker.unregister(self.tabs[self.active_tab].id);
            }
            self.tabs.remove(self.active_tab);
            if self.tabs.is_empty() {
                log::info!("Window closing: final terminal session completed");
                self.runtime.take();
                event_loop.exit();
                return;
            }
            let next = self.active_tab.min(self.tabs.len() - 1);
            self.restore_session(next);
        }
        let sync = self.terminal.input_modes().synchronized_output;
        if self
            .ui
            .as_ref()
            .and_then(|u| u.target_tab)
            .is_some_and(|id| !self.tabs.iter().any(|t| t.id == id))
        {
            self.close_ui();
        }
        if !sync {
            self.last_sync = Instant::now();
        }
        if sync && self.last_sync.elapsed() > Duration::from_secs(1) {
            self.tabs[self.active_tab]
                .privacy
                .commit_timeout(&self.terminal, self.offset);
        }
        let now = Instant::now();
        if self.tabs.iter().any(|t| {
            t.attention.is_some_and(|at| {
                (now.duration_since(at).as_millis() as u64) < ui::ATTENTION_FLASH_MS
            })
        }) {
            self.dirty = true;
        }
        if self.dirty
            && !self.minimized
            && (!sync || self.privacy_dirty || self.last_sync.elapsed() > Duration::from_secs(1))
        {
            if let Some(w) = self.window.as_ref() {
                w.request_redraw();
            }
        }
        if self.last_report.elapsed() > Duration::from_secs(10) {
            self.last_report = Instant::now();
            if let Some(r) = self.runtime.as_ref() {
                log::info!(
                    "stream={:?}, history={}",
                    r.stream.stats(),
                    self.terminal.scrollback_len()
                );
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }
}
pub fn run(cols: u16, rows: u16) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        // SAFETY: the static UTF-16 application ID is NUL-terminated and is set before any UI is created.
        unsafe {
            windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(windows::core::w!(
                "LeakTechnologies.Juna.Terminal.Desktop"
            ))?;
        }
    }
    let path = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Juna");
    std::fs::create_dir_all(&path)?;
    let (log_path, log) = new_window::diagnostic_file(&path)?;
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder
        .target(env_logger::Target::Pipe(Box::new(log)))
        .try_init()?;
    let previous_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(location) = info.location() {
            log::error!(
                "Application panic at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        } else {
            log::error!("Application panic at unknown source location");
        }
        // Panic payloads can contain user data; record location only in the diagnostic file.
        log::logger().flush();
        previous_panic_hook(info);
    }));
    log::info!(
        "Juna version={} commit={}",
        env!("CARGO_PKG_VERSION"),
        env!("JUNA_BUILD_COMMIT")
    );
    log::info!(
        "Diagnostic file: {}",
        log_path.file_name().unwrap_or_default().to_string_lossy()
    );
    #[cfg(windows)]
    log::info!("Taskbar application ID: {APP_ID}");
    if let Err(error) = std::fs::write(path.join("font-licenses.txt"), crate::fonts::notices()) {
        log::warn!("Writing bundled font notices: {error}");
    }
    let event_loop = EventLoop::new()?;
    let mut app = App::new(cols, rows);
    event_loop.run_app(&mut app)?;
    if let Some(failure) = app.failure.take() {
        return Err(failure.into());
    }
    Ok(())
}
pub fn run_update_helper(path: &std::path::Path) -> Result<(), String> {
    let result = crate::updates::helper::run(path);
    if let Err(error) = result.as_ref() {
        clipboard::error_dialog(&format!("Juna update: {error}"));
    }
    result
}
pub fn update_error_dialog(message: &str) {
    clipboard::error_dialog(message);
}

#[cfg(test)]
mod option_tests {
    #[test]
    fn protected_fields_use_real_input_gestures_and_copy_only_shaded_cells() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let mut app = App::new(20, 3);
        app.store = crate::settings::Store::new(root.clone());
        app.settings = crate::settings::Settings::default();
        let token = "0123456789abcdef0123456789abcdef";
        app.terminal.advance(
            format!("\x1b]777;JunaPrivate;{token}\x1b\\░░░░░\x1b]777;JunaPrivate;0\x1b\\")
                .as_bytes(),
        );
        app.tabs[0]
            .privacy
            .accept(vec![(token.into(), "192.0.2.55".into())]);
        app.pointer_px = [PAD + 5., PAD + ui::TAB_HEIGHT + 5.];
        let moved = WindowEvent::CursorMoved {
            device_id: winit::event::DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(
                app.pointer_px[0] as f64,
                app.pointer_px[1] as f64,
            ),
        };
        app.privacy_event(&moved);
        assert!(app.privacy_overlay(800, 600).is_none());
        app.privacy_event(&WindowEvent::ModifiersChanged(ModifiersState::SHIFT.into()));
        assert!(app.privacy_overlay(800, 600).is_none());
        app.privacy_event(&moved);
        assert!(app
            .privacy_overlay(800, 600)
            .unwrap()
            .text
            .iter()
            .any(|text| text.value == "192.0.2.55"));
        app.selection = Some(((0, 1), (0, 3)));
        assert_eq!(app.selection_text(), "░░░");
        app.terminal.advance(b"\x1b[?2026h\rchanged");
        app.privacy_event(&WindowEvent::ModifiersChanged(
            ModifiersState::empty().into(),
        ));
        assert!(app.privacy_overlay(800, 600).is_none());
        assert!(app.privacy_dirty);
        app.open_ui(ui::Kind::Privacy);
        app.ui_action(ui::Action::PrivacyMode("hover".into()));
        assert_eq!(app.store.load().0.privacy_mode, "hover");
        app.close_ui();
        app.privacy_event(&moved);
        assert!(app.privacy_overlay(800, 600).is_some());
        app.privacy_event(&WindowEvent::Focused(false));
        assert!(app.privacy_overlay(800, 600).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn updater_channel_persists_and_install_requires_a_verified_candidate() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let mut app = App::new(20, 3);
        app.store = crate::settings::Store::new(root.clone());
        app.settings = crate::settings::Settings::default();
        let cells = app.terminal.snapshot(0).cells;
        app.open_ui(ui::Kind::Updates);
        app.ui_action(ui::Action::InstallUpdate);
        assert_eq!(app.ui.as_ref().unwrap().kind, ui::Kind::Updates);
        assert!(!app.closing_all);
        assert!(app.update_worker.is_none());
        app.ui_action(ui::Action::UpdateChannel("stable".into()));
        assert_eq!(app.store.load().0.update_channel, "stable");
        app.command_update_pending = true;
        app.command_update_channel = Some("preview".into());
        app.accept_downloaded_update(crate::updates::Staged {
            version: "0.1.1-dev999".into(),
            commit: "0".repeat(40),
            exe_hash: "0".repeat(64),
            root: root.join("unopened-fixture"),
        });
        assert_eq!(app.ui.as_ref().unwrap().kind, ui::Kind::ConfirmUpdate);
        assert_eq!(app.ui.as_ref().unwrap().focus, 1);
        assert!(!app.closing_all);
        assert!(app.update_worker.is_none());
        assert_eq!(app.settings.update_channel, "stable");
        assert_eq!(app.terminal.snapshot(0).cells, cells);
        assert_eq!(app.store.load().0.update_channel, "stable");
        assert_eq!(app.terminal.snapshot(0).cells, cells);
        app.open_ui(ui::Kind::ConfirmUpdate);
        let ui = app.ui.as_ref().unwrap();
        let rows = ui.rows(&app.themes, &app.settings);
        assert_eq!(ui.focus, 1);
        assert_eq!(rows[1].action, ui::Action::Updates);
        assert!(app.update_worker.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fetch_profile_choices_persist_and_fonts_return_to_appearance() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let mut app = App::new(20, 3);
        app.store = crate::settings::Store::new(root.clone());
        app.settings = crate::settings::Settings::default();
        app.open_ui(ui::Kind::Settings);
        app.ui_action(ui::Action::Tools);
        app.ui_action(ui::Action::Junafetch);
        app.ui_action(ui::Action::FetchProfiles);
        app.ui_action(ui::Action::FetchProfile("brief".into()));
        app.ui_action(ui::Action::FetchLogo("none".into()));
        app.close_ui();
        let saved = app.store.load().0;
        assert_eq!(saved.junafetch_profile, "brief");
        assert_eq!(saved.junafetch_logo, "none");
        app.open_ui(ui::Kind::Appearance);
        app.ui_action(ui::Action::Fonts);
        app.ui_action(ui::Action::PickFont("Fira Code".into()));
        assert_eq!(app.ui.as_ref().unwrap().kind, ui::Kind::Appearance);
        let mut preview = app.settings.clone();
        preview.font_family = "JetBrains Mono".into();
        app.settings = preview;
        app.rebuild_renderer();
        app.open_ui(ui::Kind::Fonts);
        app.ui_action(ui::Action::Appearance);
        assert_eq!(app.settings.font_family, "Fira Code");
        app.close_ui();
        assert_eq!(app.store.load().0.junafetch_profile, "brief");
        std::fs::remove_dir_all(root).unwrap();
    }
    use super::*;
    #[test]
    fn editing_panels_ignore_right_click_and_numeric_choices_persist() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let mut app = App::new(20, 3);
        app.store = crate::settings::Store::new(root.clone());
        app.settings = crate::settings::Settings::default();
        app.open_ui(ui::Kind::Settings);
        let right = WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Right,
        };
        assert!(app.handle_ui(&right));
        assert_eq!(app.ui.as_ref().unwrap().kind, ui::Kind::Settings);
        app.ui_action(ui::Action::Setting(1));
        app.ui.as_mut().unwrap().set_number_value("18".into());
        app.ui_action(ui::Action::ApplySetting);
        app.ui_action(ui::Action::ToggleLineNumbers);
        app.ui_action(ui::Action::Cancel);
        let (saved, _, errors) = app.store.load();
        assert!(errors.is_empty());
        assert_eq!(saved.font_size, 18.);
        assert!(saved.show_line_numbers);
        app.settings.show_line_numbers = true;
        app.fit(1024, 700);
        let primary = app.cols;
        assert!(app.left_inset > 0.);
        app.terminal.advance(b"\x1b[?1049h");
        app.fit(1024, 700);
        assert_eq!(app.left_inset, 0.);
        assert!(app.cols > primary);
        app.terminal.advance(b"\x1b[?1049l");
        app.fit(1024, 700);
        assert_eq!(app.cols, primary);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn family_bulk_controls_clear_search_without_touching_terminal() {
        let mut app = App::new(20, 3);
        let before = app.terminal.snapshot(0).cells;
        app.open_ui(ui::Kind::Themes);
        app.ui_action(ui::Action::ExpandAll);
        let panel = app.ui.as_ref().unwrap();
        assert_eq!(
            panel
                .rows(&app.themes, &app.settings)
                .iter()
                .filter(|row| matches!(row.action, ui::Action::Pick(_)))
                .count(),
            60 + app
                .settings
                .favourite_theme_ids
                .iter()
                .filter(|id| app.themes.iter().any(|t| &t.id == *id))
                .count()
        );
        app.ui.as_mut().unwrap().query = "catppuccin".into();
        app.ui_action(ui::Action::CollapseAll);
        let panel = app.ui.as_ref().unwrap();
        assert!(panel.query.is_empty());
        assert!(panel.expanded.is_empty());
        assert!(!panel
            .rows(&app.themes, &app.settings)
            .iter()
            .any(|row| matches!(row.action, ui::Action::Pick(_))));
        assert_eq!(app.terminal.snapshot(0).cells, before);
    }
    #[test]
    fn middle_click_confirmation_defaults_to_cancel() {
        let mut app = App::new(20, 3);
        app.pointer_px = [100., 12.];
        let middle = WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Middle,
        };
        assert!(app.tab_controls(&middle));
        let panel = app.ui.as_ref().unwrap();
        assert_eq!(panel.kind, ui::Kind::ConfirmClose);
        assert_eq!(panel.target_tab, Some(1));
        assert_eq!(panel.focus, 1);
        app.ui_action(ui::Action::Cancel);
        assert!(!app.active_closing);
        app.pointer_px = [100., 100.];
        assert!(!app.tab_controls(&middle));
    }
    #[test]
    fn favourites_persist_without_committing_or_cancelling_font_preview() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let mut app = App::new(20, 3);
        app.store = crate::settings::Store::new(root.clone());
        app.settings = crate::settings::Settings::default();
        app.open_ui(ui::Kind::Settings);
        app.settings.font_size = 24.;
        app.ui_action(ui::Action::ToggleFavourite("nord".into()));
        let (saved, _, errors) = app.store.load();
        assert!(errors.is_empty());
        assert_eq!(saved.font_size, 16.);
        assert_eq!(app.settings.font_size, 24.);
        app.ui_action(ui::Action::Cancel);
        assert_eq!(app.settings.font_size, 16.);
        assert_eq!(app.settings.favourite_theme_ids, vec!["nord"]);
        app.ui_action(ui::Action::ToggleFavourite("nord".into()));
        assert!(app.store.load().0.favourite_theme_ids.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn right_click_reanchors_and_switches_between_body_and_tab_menu() {
        let mut app = App::new(20, 3);
        app.pointer_px = [100., 100.];
        app.open_context_menu();
        assert_eq!(app.ui.as_ref().unwrap().kind, ui::Kind::Context);
        let right = WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Right,
        };
        app.pointer_px = [300., 200.];
        assert!(app.handle_ui(&right));
        assert_eq!(app.ui.as_ref().unwrap().anchor, [300., 200.]);
        app.pointer_px = [100., 12.];
        assert!(app.handle_ui(&right));
        assert_eq!(app.ui.as_ref().unwrap().kind, ui::Kind::TabContext);
        assert_eq!(app.ui.as_ref().unwrap().target_tab, Some(1));
    }
    #[test]
    fn tab_menu_targets_stable_identity_and_rename_is_transactional() {
        let mut app = App::new(20, 3);
        app.tabs.push(tabs::Tab {
            id: 42,
            label: "Second".into(),
            pinned: false,
            shell: "PowerShell".into(),
            git: git_status::View::new(None),
            attention: None,
            privacy: Default::default(),
            saved: Some(tabs::SavedSession {
                terminal: TerminalState::new(20, 3),
                runtime: None,
                offset: 0,
                selection: None,
                failure: None,
                closing: false,
            }),
        });
        app.open_ui(ui::Kind::TabContext);
        app.ui.as_mut().unwrap().target_tab = Some(42);
        app.ui_action(ui::Action::RenameTab);
        app.ui.as_mut().unwrap().paste_text("Work 日本語");
        app.ui_action(ui::Action::Cancel);
        assert_eq!(app.tabs[1].label, "Second");
        app.open_ui(ui::Kind::TabContext);
        app.ui.as_mut().unwrap().target_tab = Some(42);
        app.ui_action(ui::Action::RenameTab);
        app.ui.as_mut().unwrap().paste_text("Work 日本語");
        app.ui_action(ui::Action::SaveTabName);
        assert_eq!(app.tabs[1].label, "Work 日本語");
        assert!(app.tabs[1].pinned);
        assert_eq!(app.active_tab, 0);
        app.open_ui(ui::Kind::TabContext);
        app.ui.as_mut().unwrap().target_tab = Some(42);
        app.ui_action(ui::Action::CloseTargetTab);
        assert!(app.tabs[1].saved.as_ref().unwrap().closing);
        assert!(!app.active_closing);
        app.open_ui(ui::Kind::TabContext);
        app.ui.as_mut().unwrap().target_tab = Some(999);
        app.ui_action(ui::Action::CloseTargetTab);
        assert!(!app.active_closing);
    }
    #[test]
    fn reordering_keeps_the_active_engine_with_its_tab_record() {
        let mut app = App::new(20, 3);
        app.terminal.advance(b"active");
        let active = app.terminal.snapshot(0).cells;
        for id in [2, 3] {
            app.tabs.push(tabs::Tab {
                id,
                label: format!("Tab {id}"),
                pinned: false,
                shell: "PowerShell".into(),
                git: git_status::View::new(None),
                attention: None,
                privacy: Default::default(),
                saved: Some(tabs::SavedSession {
                    terminal: TerminalState::new(20, 3),
                    runtime: None,
                    offset: 0,
                    selection: None,
                    failure: None,
                    closing: false,
                }),
            });
        }
        app.move_tab(0, 3);
        assert_eq!(
            app.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>(),
            [2, 3, 1]
        );
        assert_eq!(app.active_tab, 2);
        assert_eq!(app.tabs[app.active_tab].id, 1);
        assert!(app.tabs[app.active_tab].saved.is_none());
        assert!(app.tabs[0].saved.is_some() && app.tabs[1].saved.is_some());
        assert_eq!(app.terminal.snapshot(0).cells, active);
        // Dropping into either adjacent gap leaves the order unchanged.
        app.move_tab(2, 2);
        assert_eq!(
            app.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>(),
            [2, 3, 1]
        );
    }
    #[test]
    fn switching_tabs_moves_complete_screen_and_view_state() {
        let mut app = App::new(20, 3);
        app.terminal.advance(b"first\x1b[?1h");
        app.selection = Some(((0, 0), (0, 2)));
        let first = app.terminal.snapshot(0).cells;
        let mut second = TerminalState::new(20, 3);
        second.advance(b"second\x1b[31m");
        app.tabs.push(tabs::Tab {
            id: 2,
            git: git_status::View::new(None),
            label: "Second".into(),
            pinned: false,
            shell: "PowerShell".into(),
            attention: None,
            privacy: Default::default(),
            saved: Some(tabs::SavedSession {
                terminal: second,
                runtime: None,
                offset: 0,
                selection: None,
                failure: None,
                closing: false,
            }),
        });
        app.switch_tab(1);
        assert!(app.selection.is_none());
        assert!(!app.terminal.input_modes().application_cursor);
        let second = app.terminal.snapshot(0).cells;
        app.switch_tab(0);
        assert_eq!(app.terminal.snapshot(0).cells, first);
        assert!(app.terminal.input_modes().application_cursor);
        assert_eq!(app.selection, Some(((0, 0), (0, 2))));
        app.switch_tab(1);
        assert_eq!(app.terminal.snapshot(0).cells, second);
        assert_eq!(app.tabs.iter().filter(|t| t.saved.is_none()).count(), 1);
    }
    #[test]
    fn font_previews_cancel_save_and_zoom_persist_without_touching_cells() {
        let dir = std::env::temp_dir().join(crate::settings::new_id());
        let mut app = App::new(20, 3);
        app.store = crate::settings::Store::new(dir.clone());
        app.settings = crate::settings::Settings::default();
        app.terminal.advance(b"font preview preserves bytes");
        let cells = app.terminal.snapshot(0).cells;
        app.open_ui(ui::Kind::Settings);
        app.ui_action(ui::Action::Fonts);
        app.ui_action(ui::Action::PickFont("Fira Code".into()));
        app.ui_action(ui::Action::Resize(1));
        assert_eq!(app.settings.font_size, 17.);
        app.ui_action(ui::Action::Cancel);
        assert_eq!(app.settings.font_family, "Fira Code");
        assert_eq!(app.settings.font_size, 17.);
        assert_eq!(app.store.load().0.font_family, "Fira Code");
        app.open_ui(ui::Kind::Fonts);
        app.ui_action(ui::Action::PickFont("JetBrains Mono".into()));
        app.ui_action(ui::Action::SaveSettings);
        assert_eq!(app.store.load().0.font_family, "JetBrains Mono");
        app.resize_font(input::Zoom::Step(100));
        assert_eq!(app.settings.font_size, 40.);
        app.resize_font(input::Zoom::Step(-100));
        assert_eq!(app.settings.font_size, 8.);
        app.resize_font(input::Zoom::Reset);
        assert_eq!(app.store.load().0.font_size, 16.);
        app.modifiers = ModifiersState::CONTROL;
        let wheel = |y| WindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0., y)),
            phase: winit::event::TouchPhase::Moved,
        };
        assert!(app.handle_zoom(&wheel(20.)));
        assert_eq!(app.settings.font_size, 16.);
        assert!(app.handle_zoom(&wheel(20.)));
        assert_eq!(app.store.load().0.font_size, 17.);
        app.modifiers = ModifiersState::empty();
        assert!(!app.handle_zoom(&wheel(40.)));
        assert_eq!(app.terminal.snapshot(0).cells, cells);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn previews_cancel_and_custom_saves_preserve_engine_cells() {
        let dir = std::env::temp_dir().join(crate::settings::new_id());
        let mut app = App::new(10, 3);
        app.store = crate::settings::Store::new(dir.clone());
        app.settings = crate::settings::Settings::default();
        app.themes = crate::theme::builtins();
        app.terminal.advance(b"terminal stays here");
        let before = app.terminal.snapshot(0).cells;
        app.ui_action(ui::Action::Themes);
        app.ui_action(ui::Action::Pick("nord".into()));
        assert_eq!(app.settings.active_theme_id, "nord");
        app.ui_action(ui::Action::Duplicate);
        assert!(app.handle_ui(&WindowEvent::Ime(Ime::Commit("My custom".into()))));
        assert_eq!(
            app.ui.as_ref().unwrap().draft.as_ref().unwrap().name,
            "My custom"
        );
        app.ui_action(ui::Action::Cancel);
        assert_eq!(app.settings.active_theme_id, "nord");
        assert_eq!(app.themes.len(), 60);
        app.ui_action(ui::Action::Duplicate);
        app.handle_ui(&WindowEvent::Ime(Ime::Commit("Stored custom".into())));
        app.ui_action(ui::Action::Save);
        assert_eq!(app.themes.len(), 61);
        let (settings, themes, errors) = app.store.load();
        assert!(errors.is_empty());
        assert_eq!(settings.active_theme_id, app.settings.active_theme_id);
        assert!(themes.iter().any(|t| t.name == "Stored custom"));
        assert_eq!(app.terminal.snapshot(0).cells, before);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
