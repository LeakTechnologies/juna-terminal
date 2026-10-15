//! Hidden helper transaction: wait for exit, stage beside installation, retry, rollback, restart.
use super::{hash, read_bounded, version::Version, Receipt, ReceiptStatus, Staged};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    target: PathBuf,
    old_version: String,
    old_commit: String,
    old_hash: String,
    new_version: String,
    new_commit: String,
    new_hash: String,
    helper_hash: String,
    parent: u32,
    shell: String,
    cwd: PathBuf,
}
fn no_link(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "Update path is unavailable")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Update paths cannot be reparse points".into());
        }
    }
    if metadata.file_type().is_symlink() {
        return Err("Update paths cannot be symbolic links".into());
    }
    Ok(())
}
fn identity(path: &Path) -> Result<(String, String, String), String> {
    let bytes = read_bounded(path, 192 * 1024 * 1024)?;
    let fields = crate::release_image::version_strings(&bytes)?;
    if fields.get("ProductName").map(String::as_str) != Some("Juna Terminal") {
        return Err("Only a Juna installation can be updated".into());
    }
    let version = fields
        .get("ProductVersion")
        .filter(|v| Version::parse(v).is_some())
        .ok_or("Executable version missing")?
        .clone();
    let commit = fields
        .get("Comments")
        .and_then(|s| s.strip_prefix("Source commit: "))
        .filter(|s| super::hex(s, 40))
        .ok_or("Executable source identity missing")?
        .to_owned();
    Ok((version, commit, hash(&bytes)?))
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let data = serde_json::to_vec(value).map_err(|_| "Cannot encode update record")?;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| "Cannot create update record")?;
    file.write_all(&data)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot write update record".to_owned())
}
pub fn prepare(
    stage: &Staged,
    target: &Path,
    shell: &crate::session::SessionSpec,
    cwd: &Path,
    directory: &Path,
) -> Result<PathBuf, String> {
    let executable =
        std::env::current_exe().map_err(|_| "Cannot locate update helper executable")?;
    prepare_from(
        stage,
        target,
        shell,
        cwd,
        directory,
        &executable,
        std::process::id(),
    )
}
#[doc(hidden)]
pub fn prepare_from(
    stage: &Staged,
    target: &Path,
    shell: &crate::session::SessionSpec,
    cwd: &Path,
    directory: &Path,
    executable: &Path,
    parent: u32,
) -> Result<PathBuf, String> {
    let target =
        std::fs::canonicalize(target).map_err(|_| "Installed executable is unavailable")?;
    let root =
        std::fs::canonicalize(&stage.root).map_err(|_| "Update staging folder is unavailable")?;
    let updates = std::fs::canonicalize(directory.join("updates"))
        .map_err(|_| "Updates folder unavailable")?;
    if root.parent() != Some(updates.as_path()) {
        return Err("Update staging is outside its managed folder".into());
    }
    no_link(&root)?;
    no_link(&target)?;
    let (old_version, old_commit, old_hash) = identity(&target)?;
    if Version::parse(&stage.version) <= Version::parse(&old_version) {
        return Err("Update must be newer than installed version".into());
    }
    let new = read_bounded(&root.join("package/juna.exe"), 192 * 1024 * 1024)?;
    if hash(&new)? != stage.exe_hash {
        return Err("Staged executable changed".into());
    }
    crate::release_image::verify_identity(&new, &stage.version, &stage.commit)?;
    let cwd = std::fs::canonicalize(cwd).map_err(|_| "Restart directory is unavailable")?;
    if !cwd.is_dir() {
        return Err("Restart directory is not a folder".into());
    }
    let shell = shell
        .program
        .to_str()
        .filter(|s| !s.is_empty() && s.len() < 4096 && !s.chars().any(char::is_control))
        .ok_or("Restart shell is unavailable")?
        .to_owned();
    let helper = root.join("helper.exe");
    for name in ["helper.exe", "install.json", "restart.ready", "restart.pid"] {
        let old = root.join(name);
        if old.exists() {
            no_link(&old)?;
            std::fs::remove_file(old).map_err(|_| "Previous helper staging is still in use")?;
        }
    }
    let mut input = std::fs::File::open(executable).map_err(|_| "Cannot read helper executable")?;
    let mut output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&helper)
        .map_err(|_| "Cannot stage helper executable")?;
    std::io::copy(&mut input, &mut output)
        .and_then(|_| output.sync_all())
        .map_err(|_| "Cannot copy update helper")?;
    drop(output);
    let plan = Plan {
        target,
        old_version,
        old_commit,
        old_hash,
        new_version: stage.version.clone(),
        new_commit: stage.commit.clone(),
        new_hash: stage.exe_hash.clone(),
        helper_hash: hash(&read_bounded(&helper, 192 * 1024 * 1024)?)?,
        parent,
        shell,
        cwd,
    };
    let path = root.join("install.json");
    write_json(&path, &plan)?;
    Ok(path)
}
pub fn launch(plan: &Path) -> Result<(), String> {
    let helper = plan
        .parent()
        .ok_or("Helper plan path is invalid")?
        .join("helper.exe");
    let mut command = Command::new(helper);
    command
        .arg("--update-helper")
        .arg(plan)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.spawn().map_err(|_| "Cannot start update helper")?;
    Ok(())
}
/// First successful rendered frame acknowledges the new app without exposing any path to the shell.
pub fn acknowledge_restart() -> Result<(), String> {
    let Some(token) = std::env::var("JUNA_UPDATE_TOKEN").ok() else {
        return Ok(());
    };
    if !token.starts_with("custom-") || !crate::theme::valid_id(&token) {
        return Err("Invalid update startup token".into());
    }
    let directory = crate::settings::application_directory().join("updates");
    let root = directory.join(token);
    no_link(&root)?;
    if std::fs::canonicalize(&root)
        .map_err(|_| "Update startup folder unavailable")?
        .parent()
        != Some(
            std::fs::canonicalize(directory)
                .map_err(|_| "Updates folder unavailable")?
                .as_path(),
        )
    {
        return Err("Update startup folder is not managed".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(root.join("restart.ready"))
        .map_err(|_| "Cannot acknowledge update startup".to_owned())?;
    file.write_all(
        format!(
            "{}\n{}",
            env!("CARGO_PKG_VERSION"),
            env!("JUNA_BUILD_COMMIT")
        )
        .as_bytes(),
    )
    .and_then(|_| file.sync_all())
    .map_err(|_| "Cannot acknowledge update startup".to_owned())
}
fn copy_tree(source: &Path, destination: &Path, remaining: &mut u64) -> Result<(), String> {
    no_link(source)?;
    let metadata = std::fs::metadata(source).map_err(|_| "Cannot inspect staged file")?;
    if metadata.is_dir() {
        std::fs::create_dir(destination).map_err(|_| "Cannot stage installation directory")?;
        for entry in std::fs::read_dir(source).map_err(|_| "Cannot read staged directory")? {
            let entry = entry.map_err(|_| "Cannot read staged directory")?;
            copy_tree(
                &entry.path(),
                &destination.join(entry.file_name()),
                remaining,
            )?;
        }
    } else if metadata.is_file() {
        *remaining = remaining
            .checked_sub(metadata.len())
            .ok_or("Staged installation is too large")?;
        let mut input = std::fs::File::open(source).map_err(|_| "Cannot read staged file")?;
        let mut out = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)
            .map_err(|_| "Cannot stage installation file")?;
        std::io::copy(&mut input, &mut out)
            .and_then(|_| out.sync_all())
            .map_err(|_| "Cannot copy installation file")?;
    } else {
        return Err("Unsupported staged filesystem entry".into());
    }
    Ok(())
}
struct Move {
    target: PathBuf,
    old: PathBuf,
    new: PathBuf,
    saved: bool,
    installed: bool,
}
pub fn installation_state() -> Option<bool> {
    let executable = std::env::current_exe().ok()?;
    let marker = executable.parent()?.join(".juna-installing");
    if !marker.exists() {
        return None;
    }
    let pid = read_bounded(&marker, 32)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|text| text.trim().parse::<u32>().ok())
        .filter(|pid| *pid > 0);
    #[cfg(windows)]
    {
        use windows::Win32::{
            Foundation::{CloseHandle, WAIT_TIMEOUT},
            System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
        };
        let Some(pid) = pid else {
            return Some(false);
        };
        // SAFETY: requests only synchronization access and closes the returned process handle exactly once; no termination or data access is performed.
        Some(unsafe {
            match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
                Ok(handle) => {
                    let alive = WaitForSingleObject(handle, 0) == WAIT_TIMEOUT;
                    let _ = CloseHandle(handle);
                    alive
                }
                Err(error) => error.code() != windows::core::HRESULT(0x80070057u32 as i32),
            }
        })
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        Some(false)
    }
}
struct InstallationGuard(PathBuf);
impl Drop for InstallationGuard {
    fn drop(&mut self) {
        if std::fs::read_to_string(&self.0).ok().as_deref() == Some(&std::process::id().to_string())
        {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}
fn transaction(plan: &Plan, root: &Path, fail_after: Option<usize>) -> Result<PathBuf, String> {
    let installation = plan.target.parent().ok_or("Installation folder missing")?;
    no_link(installation)?;
    if identity(&plan.target)?
        != (
            plan.old_version.clone(),
            plan.old_commit.clone(),
            plan.old_hash.clone(),
        )
    {
        return Err("Installed executable changed since preparation".into());
    }
    let temp = installation.join(format!(".juna-update-{}", crate::settings::new_id()));
    let backup = installation.join(format!(".juna-rollback-{}", crate::settings::new_id()));
    std::fs::create_dir(&temp).map_err(|_| "Installation folder is not writable")?;
    // Same-volume staging makes each commit/restore a rename, even for portable installs on another drive.
    let package = root.join("package");
    let mut budget = 256 * 1024 * 1024;
    for name in ["juna.exe", "notices", "RELEASE_NOTES.md"] {
        copy_tree(&package.join(name), &temp.join(name), &mut budget)?;
    }
    let bytes = read_bounded(&temp.join("juna.exe"), 192 * 1024 * 1024)?;
    if hash(&bytes)? != plan.new_hash {
        return Err("Staged executable integrity mismatch".into());
    }
    crate::release_image::verify_identity(&bytes, &plan.new_version, &plan.new_commit)?;
    drop(bytes);
    std::fs::create_dir(&backup).map_err(|_| "Cannot create rollback folder")?;
    let marker = installation.join(".juna-installing");
    let mut lock = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
        .map_err(|_| "An installation is already running or was interrupted")?;
    lock.write_all(std::process::id().to_string().as_bytes())
        .and_then(|_| lock.sync_all())
        .map_err(|_| "Cannot record installation guard")?;
    drop(lock);
    let _guard = InstallationGuard(marker);
    let mut moves = vec![Move {
        target: plan.target.clone(),
        old: backup.join("juna.exe"),
        new: temp.join("juna.exe"),
        saved: false,
        installed: false,
    }];
    for name in ["notices", "RELEASE_NOTES.md"] {
        moves.push(Move {
            target: installation.join(name),
            old: backup.join(name),
            new: temp.join(name),
            saved: false,
            installed: false,
        });
    }
    let result = (|| {
        for (index, item) in moves.iter_mut().enumerate() {
            if item.target.exists() {
                no_link(&item.target)?;
                let deadline = Instant::now() + Duration::from_secs(30);
                loop {
                    match std::fs::rename(&item.target, &item.old) {
                        Ok(()) => {
                            item.saved = true;
                            break;
                        }
                        Err(_) if Instant::now() < deadline => {
                            std::thread::sleep(Duration::from_millis(500))
                        }
                        Err(_) => return Err(
                            "Replacement remained locked; close other Juna windows and try again"
                                .into(),
                        ),
                    }
                }
            }
            std::fs::rename(&item.new, &item.target).map_err(|_| "Cannot install staged update")?;
            item.installed = true;
            if fail_after == Some(index) {
                return Err("Controlled transaction failure".into());
            }
        }
        let bytes = read_bounded(&plan.target, 192 * 1024 * 1024)?;
        if hash(&bytes)? != plan.new_hash {
            return Err("Installed executable verification failed".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut restored = true;
        for item in moves.iter_mut().rev() {
            if item.installed && rename_retry(&item.target, &item.new).is_err() {
                restored = false;
            }
            if item.saved && rename_retry(&item.old, &item.target).is_err() {
                restored = false;
            }
        }
        if !restored {
            return Err(
                "Rollback could not finish; recover files from the .juna-rollback folder".into(),
            );
        }
        let _ = std::fs::remove_dir(&temp);
        let _ = std::fs::remove_dir(&backup);
        return Err(error);
    }
    let _ = std::fs::remove_dir(&temp);
    Ok(backup)
}
fn rename_retry(source: &Path, destination: &Path) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match std::fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => return Err("Rollback rename remained locked".into()),
        }
    }
}
fn receipt_path(directory: &Path, target: &Path) -> Result<PathBuf, String> {
    let target = std::fs::canonicalize(target)
        .or_else(|_| {
            let parent = target
                .parent()
                .ok_or_else(|| std::io::Error::other("Missing installation folder"))?;
            Ok::<_, std::io::Error>(
                std::fs::canonicalize(parent)?.join(
                    target
                        .file_name()
                        .ok_or_else(|| std::io::Error::other("Missing executable name"))?,
                ),
            )
        })
        .map_err(|_| "Installed executable unavailable")?;
    let key = hash(target.to_string_lossy().to_lowercase().as_bytes())?;
    Ok(directory
        .join("updates")
        .join(format!("result-{}.json", &key[..32])))
}
fn receipt(directory: &Path, plan: &Plan, status: ReceiptStatus) -> Result<(), String> {
    let path = receipt_path(directory, &plan.target)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|_| "Cannot replace update result record")?;
    }
    write_json(
        &path,
        &Receipt {
            version: plan.new_version.clone(),
            status,
        },
    )
}
pub fn take_receipt(directory: &Path) -> Option<Receipt> {
    let executable = std::env::current_exe().ok()?;
    let path = receipt_path(directory, &executable).ok()?;
    let bytes = read_bounded(&path, 65536).ok()?;
    let result: Receipt = serde_json::from_slice(&bytes).ok()?;
    Version::parse(&result.version)?;
    std::fs::remove_file(path).ok()?;
    Some(result)
}
#[cfg(windows)]
fn wait_parent(pid: u32) -> Result<(), String> {
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
    };
    // SAFETY: only SYNCHRONIZE access is requested for the recorded parent PID; the returned handle is closed once after waiting.
    unsafe {
        let handle = match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(handle) => handle,
            Err(error) if error.code() == windows::core::HRESULT(0x80070057u32 as i32) => {
                return Ok(())
            }
            Err(_) => return Err("Cannot wait for the original Juna process".into()),
        };
        let result = WaitForSingleObject(handle, 30000);
        let _ = CloseHandle(handle);
        if result != WAIT_OBJECT_0 {
            return Err("Juna did not finish closing; update was not installed".into());
        }
    }
    Ok(())
}
#[cfg(not(windows))]
fn wait_parent(_: u32) -> Result<(), String> {
    Err("Update installation requires Windows".into())
}
pub fn run(path: &Path) -> Result<(), String> {
    let directory = crate::settings::application_directory();
    let path = std::fs::canonicalize(path).map_err(|_| "Helper plan unavailable")?;
    let root = path.parent().ok_or("Helper plan has no folder")?;
    let updates = std::fs::canonicalize(directory.join("updates"))
        .map_err(|_| "Managed updates folder unavailable")?;
    if path.file_name().is_none_or(|name| name != "install.json")
        || root.parent() != Some(updates.as_path())
    {
        return Err("Helper plan is outside managed staging".into());
    }
    no_link(root)?;
    no_link(&path)?;
    let plan: Plan =
        serde_json::from_slice(&read_bounded(&path, 65536)?).map_err(|_| "Invalid helper plan")?;
    if !plan.target.is_absolute()
        || !plan.cwd.is_absolute()
        || plan.parent == 0
        || Version::parse(&plan.new_version).is_none()
        || Version::parse(&plan.old_version).is_none()
        || Version::parse(&plan.new_version) <= Version::parse(&plan.old_version)
        || !super::hex(&plan.new_hash, 64)
        || !super::hex(&plan.old_hash, 64)
        || !super::hex(&plan.new_commit, 40)
        || !super::hex(&plan.old_commit, 40)
        || plan.shell.is_empty()
        || plan.shell.len() > 4096
        || plan.shell.chars().any(char::is_control)
    {
        return Err("Invalid helper installation identity".into());
    }
    let executable = std::env::current_exe().map_err(|_| "Helper identity unavailable")?;
    if std::fs::canonicalize(executable).map_err(|_| "Helper identity unavailable")?
        != root.join("helper.exe")
        || hash(&read_bounded(&root.join("helper.exe"), 192 * 1024 * 1024)?)? != plan.helper_hash
    {
        return Err("Helper executable identity mismatch".into());
    }
    let result = (|| {
        wait_parent(plan.parent)?;
        finish_install(&plan, root, &directory)
    })();
    if result.is_err() {
        let _ = receipt(&directory, &plan, ReceiptStatus::Failed);
    }
    result
}
fn finish_install(plan: &Plan, root: &Path, directory: &Path) -> Result<(), String> {
    let result = transaction(plan, root, None);
    let backup = match result {
        Err(error) => {
            let restored = identity(&plan.target)
                .ok()
                .is_some_and(|(_, _, hash)| hash == plan.old_hash);
            let _ = receipt(
                directory,
                plan,
                if restored {
                    ReceiptStatus::RolledBack
                } else {
                    ReceiptStatus::Failed
                },
            );
            if restored {
                let shell = crate::session::SessionSpec {
                    program: plan.shell.clone().into(),
                    ..Default::default()
                };
                let mut command =
                    crate::app::new_window::command(&plan.target, &shell, None, &plan.cwd)
                        .map_err(|_| "Restart directory is unavailable")?;
                command.env_remove("JUNA_UPDATE_TOKEN");
                command.spawn().map_err(|_| {
                    "Previous version retained but restart failed; launch Juna manually"
                })?;
                return Ok(());
            }
            return Err(error);
        }
        Ok(backup) => backup,
    };
    let shell = crate::session::SessionSpec {
        program: plan.shell.clone().into(),
        ..Default::default()
    };
    let mut command = crate::app::new_window::command(&plan.target, &shell, None, &plan.cwd)
        .map_err(|_| "Restart directory is unavailable")?;
    command.env(
        "JUNA_UPDATE_TOKEN",
        root.file_name().ok_or("Startup token missing")?,
    );
    let restart = (|| {
        let mut child = command
            .spawn()
            .map_err(|_| "Updated executable could not restart")?;
        if std::fs::write(root.join("restart.pid"), child.id().to_string()).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cannot record restart result");
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        let expected = format!("{}\n{}", plan.new_version, plan.new_commit);
        loop {
            if std::fs::read_to_string(root.join("restart.ready"))
                .ok()
                .as_ref()
                == Some(&expected)
            {
                return Ok(());
            }
            if child
                .try_wait()
                .map_err(|_| "Cannot monitor updated startup")?
                .is_some()
            {
                return Err("Updated application exited before startup completed");
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Updated application did not acknowledge startup");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    })();
    if restart.is_err() {
        let quarantine = plan
            .target
            .parent()
            .unwrap()
            .join(format!(".juna-failed-{}", crate::settings::new_id()));
        std::fs::create_dir(&quarantine)
            .map_err(|_| "Cannot prepare rollback after restart failure")?;
        for (target, old) in [
            (plan.target.clone(), backup.join("juna.exe")),
            (
                plan.target.parent().unwrap().join("notices"),
                backup.join("notices"),
            ),
            (
                plan.target.parent().unwrap().join("RELEASE_NOTES.md"),
                backup.join("RELEASE_NOTES.md"),
            ),
        ] {
            if target.exists() {
                std::fs::rename(&target, quarantine.join(target.file_name().unwrap())).map_err(
                    |_| "Cannot quarantine failed update; recover the rollback folder manually",
                )?;
            }
            if old.exists() {
                std::fs::rename(&old, &target).map_err(|_| {
                    "Cannot restore previous version; recover the rollback folder manually"
                })?;
            }
        }
        receipt(directory, plan, ReceiptStatus::RolledBack)?;
        let mut command = crate::app::new_window::command(&plan.target, &shell, None, &plan.cwd)
            .map_err(|_| "Restart directory is unavailable")?;
        command.env_remove("JUNA_UPDATE_TOKEN");
        command
            .spawn()
            .map_err(|_| "Previous version restored but restart failed; launch Juna manually")?;
        return Ok(());
    }
    receipt(directory, plan, ReceiptStatus::Updated)?;
    Ok(())
}
#[cfg(all(test, windows))]
mod tests {
    #[test]
    #[ignore = "Real PE fixtures; recorded partial-install failure and rollback"]
    fn partial_replacement_restores_old_executable_and_notices() {
        use super::*;
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = project
            .join("target/acceptance/rollback")
            .join(crate::settings::new_id());
        let install = root.join("portable");
        let stage = root.join("stage");
        std::fs::create_dir_all(install.join("notices")).unwrap();
        std::fs::create_dir_all(stage.join("package/notices")).unwrap();
        let target = install.join("juna.exe");
        std::fs::copy(
            project.join("target/downloads/dev11/unpacked/juna.exe"),
            &target,
        )
        .unwrap();
        std::fs::write(install.join("notices/old.txt"), "old notices").unwrap();
        std::fs::write(install.join("RELEASE_NOTES.md"), "old notes").unwrap();
        let source = std::env::var_os("JUNA_UPDATE_REHEARSAL_EXE")
            .map(PathBuf::from)
            .expect("Set JUNA_UPDATE_REHEARSAL_EXE to this source's release build");
        let bytes = std::fs::read(&source).unwrap();
        crate::release_image::verify_identity(
            &bytes,
            env!("CARGO_PKG_VERSION"),
            env!("JUNA_BUILD_COMMIT"),
        )
        .unwrap();
        std::fs::copy(&source, stage.join("package/juna.exe")).unwrap();
        std::fs::write(stage.join("package/notices/new.txt"), "new notices").unwrap();
        std::fs::write(stage.join("package/RELEASE_NOTES.md"), "new notes").unwrap();
        let (old_version, old_commit, old_hash) = identity(&target).unwrap();
        let (new_version, new_commit, new_hash) =
            identity(&stage.join("package/juna.exe")).unwrap();
        let plan = Plan {
            target: target.clone(),
            old_version,
            old_commit,
            old_hash: old_hash.clone(),
            new_version,
            new_commit,
            new_hash,
            helper_hash: "a".repeat(64),
            parent: std::process::id(),
            shell: "cmd.exe".into(),
            cwd: install.clone(),
        };
        assert!(transaction(&plan, &stage, Some(1)).is_err());
        assert_eq!(hash(&std::fs::read(target).unwrap()).unwrap(), old_hash);
        assert_eq!(
            std::fs::read_to_string(install.join("notices/old.txt")).unwrap(),
            "old notices"
        );
        assert!(!install.join("notices/new.txt").exists());
        assert_eq!(
            std::fs::read_to_string(install.join("RELEASE_NOTES.md")).unwrap(),
            "old notes"
        );
    }
}
