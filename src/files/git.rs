//! Git state for the files panel: branch, ahead/behind, and which files changed.
//!
//! This runs the `git` you already have (`git status --porcelain=v2`) on a worker thread
//! instead of linking a git library, so `.gitignore`, worktrees and config behave exactly as
//! in your shell. `--no-optional-locks` keeps it from taking `index.lock`, so it never gets
//! in the way of git commands you or an agent run at the same time.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How a changed path differs from HEAD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Modified, added, renamed or untracked: something to look at.
    Changed,
    /// A merge conflict.
    Conflict,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub root: PathBuf,
    /// Branch name; `None` when HEAD is detached.
    pub branch: Option<String>,
    /// Short commit id, shown when detached.
    pub commit: Option<String>,
    pub upstream: bool,
    pub ahead: u32,
    pub behind: u32,
    /// Changed files (absolute paths). Untracked folders appear as one entry.
    pub changes: HashMap<PathBuf, Change>,
    /// Folders (absolute) that contain a change somewhere below them.
    pub dirty_dirs: HashSet<PathBuf>,
}

impl Status {
    /// How `path` (a file or folder in the tree) differs, if it does.
    pub fn change(&self, path: &Path) -> Option<Change> {
        if let Some(c) = self.changes.get(path) {
            return Some(*c);
        }
        if self.dirty_dirs.contains(path) {
            return Some(Change::Changed);
        }
        // Inside an untracked folder (git lists the folder, not its files).
        path.ancestors()
            .skip(1)
            .take_while(|a| a.starts_with(&self.root) && *a != self.root)
            .find_map(|a| self.changes.get(a).copied())
    }

    /// Number of changed entries (files, or untracked folders).
    pub fn changed_count(&self) -> usize {
        self.changes.len()
    }
}

/// The repository `dir` is in: the nearest ancestor with a `.git` (folder, or file for
/// worktrees and submodules).
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|a| a.join(".git").exists())
        .map(Path::to_path_buf)
}

