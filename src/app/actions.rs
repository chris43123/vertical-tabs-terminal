//! App actions bound to shortcuts or picked in the command palette.

use alacritty_terminal::grid::Scroll;
use eframe::egui::{self, Key, Modifiers};

use crate::config::keybinds::Action;
use crate::workspace::Edge;

use super::App;
use super::tab::{MAX_FONT, MIN_FONT};

impl App {
    /// App-level shortcuts. Returns true if the key was consumed.
    pub(super) fn handle_shortcut(
        &mut self,
        key: Key,
        physical: Option<Key>,
        m: Modifiers,
    ) -> bool {
        match self.keybinds.lookup(key, physical, m) {
            Some(action) => self.run_action(action),
            None => false,
        }
    }

    /// Perform an app action. Returns false if it didn't apply, so the key should reach the shell.
    pub fn run_action(&mut self, action: Action) -> bool {
        match action {
            Action::NewTab => {
                self.new_tab(0, None);
            }
            Action::CloseTab => {
                if let Some(f) = self.ws.focused() {
                    self.close_tab(f);
                }
            }
            Action::SplitRight => {
                self.new_tab(0, Some(Edge::Right));
            }
            Action::SplitDown => {
                self.new_tab(0, Some(Edge::Bottom));
            }
            Action::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.sidebar_peek = false;
                self.side_hidden = false;
            }
            Action::ToggleSideArea => {
                self.side_hidden = !self.side_hidden;
                self.sidebar_peek = false;
            }
            Action::ReopenClosedTab => self.reopen_closed_tab(),
            Action::DuplicateTab => {
                if let Some(f) = self.ws.focused() {
                    self.duplicate_tab(f);
                }
            }
            Action::RenameTab => {
                if let Some(f) = self.ws.focused() {
                    self.start_rename(f);
                }
            }
            Action::MinimizePane => {
                if let Some(f) = self.ws.focused() {
                    self.ws.minimize(f);
                }
            }
            Action::NextTab | Action::PrevTab => {
                // With a single tab, let the key through to the shell.
                if self.ws.order.len() < 2 {
                    return false;
                }
                self.ws.cycle(action == Action::NextTab);
            }
            Action::MoveTabUp | Action::MoveTabDown => {
                if let Some(f) = self.ws.focused() {
                    self.ws.move_tab(f, action == Action::MoveTabDown);
                }
            }
            Action::GotoTab(n) => {
                if let Some(&id) = self.ws.order.get(n as usize - 1) {
                    self.ws.activate(id);
                }
            }
            Action::LastTab => {
                if let Some(&id) = self.ws.order.last() {
                    self.ws.activate(id);
                }
            }
            Action::NextActivity => self.next_activity(),
            Action::CommandPalette => self.switcher = Some(crate::ui::Switcher::default()),
            Action::ShowHelp => self.help = Some(crate::ui::Help::default()),
            Action::OpenSettings => self.open_settings(),
            Action::ToggleFiles => {
                if self.side_hidden {
                    self.zen_toggle_files();
                } else {
                    self.files_open = !self.files_open;
                }
            }
            Action::SearchFiles => {
                if self.side_hidden {
                    self.zen_show(true);
                } else {
                    self.files_open = true;
                }
                self.focus_search = true;
            }
            Action::NewGroup => {
                if let Some(f) = self.ws.focused() {
                    self.new_group(f);
                }
            }
            Action::FocusLeft | Action::FocusRight | Action::FocusUp | Action::FocusDown => {
                let dir = match action {
                    Action::FocusLeft => egui::vec2(-1.0, 0.0),
                    Action::FocusRight => egui::vec2(1.0, 0.0),
                    Action::FocusUp => egui::vec2(0.0, -1.0),
                    _ => egui::vec2(0.0, 1.0),
                };
                // With no pane in that direction, let the key through (shells may use it, e.g. Alt+Arrow for word movement).
                if !self.focus_neighbor(dir) {
                    return false;
                }
            }
            Action::ZoomIn | Action::ZoomOut | Action::ZoomReset => {
                let base = self.font_size;
                if let Some(tab) = self.ws.focused().and_then(|f| self.tabs.get_mut(&f)) {
                    tab.zoom = match action {
                        Action::ZoomIn => (tab.zoom + 1.0).min(MAX_FONT - base),
                        Action::ZoomOut => (tab.zoom - 1.0).max(MIN_FONT - base),
                        _ => 0.0,
                    };
                }
            }
            Action::UiZoomIn => self.set_ui_zoom(self.ui_zoom * 1.1),
            Action::UiZoomOut => self.set_ui_zoom(self.ui_zoom / 1.1),
            Action::UiZoomReset => self.set_ui_zoom(self.config.ui_scale),
            Action::ScrollPageUp | Action::ScrollPageDown => {
                if let Some(session) = self.focused_session() {
                    let scroll = if action == Action::ScrollPageUp {
                        Scroll::PageUp
                    } else {
                        Scroll::PageDown
                    };
                    session.term.lock().scroll_display(scroll);
                }
            }
        }
        self.mark_seen();
        self.scroll_to_focused = true;
        true
    }

    /// Move focus to the closest pane in direction `dir`. Returns false if there is none.
    fn focus_neighbor(&mut self, dir: egui::Vec2) -> bool {
        let Some(cur) = self.ws.focused() else {
            return false;
        };
        let Some(&(_, from)) = self.pane_rects.iter().find(|(t, _)| *t == cur) else {
            return false;
        };
        let best = self
            .pane_rects
            .iter()
            .filter(|(t, _)| *t != cur)
            .filter_map(|&(t, r)| {
                let d = r.center() - from.center();
                let along = d.dot(dir);
                (along > 0.0).then(|| (t, along + (d - dir * along).length() * 2.0))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((t, _)) => {
                self.activate(t);
                true
            }
            None => false,
        }
    }
}
