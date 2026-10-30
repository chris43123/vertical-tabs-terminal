//! Favorites in the app: the loaded list, which tab each one opened, and opening, resetting,
//! reordering and editing them. Every change goes through [`Store::update`], so an edit made
//! with `vtt favorites` while the app runs is never overwritten.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::favorites::{Draft, FavId, Favorite, Favorites, Store};
use crate::workspace::{Edge, TabId};

use super::App;
use super::recipe::LaunchRecipe;

/// What a favorite's status dot shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FavStatus {
    /// Its tab is open.
    Running,
    /// No tab (never opened, or the user closed it).
    Idle,
    /// Its tab ended on its own (the shell exited); a click opens a new one.
    Exited,
}

#[derive(Default)]
pub struct FavoritesState {
    pub list: Favorites,
    store: Option<Store>,
    /// The tab each favorite opened. Never inferred from a tab's cwd.
    links: HashMap<FavId, TabId>,
    exited: HashSet<FavId>,
    /// Favorites whose directory was missing when last checked (load, reload, click).
    missing: HashSet<FavId>,
}

impl FavoritesState {
    /// Load the favorites file from the data directory (problems go to the banner).
    pub fn load() -> Self {
        let mut state = Self {
            store: Store::default_path().map(Store::new),
            ..Self::default()
        };
        if let Some(store) = &state.store {
            match store.load() {
                Ok(list) => {
                    warn_problems(&list);
                    state.replace(list);
                }
                Err(err) => crate::diag::warn(err),
            }
        }
        state
    }

    pub fn store_path(&self) -> Option<&Path> {
        self.store.as_ref().map(|s| s.path.as_path())
    }

    pub fn status(&self, id: &str) -> FavStatus {
        if self.links.contains_key(id) {
            FavStatus::Running
        } else if self.exited.contains(id) {
            FavStatus::Exited
        } else {
            FavStatus::Idle
        }
    }

    pub fn is_missing(&self, id: &str) -> bool {
        self.missing.contains(id)
    }

    pub fn linked_tab(&self, id: &str) -> Option<TabId> {
        self.links.get(id).copied()
    }

    /// The favorite whose own tab `tab` is (its row in the sidebar is that favorite's).
    pub fn favorite_for_tab(&self, tab: TabId) -> Option<FavId> {
        self.links
            .iter()
            .find(|&(_, &t)| t == tab)
            .map(|(id, _)| id.clone())
    }

    /// Take a freshly read list: links and exited marks of ids that survive stay, and every
    /// path is checked again.
    fn replace(&mut self, list: Favorites) {
        let alive: HashSet<&str> = list.list.iter().map(|f| f.id.as_str()).collect();
        self.links.retain(|id, _| alive.contains(id.as_str()));
        self.exited.retain(|id| alive.contains(id.as_str()));
        self.missing = list
            .list
            .iter()
            .filter(|f| !f.path.is_dir())
            .map(|f| f.id.clone())
            .collect();
        self.list = list;
    }

    /// Apply `change` to the file as it is now on disk, then take the result.
    pub fn update<T>(
        &mut self,
        change: impl FnOnce(&mut Favorites) -> Result<T, String>,
    ) -> Result<T, String> {
        let store = self
            .store
            .as_ref()
            .ok_or("no data directory to keep favorites in")?;
        let (out, list) = store.update(change)?;
        self.replace(list);
        Ok(out)
    }

    fn link(&mut self, id: FavId, tab: TabId) {
        self.exited.remove(&id);
        self.links.insert(id, tab);
    }

    /// Forget tab `tab`; `exited` marks its favorite as ended on its own.
    fn unlink(&mut self, tab: TabId, exited: bool) {
        let ids: Vec<FavId> = self
            .links
            .iter()
            .filter(|&(_, &t)| t == tab)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.links.remove(&id);
            if exited {
                self.exited.insert(id);
            }
        }
    }

    /// Re-check one favorite's path. Returns whether it exists.
    fn check(&mut self, id: &str) -> bool {
        let exists = self.list.get(id).is_some_and(|f| f.path.is_dir());
        if exists {
            self.missing.remove(id);
        } else {
            self.missing.insert(id.to_string());
        }
        exists
    }
}

