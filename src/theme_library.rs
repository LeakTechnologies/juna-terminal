//! Bounded app-owned theme file I/O. No terminal state enters this worker.
use crate::{
    settings::Store,
    theme::{self, Theme},
};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
};
pub struct Library {
    pub themes: Vec<Theme>,
    pub sources: BTreeMap<String, PathBuf>,
    pub errors: Vec<String>,
}
pub fn folder_name(family: &str) -> String {
    let value: String = family
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .collect();
    let value = value.trim();
    let upper = value.to_ascii_uppercase();
    let device = ["CON", "PRN", "AUX", "NUL"].contains(&upper.as_str())
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit());
    if value.is_empty() {
        "Personal".into()
    } else if device {
        format!("Family-{value}")
    } else {
        value.into()
    }
}
fn reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn files(
    root: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
    errors: &mut Vec<String>,
    visited: &mut usize,
) {
    if out.len() >= 128 {
        return;
    }
    match std::fs::symlink_metadata(root) {
        Ok(metadata) if reparse(&metadata) => {
            errors.push("Theme discovery skips linked folders".into());
            return;
        }
        _ => {}
    }
    let entries = match std::fs::read_dir(root) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            errors.push(format!("Theme folder: {e}"));
            return;
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .take(257)
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    if paths.len() > 256 {
        errors.push("Theme discovery limit is 256 entries per folder".into());
        paths.truncate(256);
    }
    paths.sort();
    for path in paths {
        *visited += 1;
        if *visited > 1024 {
            errors.push("Theme discovery limit is 1024 entries".into());
            break;
        }
        if out.len() >= 128 {
            errors.push("Theme discovery limit is 128 files".into());
            break;
        }
        if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if reparse(&metadata) {
            continue;
        }
        if metadata.is_dir() && depth < 2 {
            files(&path, depth + 1, out, errors, visited);
        } else if metadata.is_file()
            && path
                .extension()
                .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("json"))
        {
            out.push(path);
        }
    }
}
fn import_id(path: &Path) -> String {
    // Stable file identity, not a cryptographic or security hash.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.to_string_lossy().replace('\\', "/").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("custom-import-{hash:016x}")
}
pub fn scan(directory: &Path) -> Library {
    let root = directory.join("themes");
    let mut result = Library {
        themes: theme::builtins(),
        sources: BTreeMap::new(),
        errors: Vec::new(),
    };
    let mut paths = Vec::new();
    files(&root, 0, &mut paths, &mut result.errors, &mut 0);
    for path in paths {
        let read = (|| -> Result<Theme, String> {
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let mut bytes = Vec::new();
            file.take(65537)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 65536 {
                return Err("Theme exceeds 64 KiB".into());
            }
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let mut t = if value.get("schema_version").is_some() {
                serde_json::from_value::<Theme>(value).map_err(|e| e.to_string())?
            } else {
                windows_scheme(&value)?
            };
            if !t.id.starts_with("custom-") {
                t.id = import_id(path.strip_prefix(&root).unwrap_or(&path));
            }
            if let Ok(relative) = path.strip_prefix(&root) {
                if relative.components().count() > 1 {
                    t.family = relative
                        .components()
                        .next()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned());
                }
            }
            t.validate()?;
            Ok(t)
        })();
        match read {
            Ok(t) if result.themes.iter().all(|old| old.id != t.id) => {
                result.sources.insert(t.id.clone(), path);
                result.themes.push(t);
            }
            Ok(_) => result
                .errors
                .push(format!("Duplicate theme ID: {}", path.display())),
            Err(error) => result.errors.push(format!("{}: {error}", path.display())),
        }
    }
    result
}
pub fn windows_scheme(value: &serde_json::Value) -> Result<Theme, String> {
    let keys = [
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "purple",
        "cyan",
        "white",
        "brightBlack",
        "brightRed",
        "brightGreen",
        "brightYellow",
        "brightBlue",
        "brightPurple",
        "brightCyan",
        "brightWhite",
    ];
    let color = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| format!("Missing Windows Terminal scheme color {key}"))
    };
    let ansi: Vec<String> = keys
        .iter()
        .map(|key| color(key))
        .collect::<Result<_, _>>()?;
    let foreground = color("foreground")?;
    let t = Theme {
        schema_version: 1,
        id: "import".into(),
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing scheme name")?
            .into(),
        family: None,
        source: None,
        colors: theme::Colors {
            foreground: foreground.clone(),
            background: color("background")?,
            cursor: value
                .get("cursorColor")
                .and_then(|v| v.as_str())
                .unwrap_or(&foreground)
                .into(),
            selection_background: value
                .get("selectionBackground")
                .and_then(|v| v.as_str())
                .unwrap_or(&ansi[12])
                .into(),
            selection_foreground: foreground,
            ansi: ansi.try_into().unwrap(),
        },
    };
    t.validate()?;
    Ok(t)
}
pub struct Worker {
    requests: Option<mpsc::SyncSender<u64>>,
    results: mpsc::Receiver<(u64, Library)>,
}
impl Worker {
    pub fn new(directory: PathBuf) -> std::io::Result<Self> {
        let (requests, rx) = mpsc::sync_channel(1);
        let (tx, results) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("juna-themes".into())
            .spawn(move || {
                while let Ok(generation) = rx.recv() {
                    let result = scan(&directory);
                    let _ = tx.try_send((generation, result));
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            results,
        })
    }
    pub fn request(&self, generation: u64) {
        if let Some(tx) = &self.requests {
            let _ = tx.try_send(generation);
        }
    }
    pub fn result(&self) -> Option<(u64, Library)> {
        self.results.try_recv().ok()
    }
}
impl Store {
    fn verify_theme_path(&self, path: &Path) -> Result<(), String> {
        let root = self
            .directory
            .join("themes")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let parent = path
            .parent()
            .ok_or("Missing theme directory")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !parent.starts_with(root) {
            return Err("Theme path must remain inside the theme folder".into());
        }
        Ok(())
    }
    pub fn save_theme_at(&self, t: &Theme, path: &Path) -> Result<PathBuf, String> {
        self.verify_theme_path(path)?;
        t.validate()?;
        if !t.id.starts_with("custom-") {
            return Err("Duplicate a preset before editing".into());
        }
        if std::fs::metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .readonly()
        {
            return Err("Theme file is read-only; duplicate it first".into());
        }
        let folder = self
            .directory
            .join("themes")
            .join(folder_name(&t.family_name()));
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let destination = folder.join(path.file_name().ok_or("Missing theme filename")?);
        self.verify_theme_path(&destination)?;
        if path.parent().and_then(|p| p.canonicalize().ok()) == folder.canonicalize().ok() {
            crate::settings::write_json(path, t)?;
            return Ok(path.into());
        }
        if destination.exists() {
            return Err("A theme already uses that destination filename".into());
        }
        crate::settings::write_json(&destination, t)?;
        if let Err(error) = self.trash_theme(path) {
            let _ = std::fs::remove_file(&destination);
            return Err(error);
        }
        Ok(destination)
    }
    pub fn trash_theme(&self, path: &Path) -> Result<(PathBuf, PathBuf), String> {
        self.verify_theme_path(path)?;
        let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || reparse(&metadata) {
            return Err("Only regular theme files can be removed".into());
        }
        let trash = self.directory.join("themes/.trash");
        std::fs::create_dir_all(&trash).map_err(|e| e.to_string())?;
        let destination = trash.join(format!("{}.json", crate::settings::new_id()));
        std::fs::rename(path, &destination).map_err(|e| e.to_string())?;
        Ok((destination, path.into()))
    }
    pub fn restore_theme(&self, pair: &(PathBuf, PathBuf)) -> Result<(), String> {
        self.verify_theme_path(&pair.0)?;
        self.verify_theme_path(&pair.1)?;
        if pair.1.exists() {
            return Err("The original filename is already in use".into());
        }
        std::fs::rename(&pair.0, &pair.1).map_err(|e| e.to_string())
    }
    pub fn initialize_theme_folder(&self) -> Result<(), String> {
        let root = self.directory.join("themes");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let guide = root.join("README.txt");
        if !guide.exists() {
            std::fs::write(&guide,"Place Juna theme JSON or a single Windows Terminal scheme JSON here. Family subfolders are discovered automatically. Example: themes/Catppuccin/My Mocha.json. Files are data only; invalid files remain untouched. Rename Juna.theme.example to a .json filename to customize it.\n").map_err(|e|e.to_string())?;
        }
        let sample = root.join("Juna.theme.example");
        if !sample.exists() {
            let mut t = theme::default_theme();
            t.id = "custom-example".into();
            t.name = "My Mocha".into();
            std::fs::write(
                sample,
                serde_json::to_vec_pretty(&t).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn family_import_refresh_and_reversible_removal_preserve_source() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        let store = Store::new(root.clone());
        store.initialize_theme_folder().unwrap();
        let dir = root.join("themes/Downloaded");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Example.JSON");
        let scheme=br##"{"name":"Downloaded example","foreground":"#ddeeFF","background":"#102030","cursorColor":"#abcdef","selectionBackground":"#303040","black":"#000001","red":"#000002","green":"#000003","yellow":"#000004","blue":"#000005","purple":"#000006","cyan":"#000007","white":"#000008","brightBlack":"#000009","brightRed":"#000010","brightGreen":"#000011","brightYellow":"#000012","brightBlue":"#000013","brightPurple":"#000014","brightCyan":"#000015","brightWhite":"#000016"}"##;
        std::fs::write(&file, scheme).unwrap();
        let library = scan(&root);
        assert!(library.errors.is_empty(), "{:?}", library.errors);
        let t = library
            .themes
            .iter()
            .find(|t| t.name == "Downloaded example")
            .unwrap();
        assert_eq!(t.family_name(), "Downloaded");
        assert_eq!(t.colors.ansi[5], "#000006");
        assert_eq!(t.colors.ansi[13], "#000014");
        let id = t.id.clone();
        assert!(id.starts_with("custom-import-"));
        let worker = Worker::new(root.clone()).unwrap();
        worker.request(7);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if let Some((generation, library)) = worker.result() {
                assert_eq!(generation, 7);
                assert!(library.themes.iter().any(|t| t.id == id));
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let pair = store.trash_theme(&file).unwrap();
        assert!(!scan(&root).themes.iter().any(|t| t.id == id));
        assert_eq!(std::fs::read(&pair.0).unwrap(), scheme);
        store.restore_theme(&pair).unwrap();
        assert!(scan(&root).themes.iter().any(|t| t.id == id));
        let bad = dir.join("bad.json");
        std::fs::write(&bad, vec![b'x'; 65537]).unwrap();
        let library = scan(&root);
        assert!(library.errors.iter().any(|e| e.contains("64 KiB")));
        assert_eq!(std::fs::metadata(&bad).unwrap().len(), 65537);
        drop(worker);
        std::fs::remove_dir_all(root).unwrap();
    }
}
