//! Session restore: the tabs open when vtt last saved, so the next launch brings them back.
//!
//! `session.toml` in the data directory holds the tabs in sidebar order (profile name,
//! directory, command, custom title, favorite link, folder), the folders, the split views
//! and which view was shown. The app writes it (atomically) a moment after the layout
//! changes and when it quits; it never watches it. `vtt session` reads or clears it.
//! Pure data and IO; no egui.

pub mod cli;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::terminal::profiles::Profile;
use crate::workspace::{Dir, FolderSpec, Node, TabId, View, Workspace};

/// Version of the file format written by this vtt.
const VERSION: u32 = 1;

/// A saved session. Tabs, folders and views refer to each other by index.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub version: u32,
    /// Index into `views` of the view that was shown.
    #[serde(default)]
    pub active: usize,
    /// Tabs in sidebar order.
    #[serde(default, rename = "tab")]
    pub tabs: Vec<SavedTab>,
    #[serde(default, rename = "folder")]
    pub folders: Vec<SavedFolder>,
    #[serde(default, rename = "view")]
    pub views: Vec<SavedView>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedTab {
    /// The shell profile's name (resolved again at restore).
    pub profile: String,
    /// The tab's last known directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The name the user gave the tab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The id of the favorite this tab is the open tab of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favorite: Option<String>,
    /// Index into the folders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedFolder {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    /// Index of the tab that had keyboard focus.
    pub focused: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub layout: Layout,
}

/// A view's split tree; a leaf is a tab index.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Layout {
    Tab(usize),
    Split {
        dir: SplitDir,
        ratio: f32,
        a: Box<Layout>,
        b: Box<Layout>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitDir {
    /// Side by side.
    Horizontal,
    /// Stacked.
    Vertical,
}

impl Layout {
    /// The tab indices, in pane order.
    pub fn tabs(&self) -> Vec<usize> {
        match self {
            Layout::Tab(i) => vec![*i],
            Layout::Split { a, b, .. } => {
                let mut out = a.tabs();
                out.extend(b.tabs());
                out
            }
        }
    }

    fn from_node(node: &Node, index: &HashMap<TabId, usize>) -> Self {
        match node {
            Node::Leaf(t) => Layout::Tab(index[t]),
            Node::Split { dir, ratio, a, b } => Layout::Split {
                dir: match dir {
                    Dir::Horizontal => SplitDir::Horizontal,
                    Dir::Vertical => SplitDir::Vertical,
                },
                ratio: *ratio,
                a: Box::new(Self::from_node(a, index)),
                b: Box::new(Self::from_node(b, index)),
            },
        }
    }

    fn to_node(&self, first_id: TabId) -> Node {
        match self {
            Layout::Tab(i) => Node::Leaf(first_id + *i as TabId),
            Layout::Split { dir, ratio, a, b } => Node::Split {
                dir: match dir {
                    SplitDir::Horizontal => Dir::Horizontal,
                    SplitDir::Vertical => Dir::Vertical,
                },
                // Same bounds as dragging a divider; a hand-edited NaN gets the middle.
                ratio: if ratio.is_nan() {
                    0.5
                } else {
                    ratio.clamp(0.1, 0.9)
                },
                a: Box::new(a.to_node(first_id)),
                b: Box::new(b.to_node(first_id)),
            },
        }
    }
}

/// What the session keeps of one open terminal tab (see [`snapshot`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabInfo {
    pub profile: String,
    /// The shell's last known cwd (unknown on Windows, or before the first poll).
    pub cwd: Option<PathBuf>,
    /// Where its recipe started it.
    pub recipe_dir: Option<PathBuf>,
    pub command: Option<String>,
    pub title: Option<String>,
    pub favorite: Option<String>,
}

