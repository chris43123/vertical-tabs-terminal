//! The files panel and zen overlay: following the focused shell's cwd and acting on paths
//! (`cd`, typing them into the shell).

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::files::FileTree;
use crate::files::git;
use crate::files::search::Search;
use crate::workspace::TabId;

use super::{App, Tab};

/// The files panel's state: the tree, which folder it follows, git status and the search box.
#[derive(Default)]
pub struct FilesPanel {
    pub tree: FileTree,
    /// The cwd the tree last followed; the user may browse elsewhere until it changes.
    followed: Option<PathBuf>,
    /// Git state of the repository the tree shows.
    pub git: git::Watcher,
    pub search: Search,
    /// The search box had keyboard focus last frame (so it keeps it).
    pub search_focused: bool,
    /// Give the search box keyboard focus next frame.
    pub focus_search: bool,
}

impl FilesPanel {
    /// Show `cwd` if the followed shell moved there (or nothing is shown yet). Otherwise
    /// the tree stays wherever the user browsed to.
    fn follow(&mut self, cwd: PathBuf) {
        if self.followed.as_ref() != Some(&cwd) || self.tree.root().is_none() {
            self.tree.set_root(cwd.clone());
            self.followed = Some(cwd);
        }
    }
}

impl App {
    /// Zen mode: show the overlay with the files panel (`files`) or the tabs, right away and
    /// until the pointer has visited it and left again.
    pub(crate) fn zen_show(&mut self, files: bool) {
        self.side.zen_files = files;
        self.side.zen_peek = true;
        self.side.zen_hovered_once = false;
        if files {
            self.follow_cwd();
        }
    }

    /// Ctrl+\ in zen mode: switch the overlay between tabs and files.
    pub(crate) fn zen_toggle_files(&mut self) {
        self.zen_show(!(self.side.zen_peek && self.side.zen_files));
    }

    /// Point the files panel at the focused shell's cwd when it changes (after `cd`).
    /// Previews don't move the tree, so clicking through files keeps your place.
    pub(super) fn follow_cwd(&mut self) {
        let Some(id) = self.ws.focused() else { return };
        let Some(tab) = self.tabs.get_mut(&id) else {
            return;
        };
        let Some(session) = tab.session() else {
            return;
        };
        if let Some(cwd) = session.proc_info().cwd {
            tab.cwd = Some(cwd);
        }
        if let Some(cwd) = tab.cwd.clone() {
            self.files.follow(cwd);
        }
    }

    /// Browse the files panel somewhere else (until the shell's cwd changes again).
    pub fn browse_files(&mut self, dir: PathBuf) {
        self.files.tree.set_root(dir);
    }

    /// Before drawing the panel: follow the focused shell if focus moved to another tab or its
    /// last polled cwd changed, and point git status at the shown folder.
    pub(crate) fn sync_files_root(&mut self, ctx: &egui::Context) {
        let focused = self.ws.focused().and_then(|f| self.tabs.get(&f));
        match focused
            .filter(|t| t.session().is_some())
            .and_then(|t| t.cwd.clone())
        {
            Some(cwd) => self.files.follow(cwd),
            None if self.files.tree.root().is_none() => {
                // Fresh tab without a polled cwd yet, or a preview: ask directly.
                if let Some(dir) = focused
                    .and_then(Tab::dir)
                    .map(Path::to_path_buf)
                    .or_else(dirs::home_dir)
                {
                    self.files.tree.set_root(dir);
                }
            }
            None => {}
        }
        if let Some(root) = self.files.tree.root().map(Path::to_path_buf) {
            self.files.git.set_dir(&root, ctx);
        }
    }

    /// The terminal that file actions type into: the focused pane, or else another terminal
    /// pane in the active view (when a preview has focus).
    fn target_terminal(&self) -> Option<TabId> {
        let focused = self.ws.focused()?;
        std::iter::once(focused)
            .chain(self.ws.visible())
            .find(|id| self.tabs.get(id).is_some_and(|t| t.session().is_some()))
    }

    /// `cd` the focused shell into `dir`. If a program is running in it, open a new tab there
    /// instead of typing into that program.
    pub fn cd_focused(&mut self, dir: &Path) {
        let idle = self.target_terminal().and_then(|id| {
            let session = self.tabs.get(&id)?.session()?;
            session.shell_idle().then(|| (id, session.proc_info().cwd))
        });
        match idle {
            Some((id, cwd)) => {
                self.activate(id);
                // Below the shell's cwd, type the short relative form (`cd src/ui`).
                let target = cwd
                    .and_then(|cwd| dir.strip_prefix(cwd).ok().map(Path::to_path_buf))
                    .filter(|rel| !rel.as_os_str().is_empty())
                    .unwrap_or_else(|| dir.to_path_buf());
                let line = format!("cd {}\r", crate::terminal::shell_quote(&target));
                self.send_input(line.into_bytes());
            }
            None => self.new_tab_in(dir.to_path_buf()),
        }
    }

    /// Type a (quoted) path into the terminal, as if pasted.
    pub fn insert_path(&mut self, path: &Path) {
        if let Some(id) = self.target_terminal() {
            self.activate(id);
            self.paste(&format!("{} ", crate::terminal::shell_quote(path)));
        }
    }
}