/// A favorite with an open tab, as the sidebar shows it: the favorite's row is that tab,
/// and the other tabs of its split view are listed under it instead of among the tabs.
#[derive(Clone, Debug, PartialEq)]
pub struct FavoriteTabs {
    pub id: FavId,
    pub tab: TabId,
    /// The rest of its split view, in sidebar order.
    pub siblings: Vec<TabId>,
}

/// Group the open favorites' tabs. `link` gives a favorite's live tab, `view_of` a tab's
/// view. A tab belongs to at most one favorite (the first in the list whose view holds it),
/// and another favorite's own tab is never someone's sibling.
fn group_favorite_tabs(
    favorites: &[Favorite],
    link: impl Fn(&str) -> Option<TabId>,
    order: &[TabId],
    view_of: impl Fn(TabId) -> Option<usize>,
) -> Vec<FavoriteTabs> {
    let linked: Vec<(FavId, TabId)> = favorites
        .iter()
        .filter_map(|f| Some((f.id.clone(), link(&f.id)?)))
        .collect();
    let mut claimed: HashSet<TabId> = linked.iter().map(|&(_, t)| t).collect();
    linked
        .into_iter()
        .map(|(id, tab)| {
            let view = view_of(tab);
            let siblings = order
                .iter()
                .copied()
                .filter(|&t| view.is_some() && view_of(t) == view)
                .filter(|&t| claimed.insert(t))
                .collect();
            FavoriteTabs { id, tab, siblings }
        })
        .collect()
}

/// Whether `tab` is a favorite's own tab or one of its split's other tabs.
fn in_favorite_split(groups: &[FavoriteTabs], tab: TabId) -> bool {
    groups
        .iter()
        .any(|g| g.tab == tab || g.siblings.contains(&tab))
}

/// How a broken favorites file is reported (see [`Store::load`]).
const INVALID_FILE: &str = "invalid favorites file";

/// Report what loading had to fix in a hand-edited file.
fn warn_problems(list: &Favorites) {
    for p in &list.problems {
        crate::diag::warn(p.clone());
    }
}

impl App {
    /// The favorites file changed on disk: reload it (a broken file keeps the old list).
    pub(super) fn reload_favorites(&mut self) {
        let Some(store) = self.favorites.store.clone() else {
            return;
        };
        match store.load() {
            Ok(list) => {
                // The file is readable again: drop the banner that said it wasn't.
                self.problems.retain(|p| !p.starts_with(INVALID_FILE));
                warn_problems(&list);
                self.favorites.replace(list);
                self.warn_unknown_profiles();
                self.sync_favorite_order();
            }
            Err(err) => crate::diag::warn(format!("{err} (keeping the previous favorites)")),
        }
    }

    /// Favorites naming a profile that doesn't exist (anymore) open with the default one.
    pub(super) fn warn_unknown_profiles(&self) {
        for f in &self.favorites.list.list {
            if let Some(p) = &f.profile
                && !self.profiles.iter().any(|known| &known.name == p)
            {
                crate::diag::warn(format!(
                    "favorite `{}`: unknown profile `{p}`, using the default",
                    f.name
                ));
            }
        }
    }

    /// The recipe favorite `fav` opens with now: its profile (else the default), its path and
    /// its command.
    fn favorite_recipe(&self, fav: &Favorite) -> Option<LaunchRecipe> {
        let profile = fav
            .profile
            .as_ref()
            .and_then(|name| self.profiles.iter().find(|p| &p.name == name))
            .or(self.profiles.first())?
            .clone();
        let mut recipe = LaunchRecipe::new(profile, Some(fav.path.clone()));
        // Loading already drops bad commands; this keeps typing to one Enter whatever the source.
        recipe.command = fav
            .command
            .clone()
            .filter(|c| crate::favorites::check_command(c).is_ok());
        recipe.favorite = Some(fav.id.clone());
        Some(recipe)
    }

    /// For Reset: a tab opened from a favorite that still exists (with its directory) starts
    /// from the favorite's current recipe, keeping the tab's name; any other from its own.
    pub(super) fn current_recipe(&self, recipe: LaunchRecipe) -> LaunchRecipe {
        let fav = recipe
            .favorite
            .as_deref()
            .and_then(|id| self.favorites.list.get(id))
            .filter(|f| f.path.is_dir());
        match fav.and_then(|f| self.favorite_recipe(f)) {
            Some(current) => current.with_title(recipe.custom_title),
            None => recipe,
        }
    }

