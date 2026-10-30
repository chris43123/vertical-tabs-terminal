//! Tab lifecycle: opening, closing, reopening, duplicating, renaming and grouping tabs, and
//! moving focus between them.

use std::path::{Path, PathBuf};

use crate::terminal::session::{GridSize, Listener, Session, Source};
use crate::ui::view_names::{ViewDirs, auto_names};
use crate::workspace::{Drop, Edge, GroupId, TabId};

use super::App;
use super::recipe::LaunchRecipe;
use super::tab::{ClosedTab, Content, MAX_CLOSED, RenameTarget, Tab};

impl App {
    /// Open a tab with profile `profile_idx` in the focused tab's directory. Returns its id.
    /// `split`: split the focused pane of the active view on that edge instead of opening a standalone tab.
    pub fn new_tab(&mut self, profile_idx: usize, split: Option<Edge>) -> Option<TabId> {
        let profile = self
            .profiles
            .get(profile_idx)
            .or(self.profiles.first())?
            .clone();
        // Start in the focused tab's directory, like most terminals do.
        let cwd = self.ws.focused().and_then(|f| self.focused_dir(f));
        self.spawn_tab(LaunchRecipe::new(profile, cwd), split)
    }

    pub fn new_tab_in(&mut self, dir: PathBuf) {
        if let Some(profile) = self.profiles.first().cloned() {
            self.spawn_tab(LaunchRecipe::new(profile, Some(dir)), None);
        }
    }

    /// Fresh directory of tab `id` (re-queried for a shell).
    pub(super) fn focused_dir(&mut self, id: TabId) -> Option<PathBuf> {
        let tab = self.tabs.get_mut(&id)?;
        if let Some(session) = tab.session() {
            let cwd = session.proc_info().cwd;
            if cwd.is_some() {
                tab.cwd = cwd;
            }
        }
        tab.dir().map(Path::to_path_buf)
    }

    /// Open a terminal tab from `recipe`. Returns its id.
    pub(super) fn spawn_tab(&mut self, recipe: LaunchRecipe, split: Option<Edge>) -> Option<TabId> {
        let id = self.next_id;
        // Real size is applied on the first layout pass.
        let size = GridSize {
            cols: 80,
            lines: 24,
        };
        let session = self.start_session(id, &recipe, size)?;
        self.next_id += 1;
        let focused = self.ws.focused();
        let mut tab = Tab::new(Content::Term(session), recipe.profile.clone());
        tab.custom_title = recipe.custom_title.clone();
        tab.pending_input = recipe.typed_input();
        self.link_favorite(&recipe, id);
        tab.recipe = Some(recipe);
        self.tabs.insert(id, tab);
        self.ws.add(id, focused);
        if let (Some(edge), Some(target)) = (split, focused) {
            self.ws.drop_on(id, target, Drop::Edge(edge));
        }
        self.scroll_to_focused = true;
        Some(id)
    }

    /// Start `recipe`'s shell for tab `id`, with a new generation.
    fn start_session(
        &mut self,
        id: TabId,
        recipe: &LaunchRecipe,
        size: GridSize,
    ) -> Option<Session> {
        let source = Source {
            tab: id,
            generation: self.next_generation,
        };
        self.next_generation += 1;
        match Session::spawn(
            &recipe.profile,
            recipe.dir.clone(),
            crate::terminal::session::term_config(self.config.scrollback, &self.config.cursor),
            size,
            self.fonts.cell_px(),
            Listener::new(source, self.tx.clone(), self.ctx.clone()),
        ) {
            Ok(s) => Some(s),
            Err(err) => {
                crate::diag::warn(format!("failed to start {}: {err}", recipe.profile.command));
                None
            }
        }
    }

    /// Reset tab `id`: ask first if something other than the idle shell is running (and
    /// `confirm_reset` is on), else [`Self::reset_tab_now`]. Previews can't be reset.
    pub fn reset_tab(&mut self, id: TabId) {
        let Some(tab) = self.tabs.get(&id) else {
            return;
        };
        let Some(session) = tab.session().filter(|_| tab.recipe.is_some()) else {
            return;
        };
        if self.config.confirm_reset && !session.shell_idle() {
            let process = session
                .proc_info()
                .process
                .or_else(|| tab.process.clone())
                .unwrap_or_else(|| "A program".into());
            self.reset_prompt = Some(crate::ui::ResetPrompt::new(
                id,
                tab.title().to_string(),
                process,
            ));
            return;
        }
        self.reset_tab_now(id);
    }