/// Parse `git status --porcelain=v2 --branch -z` output for the repo at `root`.
pub fn parse(root: &Path, out: &[u8]) -> Status {
    let mut status = Status {
        root: root.to_path_buf(),
        ..Default::default()
    };
    let text = String::from_utf8_lossy(out);
    let mut records = text.split('\0').filter(|r| !r.is_empty());
    while let Some(record) = records.next() {
        let mut add = |rel: &str, change: Change| {
            let rel = rel.trim_end_matches('/');
            status.changes.insert(root.join(rel), change);
        };
        if let Some(header) = record.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            match key {
                "branch.oid" => status.commit = Some(value.chars().take(7).collect()),
                "branch.head" if value != "(detached)" => status.branch = Some(value.into()),
                "branch.upstream" => status.upstream = true,
                "branch.ab" => {
                    for part in value.split(' ') {
                        if let Some(n) = part.strip_prefix('+') {
                            status.ahead = n.parse().unwrap_or(0);
                        } else if let Some(n) = part.strip_prefix('-') {
                            status.behind = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
        } else if let Some(rest) = record.strip_prefix("1 ") {
            // 1 XY sub mH mI mW hH hI path
            if let Some(path) = rest.splitn(8, ' ').nth(7) {
                add(path, Change::Changed);
            }
        } else if let Some(rest) = record.strip_prefix("2 ") {
            // 2 XY sub mH mI mW hH hI Xscore path, then the original path as its own record.
            if let Some(path) = rest.splitn(9, ' ').nth(8) {
                add(path, Change::Changed);
            }
            records.next();
        } else if let Some(rest) = record.strip_prefix("u ") {
            // u XY sub m1 m2 m3 mW h1 h2 h3 path
            if let Some(path) = rest.splitn(10, ' ').nth(9) {
                add(path, Change::Conflict);
            }
        } else if let Some(path) = record.strip_prefix("? ") {
            add(path, Change::Changed);
        }
    }
    // Deleted files have no row in the tree; their folders still count as changed.
    let dirty: HashSet<PathBuf> = status
        .changes
        .keys()
        .flat_map(|p| {
            p.ancestors()
                .skip(1)
                .take_while(|a| a.starts_with(root) && *a != root)
                .map(Path::to_path_buf)
                .collect::<Vec<_>>()
        })
        .collect();
    status.dirty_dirs = dirty;
    status
}

fn run_status(root: &Path) -> Option<Status> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=normal",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| parse(root, &out.stdout))
}

/// Keeps the status of one repository fresh, refreshing on a worker thread.
#[derive(Default)]
pub struct Watcher {
    root: Option<PathBuf>,
    latest: Arc<Mutex<Option<Arc<Status>>>>,
    running: Arc<Mutex<bool>>,
    last_run: Option<Instant>,
}

/// Re-run `git status` at most this often while nothing asks for it sooner.
const INTERVAL: Duration = Duration::from_secs(3);

impl Watcher {
    /// Follow the repository containing `dir` (or none). Refreshes right away on a change.
    pub fn set_dir(&mut self, dir: &Path, ctx: &eframe::egui::Context) {
        let root = repo_root(dir);
        if root != self.root {
            self.root = root;
            *self.latest.lock().unwrap() = None;
            self.refresh(ctx);
        }
    }

    /// Re-read the status if it's due (or `now`), without blocking.
    pub fn tick(&mut self, ctx: &eframe::egui::Context, now: bool) {
        let due = self.last_run.is_none_or(|t| t.elapsed() >= INTERVAL);
        // A refresh skipped while switching repositories leaves nothing to show yet.
        let missing = self.root.is_some() && self.status().is_none();
        if now || due || missing {
            self.refresh(ctx);
        }
    }

    fn refresh(&mut self, ctx: &eframe::egui::Context) {
        let Some(root) = self.root.clone() else {
            return;
        };
        {
            let mut running = self.running.lock().unwrap();
            if *running {
                return;
            }
            *running = true;
        }
        self.last_run = Some(Instant::now());
        let (latest, running, ctx) = (self.latest.clone(), self.running.clone(), ctx.clone());
        let spawned = std::thread::Builder::new()
            .name("vtt git status".into())
            .spawn(move || {
                let status = run_status(&root).map(Arc::new);
                {
                    let mut slot = latest.lock().unwrap();
                    // Only keep it if we're still looking at this repo.
                    let current = slot.as_ref().map(|s| s.root.clone());
                    if current.is_none() || current.as_ref() == Some(&root) {
                        if *slot != status {
                            ctx.request_repaint();
                        }
                        *slot = status;
                    }
                }
                *running.lock().unwrap() = false;
            });
        if spawned.is_err() {
            *self.running.lock().unwrap() = false;
        }
    }

    /// The latest status of the followed repository, if it's a repository.
    pub fn status(&self) -> Option<Arc<Status>> {
        let status = self.latest.lock().unwrap().clone()?;
        (Some(&status.root) == self.root.as_ref()).then_some(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn porcelain(records: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for r in records {
            out.extend_from_slice(r.as_bytes());
            out.push(0);
        }
        out
    }

    #[test]
    fn parses_branch_and_changes() {
        let root = Path::new("/repo");
        let out = porcelain(&[
            "# branch.oid 0123456789abcdef",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
            "1 .M N... 100644 100644 100644 aaaa bbbb src/main.rs",
            "1 A. N... 000000 100644 100644 0000 cccc docs/new file.md",
            "2 R. N... 100644 100644 100644 dddd dddd R100 src/renamed.rs",
            "src/old.rs",
            "u UU N... 100644 100644 100644 100644 e f g conflict.txt",
            "? scratch/",
        ]);
        let s = parse(root, &out);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.commit.as_deref(), Some("0123456"));
        assert!(s.upstream);
        assert_eq!((s.ahead, s.behind), (2, 1));
        assert_eq!(s.changed_count(), 5);
        assert_eq!(
            s.change(Path::new("/repo/src/main.rs")),
            Some(Change::Changed)
        );
        assert_eq!(
            s.change(Path::new("/repo/docs/new file.md")),
            Some(Change::Changed)
        );
        assert_eq!(
            s.change(Path::new("/repo/src/renamed.rs")),
            Some(Change::Changed)
        );
        assert_eq!(s.change(Path::new("/repo/src/old.rs")), None);
        assert_eq!(
            s.change(Path::new("/repo/conflict.txt")),
            Some(Change::Conflict)
        );
        // Folders holding changes, and files inside an untracked folder.
        assert_eq!(s.change(Path::new("/repo/src")), Some(Change::Changed));
        assert_eq!(s.change(Path::new("/repo/scratch")), Some(Change::Changed));
        assert_eq!(
            s.change(Path::new("/repo/scratch/a/b.txt")),
            Some(Change::Changed)
        );
        assert_eq!(s.change(Path::new("/repo/Cargo.toml")), None);
        assert_eq!(s.change(Path::new("/repo")), None);
    }

    #[test]
    fn detached_head_has_no_branch() {
        let out = porcelain(&["# branch.oid abcdef1234", "# branch.head (detached)"]);
        let s = parse(Path::new("/r"), &out);
        assert_eq!(s.branch, None);
        assert_eq!(s.commit.as_deref(), Some("abcdef1"));
        assert!(!s.upstream);
    }

    #[test]
    fn reads_a_real_repository() {
        let dir = std::env::temp_dir().join(format!("vtt-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if !git(&["init", "-q", "-b", "trunk"]) {
            return; // no git available
        }
        std::fs::write(dir.join("sub/new.txt"), "x").unwrap();
        assert_eq!(repo_root(&dir.join("sub")).as_deref(), Some(dir.as_path()));
        let s = run_status(&dir).expect("status");
        assert_eq!(s.branch.as_deref(), Some("trunk"));
        assert_eq!(s.change(&dir.join("sub/new.txt")), Some(Change::Changed));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