/// The session as it stands: every tab `info` describes, in sidebar order. Tabs it skips
/// (previews, the settings tab) are left out, and their panes dropped from their splits.
pub fn snapshot(ws: &Workspace, info: impl Fn(TabId) -> Option<TabInfo>) -> Session {
    let mut session = Session {
        version: VERSION,
        ..Session::default()
    };
    let mut index: HashMap<TabId, usize> = HashMap::new();
    let mut folders = HashMap::new();
    for &id in &ws.order {
        let Some(t) = info(id) else { continue };
        let folder = ws.group_of(id).and_then(|g| {
            let group = ws.group(g)?;
            Some(*folders.entry(g).or_insert_with(|| {
                session.folders.push(SavedFolder {
                    name: group.name.clone(),
                    color: group.color,
                    collapsed: group.collapsed,
                });
                session.folders.len() - 1
            }))
        });
        index.insert(id, session.tabs.len());
        session.tabs.push(SavedTab {
            profile: t.profile,
            dir: t.cwd.or(t.recipe_dir),
            command: t.command,
            title: t.title,
            favorite: t.favorite,
            folder,
        });
    }
    for (vi, view) in ws.views.iter().enumerate() {
        let mut root = Some(view.root.clone());
        for leaf in view.root.leaves() {
            if !index.contains_key(&leaf) {
                root = root.and_then(|r| r.remove(leaf));
            }
        }
        let Some(root) = root else { continue };
        let focused = if index.contains_key(&view.focused) {
            view.focused
        } else {
            root.leaves()[0]
        };
        if vi == ws.active {
            session.active = session.views.len();
        }
        session.views.push(SavedView {
            focused: index[&focused],
            name: view.name.clone(),
            layout: Layout::from_node(&root, &index),
        });
    }
    session
}

/// A tab to bring back, with everything resolved against the profiles and favorites of now.
#[derive(Clone, Debug, PartialEq)]
pub struct RestoredTab {
    pub id: TabId,
    pub profile: Profile,
    pub dir: Option<PathBuf>,
    pub command: Option<String>,
    pub title: Option<String>,
    pub favorite: Option<String>,
}

/// The result of [`Session::rebuild`].
#[derive(Debug)]
pub struct Restored {
    pub ws: Workspace,
    /// In sidebar order.
    pub tabs: Vec<RestoredTab>,
    /// What had to change, for the banner.
    pub warnings: Vec<String>,
}

impl Session {
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let session: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if session.version > VERSION {
            return Err(format!(
                "written by a newer vtt (version {}; this one reads {VERSION})",
                session.version
            ));
        }
        Ok(session)
    }

    pub fn to_toml(&self) -> String {
        let data = Self {
            version: VERSION,
            ..self.clone()
        };
        let body = toml::to_string(&data).expect("session serializes");
        format!(
            "# vtt session: the tabs open when vtt last saved, restored at launch.\n\
             # Written by vtt (`restore_session`). Quit vtt before `vtt session clear`.\n\n\
             {body}"
        )
    }

    /// The view holding tab `tab` (an index into `tabs`).
    pub fn view_of(&self, tab: usize) -> Option<usize> {
        self.views
            .iter()
            .position(|v| v.layout.tabs().contains(&tab))
    }

    /// The tab that had keyboard focus.
    pub fn focused(&self) -> Option<usize> {
        self.views
            .get(self.active)
            .map(|v| v.focused)
            .filter(|t| *t < self.tabs.len())
    }

    /// Turn the saved data back into a workspace whose tab ids start at `first_id`, with each
    /// tab's profile, directory and favorite resolved: an unknown profile becomes the
    /// default one (`profiles[0]`) without the command, so nothing is typed into a shell the
    /// user didn't choose; a missing directory becomes the profile's cwd, else `home`; and a
    /// favorite that no longer exists (per `favorite_exists`) loses its link.
    pub fn rebuild(
        &self,
        first_id: TabId,
        profiles: &[Profile],
        favorite_exists: &dyn Fn(&str) -> bool,
        home: Option<&Path>,
    ) -> Restored {
        let mut warnings = Vec::new();
        let mut tabs = Vec::new();
        let mut order = Vec::new();
        for (i, t) in self.tabs.iter().enumerate() {
            let known = profiles.iter().find(|p| p.name == t.profile);
            let Some(profile) = known.or(profiles.first()) else {
                continue;
            };
            if known.is_none() {
                let w = format!(
                    "session: unknown profile `{}`, its tabs open `{}` instead",
                    t.profile, profile.name
                );
                if !warnings.contains(&w) {
                    warnings.push(w);
                }
            }
            let id = first_id + i as TabId;
            let dir = t
                .dir
                .clone()
                .filter(|d| d.is_dir())
                .or_else(|| profile.cwd.clone())
                .or_else(|| home.map(Path::to_path_buf));
            tabs.push(RestoredTab {
                id,
                profile: profile.clone(),
                dir,
                command: t.command.clone().filter(|_| known.is_some()),
                title: t.title.clone(),
                favorite: t.favorite.clone().filter(|f| favorite_exists(f)),
            });
            order.push((id, t.folder));
        }
        let folders: Vec<FolderSpec> = self
            .folders
            .iter()
            .map(|f| FolderSpec {
                name: f.name.clone(),
                color: f.color,
                collapsed: f.collapsed,
            })
            .collect();
        let views = self
            .views
            .iter()
            .map(|v| View {
                root: v.layout.to_node(first_id),
                focused: first_id + v.focused as TabId,
                name: v.name.clone(),
            })
            .collect();
        Restored {
            ws: Workspace::restore(&order, &folders, views, self.active),
            tabs,
            warnings,
        }
    }
}

