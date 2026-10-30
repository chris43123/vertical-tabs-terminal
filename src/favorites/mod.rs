//! Favorite directories: named launch recipes (a path, an optional profile and command)
//! pinned above the tabs in the sidebar, and managed from the `vtt favorites` CLI.
//!
//! They live in their own per-machine file, `favorites.toml` in the data directory (never
//! synced through the config). The app and the CLI both change it the same way: re-read the
//! file, apply the change, write it back atomically, so neither overwrites the other's edit.
//! Pure data and IO; no egui.

pub mod cli;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A favorite's stable id: short, unique in the file and never reused.
pub type FavId = String;

/// Version of the file format written by this vtt.
const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Favorite {
    pub id: FavId,
    /// Shown in the sidebar; emoji allowed, names may repeat.
    pub name: String,
    pub path: PathBuf,
    /// A profile name; unset means the default profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Typed into the shell once it's ready (one line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// What a favorite is made of, before it has an id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draft {
    pub name: String,
    pub path: PathBuf,
    pub profile: Option<String>,
    pub command: Option<String>,
}

/// The editable fields, as the CLI names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Path,
    Profile,
    Command,
}

impl Field {
    pub const NAMES: &str = "name, path, profile, command";

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "name" => Self::Name,
            "path" => Self::Path,
            "profile" => Self::Profile,
            "command" => Self::Command,
            _ => return None,
        })
    }
}

/// The favorites list, in display order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Favorites {
    pub list: Vec<Favorite>,
    /// Number used for the next id, so removed ids are never handed out again.
    next_id: u64,
    /// What loading had to fix in a hand-edited file (shown as warnings).
    pub problems: Vec<String>,
}

/// On-disk form.
#[derive(Serialize, Deserialize)]
struct FileData {
    version: u32,
    #[serde(default)]
    next_id: u64,
    #[serde(default, rename = "favorite")]
    favorites: Vec<Favorite>,
}

