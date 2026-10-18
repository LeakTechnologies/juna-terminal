use super::{capture, publish, LIMIT};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
            WM_CLOSE, WM_KEYDOWN, WM_KEYUP,
        },
    },
};
pub struct Owned {
    pub child: Child,
    pub hwnd: HWND,
}
impl Owned {
    pub fn alive(&mut self) -> Result<(), String> {
        match self.child.try_wait() {
            Ok(None) => Ok(()),
            _ => Err("Owned process exited before measurement completed".into()),
        }
    }
    pub fn find_window(&mut self) -> Result<(), String> {
        let start = Instant::now();
        while self.hwnd.is_invalid() {
            self.alive()?;
            let mut state = (self.child.id(), HWND::default());
            // SAFETY: callback receives synchronous writable state and queries enumerated windows.
            unsafe {
                let _ = EnumWindows(Some(find), LPARAM(&mut state as *mut _ as isize));
            }
            self.hwnd = state.1;
            if start.elapsed() > LIMIT {
                return Err("Owned window startup timed out".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + LIMIT;
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|_| "Process exit query failed")?
            {
                return if status.success() {
                    Ok(())
                } else {
                    Err("Owned process shutdown failed; measurements rejected".into())
                };
            }
            if Instant::now() > deadline {
                return Err("Owned process shutdown timed out".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        if !self.hwnd.is_invalid() {
            // SAFETY: check against the still-owned child; never use a foreground window.
            unsafe {
                let mut pid = 0;
                GetWindowThreadProcessId(self.hwnd, Some(&mut pid));
                if pid == self.child.id() {
                    let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(10) {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        eprintln!("juna-bench: owned process termination could not be confirmed; run rejected");
    }
}
fn matches_window(hwnd: HWND, pid: u32) -> bool {
    let mut actual = 0;
    let mut title = [0u16; 256];
    // SAFETY: native queries write into bounded live output storage.
    let visible = unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut actual));
        GetWindowTextW(hwnd, &mut title);
        IsWindowVisible(hwnd).as_bool()
    };
    let end = title.iter().position(|c| *c == 0).unwrap_or(title.len());
    actual == pid
        && visible
        && juna::window_identity::recognized(&String::from_utf16_lossy(&title[..end]))
}
unsafe extern "system" fn find(hwnd: HWND, arg: LPARAM) -> BOOL {
    // SAFETY: synchronous enumeration holds the referenced (PID, HWND) state alive.
    let state = unsafe { &mut *(arg.0 as *mut (u32, HWND)) };
    if matches_window(hwnd, state.0) {
        state.1 = hwnd;
        BOOL(0)
    } else {
        BOOL(1)
    }
}
pub struct Observer {
    process: Owned,
    root: PathBuf,
    epoch: u32,
    requested: bool,
}
impl Observer {
    pub fn new(app: &Owned, root: &Path) -> Result<Self, String> {
        let child = Command::new(std::env::current_exe().map_err(|_| "Benchmark unavailable")?)
            .args([
                OsString::from("--observe"),
                app.child.id().to_string().into(),
                (app.hwnd.0 as usize).to_string().into(),
                root.as_os_str().to_owned(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Capture helper launch failed")?;
        let mut observer = Self {
            process: Owned {
                child,
                hwnd: HWND::default(),
            },
            root: root.to_owned(),
            epoch: 0,
            requested: false,
        };
        observer.wait("observer-ready")?;
        Ok(observer)
    }
    fn wait(&mut self, name: &str) -> Result<(), String> {
        let deadline = Instant::now() + LIMIT;
        while !self.root.join(name).is_file() {
            self.process.alive()?;
            if self.root.join("observer-error").is_file() {
                let reason = super::bounded_read(&self.root.join("observer-error"))?;
                return Err(format!(
                    "Capture unavailable; no visual timing result: {reason}"
                ));
            }
            if Instant::now() > deadline {
                return Err("Capture timed out; no visual timing result".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }
    pub fn request(&mut self, epoch: u32) -> Result<(), String> {
        if epoch != self.epoch || self.requested {
            return Err("Nonsequential visual marker request".into());
        }
        publish(&self.root.join(format!("observe-{epoch}")), b"observe")?;
        self.requested = true;
        Ok(())
    }
    pub fn observed(&mut self, epoch: u32) -> Result<i64, String> {
        if epoch != self.epoch || !self.requested {
            return Err("Visual marker was not requested".into());
        }
        self.wait(&format!("painted-{epoch}"))?;
        let timestamp = super::bounded_read(&self.root.join(format!("painted-{epoch}")))?
            .parse()
            .map_err(|_| "Invalid frame timestamp")?;
        self.epoch += 1;
        self.requested = false;
        Ok(timestamp)
    }
    pub fn expect(&mut self, epoch: u32) -> Result<i64, String> {
        self.request(epoch)?;
        self.observed(epoch)
    }
    pub fn finish(mut self) -> Result<(), String> {
        publish(&self.root.join("observer-stop"), b"stop")?;
        self.process.finish()
    }
}
pub fn observe(pid: u32, hwnd: HWND, root: &Path) -> Result<(), String> {
    if !matches_window(hwnd, pid) {
        return Err("Capture ownership mismatch".into());
    }
    let _apartment = capture::Apartment::new().map_err(|_| "WinRT capture unavailable")?;
    let capture = capture::Capture::new(hwnd).map_err(|_| "Window capture unsupported")?;
    publish(&root.join("observer-ready"), b"ready")?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut epoch = 0;
    loop {
        if root.join("observer-stop").is_file() {
            return Ok(());
        }
        if Instant::now() > deadline || !matches_window(hwnd, pid) {
            return Err("Capture target expired".into());
        }
        if epoch >= 64 {
            return Err("Too many visual requests".into());
        }
        if root.join(format!("observe-{epoch}")).is_file()
            && capture
                .marker_count(super::report::marker(epoch))
                .map_err(|e| format!("Frame read failed HRESULT {:08x}", e.code().0 as u32))?
                .is_some_and(|n| n > 100)
        {
            let observed = super::clock::ticks()?;
            publish(
                &root.join(format!("painted-{epoch}")),
                observed.to_string().as_bytes(),
            )?;
            epoch += 1;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
pub fn key(app: &Owned, up: bool) -> Result<(), String> {
    if !matches_window(app.hwnd, app.child.id()) {
        return Err("Input ownership mismatch".into());
    }
    // SAFETY: target only the fresh owned Juna HWND after PID/title validation; no global input injection.
    unsafe {
        PostMessageW(
            Some(app.hwnd),
            if up { WM_KEYUP } else { WM_KEYDOWN },
            WPARAM(0x41),
            LPARAM(if up {
                0xc01e0001u32 as i32 as isize
            } else {
                0x001e0001
            }),
        )
        .map_err(|_| "Targeted key delivery failed".into())
    }
}
