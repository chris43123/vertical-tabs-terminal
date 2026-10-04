//! File tree model for the files panel: directories are listed lazily when expanded, and
//! re-listed when their modification time changes (a directory's mtime changes whenever an
//! entry is added, removed or renamed in it).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Directories with more entries than this show only the first ones.
const MAX_ENTRIES: usize = 5000;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub hidden: bool,
}

#[derive(Debug, Default)]
struct Listing {
    entries: Vec<Entry>,
    mtime: Option<SystemTime>,
    /// Entries left out beyond `MAX_ENTRIES`.
    truncated: usize,
    error: Option<String>,
}

/// One visible line of the tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Entry {
        entry: Entry,
        depth: usize,
        expanded: bool,
    },
    /// "… N more" under a truncated directory.
    More { depth: usize, count: usize },
    /// A directory that couldn't be read.
    Error { depth: usize, message: String },
}

#[derive(Debug, Default)]
pub struct FileTree {
    root: Option<PathBuf>,
    expanded: HashSet<PathBuf>,
    listings: HashMap<PathBuf, Listing>,
    pub show_hidden: bool,
}

impl FileTree {
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Show `dir` as the tree's root. Expanded folders are remembered per path, so coming
    /// back to a directory restores how it was opened.
    pub fn set_root(&mut self, dir: PathBuf) {
        if self.root.as_ref() == Some(&dir) {
            return;
        }
        self.root = Some(dir);
        // Keep the cache to what's reachable from the new root.
        let root = self.root.clone().unwrap();
        self.listings
            .retain(|p, _| p.starts_with(&root) && (*p == root || self.expanded.contains(p)));
    }

    pub fn toggle(&mut self, dir: &Path) {
        if !self.expanded.remove(dir) {
            self.expanded.insert(dir.to_path_buf());
        }
    }

    /// Collapse every folder below the root.
    pub fn collapse_all(&mut self) {
        if let Some(root) = &self.root {
            self.expanded.retain(|p| !p.starts_with(root));
        }
    }

    /// Re-list cached directories whose mtime changed. Returns true if anything did.
    pub fn refresh(&mut self) -> bool {
        let mut changed = false;
        for (path, listing) in &mut self.listings {
            let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            if mtime != listing.mtime || mtime.is_none() {
                let fresh = list_dir(path);
                if fresh.entries != listing.entries || fresh.error != listing.error {
                    changed = true;
                }
                *listing = fresh;
            }
        }
        changed
    }

    /// The visible rows: the root's entries, plus the contents of every expanded folder.
    pub fn rows(&mut self) -> Vec<Row> {
        let mut out = Vec::new();
        if let Some(root) = self.root.clone() {
            self.push_rows(&root, 0, &mut out);
        }
        out
    }

    fn push_rows(&mut self, dir: &Path, depth: usize, out: &mut Vec<Row>) {
        let listing = self
            .listings
            .entry(dir.to_path_buf())
            .or_insert_with(|| list_dir(dir));
        if let Some(message) = &listing.error {
            out.push(Row::Error {
                depth,
                message: message.clone(),
            });
            return;
        }
        let truncated = listing.truncated;
        let entries: Vec<Entry> = listing
            .entries
            .iter()
            .filter(|e| self.show_hidden || !e.hidden)
            .cloned()
            .collect();
        for entry in entries {
            let expanded = entry.is_dir && self.expanded.contains(&entry.path);
            let path = entry.path.clone();
            out.push(Row::Entry {
                entry,
                depth,
                expanded,
            });
            if expanded {
                self.push_rows(&path, depth + 1, out);
            }
        }
        if truncated > 0 {
            out.push(Row::More {
                depth,
                count: truncated,
            });
        }
    }
}

fn list_dir(dir: &Path) -> Listing {
    let mtime = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(err) => {
            return Listing {
                mtime,
                error: Some(err.to_string()),
                ..Default::default()
            };
        }
    };
    let mut entries: Vec<Entry> = read
        .filter_map(Result::ok)
        .map(|e| {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            // Follow symlinks so a link to a folder can be expanded.
            let is_dir = std::fs::metadata(&path).is_ok_and(|m| m.is_dir());
            Entry {
                hidden: is_hidden(&name, &e),
                name,
                path,
                is_dir,
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| natural_cmp(&a.name, &b.name))
    });
    let truncated = entries.len().saturating_sub(MAX_ENTRIES);
    entries.truncate(MAX_ENTRIES);
    Listing {
        entries,
        mtime,
        truncated,
        error: None,
    }
}

#[cfg(windows)]
fn is_hidden(name: &str, entry: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    name.starts_with('.')
        || entry
            .metadata()
            .is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
}

#[cfg(not(windows))]
fn is_hidden(name: &str, _entry: &std::fs::DirEntry) -> bool {
    name.starts_with('.')
}

/// Case-insensitive order where digit runs compare by value ("file2" < "file10").
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (na, nb) = (take(&mut a), take(&mut b));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vtt-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                Row::Entry { entry, depth, .. } => format!("{}{}", "  ".repeat(*depth), entry.name),
                Row::More { count, .. } => format!("+{count}"),
                Row::Error { .. } => "!".into(),
            })
            .collect()
    }

    #[test]
    fn natural_order() {
        assert_eq!(natural_cmp("file2", "file10"), Ordering::Less);
        assert_eq!(natural_cmp("B", "a"), Ordering::Greater);
        assert_eq!(natural_cmp("a", "a1"), Ordering::Less);
        assert_eq!(natural_cmp("x007", "x7"), Ordering::Equal);
    }

    #[test]
    fn lists_dirs_first_hides_dotfiles_and_expands() {
        let dir = temp_dir("tree");
        std::fs::create_dir(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "").unwrap();
        std::fs::write(dir.join("README.md"), "").unwrap();
        std::fs::write(dir.join(".env"), "").unwrap();

        let mut tree = FileTree::default();
        tree.set_root(dir.clone());
        assert_eq!(names(&tree.rows()), ["src", "README.md"]);

        tree.toggle(&dir.join("src"));
        assert_eq!(names(&tree.rows()), ["src", "  main.rs", "README.md"]);

        tree.show_hidden = true;
        assert_eq!(
            names(&tree.rows()),
            ["src", "  main.rs", ".env", "README.md"]
        );

        // A new file shows up after a refresh.
        std::fs::write(dir.join("src/lib.rs"), "").unwrap();
        tree.listings.get_mut(&dir.join("src")).unwrap().mtime = None;
        assert!(tree.refresh());
        assert_eq!(
            names(&tree.rows()),
            ["src", "  lib.rs", "  main.rs", ".env", "README.md"]
        );

        // Expansion is remembered when the root moves away and back.
        tree.set_root(dir.join("src"));
        assert_eq!(names(&tree.rows()), ["lib.rs", "main.rs"]);
        tree.set_root(dir.clone());
        assert_eq!(names(&tree.rows())[1], "  lib.rs");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unreadable_root_is_an_error_row() {
        let mut tree = FileTree::default();
        tree.set_root(PathBuf::from("/definitely/not/here"));
        assert_eq!(names(&tree.rows()), ["!"]);
    }
}