impl Favorites {
    pub fn parse(text: &str) -> Result<Self, String> {
        let data: FileData = toml::from_str(text).map_err(|e| e.to_string())?;
        if data.version > VERSION {
            return Err(format!(
                "written by a newer vtt (version {}; this one reads {VERSION})",
                data.version
            ));
        }
        for (i, f) in data.favorites.iter().enumerate() {
            if f.id.trim().is_empty() {
                return Err(format!("favorite {} has no id", i + 1));
            }
            if data.favorites[..i].iter().any(|g| g.id == f.id) {
                return Err(format!("id `{}` is used twice", f.id));
            }
        }
        // Hand edits may have dropped the counter; never hand out an id that's in the file.
        let max = data
            .favorites
            .iter()
            .filter_map(|f| f.id.strip_prefix('f')?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        let mut favorites = data.favorites;
        let problems = favorites.iter_mut().filter_map(sanitize).collect();
        Ok(Self {
            list: favorites,
            next_id: data.next_id.max(max + 1).max(1),
            problems,
        })
    }

    pub fn to_toml(&self) -> String {
        let data = FileData {
            version: VERSION,
            next_id: self.next_id.max(1),
            favorites: self.list.clone(),
        };
        let body = toml::to_string(&data).expect("favorites serialize");
        format!(
            "# vtt favorites: directories pinned above the tabs.\n\
             # Edit with `vtt favorites` (see `vtt favorites --help`), from the sidebar, or by hand.\n\n\
             {body}"
        )
    }

    pub fn get(&self, id: &str) -> Option<&Favorite> {
        self.list.iter().find(|f| f.id == id)
    }

    /// Index of `<fav>`: a 1-based position, else an exact id, else an exact name. A name
    /// shared by several favorites is an error that lists their ids.
    pub fn resolve(&self, key: &str) -> Result<usize, String> {
        if let Ok(pos) = key.parse::<usize>()
            && (1..=self.list.len()).contains(&pos)
        {
            return Ok(pos - 1);
        }
        if let Some(i) = self.list.iter().position(|f| f.id == key) {
            return Ok(i);
        }
        let named: Vec<usize> = (0..self.list.len())
            .filter(|&i| self.list[i].name == key)
            .collect();
        match named[..] {
            [i] => Ok(i),
            [] => Err(format!(
                "no favorite `{key}` (give a position, id or name; see `vtt favorites list`)"
            )),
            _ => {
                let ids: Vec<&str> = named.iter().map(|&i| self.list[i].id.as_str()).collect();
                Err(format!(
                    "`{key}` names {} favorites ({}); use the id instead",
                    named.len(),
                    ids.join(", ")
                ))
            }
        }
    }

    /// Add a favorite at the end. Returns its index.
    pub fn add(&mut self, draft: Draft, profiles: &[String]) -> Result<usize, String> {
        let draft = self.validate(draft, None, profiles)?;
        let id = format!("f{}", self.next_id.max(1));
        self.next_id = self.next_id.max(1) + 1;
        self.list.push(Favorite {
            id,
            name: draft.name,
            path: draft.path,
            profile: draft.profile,
            command: draft.command,
        });
        Ok(self.list.len() - 1)
    }

    /// Replace every field of favorite `index` (the edit dialog).
    pub fn edit(&mut self, index: usize, draft: Draft, profiles: &[String]) -> Result<(), String> {
        self.check_index(index)?;
        let draft = self.validate(draft, Some(index), profiles)?;
        let f = &mut self.list[index];
        f.name = draft.name;
        f.path = draft.path;
        f.profile = draft.profile;
        f.command = draft.command;
        Ok(())
    }

    /// Set one field (`None` unsets it; only profile and command can be unset).
    pub fn set(
        &mut self,
        index: usize,
        field: Field,
        value: Option<&str>,
        profiles: &[String],
    ) -> Result<(), String> {
        self.check_index(index)?;
        let f = &self.list[index];
        let mut draft = Draft {
            name: f.name.clone(),
            path: f.path.clone(),
            profile: f.profile.clone(),
            command: f.command.clone(),
        };
        match (field, value) {
            (Field::Name, Some(v)) => draft.name = v.to_string(),
            (Field::Path, Some(v)) => draft.path = PathBuf::from(v),
            (Field::Profile, v) => draft.profile = v.map(str::to_string),
            (Field::Command, v) => draft.command = v.map(str::to_string),
            (Field::Name | Field::Path, None) => {
                return Err("name and path can't be unset (`rm` removes the favorite)".into());
            }
        }
        self.edit(index, draft, profiles)
    }

    /// Move favorite `index` to 1-based position `pos`.
    pub fn move_to(&mut self, index: usize, pos: usize) -> Result<(), String> {
        self.check_index(index)?;
        if !(1..=self.list.len()).contains(&pos) {
            return Err(format!(
                "position {pos} is out of range (1 to {})",
                self.list.len()
            ));
        }
        let f = self.list.remove(index);
        self.list.insert(pos - 1, f);
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<Favorite, String> {
        self.check_index(index)?;
        Ok(self.list.remove(index))
    }

    fn check_index(&self, index: usize) -> Result<(), String> {
        if index < self.list.len() {
            Ok(())
        } else {
            Err(format!("no favorite at position {}", index + 1))
        }
    }

    /// Check a draft and normalize it: trimmed name, canonical path, empty profile and
    /// command turned into `None`. `except` is the favorite being edited: an unchanged path
    /// isn't required to exist, so a favorite whose directory is gone for now can still be
    /// renamed or get its command fixed. The same directory may be pinned several times
    /// (a shell and `k9s` in one place); favorites are told apart by id.
    fn validate(
        &self,
        draft: Draft,
        except: Option<usize>,
        profiles: &[String],
    ) -> Result<Draft, String> {
        let name = draft.name.trim().to_string();
        if name.is_empty() {
            return Err("the name is empty".into());
        }
        if name.chars().any(char::is_control) {
            return Err("the name must be one line, without control characters".into());
        }
        let unchanged = except.is_some_and(|i| self.list[i].path == draft.path);
        let path = if unchanged {
            draft.path
        } else {
            canonical_dir(&draft.path)?
        };
        let profile = draft
            .profile
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty());
        if let Some(p) = &profile
            && !profiles.iter().any(|known| known == p)
        {
            return Err(format!(
                "unknown profile `{p}` (known: {})",
                profiles.join(", ")
            ));
        }
        let command = draft.command.filter(|c| !c.trim().is_empty());
        if let Some(c) = &command {
            check_command(c)?;
        }
        Ok(Draft {
            name,
            path,
            profile,
            command,
        })
    }
}

/// Make a favorite read from the file safe to show and type: control characters in its
/// name become U+FFFD, and a command that isn't one plain line is dropped. Returns what was
/// fixed.
fn sanitize(f: &mut Favorite) -> Option<String> {
    let mut fixed = Vec::new();
    if f.name.chars().any(char::is_control) {
        f.name = f
            .name
            .chars()
            .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
            .collect();
        fixed.push("its name had control characters (replaced)".to_string());
    }
    if let Some(err) = f.command.as_deref().and_then(|c| check_command(c).err()) {
        f.command = None;
        fixed.push(format!("its command was ignored: {err}"));
    }
    (!fixed.is_empty()).then(|| format!("favorite `{}` ({}): {}", f.name, f.id, fixed.join("; ")))
}

/// A command must be one line with no control characters (newlines, carriage returns,
/// ESC, …), so typing it sends exactly one Enter.
pub fn check_command(command: &str) -> Result<(), String> {
    match command.chars().find(|c| c.is_control()) {
        None => Ok(()),
        Some('\n' | '\r') => Err("the command must be a single line".into()),
        Some(c) => Err(format!(
            "the command contains a control character (U+{:04X}); only plain text is allowed",
            c as u32
        )),
    }
}

/// `path` made absolute and canonical; it must be an existing directory.
fn canonical_dir(path: &Path) -> Result<PathBuf, String> {
    let expanded = expand_tilde(path);
    match std::fs::canonicalize(&expanded).map(simplify) {
        Ok(p) if p.is_dir() => Ok(p),
        Ok(p) => Err(format!("{} is not a directory", p.display())),
        Err(_) => Err(format!("{} doesn't exist", expanded.display())),
    }
}

/// A canonical path in the form shells and people use. On Windows `canonicalize` returns
/// verbatim paths (`\\?\C:\x`), which cmd.exe refuses as a working directory.
fn simplify(path: PathBuf) -> PathBuf {
    match path.to_str() {
        Some(s) => PathBuf::from(strip_verbatim(s)),
        None => path,
    }
}

/// `\\?\C:\x` → `C:\x`, `\\?\UNC\server\share` → `\\server\share`; anything else as is.
fn strip_verbatim(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\")
        && rest.as_bytes().get(1) == Some(&b':')
    {
        rest.to_string()
    } else {
        path.to_string()
    }
}

/// `~` or `~/…` with the home directory filled in.
pub fn expand_tilde(path: &Path) -> PathBuf {
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    let rest = if s == "~" {
        Some("")
    } else {
        s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\"))
    };
    match (rest, dirs::home_dir()) {
        (Some(""), Some(home)) => home,
        (Some(rest), Some(home)) => home.join(rest),
        _ => path.to_path_buf(),
    }
}

/// A default name for a favorite at `path`: its last component.
pub fn default_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The favorites file on disk.
#[derive(Clone, Debug, PartialEq)]
pub struct Store {
    pub path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// `<data dir>/vtt/favorites.toml`, when the system has a data directory.
    pub fn default_path() -> Option<PathBuf> {
        dirs::data_dir().map(|d| d.join("vtt").join("favorites.toml"))
    }