/// The session file on disk.
#[derive(Clone, Debug, PartialEq)]
pub struct Store {
    pub path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// `<data dir>/vtt/session.toml`, when the system has a data directory.
    pub fn default_path() -> Option<PathBuf> {
        dirs::data_dir().map(|d| d.join("vtt").join("session.toml"))
    }

    /// Where a file that couldn't be read is moved: `session.toml.bad`.
    pub fn bad_path(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".bad");
        self.path.with_file_name(name)
    }

    /// Read the file; a missing file is an empty session.
    pub fn load(&self) -> Result<Session, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Session::parse(&text)
                .map_err(|e| format!("invalid session file {}: {e}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Session::default()),
            Err(e) => Err(format!("can't read {}: {e}", self.path.display())),
        }
    }

    /// Read the file for a restore. One that can't be read or parsed is moved aside to
    /// [`Self::bad_path`] (so the next save doesn't destroy it) and gives an empty session,
    /// with a warning saying so.
    pub fn load_for_restore(&self) -> (Session, Option<String>) {
        match self.load() {
            Ok(session) => (session, None),
            Err(e) => {
                let bad = self.bad_path();
                let warning = match std::fs::rename(&self.path, &bad) {
                    Ok(()) => format!(
                        "{e}; starting fresh (the old file is now {})",
                        bad.display()
                    ),
                    Err(_) => format!("{e}; starting fresh"),
                };
                (Session::default(), Some(warning))
            }
        }
    }

    pub fn save(&self, session: &Session) -> Result<(), String> {
        crate::fsutil::write_atomic(&self.path, &session.to_toml())
    }

    /// Delete the file. Returns whether there was one.
    pub fn clear(&self) -> Result<bool, String> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!("can't delete {}: {e}", self.path.display())),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::favorites::tests::temp_dir;
    use crate::workspace::{Drop, Edge};

    pub(crate) fn profile(name: &str, cwd: Option<&Path>) -> Profile {
        Profile {
            name: name.into(),
            command: format!("/bin/{name}"),
            args: Vec::new(),
            cwd: cwd.map(Path::to_path_buf),
            env: HashMap::new(),
            icon: name[..1].to_uppercase(),
            color: None,
        }
    }

    fn info(profile: &str, dir: &str) -> TabInfo {
        TabInfo {
            profile: profile.into(),
            recipe_dir: Some(dir.into()),
            ..TabInfo::default()
        }
    }

    /// Five tabs: 1 and 2 in folder "work" (split side by side), 3 alone, 4 and 5 in folder
    /// "ops" (collapsed, colored), with names, a command and a favorite link.
    fn sample_workspace() -> Workspace {
        let mut w = Workspace::default();
        for i in 1..=5 {
            w.add(i, None);
        }
        w.activate(1);
        w.drop_on(2, 1, Drop::Edge(Edge::Right));
        let work = w.new_group(1, "work".into()).unwrap();
        let v = w.view_of(1).unwrap();
        w.views[v].name = Some("api + web".into());
        w.views[v].root.set_ratio(&[], 0.3);
        let ops = w.new_group(4, "ops".into()).unwrap();
        w.set_group(5, Some(ops));
        let g = w.group_mut(ops).unwrap();
        g.collapsed = true;
        g.color = Some(3);
        w.group_mut(work).unwrap().color = None;
        w.activate(2);
        w
    }

    fn sample_info(id: TabId) -> Option<TabInfo> {
        let mut t = info(
            if id == 4 { "bash" } else { "zsh" },
            &format!("/start/{id}"),
        );
        match id {
            1 => {
                t.title = Some("api".into());
                t.command = Some("cargo watch".into());
            }
            3 => t.favorite = Some("f2".into()),
            _ => {}
        }
        Some(t)
    }

    /// The parts of a workspace that a session keeps, with ids renumbered by sidebar position.
    fn shape(w: &Workspace) -> String {
        let pos = |t: &TabId| w.order.iter().position(|o| o == t).unwrap();
        let mut out = String::new();
        for t in &w.order {
            let g = w.group_of(*t).and_then(|g| w.group(g));
            out += &format!(
                "tab {} folder {:?}\n",
                pos(t),
                g.map(|g| (&g.name, g.color, g.collapsed))
            );
        }
        for (i, v) in w.views.iter().enumerate() {
            let mut node = v.root.clone();
            for leaf in v.root.leaves() {
                node.replace(leaf, 1000 + pos(&leaf) as TabId);
            }
            out += &format!(
                "view {i} {:?} focused {} name {:?} active {}\n",
                node,
                pos(&v.focused),
                v.name,
                i == w.active
            );
        }
        out
    }

    fn rebuild_all(s: &Session, first: TabId) -> Restored {
        let profiles = [profile("zsh", None), profile("bash", None)];
        s.rebuild(first, &profiles, &|_| true, Some(Path::new("/home/u")))
    }

    #[test]
    fn snapshot_then_rebuild_round_trips_the_layout() {
        let w = sample_workspace();
        let s = snapshot(&w, sample_info);
        // Through the file format too.
        let s = Session::parse(&s.to_toml()).unwrap();
        let r = rebuild_all(&s, 100);
        assert_eq!(shape(&r.ws), shape(&w));
        assert!(r.warnings.is_empty());
        // Focus and the shown view come back.
        assert_eq!(
            r.ws.focused()
                .map(|f| r.ws.order.iter().position(|t| *t == f)),
            w.focused().map(|f| w.order.iter().position(|t| *t == f))
        );
        // Titles, commands, profiles and favorite links, in sidebar order.
        let by_pos: Vec<_> =
            r.ws.order
                .iter()
                .map(|id| r.tabs.iter().find(|t| t.id == *id).unwrap())
                .collect();
        let orig: Vec<_> = w.order.iter().map(|id| sample_info(*id).unwrap()).collect();
        for (t, o) in by_pos.iter().zip(&orig) {
            assert_eq!(t.title, o.title);
            assert_eq!(t.command, o.command);
            assert_eq!(t.favorite, o.favorite);
            assert_eq!(t.profile.name, o.profile);
        }
    }

    #[test]
    fn ratios_and_split_direction_survive_the_file() {
        let w = sample_workspace();
        let s = snapshot(&w, sample_info);
        let back = Session::parse(&s.to_toml()).unwrap();
        assert_eq!(back, s);
        let view = &back.views[back.view_of(0).unwrap()];
        assert!(matches!(
            view.layout,
            Layout::Split { dir: SplitDir::Horizontal, ratio, .. } if ratio == 0.3
        ));
    }

    #[test]
    fn saved_dir_is_the_last_known_cwd() {
        let mut w = Workspace::default();
        w.add(1, None);
        w.add(2, None);
        let s = snapshot(&w, |id| {
            let mut t = info("zsh", "/recipe");
            if id == 1 {
                t.cwd = Some("/moved/on".into());
            }
            Some(t)
        });
        assert_eq!(s.tabs[0].dir.as_deref(), Some(Path::new("/moved/on")));
        // Without a known cwd (Windows), the recipe's dir.
        assert_eq!(s.tabs[1].dir.as_deref(), Some(Path::new("/recipe")));
    }

    #[test]
    fn tabs_without_a_recipe_are_left_out() {
        let mut w = Workspace::default();
        for i in 1..=3 {
            w.add(i, None);
        }
        w.activate(1);
        w.drop_on(2, 1, Drop::Edge(Edge::Bottom));
        // Tab 2 (a preview) is focused in the split shown.
        let s = snapshot(&w, |id| (id != 2).then(|| info("zsh", "/x")));
        assert_eq!(s.tabs.len(), 2);
        assert_eq!(s.views.len(), 2);
        assert_eq!(s.views[s.active].layout, Layout::Tab(0));
        assert_eq!(s.focused(), Some(0));
        // A preview showing alone: its view is gone, another is shown.
        w.minimize(2);
        w.activate(2);
        let s = snapshot(&w, |id| (id != 2).then(|| info("zsh", "/x")));
        assert_eq!(s.views.len(), 2);
        assert!(s.active < s.views.len());
    }

    #[test]
    fn unknown_profile_uses_the_default_without_its_command() {
        let s = Session {
            tabs: vec![SavedTab {
                profile: "fish".into(),
                command: Some("rm -rf build".into()),
                ..SavedTab::default()
            }],
            ..Session::default()
        };
        let r = rebuild_all(&s, 1);
        assert_eq!(r.tabs[0].profile.name, "zsh");
        assert_eq!(r.tabs[0].command, None);
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("fish"), "{:?}", r.warnings);
        // No views saved: the tab still gets one.
        assert_eq!(r.ws.visible(), vec![1]);
    }

    #[test]
    fn missing_dir_falls_back_to_the_profile_then_home() {
        let root = temp_dir("session-dir");
        let gone = root.join("gone");
        let s = Session {
            tabs: vec![
                SavedTab {
                    profile: "zsh".into(),
                    dir: Some(root.clone()),
                    ..SavedTab::default()
                },
                SavedTab {
                    profile: "zsh".into(),
                    dir: Some(gone.clone()),
                    ..SavedTab::default()
                },
                SavedTab {
                    profile: "srv".into(),
                    dir: Some(gone),
                    ..SavedTab::default()
                },
            ],
            ..Session::default()
        };
        let profiles = [
            profile("zsh", None),
            profile("srv", Some(Path::new("/srv"))),
        ];
        let r = s.rebuild(1, &profiles, &|_| true, Some(Path::new("/home/u")));
        assert_eq!(r.tabs[0].dir.as_deref(), Some(root.as_path()));
        assert_eq!(r.tabs[1].dir.as_deref(), Some(Path::new("/home/u")));
        assert_eq!(r.tabs[2].dir.as_deref(), Some(Path::new("/srv")));
    }

    #[test]
    fn removed_favorite_drops_the_link() {
        let tab = |f: &str| SavedTab {
            profile: "zsh".into(),
            favorite: Some(f.into()),
            ..SavedTab::default()
        };
        let s = Session {
            tabs: vec![tab("f1"), tab("f9")],
            ..Session::default()
        };
        let profiles = [profile("zsh", None)];
        let r = s.rebuild(1, &profiles, &|id| id == "f1", None);
        assert_eq!(r.tabs[0].favorite.as_deref(), Some("f1"));
        assert_eq!(r.tabs[1].favorite, None);
    }

    #[test]
    fn corrupt_or_newer_file_is_moved_aside() {
        let root = temp_dir("session-bad");
        let store = Store::new(root.join("session.toml"));
        // Absent: empty, no warning.
        let (s, w) = store.load_for_restore();
        assert!(s.is_empty() && w.is_none());

        for text in ["not = [toml", "version = 99\n"] {
            std::fs::write(&store.path, text).unwrap();
            let (s, w) = store.load_for_restore();
            assert!(s.is_empty());
            let w = w.unwrap();
            assert!(w.contains("session.toml.bad"), "{w}");
            assert!(!store.path.exists());
            assert_eq!(std::fs::read_to_string(store.bad_path()).unwrap(), text);
        }
        assert!(Session::parse("version = 2").unwrap_err().contains("newer"));
    }

    #[test]
    fn hand_edited_layouts_never_panic() {
        let text = "version = 1\nactive = 7\n\
                    [[tab]]\nprofile = \"zsh\"\n\
                    [[tab]]\nprofile = \"zsh\"\nfolder = 4\n\
                    [[view]]\nfocused = 9\nlayout = { dir = \"vertical\", ratio = 5.0, a = 0, b = 0 }\n\
                    [[view]]\nfocused = 1\nlayout = { dir = \"horizontal\", ratio = 0.5, a = 1, b = 12 }\n";
        let s = Session::parse(text).unwrap();
        let r = rebuild_all(&s, 1);
        let mut all: Vec<TabId> = r.ws.views.iter().flat_map(|v| v.root.leaves()).collect();
        all.sort();
        assert_eq!(all, vec![1, 2]);
        assert_eq!(r.ws.order, vec![1, 2]);
        assert!(r.ws.groups.is_empty());
        assert!(r.ws.focused().is_some());
    }

    #[test]
    fn save_and_clear() {
        let root = temp_dir("session-save");
        let store = Store::new(root.join("data").join("session.toml"));
        let s = snapshot(&sample_workspace(), sample_info);
        store.save(&s).unwrap();
        assert_eq!(store.load().unwrap(), s);
        assert!(store.clear().unwrap());
        assert!(!store.clear().unwrap());
        assert!(store.load().unwrap().is_empty());
    }
}