    /// Start tab `id`'s recipe again in place and kill everything that ran in it: same id, so
    /// its position, split, folder, custom title and zoom stay. The old session's late events
    /// carry an old generation and are dropped, so its exit doesn't close the tab.
    pub fn reset_tab_now(&mut self, id: TabId) {
        let Some(tab) = self.tabs.get(&id) else {
            return;
        };
        let (Some(recipe), Some(size)) = (tab.recipe.clone(), tab.session().map(|s| s.size)) else {
            return;
        };
        let recipe = self.current_recipe(recipe);
        // Start the new shell first: if that fails (reported in the banner), the tab and its
        // old session stay as they were.
        let Some(session) = self.start_session(id, &recipe, size) else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(&id) else {
            return;
        };
        if let Some(old) = tab.session() {
            old.kill_tree();
        }
        // Dropping the old session stops its IO thread and hangs up the PTY.
        tab.content = Content::Term(session);
        tab.osc_title = None;
        tab.auto_title = None;
        tab.process = None;
        tab.activity = false;
        tab.bell = false;
        tab.bell_flash = None;
        tab.cwd = recipe.dir.clone();
        tab.pending_input = recipe.typed_input();
        tab.profile = recipe.profile.clone();
        tab.recipe = Some(recipe);
        if self.reset_prompt.as_ref().is_some_and(|p| p.tab == id) {
            self.reset_prompt = None;
        }
    }

    pub fn close_tab(&mut self, id: TabId) {
        if let Some(tab) = self.tabs.get(&id) {
            // Reopen where the shell was last, not where it started.
            let cwd = tab
                .session()
                .and_then(|s| s.proc_info().cwd)
                .or(tab.cwd.clone());
            self.closed.push(ClosedTab {
                recipe: tab.recipe.clone().map(|r| LaunchRecipe {
                    dir: cwd.or(r.dir),
                    custom_title: tab.custom_title.clone(),
                    ..r
                }),
                custom_title: tab.custom_title.clone(),
                index: self.ws.order.iter().position(|t| *t == id).unwrap_or(0),
                preview: tab.preview().map(|p| p.path.clone()),
            });
            if self.closed.len() > MAX_CLOSED {
                self.closed.remove(0);
            }
        }
        self.ws.close(id);
        self.tabs.remove(&id);
        self.unlink_favorite(id, false);
        if self.reset_prompt.as_ref().is_some_and(|p| p.tab == id) {
            self.reset_prompt = None;
        }
        if self.renaming.as_ref().is_some_and(
            |(r, _)| matches!(*r, RenameTarget::Tab(t) | RenameTarget::View(t) if t == id),
        ) {
            self.renaming = None;
        }
        self.scroll_to_focused = true;
    }

    /// Reopen the most recently closed tab: same profile, directory, name and sidebar position.
    /// (The old process is gone, so this starts a fresh shell.)
    pub fn reopen_closed_tab(&mut self) {
        let Some(closed) = self.closed.pop() else {
            return;
        };
        let id = match (closed.preview, closed.recipe) {
            (Some(path), _) => self.spawn_preview(path, None),
            (None, Some(recipe)) => self.spawn_tab(recipe, None),
            (None, None) => None,
        };
        if let Some(id) = id {
            // A terminal got its title from the recipe; a preview has none.
            if let Some(tab) = self.tabs.get_mut(&id).filter(|t| t.recipe.is_none()) {
                tab.custom_title = closed.custom_title;
            }
            self.ws.reorder(id, closed.index);
            // A favorite's tab came back (and relinked): its row leads, outside any folder.
            if self.favorites.favorite_for_tab(id).is_some() {
                self.ws.set_group(id, None);
                self.sync_favorite_order();
            }
        }
    }