    /// A tab was opened from `recipe` as `tab`: link it to its favorite unless that one
    /// already has a live tab.
    pub(super) fn link_favorite(&mut self, recipe: &LaunchRecipe, tab: TabId) {
        let Some(id) = recipe.favorite.clone() else {
            return;
        };
        let live = self
            .favorites
            .linked_tab(&id)
            .is_some_and(|t| self.tabs.contains_key(&t));
        if !live && self.favorites.list.get(&id).is_some() {
            self.favorites.link(id, tab);
        }
    }

    /// The open favorites with their tabs (see [`FavoriteTabs`]).
    pub fn favorite_tabs(&self) -> Vec<FavoriteTabs> {
        group_favorite_tabs(
            &self.favorites.list.list,
            |id| {
                self.favorites
                    .linked_tab(id)
                    .filter(|t| self.tabs.contains_key(t))
            },
            &self.ws.order,
            |t| self.ws.view_of(t),
        )
    }

    /// Keep the favorites' tabs (and their splits) at the front of the tab order, in the
    /// favorites' order, so next/previous tab and "go to tab N" follow what the sidebar shows.
    pub(super) fn sync_favorite_order(&mut self) {
        for g in self.favorite_tabs().into_iter().rev() {
            self.ws.reorder(g.tab, 0);
        }
    }

    /// Tabs the sidebar shows under a favorite rather than in the tab list.
    pub fn tabs_under_favorites(&self) -> HashSet<TabId> {
        self.favorite_tabs()
            .into_iter()
            .flat_map(|g| std::iter::once(g.tab).chain(g.siblings))
            .collect()
    }

    /// Tab `tab` is going away; `exited` when its shell ended on its own.
    pub(super) fn unlink_favorite(&mut self, tab: TabId, exited: bool) {
        self.favorites.unlink(tab, exited);
    }

    /// Click on a favorite: focus its tab if open; otherwise open one from its recipe (in a
    /// split on `split`), unless its directory is gone.
    pub fn open_favorite(&mut self, id: &str, split: Option<Edge>) {
        if let Some(tab) = self
            .favorites
            .linked_tab(id)
            .filter(|t| self.tabs.contains_key(t))
        {
            self.activate(tab);
            self.scroll_to_focused = true;
            return;
        }
        let Some(fav) = self.favorites.list.get(id).cloned() else {
            return;
        };
        if !self.favorites.check(id) {
            self.notify(
                format!(
                    "{}: {} doesn't exist",
                    fav.name,
                    crate::ui::display_path(&fav.path)
                ),
                true,
            );
            return;
        }
        let Some(mut recipe) = self.favorite_recipe(&fav) else {
            return;
        };
        // Splitting into another favorite's view: a plain tab started like this favorite
        // joins that favorite's split, and this favorite's own row stays as it was (two
        // favorite rows sharing one split would be bracketed apart).
        let into_favorite = split.is_some()
            && self
                .ws
                .focused()
                .is_some_and(|t| in_favorite_split(&self.favorite_tabs(), t));
        if into_favorite {
            recipe.favorite = None;
        }
        if let Some(tab) = self.spawn_tab(recipe, split)
            && split.is_none()
        {
            // Its row is the favorite's, above the tabs: it doesn't join the focused folder.
            self.ws.set_group(tab, None);
        }
        self.sync_favorite_order();
    }

    /// `open_favorite_N`: the Nth favorite (1-based).
    pub(super) fn open_favorite_at(&mut self, n: usize) -> bool {
        let Some(id) = self
            .favorites
            .list
            .list
            .get(n.wrapping_sub(1))
            .map(|f| f.id.clone())
        else {
            return false;
        };
        self.open_favorite(&id, None);
        true
    }

    /// ↻ on a favorite: reset its tab (asking first if something runs), or open it.
    pub fn reset_favorite(&mut self, id: &str) {
        match self
            .favorites
            .linked_tab(id)
            .filter(|t| self.tabs.contains_key(t))
        {
            Some(tab) => self.reset_tab(tab),
            None => self.open_favorite(id, None),
        }
    }

    /// Show the favorite's directory in the files panel.
    pub fn reveal_favorite(&mut self, id: &str) {
        let Some(path) = self.favorites.list.get(id).map(|f| f.path.clone()) else {
            return;
        };
        if self.side.hidden {
            self.zen_show(true);
        } else {
            self.side.files_open = true;
        }
        self.browse_files(path);
    }

