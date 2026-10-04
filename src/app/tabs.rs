//! Tab lifecycle: opening, closing, reopening, duplicating, renaming and grouping tabs, and
//! moving focus between them.

use std::path::{Path, PathBuf};

use crate::terminal::profiles::Profile;
use crate::terminal::session::{GridSize, Listener, Session};
use crate::workspace::{Drop, Edge, GroupId, TabId};

use super::App;
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
        self.spawn_tab(profile, cwd, split)
    }

    pub fn new_tab_in(&mut self, dir: PathBuf) {
        if let Some(profile) = self.profiles.first().cloned() {
            self.spawn_tab(profile, Some(dir), None);
        }
    }

    /// Fresh directory of tab `id` (re-queried for a shell).
    fn focused_dir(&mut self, id: TabId) -> Option<PathBuf> {
        let tab = self.tabs.get_mut(&id)?;
        if let Some(session) = tab.session() {
            let cwd = session.proc_info().cwd;
            if cwd.is_some() {
                tab.cwd = cwd;
            }
        }
        tab.dir().map(Path::to_path_buf)
    }

    pub(super) fn spawn_tab(
        &mut self,
        profile: Profile,
        cwd: Option<PathBuf>,
        split: Option<Edge>,
    ) -> Option<TabId> {
        let id = self.next_id;
        self.next_id += 1;
        let focused = self.ws.focused();

        // Real size is applied on the first layout pass.
        let size = GridSize {
            cols: 80,
            lines: 24,
        };
        let session = match Session::spawn(
            &profile,
            cwd,
            crate::terminal::session::term_config(self.config.scrollback, &self.config.cursor),
            size,
            self.fonts.cell_px(),
            Listener::new(id, self.tx.clone(), self.ctx.clone()),
        ) {
            Ok(s) => s,
            Err(err) => {
                crate::diag::warn(format!("failed to start {}: {err}", profile.command));
                return None;
            }
        };
        self.tabs
            .insert(id, Tab::new(Content::Term(session), profile));
        self.ws.add(id, focused);
        if let (Some(edge), Some(target)) = (split, focused) {
            self.ws.drop_on(id, target, Drop::Edge(edge));
        }
        self.scroll_to_focused = true;
        Some(id)
    }

    pub fn close_tab(&mut self, id: TabId) {
        if let Some(tab) = self.tabs.get(&id) {
            self.closed.push(ClosedTab {
                profile: tab.profile.clone(),
                cwd: tab
                    .session()
                    .and_then(|s| s.proc_info().cwd)
                    .or(tab.cwd.clone()),
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
        if self
            .renaming
            .as_ref()
            .is_some_and(|(r, _)| *r == RenameTarget::Tab(id))
        {
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
        let id = match closed.preview {
            Some(path) => self.spawn_preview(path, None),
            None => self.spawn_tab(closed.profile, closed.cwd, None),
        };
        if let Some(id) = id {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.custom_title = closed.custom_title;
            }
            self.ws.reorder(id, closed.index);
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
    pub fn start_rename(&mut self, id: TabId) {
        let current = self
            .tabs
            .get(&id)
            .map(|t| t.title().to_string())
            .unwrap_or_default();
        self.renaming = Some((RenameTarget::Tab(id), current));
        self.scroll_to_focused = true;
        self.side_hidden = false;
        if self.sidebar_collapsed {
            self.sidebar_peek = true;
        }
    }

    /// Put tab `id` into a new folder and start naming it.
    pub fn new_group(&mut self, id: TabId) {
        let name = format!("Group {}", self.ws.groups.len() + 1);
        if let Some(g) = self.ws.new_group(id, name.clone()) {
            self.renaming = Some((RenameTarget::Group(g), name));
            self.side_hidden = false;
            if self.sidebar_collapsed {
                self.sidebar_peek = true;
            }
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
