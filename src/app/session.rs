//! Session restore in the app: bringing the saved tabs back at launch (dormant until shown),
//! waking them, and saving the session a moment after it changes and on quit.
//!
//! Changes are found by comparing the session as it stands with the one last written, once
//! per frame that runs anyway; whatever path changed the layout, a cwd or a link, it shows up
//! there. A differing session schedules one save [`DEBOUNCE`] later; nothing is scheduled
//! while nothing changes.

use std::time::{Duration, Instant};

use crate::session::{Session, Store, TabInfo, snapshot};
use crate::terminal::procinfo::{self, ProcInfo};
use crate::terminal::session::GridSize;
use crate::workspace::TabId;

use super::recipe::LaunchRecipe;
use super::tab::{Content, Tab};
use super::{App, FavStatus};

/// How long after a change the session is written (so a burst of changes is one write).
const DEBOUNCE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub struct SessionState {
    store: Option<Store>,
    /// The session as last written or restored.
    saved: Session,
    /// When the session first differed from `saved`; a save is due [`DEBOUNCE`] after it.
    dirty_since: Option<Instant>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            store: Store::default_path().map(Store::new),
            ..Self::default()
        }
    }
}

impl App {
    /// Bring back the saved session (with `restore_session`). Every tab comes back dormant
    /// except the panes on screen. Returns false when there was nothing to restore.
    pub(super) fn restore_session(&mut self) -> bool {
        if !self.config.restore_session {
            return false;
        }
        let Some(store) = self.session.store.clone() else {
            return false;
        };
        let (saved, warning) = store.load_for_restore();
        if let Some(w) = warning {
            crate::diag::warn(w);
        }
        if saved.is_empty() {
            return false;
        }
        let first = self.next_id;
        let restored = saved.rebuild(
            first,
            &self.profiles,
            &|id| self.favorites.list.get(id).is_some(),
            dirs::home_dir().as_deref(),
        );
        for w in restored.warnings {
            crate::diag::warn(w);
        }
        self.next_id = first + saved.tabs.len() as TabId;
        for t in restored.tabs {
            let recipe = LaunchRecipe {
                profile: t.profile,
                dir: t.dir,
                command: t.command,
                custom_title: t.title,
                favorite: t.favorite,
                ephemeral: false,
            };
            let mut tab = Tab::new(Content::Dormant, recipe.profile.clone());
            tab.custom_title = recipe.custom_title.clone();
            tab.cwd = recipe.dir.clone();
            tab.auto_title = procinfo::format_title(&ProcInfo {
                process: None,
                cwd: recipe.dir.clone(),
            });
            self.link_favorite(&recipe, t.id);
            tab.recipe = Some(recipe);
            self.tabs.insert(t.id, tab);
        }
        self.ws = restored.ws;
        self.wake_visible();
        self.session.saved = self.session_snapshot();
        self.scroll_to_focused = true;
        !self.tabs.is_empty()
    }

    /// Start the shells of the dormant tabs on screen (however they got there: a click, the
    /// switcher, next/previous tab, a favorite, a drop into a split).
    pub(super) fn wake_visible(&mut self) {
        for id in self.ws.visible() {
            if self.tabs.get(&id).is_some_and(Tab::is_dormant) {
                self.wake(id);
            }
        }
    }

    /// Start dormant tab `id`'s shell in place from its recipe: same id, so its position,
    /// split, folder, name and favorite link stay. Only the shell starts, in the saved
    /// directory: the saved command is not typed, so coming back never relaunches a
    /// long-running program on its own. It stays in the recipe for an explicit Reset. A shell
    /// that fails to start (reported in the banner) closes the tab, so it isn't retried every
    /// frame.
    pub(super) fn wake(&mut self, id: TabId) {
        let Some(recipe) = self
            .tabs
            .get(&id)
            .filter(|t| t.is_dormant())
            .and_then(|t| t.recipe.clone())
        else {
            return;
        };
        // Real size is applied on the first layout pass.
        let size = GridSize {
            cols: 80,
            lines: 24,
        };
        let Some(session) = self.start_session(id, &recipe, size) else {
            self.close_tab(id);
            return;
        };
        if let Some(tab) = self.tabs.get_mut(&id) {
            tab.content = super::Content::Term(session);
            tab.pending_input = None;
            tab.cwd = recipe.dir.clone();
        }
        self.ctx.request_repaint();
    }

    /// The favorite's status dot: a link to a dormant tab has no session running yet.
    pub fn favorite_status(&self, id: &str) -> FavStatus {
        match self.favorites.status(id) {
            FavStatus::Running
                if self
                    .favorites
                    .linked_tab(id)
                    .and_then(|t| self.tabs.get(&t))
                    .is_some_and(Tab::is_dormant) =>
            {
                FavStatus::Idle
            }
            status => status,
        }
    }

    /// The session as it stands: every terminal tab started from a recipe, with its last
    /// known directory.
    fn session_snapshot(&self) -> Session {
        snapshot(&self.ws, |id| {
            let tab = self.tabs.get(&id)?;
            let recipe = tab.recipe.as_ref().filter(|r| !r.ephemeral)?;
            Some(TabInfo {
                profile: recipe.profile.name.clone(),
                cwd: tab.cwd.clone(),
                recipe_dir: recipe.dir.clone(),
                command: recipe.command.clone(),
                title: tab.custom_title.clone(),
                favorite: self.favorites.favorite_for_tab(id),
            })
        })
    }

    /// Once a frame: note whether the session changed, and write it once it has been
    /// different for [`DEBOUNCE`], waking up for that then.
    pub(super) fn session_tick(&mut self) {
        if !self.config.restore_session || self.session.store.is_none() {
            self.session.dirty_since = None;
            return;
        }
        if self.session_snapshot() == self.session.saved {
            self.session.dirty_since = None;
            return;
        }
        let since = *self.session.dirty_since.get_or_insert_with(Instant::now);
        match DEBOUNCE.checked_sub(since.elapsed()) {
            Some(left) if !left.is_zero() => self.ctx.request_repaint_after(left),
            _ => self.save_session(),
        }
    }

    /// Write the session now if it changed (with `restore_session`). On quit, the cwds are
    /// read fresh first, since the poll only runs while the sidebar shows.
    pub(super) fn save_session(&mut self) {
        let Some(store) = self
            .session
            .store
            .as_ref()
            .filter(|_| self.config.restore_session)
        else {
            return;
        };
        let session = self.session_snapshot();
        self.session.dirty_since = None;
        if session == self.session.saved {
            return;
        }
        match store.save(&session) {
            Ok(()) => self.session.saved = session,
            // Reported once; the next change tries again.
            Err(e) => {
                crate::diag::warn(e);
                self.session.saved = session;
            }
        }
    }

    /// Before the last save: refresh every running shell's cwd.
    pub(super) fn refresh_cwds(&mut self) {
        for tab in self.tabs.values_mut() {
            if let Some(cwd) = tab.session().and_then(|s| s.proc_info().cwd) {
                tab.cwd = Some(cwd);
            }
        }
    }
}