    pub fn remove_favorite(&mut self, id: &str) {
        let result = self.favorites.update(|f| {
            let i = f
                .list
                .iter()
                .position(|x| x.id == id)
                .ok_or("that favorite was already removed")?;
            f.remove(i).map(|_| ())
        });
        if let Err(err) = result {
            self.notify(err, true);
        }
    }

    /// Drag reorder: move favorite `id` to index `to` (0-based, in the list before the move).
    pub fn move_favorite(&mut self, id: &str, to: usize) {
        let result = self.favorites.update(|f| {
            let i = f
                .list
                .iter()
                .position(|x| x.id == id)
                .ok_or("that favorite was removed")?;
            // Dropping below itself shifts by one once it's taken out.
            let pos = if to > i { to } else { to + 1 };
            f.move_to(i, pos.clamp(1, f.list.len()))
        });
        match result {
            Ok(()) => self.sync_favorite_order(),
            Err(err) => self.notify(err, true),
        }
    }

    /// Open the favorite dialog to add `dir` (default: the focused tab's directory).
    pub fn start_add_favorite(&mut self, dir: Option<PathBuf>) {
        let dir = dir
            .or_else(|| self.ws.focused().and_then(|f| self.focused_dir(f)))
            .or_else(dirs::home_dir)
            .unwrap_or_default();
        self.favorite_dialog = Some(crate::ui::FavoriteDialog::add(&dir));
    }

    pub fn start_edit_favorite(&mut self, id: &str) {
        if let Some(fav) = self.favorites.list.get(id) {
            self.favorite_dialog = Some(crate::ui::FavoriteDialog::edit(fav));
        }
    }

    /// Save the dialog's draft as a new favorite or over `editing`. The error is shown inline.
    pub fn save_favorite(&mut self, editing: Option<&str>, draft: Draft) -> Result<(), String> {
        let profiles: Vec<String> = self.profiles.iter().map(|p| p.name.clone()).collect();
        self.favorites.update(|f| match editing {
            None => f.add(draft, &profiles).map(|_| ()),
            Some(id) => {
                let i = f
                    .list
                    .iter()
                    .position(|x| x.id == id)
                    .ok_or("this favorite was removed meanwhile")?;
                f.edit(i, draft, &profiles)
            }
        })
    }