    pub fn duplicate_tab(&mut self, id: TabId) {
        let Some(tab) = self.tabs.get(&id) else {
            return;
        };
        if let Some(p) = tab.preview() {
            let path = p.path.clone();
            self.spawn_preview(path, None);
            return;
        }
        let idx = self
            .profiles
            .iter()
            .position(|p| p.name == tab.profile.name)
            .unwrap_or(0);
        self.ws.activate(id);
        self.new_tab(idx, None);
    }

    /// Start renaming a tab in the sidebar (peeking it open if collapsed).
    /// A favorite's own tab has no row of its own to edit in: its name is the favorite's, so
    /// this opens the favorite's edit dialog instead.
    pub fn start_rename(&mut self, id: TabId) {
        if let Some(fav) = self.favorites.favorite_for_tab(id) {
            self.start_edit_favorite(&fav);
            return;
        }
        let current = self
            .tabs
            .get(&id)
            .map(|t| t.title().to_string())
            .unwrap_or_default();
        self.renaming = Some((RenameTarget::Tab(id), current));
        self.scroll_to_focused = true;
        self.side.reveal_sidebar();
    }

    /// Recompute the automatic names of the split views.
    pub fn refresh_view_names(&mut self) {
        let views: Vec<ViewDirs> = self
            .ws
            .views
            .iter()
            .map(|v| {
                let leaves = v.root.leaves();
                ViewDirs {
                    seed: leaves.iter().copied().min().unwrap_or(0),
                    dirs: leaves
                        .iter()
                        .filter_map(|t| self.tabs.get(t)?.dir())
                        .collect(),
                    focused: self.tabs.get(&v.focused).and_then(|t| t.dir()),
                }
            })
            .collect();
        self.view_names = auto_names(&views, dirs::home_dir().as_deref());
    }

    /// What the sidebar calls view `v`: the user's name, the automatic one, or the focused
    /// tab's title when no directory is known.
    pub fn view_name(&self, v: usize) -> String {
        let Some(view) = self.ws.views.get(v) else {
            return String::new();
        };
        view.name
            .clone()
            .or_else(|| Some(self.view_names.get(v)?.as_ref()?.name.clone()))
            .or_else(|| Some(self.tabs.get(&view.focused)?.title().to_string()))
            .unwrap_or_default()
    }

    /// Start renaming the split view holding `id`.
    /// A split under a favorite is named by the favorite: that opens its edit dialog.
    pub fn start_view_rename(&mut self, id: TabId) {
        if let Some(g) = self
            .favorite_tabs()
            .into_iter()
            .find(|g| g.tab == id || g.siblings.contains(&id))
        {
            self.start_edit_favorite(&g.id);
            return;
        }
        let Some(v) = self.ws.view_of(id) else {
            return;
        };
        self.renaming = Some((RenameTarget::View(id), self.view_name(v)));
        self.side.reveal_sidebar();
    }

    /// Put tab `id` into a new folder and start naming it.
    pub fn new_group(&mut self, id: TabId) {
        let name = format!("Group {}", self.ws.groups.len() + 1);
        if let Some(g) = self.ws.new_group(id, name.clone()) {
            self.renaming = Some((RenameTarget::Group(g), name));
            self.side.reveal_sidebar();
        }
    }

    /// Close every tab in a folder.
    pub fn close_group(&mut self, group: GroupId) {
        for id in self.ws.members(group) {
            self.close_tab(id);
        }
    }

    /// Show a tab and clear its unread markers.
    pub fn activate(&mut self, id: TabId) {
        self.ws.activate(id);
        self.mark_seen();
    }

    pub(super) fn mark_seen(&mut self) {
        for id in self.ws.visible() {
            if let Some(t) = self.tabs.get_mut(&id) {
                t.activity = false;
                if Some(id) == self.ws.focused() {
                    t.bell = false;
                }
            }
        }
    }

    /// Jump to the next tab (after the focused one, wrapping) that has a bell or unread output.
    pub(super) fn next_activity(&mut self) {
        let n = self.ws.order.len();
        let start = self
            .ws
            .focused()
            .and_then(|f| self.ws.order.iter().position(|t| *t == f))
            .unwrap_or(0);
        let next = (1..=n)
            .map(|i| self.ws.order[(start + i) % n])
            .find(|id| self.tabs.get(id).is_some_and(|t| t.bell || t.activity));
        if let Some(id) = next {
            self.activate(id);
        }
    }
}
