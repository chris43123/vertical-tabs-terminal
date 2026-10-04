//! The files panel and zen overlay: following the focused shell's cwd and acting on paths
//! (`cd`, typing them into the shell).

use std::path::{Path, PathBuf};

use crate::workspace::TabId;

use super::App;

impl App {
    /// The files tree is on screen: as the side panel, or as the zen overlay.
    pub(super) fn files_visible(&self) -> bool {
        if self.side_hidden {
            self.zen_peek && self.zen_files
        } else {
            self.files_open
        }
    }

    /// Zen mode: show the overlay with the files panel (`files`) or the tabs, right away and
    /// until the pointer has visited it and left again.
    pub(crate) fn zen_show(&mut self, files: bool) {
        self.zen_files = files;
        self.zen_peek = true;
        self.zen_hovered_once = false;
        if files {
            self.follow_cwd();
        }
    }

    /// Ctrl+\ in zen mode: switch the overlay between tabs and files.
    pub(crate) fn zen_toggle_files(&mut self) {
        self.zen_show(!(self.zen_peek && self.zen_files));
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
        let Some(cwd) = tab.cwd.clone() else { return };
        if self.files_followed.as_ref() != Some(&cwd) || self.files.root().is_none() {
            self.files.set_root(cwd.clone());
            self.files_followed = Some(cwd);
        }
    }

    /// Browse the files panel somewhere else (until the shell's cwd changes again).
    pub fn browse_files(&mut self, dir: PathBuf) {
        self.files.set_root(dir);
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