    /// A short note at the bottom of the window.
    pub fn notify(&mut self, message: String, failed: bool) {
        self.toast = Some(crate::ui::Toast::new(message, None, failed));
        self.ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::favorites::tests::temp_dir;

    fn state(name: &str) -> (PathBuf, FavoritesState) {
        let root = temp_dir(name);
        let store = Store::new(root.join("favorites.toml"));
        let state = FavoritesState {
            store: Some(store),
            ..FavoritesState::default()
        };
        (root, state)
    }

    fn add(state: &mut FavoritesState, root: &Path, dir: &str) -> FavId {
        std::fs::create_dir_all(root.join(dir)).unwrap();
        let draft = Draft {
            name: "same name".into(),
            path: root.join(dir),
            ..Draft::default()
        };
        let i = state.update(|f| f.add(draft, &[])).unwrap();
        state.list.list[i].id.clone()
    }

    #[test]
    fn links_are_by_id_and_track_how_tabs_ended() {
        let (root, mut s) = state("links");
        let a = add(&mut s, &root, "a");
        let b = add(&mut s, &root, "b");
        assert_eq!(s.status(&a), FavStatus::Idle);
        s.link(a.clone(), 1);
        s.link(b.clone(), 2);
        // Same name, separate links.
        assert_eq!(s.linked_tab(&a), Some(1));
        assert_eq!(s.linked_tab(&b), Some(2));
        // Renaming tab 1 or 2 edits that favorite (not some row that isn't drawn).
        assert_eq!(s.favorite_for_tab(1), Some(a.clone()));
        assert_eq!(s.favorite_for_tab(2), Some(b.clone()));
        assert_eq!(s.favorite_for_tab(9), None);
        s.unlink(1, true);
        assert_eq!(s.favorite_for_tab(1), None);
        assert_eq!(s.status(&a), FavStatus::Exited);
        s.unlink(2, false);
        assert_eq!(s.status(&b), FavStatus::Idle);
        // Opening again clears the exited mark.
        s.link(a.clone(), 3);
        assert_eq!(s.status(&a), FavStatus::Running);
    }

    #[test]
    fn a_hand_edited_multiline_command_is_never_typed() {
        let (root, mut s) = state("typed");
        std::fs::create_dir(root.join("a")).unwrap();
        let path = root.join("a").display().to_string().replace('\\', "\\\\");
        let text = format!(
            "version = 1\n[[favorite]]\nid = \"f1\"\nname = \"a\"\npath = \"{path}\"\n\
             command = \"make\\nrm -rf ~\"\n"
        );
        std::fs::write(s.store_path().unwrap(), text).unwrap();
        let list = s.store.as_ref().unwrap().load().unwrap();
        assert_eq!(list.problems.len(), 1);
        s.replace(list);
        let fav = s.list.get("f1").unwrap();
        assert_eq!(fav.command, None);
        // Nothing to type, so the shell only ever gets what the user types.
        let mut recipe = LaunchRecipe::new(test_profile(), Some(fav.path.clone()));
        recipe.command = fav.command.clone();
        assert_eq!(recipe.typed_input(), None);
    }

    fn test_profile() -> crate::terminal::profiles::Profile {
        crate::terminal::profiles::Profile {
            name: "sh".into(),
            command: "/bin/sh".into(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            icon: "$".into(),
            color: None,
        }
    }

    #[test]
    fn favorite_rows_take_their_tab_and_its_split() {
        let fav = |id: &str| Favorite {
            id: id.into(),
            name: "same".into(),
            path: "/x".into(),
            profile: None,
            command: None,
        };
        let favs = [fav("f1"), fav("f2"), fav("f3")];
        // Tabs 1..=6. f1 is tab 2, split with tabs 3 and 5 (view 0). f2 is tab 4, alone
        // (view 1). f3 has no tab. Tabs 1 and 6 are plain tabs in views 2 and 3.
        let link = |id: &str| match id {
            "f1" => Some(2),
            "f2" => Some(4),
            _ => None,
        };
        let view_of = |t: TabId| {
            Some(match t {
                2 | 3 | 5 => 0,
                4 => 1,
                1 => 2,
                _ => 3,
            })
        };
        let groups = group_favorite_tabs(&favs, link, &[1, 2, 3, 4, 5, 6], view_of);
        assert_eq!(
            groups,
            [
                FavoriteTabs {
                    id: "f1".into(),
                    tab: 2,
                    siblings: vec![3, 5]
                },
                FavoriteTabs {
                    id: "f2".into(),
                    tab: 4,
                    siblings: vec![]
                },
            ]
        );
        // Two favorites' tabs in one split: neither lists the other, and a plain tab in
        // that split goes to the first one only.
        let view_of = |t: TabId| Some(if t == 6 { 1 } else { 0 });
        let groups = group_favorite_tabs(&favs, link, &[2, 3, 4, 6], view_of);
        assert_eq!(groups[0].siblings, vec![3]);
        assert_eq!(groups[1].siblings, Vec::<TabId>::new());
    }

    #[test]
    fn a_split_into_a_favorite_is_found_from_any_of_its_tabs() {
        let groups = [
            FavoriteTabs {
                id: "f1".into(),
                tab: 2,
                siblings: vec![3, 5],
            },
            FavoriteTabs {
                id: "f2".into(),
                tab: 4,
                siblings: vec![],
            },
        ];
        for t in [2, 3, 5, 4] {
            assert!(in_favorite_split(&groups, t), "{t}");
        }
        for t in [1, 6] {
            assert!(!in_favorite_split(&groups, t), "{t}");
        }
        assert!(!in_favorite_split(&[], 2));
    }

    #[test]
    fn reload_keeps_surviving_links_and_rechecks_paths() {
        let (root, mut s) = state("reload");
        let a = add(&mut s, &root, "a");
        let b = add(&mut s, &root, "b");
        s.link(a.clone(), 1);
        s.link(b.clone(), 2);
        // The CLI removes `a` and the directory of `b` disappears.
        let other = Store::new(s.store_path().unwrap().to_path_buf());
        other.update(|f| f.remove(0)).unwrap();
        std::fs::remove_dir(root.join("b")).unwrap();
        let list = other.load().unwrap();
        s.replace(list);
        assert_eq!(s.linked_tab(&a), None);
        assert_eq!(s.linked_tab(&b), Some(2));
        assert!(s.is_missing(&b));
        std::fs::create_dir(root.join("b")).unwrap();
        assert!(s.check(&b));
        assert!(!s.is_missing(&b));
    }
}
