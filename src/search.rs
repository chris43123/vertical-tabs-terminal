//! File search for the files panel: fuzzy-find any file below the panel's folder.
//!
//! In a git repository the file list comes from `git ls-files` (tracked plus untracked, minus
//! ignored), so build output and dependencies never show up. Elsewhere the folder is walked,
//! skipping hidden folders and the usual dependency/build folders, up to a cap. The list is
//! built on a worker thread and matched in memory on every keystroke.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Stop listing past this many files (a walk of `/` or `~` would never end).
const MAX_FILES: usize = 200_000;
/// Results shown for a query.
pub const MAX_RESULTS: usize = 200;
/// An index older than this is rebuilt when search is used again.
const STALE_AFTER: Duration = Duration::from_secs(20);
/// Folders the walk never enters (outside git repositories).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "__pycache__",
    "venv",
    "dist",
    "build",
];

pub struct Index {
    pub root: PathBuf,
    /// Paths relative to `root`, with `/` separators.
    pub files: Vec<String>,
    /// The cap was hit; some files are missing.
    pub truncated: bool,
    built: Instant,
}

impl Index {
    pub fn build(root: &Path) -> Self {
        let (files, truncated) = match git_files(root) {
            Some(files) => files,
            None => walk(root),
        };
        Self {
            root: root.to_path_buf(),
            files,
            truncated,
            built: Instant::now(),
        }
    }

    /// Indices of the best matches for `query`, best first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<usize> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(i32, usize)> = self
            .files
            .iter()
            .enumerate()
            .filter_map(|(i, path)| score(query, path).map(|s| (s, i)))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.truncate(limit);
        scored.into_iter().map(|(_, i)| i).collect()
    }
}

/// A path's match score: the better of matching the whole path and matching just the file
/// name (preferred, since that's usually what you type), with a nudge towards shorter paths.
fn score(query: &str, path: &str) -> Option<i32> {
    let whole = crate::fuzzy::score(query, path)?;
    let name = path.rsplit('/').next().unwrap_or(path);
    let by_name = crate::fuzzy::score(query, name).map(|s| s + 20);
    Some(whole.max(by_name.unwrap_or(i32::MIN)) - (path.len() / 16) as i32)
}

fn git_files(root: &Path) -> Option<(Vec<String>, bool)> {
    crate::git::repo_root(root)?;
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "--no-optional-locks",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut files: Vec<String> = text
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    // A file deleted from the work tree is still listed as cached.
    files.retain(|f| root.join(f).exists());
    files.sort();
    files.dedup();
    let truncated = files.len() > MAX_FILES;
    files.truncate(MAX_FILES);
    Some((files, truncated))
}

fn walk(root: &Path) -> (Vec<String>, bool) {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                if !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_ref()) {
                    stack.push(path);
                }
            } else if let Ok(rel) = path.strip_prefix(root) {
                files.push(rel.to_string_lossy().replace('\\', "/"));
                if files.len() >= MAX_FILES {
                    files.sort();
                    return (files, true);
                }
            }
        }
    }
    files.sort();
    (files, false)
}

/// The search box state: the query, the selected result, and the index being used.
#[derive(Default)]
pub struct Search {
    pub query: String,
    pub selected: usize,
    index: Arc<Mutex<Option<Arc<Index>>>>,
    building: Arc<Mutex<bool>>,
    /// Results for (query, index build time).
    cache: Option<(String, Instant, Arc<Vec<usize>>)>,
}

impl Search {
    /// Make sure an index of `root` exists and is fresh, building it in the background.
    pub fn ensure_index(&mut self, root: &Path, ctx: &eframe::egui::Context) {
        let current = self.index.lock().unwrap().clone();
        let fresh = current
            .as_ref()
            .is_some_and(|i| i.root == root && i.built.elapsed() < STALE_AFTER);
        if fresh {
            return;
        }
        {
            let mut building = self.building.lock().unwrap();
            if *building {
                return;
            }
            *building = true;
        }
        let (slot, building, ctx) = (self.index.clone(), self.building.clone(), ctx.clone());
        let root = root.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("vtt file index".into())
            .spawn(move || {
                let index = Index::build(&root);
                *slot.lock().unwrap() = Some(Arc::new(index));
                *building.lock().unwrap() = false;
                ctx.request_repaint();
            });
        if spawned.is_err() {
            *self.building.lock().unwrap() = false;
        }
    }

    /// The index for `root`, if one has been built (it may be a little stale).
    pub fn index(&self, root: &Path) -> Option<Arc<Index>> {
        self.index
            .lock()
            .unwrap()
            .clone()
            .filter(|i| i.root == root)
    }

    pub fn building(&self) -> bool {
        *self.building.lock().unwrap()
    }

    /// Matches for the current query in `index`, cached until either changes.
    pub fn results(&mut self, index: &Index) -> Arc<Vec<usize>> {
        if let Some((q, built, results)) = &self.cache
            && *q == self.query
            && *built == index.built
        {
            return results.clone();
        }
        let results = Arc::new(index.search(&self.query, MAX_RESULTS));
        self.cache = Some((self.query.clone(), index.built, results.clone()));
        if self.selected >= results.len() {
            self.selected = 0;
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(files: &[&str]) -> Index {
        Index {
            root: PathBuf::from("/r"),
            files: files.iter().map(|s| s.to_string()).collect(),
            truncated: false,
            built: Instant::now(),
        }
    }

    #[test]
    fn prefers_file_name_matches_and_short_paths() {
        let idx = index(&[
            "src/ui/preview.rs",
            "src/preview.rs",
            "docs/previous/review.md",
            "README.md",
        ]);
        let hits: Vec<&str> = idx
            .search("preview", 10)
            .into_iter()
            .map(|i| idx.files[i].as_str())
            .collect();
        assert_eq!(hits[0], "src/preview.rs");
        assert_eq!(hits[1], "src/ui/preview.rs");
        assert!(!hits.contains(&"README.md"));
        assert!(idx.search("", 10).is_empty());
        // Path segments narrow things down.
        let ui = idx.search("ui/prev", 10);
        assert_eq!(idx.files[ui[0]], "src/ui/preview.rs");
    }

    #[test]
    fn walks_folders_skipping_hidden_and_build_dirs() {
        let dir = std::env::temp_dir().join(format!("vtt-search-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["src/deep", "node_modules/x", ".hidden", "target"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        for f in [
            "a.txt",
            "src/deep/b.rs",
            "node_modules/x/c.js",
            ".hidden/d",
            "target/e",
        ] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let (files, truncated) = walk(&dir);
        assert_eq!(files, ["a.txt", "src/deep/b.rs"]);
        assert!(!truncated);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
