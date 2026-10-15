//! Owned update metadata and bounded I/O work, entirely outside terminal state/transport.
pub mod archive;
pub mod cli;
pub mod helper;
mod http;
pub mod version;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
};
pub const REPOSITORY: &str = "LeakTechnologies/juna-terminal";
#[derive(Clone, Debug)]
struct Asset {
    url: String,
    size: u64,
    digest: String,
}
#[derive(Clone, Debug)]
pub struct Candidate {
    pub version: String,
    archive: Asset,
    info: Asset,
    checksums: Asset,
}
#[derive(Clone, Debug)]
pub struct Staged {
    pub version: String,
    pub commit: String,
    pub root: PathBuf,
    pub exe_hash: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub version: String,
    pub status: ReceiptStatus,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    Updated,
    RolledBack,
    Failed,
}
pub enum Task {
    Check {
        channel: String,
        current: String,
        directory: PathBuf,
        discard: Option<PathBuf>,
    },
    Download {
        candidate: Candidate,
        directory: PathBuf,
        discard: Option<PathBuf>,
    },
    Discard {
        root: PathBuf,
        directory: PathBuf,
    },
    Prepare {
        stage: Staged,
        target: PathBuf,
        shell: crate::session::SessionSpec,
        cwd: PathBuf,
        directory: PathBuf,
    },
}
pub enum Outcome {
    Discarded,
    Checked(Option<Candidate>),
    Downloaded(Staged),
    Prepared(PathBuf),
}
pub struct Worker {
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<AtomicU64>,
    receiver: mpsc::Receiver<Result<Outcome, String>>,
    join: Option<thread::JoinHandle<()>>,
    directory: PathBuf,
}
impl Worker {
    pub fn start(task: Task) -> Result<Self, String> {
        let directory = match &task {
            Task::Check { directory, .. }
            | Task::Download { directory, .. }
            | Task::Prepare { directory, .. }
            | Task::Discard { directory, .. } => directory.clone(),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let flag = cancel.clone();
        let bytes = progress.clone();
        let (tx, receiver) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("juna-update-io".into())
            .spawn(move || {
                let result = match task {
                    Task::Check {
                        channel,
                        current,
                        directory,
                        discard,
                    } => {
                        if let Some(root) = discard {
                            let _ = cleanup_stage(&root, &directory);
                        }
                        check(&channel, &current, &flag, &bytes).map(Outcome::Checked)
                    }
                    Task::Download {
                        candidate,
                        directory,
                        discard,
                    } => {
                        if let Some(root) = discard {
                            let _ = cleanup_stage(&root, &directory);
                        }
                        download(&candidate, &directory, &flag, &bytes).map(Outcome::Downloaded)
                    }
                    Task::Discard { root, directory } => {
                        cleanup_stage(&root, &directory).map(|_| Outcome::Discarded)
                    }
                    Task::Prepare {
                        stage,
                        target,
                        shell,
                        cwd,
                        directory,
                    } => helper::prepare(&stage, &target, &shell, &cwd, &directory)
                        .map(Outcome::Prepared),
                };
                let _ = tx.send(result);
            })
            .map_err(|_| "Cannot start update worker")?;
        Ok(Self {
            cancel,
            progress,
            receiver,
            join: Some(join),
            directory,
        })
    }
    pub fn poll(&mut self) -> Option<Result<Outcome, String>> {
        match self.receiver.try_recv() {
            Ok(result) => {
                if let Some(join) = self.join.take() {
                    let _ = join.join();
                }
                Some(result)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Update worker stopped unexpectedly".into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        if let Ok(Ok(outcome)) = self.receiver.try_recv() {
            let root = match outcome {
                Outcome::Downloaded(stage) => Some(stage.root),
                Outcome::Prepared(path) => path.parent().map(Path::to_path_buf),
                _ => None,
            };
            if let Some(root) = root {
                let _ = cleanup_stage(&root, &self.directory);
            }
        }
    }
}
fn hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn asset(release: &serde_json::Value, name: &str, maximum: u64) -> Option<Asset> {
    let entries = release["assets"].as_array()?;
    let mut matches = entries
        .iter()
        .filter(|asset| asset["name"].as_str() == Some(name));
    let value = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let size = value["size"].as_u64()?;
    if size == 0 || size > maximum || value["state"] != "uploaded" {
        return None;
    }
    let tag = release["tag_name"].as_str()?;
    let url = value["browser_download_url"].as_str()?;
    let expected = format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{name}");
    if url != expected {
        return None;
    }
    let digest = value["digest"].as_str()?.strip_prefix("sha256:")?;
    if !hex(digest, 64) {
        return None;
    }
    Some(Asset {
        url: url.into(),
        size,
        digest: digest.into(),
    })
}
pub fn select(
    releases: &serde_json::Value,
    channel: &str,
    current: &str,
) -> Result<Option<Candidate>, String> {
    if !matches!(channel, "stable" | "preview") {
        return Err("Invalid update channel".into());
    }
    let current = version::Version::parse(current)
        .ok_or("Installed version is not supported by the updater")?;
    let rows = releases.as_array().ok_or("Invalid release response")?;
    let mut selected: Option<(version::Version, Candidate)> = None;
    for release in rows {
        if release["draft"].as_bool() != Some(false) || release["published_at"].as_str().is_none() {
            continue;
        }
        let Some(tag) = release["tag_name"]
            .as_str()
            .and_then(|s| s.strip_prefix('v'))
        else {
            continue;
        };
        let Some(number) = version::Version::parse(tag) else {
            continue;
        };
        if number <= current
            || (channel == "stable" && number.preview())
            || release["prerelease"].as_bool() != Some(number.preview())
        {
            continue;
        }
        let filename = format!("Juna_{tag}_windows_amd64.zip");
        let Some(zip) = asset(release, &filename, archive::MAX_ARCHIVE as u64) else {
            continue;
        };
        let Some(info) = asset(release, "build-info.json", 65536) else {
            continue;
        };
        let Some(checksums) = asset(release, "SHA256SUMS.txt", 65536) else {
            continue;
        };
        if selected.as_ref().is_none_or(|(old, _)| number > *old) {
            selected = Some((
                number,
                Candidate {
                    version: tag.into(),
                    archive: zip,
                    info,
                    checksums,
                },
            ));
        }
    }
    Ok(selected.map(|(_, candidate)| candidate))
}
fn check(
    channel: &str,
    current: &str,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<Option<Candidate>, String> {
    let mut chosen = None;
    let mut chosen_version = None;
    for page in 1..=10 {
        let mut bytes = Vec::new();
        http::get(
            &format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=100&page={page}"),
            &mut bytes,
            2 * 1024 * 1024,
            cancel,
            progress,
        )?;
        let releases: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid GitHub release response")?;
        let candidate = select(&releases, channel, current)?;
        if let Some(candidate) = candidate {
            let number = version::Version::parse(&candidate.version).unwrap();
            if chosen_version.as_ref().is_none_or(|old| number > *old) {
                chosen_version = Some(number);
                chosen = Some(candidate);
            }
        }
        if releases.as_array().ok_or("Invalid release list")?.len() < 100 {
            return Ok(chosen);
        }
    }
    Err("Release listing exceeds the safe limit; download from GitHub manually".into())
}
#[cfg(windows)]
pub fn hash(bytes: &[u8]) -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};
    let mut digest = [0u8; 32];
    // SAFETY: the SHA-256 pseudo-handle is valid; input/output slices are owned/live and the output is exactly 32 bytes.
    unsafe {
        BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, bytes, &mut digest)
            .ok()
            .map_err(|_| "Update hash verification failed")?;
    }
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}
#[cfg(not(windows))]
pub fn hash(_: &[u8]) -> Result<String, String> {
    Err("Update hashing requires Windows".into())
}
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "Cannot read update file")?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read update file")?;
    if bytes.len() as u64 > limit {
        return Err("Update file exceeds size limit".into());
    }
    Ok(bytes)
}
fn fetch_asset(
    asset: &Asset,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    http::get(&asset.url, &mut bytes, asset.size, cancel, progress)?;
    if bytes.len() as u64 != asset.size || hash(&bytes)? != asset.digest {
        return Err("Release asset integrity mismatch".into());
    }
    Ok(bytes)
}
fn download(
    candidate: &Candidate,
    directory: &Path,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<Staged, String> {
    let info = fetch_asset(&candidate.info, cancel, progress)?;
    let checksums = fetch_asset(&candidate.checksums, cancel, progress)?;
    let info: serde_json::Value =
        serde_json::from_slice(&info).map_err(|_| "Invalid release build manifest")?;
    let filename = format!("Juna_{}_windows_amd64.zip", candidate.version);
    let commit = info["commit"]
        .as_str()
        .filter(|s| hex(s, 40))
        .ok_or("Release source identity missing")?;
    let digest = info["sha256"]
        .as_str()
        .filter(|s| hex(s, 64))
        .ok_or("Release checksum missing")?;
    let matching = std::str::from_utf8(&checksums)
        .map_err(|_| "Invalid checksum list")?
        .lines()
        .filter_map(|line| {
            let mut columns = line.split_whitespace();
            let hash = columns.next()?;
            let name = columns.next()?;
            if columns.next().is_none() && name == filename {
                Some(hash)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if info["version"] != candidate.version
        || info["target"] != "x86_64-pc-windows-gnu"
        || info["asset"] != filename
        || matching != [digest]
        || digest != candidate.archive.digest
    {
        return Err("Release manifest/checksum disagreement".into());
    }
    let updates = directory.join("updates");
    std::fs::create_dir_all(&updates).map_err(|_| "Cannot create updates folder")?;
    let root = updates.join(crate::settings::new_id());
    std::fs::create_dir(&root).map_err(|_| "Cannot create update staging folder")?;
    let result = (|| {
        let path = root.join("download.zip");
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|_| "Cannot stage download")?;
        progress.store(0, Ordering::Relaxed);
        http::get(
            &candidate.archive.url,
            &mut file,
            candidate.archive.size,
            cancel,
            progress,
        )?;
        file.sync_all().map_err(|_| "Cannot flush download")?;
        drop(file);
        let bytes = read_bounded(&path, archive::MAX_ARCHIVE as u64)?;
        if bytes.len() as u64 != candidate.archive.size || hash(&bytes)? != digest {
            return Err("Downloaded archive checksum mismatch".into());
        }
        archive::extract(&bytes, &root.join("package"), cancel)?;
        drop(bytes);
        let exe = read_bounded(&root.join("package/juna.exe"), 192 * 1024 * 1024)?;
        crate::release_image::verify_identity(&exe, &candidate.version, commit)?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Update cancelled".into());
        }
        let _ = std::fs::remove_file(path);
        Ok(Staged {
            version: candidate.version.clone(),
            commit: commit.into(),
            root: root.clone(),
            exe_hash: hash(&exe)?,
        })
    })();
    if result.is_err() {
        let _ = cleanup_stage(&root, directory);
    }
    result
}
pub fn cleanup_stage(root: &Path, directory: &Path) -> Result<(), String> {
    let managed = std::fs::canonicalize(directory.join("updates"))
        .map_err(|_| "Managed updates folder unavailable")?;
    let resolved = std::fs::canonicalize(root).map_err(|_| "Staging folder unavailable")?;
    let name = root
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid staging name")?;
    if resolved.parent() != Some(managed.as_path())
        || !name.starts_with("custom-")
        || !crate::theme::valid_id(name)
    {
        return Err("Staging cleanup refused outside managed updates folder".into());
    }
    let metadata = std::fs::symlink_metadata(root).map_err(|_| "Staging folder unavailable")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Staging cleanup refused a reparse point".into());
        }
    }
    if metadata.file_type().is_symlink() {
        return Err("Staging cleanup refused a symbolic link".into());
    }
    std::fs::remove_dir_all(resolved).map_err(|_| "Cannot remove unused update staging".to_owned())
}
#[cfg(test)]
mod tests {
    fn release(tag: &str, preview: bool) -> serde_json::Value {
        let names = [
            format!("Juna_{}_windows_amd64.zip", tag.trim_start_matches('v')),
            "build-info.json".into(),
            "SHA256SUMS.txt".into(),
        ];
        serde_json::json!({"tag_name":tag,"draft":false,"prerelease":preview,"published_at":"2026-10-14","assets":names.iter().map(|name|serde_json::json!({"name":name,"size":10,"state":"uploaded","digest":format!("sha256:{}","a".repeat(64)),"browser_download_url":format!("https://github.com/{}/releases/download/{tag}/{name}",super::REPOSITORY)})).collect::<Vec<_>>()})
    }
    #[test]
    fn release_selection_ignores_api_order_and_unsupported_candidates() {
        let rows = serde_json::json!([
            release("v0.1.1-dev9", true),
            release("v0.1.1-dev12", true),
            release("v0.1.1-dev10", true)
        ]);
        assert_eq!(
            super::select(&rows, "preview", "0.1.1-dev8")
                .unwrap()
                .unwrap()
                .version,
            "0.1.1-dev12"
        );
        assert!(super::select(&rows, "stable", "0.1.1-dev8")
            .unwrap()
            .is_none());
        assert!(super::select(&rows, "preview", "0.1.1-dev12")
            .unwrap()
            .is_none());
        let mut stable = release("v0.1.1", false);
        stable["draft"] = true.into();
        assert!(
            super::select(&serde_json::json!([stable]), "stable", "0.1.1-dev12")
                .unwrap()
                .is_none()
        );
        let mut bad = release("v0.2.0", false);
        bad["assets"][0]["browser_download_url"] = "https://evil.test/x".into();
        assert!(
            super::select(&serde_json::json!([bad]), "stable", "0.1.1-dev12")
                .unwrap()
                .is_none()
        );
    }
}