    /// Read the file; a missing file is an empty list.
    pub fn load(&self) -> Result<Favorites, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Favorites::parse(&text)
                .map_err(|e| format!("invalid favorites file {}: {e}", self.path.display())),
            // (The app matches that prefix to clear the banner once the file is fixed.)
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Favorites::default()),
            Err(e) => Err(format!("can't read {}: {e}", self.path.display())),
        }
    }

    /// Re-read the file, apply `change` and write the result, so a change made elsewhere
    /// since the caller last looked is kept. Nothing is written if `change` fails.
    pub fn update<T>(
        &self,
        change: impl FnOnce(&mut Favorites) -> Result<T, String>,
    ) -> Result<(T, Favorites), String> {
        let mut favorites = self.load()?;
        let out = change(&mut favorites)?;
        self.save(&favorites)?;
        Ok((out, favorites))
    }

    /// Write atomically: a temp file in the same directory, then a rename over the old one,
    /// so readers see the old file or the new one, never half of it.
    fn save(&self, favorites: &Favorites) -> Result<(), String> {
        use std::io::Write as _;
        let fail = |e: std::io::Error| format!("can't write {}: {e}", self.path.display());
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(fail)?;
        let tmp = self.temp_path();
        let written = std::fs::File::create(&tmp).and_then(|mut f| {
            f.write_all(favorites.to_toml().as_bytes())?;
            f.sync_all()
        });
        if let Err(e) = written.and_then(|()| std::fs::rename(&tmp, &self.path)) {
            let _ = std::fs::remove_file(&tmp);
            return Err(fail(e));
        }
        Ok(())
    }

    fn temp_path(&self) -> PathBuf {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "favorites.toml".into());
        self.path
            .with_file_name(format!(".{name}.{}.tmp", std::process::id()))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh directory under the system temp dir.
    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vtt-fav-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::canonicalize(dir).unwrap()
    }

    fn draft(name: &str, path: &Path) -> Draft {
        Draft {
            name: name.into(),
            path: path.into(),
            ..Draft::default()
        }
    }

    fn sample() -> (PathBuf, Favorites) {
        let root = temp_dir("sample");
        let mut favs = Favorites::default();
        for d in ["a", "b", "c"] {
            std::fs::create_dir(root.join(d)).unwrap();
        }
        favs.add(draft("api", &root.join("a")), &[]).unwrap();
        favs.add(draft("🚀 web", &root.join("b")), &[]).unwrap();
        favs.add(draft("api", &root.join("c")), &[]).unwrap();
        (root, favs)
    }

    #[test]
    fn resolves_by_position_then_id_then_name() {
        let (_root, favs) = sample();
        assert_eq!(favs.resolve("2"), Ok(1));
        assert_eq!(favs.resolve("f3"), Ok(2));
        assert_eq!(favs.resolve("🚀 web"), Ok(1));
        // A shared name is ambiguous, and the error lists the ids to use instead.
        let err = favs.resolve("api").unwrap_err();
        assert!(err.contains("f1") && err.contains("f3"), "{err}");
        assert!(favs.resolve("nope").is_err());
        // Out-of-range numbers aren't positions.
        assert!(favs.resolve("9").is_err());
        assert!(favs.resolve("0").is_err());
    }

    #[test]
    fn a_position_wins_over_a_numeric_name() {
        let (root, mut favs) = sample();
        std::fs::create_dir(root.join("d")).unwrap();
        favs.add(draft("1", &root.join("d")), &[]).unwrap();
        assert_eq!(favs.resolve("1"), Ok(0));
    }

    #[test]
    fn ids_are_unique_and_never_reused() {
        let (root, mut favs) = sample();
        favs.remove(2).unwrap();
        std::fs::create_dir(root.join("e")).unwrap();
        let i = favs.add(draft("e", &root.join("e")), &[]).unwrap();
        assert_eq!(favs.list[i].id, "f4");
        // The counter survives a round trip through the file.
        let again = Favorites::parse(&favs.to_toml()).unwrap();
        assert_eq!(again, favs);
        // And a file whose counter was lost still doesn't reuse ids in it.
        let lost = Favorites::parse(
            "version = 1\n[[favorite]]\nid = \"f7\"\nname = \"x\"\npath = \"/x\"\n",
        )
        .unwrap();
        assert_eq!(lost.next_id, 8);
    }

    #[test]
    fn same_dir_twice_is_allowed_but_missing_dirs_and_unknown_profiles_are_not() {
        let (root, mut favs) = sample();
        // One directory, two purposes: a shell and k9s.
        let mut k9s = draft("k9s", &root.join("b").join("..").join("a"));
        k9s.command = Some("k9s".into());
        let i = favs.add(k9s, &[]).unwrap();
        assert_eq!(favs.list[i].path, favs.list[0].path);
        assert_ne!(favs.list[i].id, favs.list[0].id);
        assert!(favs.add(draft("x", &root.join("missing")), &[]).is_err());
        let file = root.join("file.txt");
        std::fs::write(&file, "").unwrap();
        assert!(favs.add(draft("x", &file), &[]).is_err());
        std::fs::create_dir(root.join("p")).unwrap();
        let mut d = draft("x", &root.join("p"));
        d.profile = Some("fish".into());
        assert!(favs.add(d.clone(), &["zsh".into()]).is_err());
        assert!(favs.add(d, &["zsh".into(), "fish".into()]).is_ok());
        assert!(favs.add(draft("  ", &root), &[]).is_err());
        assert_eq!(favs.list.len(), 5);
    }

    #[test]
    fn editing_keeps_its_own_path_and_unset_rules() {
        let (_root, mut favs) = sample();
        favs.set(0, Field::Name, Some("API"), &[]).unwrap();
        assert_eq!(favs.list[0].name, "API");
        favs.set(0, Field::Command, Some("cargo watch"), &[])
            .unwrap();
        assert_eq!(favs.list[0].command.as_deref(), Some("cargo watch"));
        favs.set(0, Field::Command, None, &[]).unwrap();
        assert_eq!(favs.list[0].command, None);
        assert!(favs.set(0, Field::Name, None, &[]).is_err());
        let b = favs.list[1].path.display().to_string();
        assert!(favs.set(0, Field::Path, Some(&b), &[]).is_ok());
    }

    #[test]
    fn commands_must_be_one_plain_line() {
        assert!(check_command("npm run dev -- --port 3000").is_ok());
        assert!(check_command("claude 'héllo 🚀'").is_ok());
        for bad in ["a\nb", "a\rb", "a\u{1b}[31m", "tab\there", "nul\0"] {
            assert!(check_command(bad).is_err(), "{bad:?}");
        }
        let (_root, mut favs) = sample();
        let before = favs.clone();
        assert!(
            favs.set(0, Field::Command, Some("ls\nrm -rf x"), &[])
                .is_err()
        );
        assert_eq!(favs, before);
    }

    #[test]
    fn moves_and_removes() {
        let (_root, mut favs) = sample();
        favs.move_to(0, 3).unwrap();
        let ids: Vec<&str> = favs.list.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["f2", "f3", "f1"]);
        assert!(favs.move_to(0, 4).is_err());
        assert!(favs.move_to(0, 0).is_err());
        assert_eq!(favs.remove(1).unwrap().id, "f3");
        assert!(favs.remove(5).is_err());
    }

    #[test]
    fn file_format_round_trips_and_is_checked() {
        let (_root, favs) = sample();
        let text = favs.to_toml();
        assert!(text.contains("version = 1"));
        assert!(text.contains("[[favorite]]"));
        assert_eq!(Favorites::parse(&text).unwrap(), favs);
        assert!(Favorites::parse("version = 99\n").is_err());
        let dup = "version = 1\n[[favorite]]\nid = \"a\"\nname = \"x\"\npath = \"/x\"\n\
                   [[favorite]]\nid = \"a\"\nname = \"y\"\npath = \"/y\"\n";
        assert!(Favorites::parse(dup).unwrap_err().contains("twice"));
    }

    #[test]
    fn writes_are_atomic_and_keep_changes_made_in_between() {
        let root = temp_dir("store");
        let store = Store::new(root.join("vtt").join("favorites.toml"));
        // A missing file is an empty list.
        assert!(store.load().unwrap().list.is_empty());
        for d in ["one", "two"] {
            std::fs::create_dir(root.join(d)).unwrap();
        }
        let (_, stale) = store
            .update(|f| f.add(draft("one", &root.join("one")), &[]))
            .unwrap();
        // Someone else (the CLI) adds a favorite after the app read the file...
        let other = Store::new(store.path.clone());
        other
            .update(|f| f.add(draft("two", &root.join("two")), &[]))
            .unwrap();
        // ...and the app, holding its stale copy, renames the first one.
        assert_eq!(stale.list.len(), 1);
        let (_, now) = store
            .update(|f| f.set(0, Field::Name, Some("uno"), &[]))
            .unwrap();
        let names: Vec<&str> = now.list.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["uno", "two"]);
        assert_eq!(store.load().unwrap(), now);
        // No temp file is left behind.
        let leftovers: Vec<_> = std::fs::read_dir(store.path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, ["favorites.toml"]);

        // A failed change writes nothing, and a failed write leaves the old file whole.
        let before = std::fs::read_to_string(&store.path).unwrap();
        assert!(
            store
                .update(|f| f.set(0, Field::Command, Some("a\nb"), &[]))
                .is_err()
        );
        std::fs::create_dir(store.temp_path()).unwrap();
        assert!(
            store
                .update(|f| f.set(0, Field::Name, Some("blocked"), &[]))
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&store.path).unwrap(), before);
    }

    #[test]
    fn strips_windows_verbatim_prefixes() {
        assert_eq!(strip_verbatim(r"\\?\C:\Users\me\api"), r"C:\Users\me\api");
        assert_eq!(
            strip_verbatim(r"\\?\UNC\server\share\x"),
            r"\\server\share\x"
        );
        // Verbatim paths that aren't a drive (devices, volume GUIDs) keep their prefix.
        assert_eq!(strip_verbatim(r"\\?\Volume{1234}\x"), r"\\?\Volume{1234}\x");
        assert_eq!(strip_verbatim("/srv/api"), "/srv/api");
        assert_eq!(strip_verbatim(r"C:\x"), r"C:\x");
    }

    #[test]
    fn hand_edited_control_characters_never_load() {
        let text = "version = 1\n\
            [[favorite]]\nid = \"f1\"\nname = \"ok\"\npath = \"/x\"\ncommand = \"make\\nrm -rf ~\"\n\
            [[favorite]]\nid = \"f2\"\nname = \"red\\u001b[31m\"\npath = \"/y\"\ncommand = \"ls\\u001b[2J\"\n\
            [[favorite]]\nid = \"f3\"\nname = \"fine\"\npath = \"/z\"\ncommand = \"k9s\"\n";
        let favs = Favorites::parse(text).unwrap();
        // The other favorites still load; only the bad parts are dropped.
        assert_eq!(favs.list.len(), 3);
        assert_eq!(favs.list[0].command, None);
        assert_eq!(favs.list[1].command, None);
        assert_eq!(favs.list[1].name, "red\u{FFFD}[31m");
        assert_eq!(favs.list[2].command.as_deref(), Some("k9s"));
        assert_eq!(favs.problems.len(), 2);
        assert!(favs.problems[0].contains("f1"), "{:?}", favs.problems);
        assert!(
            favs.list
                .iter()
                .all(|f| !f.name.chars().any(char::is_control))
        );
    }

    #[test]
    fn a_missing_directory_can_still_be_edited() {
        let (root, mut favs) = sample();
        std::fs::remove_dir(root.join("a")).unwrap();
        favs.set(0, Field::Command, Some("htop"), &[]).unwrap();
        favs.set(0, Field::Name, Some("gone"), &[]).unwrap();
        // Changing the path still requires the new one to exist.
        let missing = root.join("nowhere").display().to_string();
        assert!(favs.set(0, Field::Path, Some(&missing), &[]).is_err());
    }

    #[test]
    fn expands_tilde_and_names_by_last_component() {
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expand_tilde(Path::new("~")), home);
            assert_eq!(expand_tilde(Path::new("~/x")), home.join("x"));
        }
        assert_eq!(expand_tilde(Path::new("/a/~")), PathBuf::from("/a/~"));
        assert_eq!(default_name(Path::new("/srv/api")), "api");
        assert_eq!(default_name(Path::new("/")), "/");
    }
}
