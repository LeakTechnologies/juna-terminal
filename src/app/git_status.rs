//! Local Git metadata worker. No engine references, shell execution, fetch or index writes.
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const LIMIT: usize = 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub branch: String,
    pub changed: usize,
    pub conflicts: usize,
    pub ahead: Option<u64>,
    pub behind: Option<u64>,
    pub has_upstream: bool,
}
pub fn parse(bytes: &[u8]) -> Result<Status, &'static str> {
    if bytes.len() > LIMIT || !bytes.ends_with(&[0]) {
        return Err("invalid Git report");
    }
    let mut status = Status {
        branch: String::new(),
        changed: 0,
        conflicts: 0,
        ahead: None,
        behind: None,
        has_upstream: false,
    };
    let mut upstream = false;
    let mut records = bytes.split(|b| *b == 0).filter(|r| !r.is_empty());
    for_record(&mut records, &mut status, &mut upstream)?;
    if status.branch.is_empty() {
        return Err("missing Git branch");
    }
    if !upstream {
        status.ahead = None;
        status.behind = None;
    }
    status.has_upstream = upstream;
    Ok(status)
}
fn for_record<'a>(
    records: &mut impl Iterator<Item = &'a [u8]>,
    status: &mut Status,
    upstream: &mut bool,
) -> Result<(), &'static str> {
    while let Some(record) = records.next() {
        if let Some(branch) = record.strip_prefix(b"# branch.head ") {
            let branch = std::str::from_utf8(branch).map_err(|_| "invalid branch")?;
            if branch.len() > 256 || branch.chars().any(char::is_control) {
                return Err("invalid branch");
            }
            status.branch = if branch == "(detached)" {
                "detached HEAD".into()
            } else {
                branch.into()
            };
        } else if record.starts_with(b"# branch.upstream ") {
            *upstream = true;
        } else if let Some(ab) = record.strip_prefix(b"# branch.ab ") {
            let ab = std::str::from_utf8(ab).map_err(|_| "invalid counts")?;
            let (a, b) = ab.split_once(' ').ok_or("invalid counts")?;
            status.ahead = Some(
                a.strip_prefix('+')
                    .ok_or("invalid counts")?
                    .parse()
                    .map_err(|_| "invalid counts")?,
            );
            status.behind = Some(
                b.strip_prefix('-')
                    .ok_or("invalid counts")?
                    .parse()
                    .map_err(|_| "invalid counts")?,
            );
        } else if record.starts_with(b"# ") {
        } else if record.starts_with(b"? ") {
            status.changed += 1;
        } else if matches!(record.first(), Some(b'1' | b'2' | b'u')) {
            let fields = if record[0] == b'u' {
                11
            } else if record[0] == b'2' {
                10
            } else {
                9
            };
            if record.splitn(fields, |b| *b == b' ').count() != fields {
                return Err("invalid changed path");
            }
            status.changed += 1;
            status.conflicts += usize::from(record[0] == b'u');
            if record[0] == b'2' && records.next().is_none() {
                return Err("missing rename path");
            }
        } else {
            return Err("unknown Git record");
        }
    }
    Ok(())
}
#[derive(Debug)]
pub enum Report {
    Repo(Status),
    NoRepository,
    Unavailable(&'static str),
}
pub struct Request {
    pub tab: u64,
    pub generation: u64,
    pub cwd: PathBuf,
}
pub struct Response {
    pub tab: u64,
    pub generation: u64,
    pub report: Report,
}
pub struct Worker {
    pub requests: SyncSender<Request>,
    pub results: Receiver<Response>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}
impl Worker {
    pub fn new() -> std::io::Result<Self> {
        let (requests, rx) = mpsc::sync_channel::<Request>(1);
        let (tx, results) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let join = thread::Builder::new()
            .name("juna-git-status".into())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    let Ok(request) = rx.recv_timeout(Duration::from_millis(50)) else {
                        continue;
                    };
                    let report = query(&request.cwd, &flag);
                    let _ = tx.try_send(Response {
                        tab: request.tab,
                        generation: request.generation,
                        report,
                    });
                }
            })?;
        Ok(Self {
            requests,
            results,
            stop,
            join: Some(join),
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
fn installed_git() -> Result<PathBuf, &'static str> {
    let path = std::env::var_os("PATH").ok_or("Git is not installed")?;
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    std::env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or("Git is not installed")
}
fn command(cwd: &Path) -> Result<Command, &'static str> {
    let mut cmd = Command::new(installed_git()?);
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            cmd.env_remove(name);
        }
    }
    cmd.env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0");
    for setting in [
        "core.fsmonitor=false",
        "core.untrackedCache=false",
        "core.splitIndex=false",
        "maintenance.auto=false",
        "gc.auto=0",
    ] {
        cmd.args(["-c", setting]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    Ok(cmd)
}
fn run(cwd: &Path, args: &[&str], stop: &AtomicBool) -> Result<(i32, Vec<u8>), &'static str> {
    run_input(cwd, args, None, stop)
}
fn run_input(
    cwd: &Path,
    args: &[&str],
    input: Option<Vec<u8>>,
    stop: &AtomicBool,
) -> Result<(i32, Vec<u8>), &'static str> {
    if stop.load(Ordering::Relaxed) {
        return Err("Git check cancelled");
    }
    let mut child = command(cwd)?
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .map_err(|_| "Git could not start")?;
    let output = child.stdout.take().ok_or("missing Git output")?;
    let writer = input.map(|bytes| {
        let mut stdin = child.stdin.take().unwrap();
        thread::spawn(move || stdin.write_all(&bytes))
    });
    let (tx, rx) = mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = output
            .take(LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = tx.send(result);
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut bytes = None;
    let result = loop {
        if stop.load(Ordering::Relaxed) || Instant::now() > deadline {
            break Err("Git check timed out");
        }
        if bytes.is_none() {
            match rx.try_recv() {
                Ok(Ok(data)) if data.len() <= LIMIT => bytes = Some(data),
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                    break Err("Git output unavailable or too large")
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match child.try_wait() {
            Ok(Some(status)) if bytes.is_some() => {
                break Ok((status.code().unwrap_or(-1), bytes.take().unwrap()))
            }
            Err(_) => break Err("Git process unavailable"),
            _ => thread::sleep(Duration::from_millis(10)),
        }
    };
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let _ = reader.join();
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    result
}
fn local(cwd: &Path) -> bool {
    cwd.is_absolute()
        && !cwd.to_string_lossy().starts_with("\\\\")
        && !cwd.to_string_lossy().starts_with("//")
}
pub fn query(cwd: &Path, stop: &AtomicBool) -> Report {
    if !local(cwd) {
        return Report::Unavailable("local folder required");
    }
    let result = (|| {
        let (ok, inside) = run(cwd, &["rev-parse", "--is-inside-work-tree"], stop)?;
        if ok != 0 || inside != b"true\n" && inside != b"true\r\n" {
            return Ok(Report::NoRepository);
        }
        let (ok, filters) = run(
            cwd,
            &[
                "config",
                "--null",
                "--get-regexp",
                r"^filter\..*\.(clean|process)$",
            ],
            stop,
        )?;
        if ok != 0 && ok != 1 {
            return Err("Git configuration unavailable");
        }
        let mut overrides = Vec::new();
        for entry in filters.split(|b| *b == 0).filter(|v| !v.is_empty()) {
            let key = entry
                .split(|b| *b == b'\n')
                .next()
                .ok_or("invalid filter configuration")?;
            let key = std::str::from_utf8(key).map_err(|_| "invalid filter configuration")?;
            let base = key
                .rsplit_once('.')
                .ok_or("invalid filter configuration")?
                .0;
            for suffix in ["clean=", "process=", "required=false"] {
                overrides.extend(["-c".to_owned(), format!("{base}.{suffix}")]);
            }
        }
        if !overrides.is_empty() {
            let (ok, root) = run(cwd, &["rev-parse", "--show-toplevel"], stop)?;
            if ok != 0 {
                return Err("Git root unavailable");
            }
            let text = std::str::from_utf8(&root)
                .map_err(|_| "Git root unavailable")?
                .trim_end_matches(['\r', '\n']);
            let root = PathBuf::from(text);
            if !local(&root) {
                return Err("local repository required");
            }
            let (ok, paths) = run(&root, &["ls-files", "-z"], stop)?;
            if ok != 0 {
                return Err("tracked paths unavailable");
            }
            let (ok, attrs) = run_input(
                &root,
                &["check-attr", "-z", "--stdin", "filter"],
                Some(paths),
                stop,
            )?;
            if ok != 0 {
                return Err("file attributes unavailable");
            }
            let entries: Vec<_> = attrs.split(|b| *b == 0).filter(|v| !v.is_empty()).collect();
            if entries.len() % 3 != 0 {
                return Err("invalid file attributes");
            }
            if entries
                .chunks_exact(3)
                .any(|row| row[2] != b"unspecified" && row[2] != b"unset")
            {
                return Ok(Report::Unavailable("external file filters in use"));
            }
        }
        overrides.extend(
            [
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=all",
                "--ignore-submodules=all",
            ]
            .into_iter()
            .map(String::from),
        );
        let args: Vec<_> = overrides.iter().map(String::as_str).collect();
        let (ok, bytes) = run(cwd, &args, stop)?;
        if ok != 0 {
            return Err("Git status unavailable");
        }
        Ok(Report::Repo(parse(&bytes)?))
    })();
    result.unwrap_or_else(Report::Unavailable)
}
pub struct View {
    pub initial: Option<PathBuf>,
    pub revision: u64,
    pub cwd: Option<PathBuf>,
    pub generation: u64,
    pub report: Option<Report>,
    pub last_check: Option<Instant>,
    pub pending: bool,
}
impl View {
    pub fn new(initial: Option<PathBuf>) -> Self {
        Self {
            initial,
            revision: 0,
            cwd: None,
            generation: 0,
            report: None,
            last_check: None,
            pending: false,
        }
    }
    pub fn sync(&mut self, revision: u64, cwd: Option<&str>) {
        if revision != 0 {
            self.initial = None;
        }
        let path = if revision == 0 {
            self.initial.clone()
        } else {
            cwd.map(PathBuf::from)
        };
        if self.cwd != path || (self.revision == 0) != (revision == 0) {
            self.cwd = path;
            self.revision = revision;
            self.generation = self.generation.wrapping_add(1);
            self.report = None;
            self.last_check = None;
            self.pending = false;
        }
        self.revision = revision;
    }
    pub fn invalidate_initial(&mut self) {
        if self.revision == 0 {
            self.initial = None;
            self.sync(0, None);
        }
    }
    pub fn label(&self) -> String {
        if self.cwd.is_none() {
            return "Git: folder reporting unavailable".into();
        }
        let suffix = if self.revision == 0 {
            " · initial folder only"
        } else {
            ""
        };
        let label = match &self.report {
            Some(Report::Repo(s)) => format!(
                "Git: {} · {} changed · {} conflicts · {}",
                s.branch,
                s.changed,
                s.conflicts,
                match (s.ahead, s.behind) {
                    (Some(a), Some(b)) => format!("↑{a} ↓{b} (last fetch)"),
                    _ if s.has_upstream => "tracking counts unavailable".into(),
                    _ => "no upstream".into(),
                }
            ),
            Some(Report::NoRepository) => "Git: no working tree".into(),
            Some(Report::Unavailable(reason)) => format!("Git: unavailable ({reason})"),
            None => "Git: checking…".into(),
        };
        format!("{label}{suffix}")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn git(path: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new("git")
            .current_dir(path)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
    fn status(path: &Path) -> Status {
        match query(path, &AtomicBool::new(false)) {
            Report::Repo(s) => s,
            other => panic!("{other:?}"),
        }
    }
    #[test]
    fn real_repositories_counts_tracking_worktrees_and_filters() {
        let root = std::env::temp_dir().join(crate::settings::new_id());
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-b", "main"]);
        git(&root, &["config", "user.name", "Fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        git(&root, &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.join("tracked"), "base\n").unwrap();
        git(&root, &["add", "tracked"]);
        git(&root, &["commit", "-m", "base"]);
        let base = String::from_utf8(git(&root, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned();
        let index = std::fs::read(root.join(".git/index")).unwrap();
        let index_time = std::fs::metadata(root.join(".git/index"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(status(&root).changed, 0);
        assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
        assert_eq!(
            std::fs::metadata(root.join(".git/index"))
                .unwrap()
                .modified()
                .unwrap(),
            index_time
        );
        std::fs::write(root.join("tracked"), "stage\n").unwrap();
        git(&root, &["add", "tracked"]);
        std::fs::write(root.join("tracked"), "unstaged\n").unwrap();
        std::fs::write(root.join("日本語 space"), "new\n").unwrap();
        assert_eq!(status(&root).changed, 2);
        git(&root, &["reset", "--hard", "HEAD"]);
        std::fs::remove_file(root.join("日本語 space")).unwrap();
        git(&root, &["checkout", "-b", "other"]);
        std::fs::write(root.join("tracked"), "other\n").unwrap();
        git(&root, &["commit", "-am", "other"]);
        let other = String::from_utf8(git(&root, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned();
        git(&root, &["checkout", "main"]);
        std::fs::write(root.join("tracked"), "main\n").unwrap();
        git(&root, &["commit", "-am", "main"]);
        git(&root, &["update-ref", "refs/remotes/origin/main", &other]);
        git(
            &root,
            &[
                "config",
                "remote.origin.url",
                "https://example.invalid/no-network",
            ],
        );
        git(
            &root,
            &[
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
        );
        git(&root, &["config", "branch.main.remote", "origin"]);
        git(&root, &["config", "branch.main.merge", "refs/heads/main"]);
        let s = status(&root);
        assert_eq!((s.ahead, s.behind), (Some(1), Some(1)));
        let merge = Command::new("git")
            .current_dir(&root)
            .args(["merge", "other"])
            .output()
            .unwrap();
        assert!(!merge.status.success());
        assert_eq!(status(&root).conflicts, 1);
        git(&root, &["reset", "--hard", "HEAD"]);
        git(&root, &["checkout", "--detach", &base]);
        assert_eq!(status(&root).branch, "detached HEAD");
        let worktree = root.with_extension("worktree");
        git(
            &root,
            &[
                "worktree",
                "add",
                "-b",
                "work",
                worktree.to_str().unwrap(),
                &base,
            ],
        );
        assert_eq!(status(&worktree).branch, "work");
        git(&root, &["config", "core.fsmonitor", "touch EXTERNAL_RAN"]);
        assert_eq!(status(&root).changed, 0);
        assert!(!root.join("EXTERNAL_RAN").exists());
        git(&root, &["config", "filter.bad.clean", "touch EXTERNAL_RAN"]);
        std::fs::write(root.join(".gitattributes"), "tracked filter=bad\n").unwrap();
        assert!(matches!(
            query(&root, &AtomicBool::new(false)),
            Report::Unavailable("external file filters in use")
        ));
        assert!(!root.join("EXTERNAL_RAN").exists());
        std::fs::remove_dir_all(worktree).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cwd_changes_and_initial_invalidation_discard_stale_reports() {
        let mut view = View::new(Some(PathBuf::from("initial")));
        view.sync(0, None);
        let generation = view.generation;
        view.report = Some(Report::NoRepository);
        view.invalidate_initial();
        assert!(view.report.is_none());
        assert!(view.generation > generation);
        view.sync(1, Some("C:/next"));
        let generation = view.generation;
        view.sync(2, Some("C:/next"));
        assert_eq!(view.generation, generation);
        view.sync(3, None);
        assert!(view.cwd.is_none());
        assert!(view.report.is_none());
    }
    #[test]
    fn bounded_nul_reports_handle_rename_paths_and_reject_truncation() {
        let s=parse(b"# branch.head main\x00# branch.upstream origin/main\x00# branch.ab +2 -3\x002 R. N... 100644 100644 100644 a b R100 new name\x00old\nname\x00? file\x00").unwrap();
        assert_eq!((s.changed, s.ahead, s.behind), (2, Some(2), Some(3)));
        assert!(parse(b"# branch.head main\x00? truncated").is_err());
        assert!(matches!(
            query(Path::new("relative"), &AtomicBool::new(false)),
            Report::Unavailable(_)
        ));
    }
    #[test]
    fn worker_shutdown_is_bounded_and_cancelled_query_does_not_run_status() {
        let worker = Worker::new().unwrap();
        let start = Instant::now();
        drop(worker);
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(matches!(
            query(&std::env::temp_dir(), &AtomicBool::new(true)),
            Report::Unavailable(_)
        ));
    }
}
