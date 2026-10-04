//! The vertical tab sidebar: tab rows, split groups, folders (Zen-style tab groups), badges,
//! rename, reorder, drag source, a long-hover card with the tab's cwd, and the collapsed icon
//! strip that peeks open on hover.

mod folder;
mod tab_row;
mod zen;

use eframe::egui::{self, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::app::{App, RenameTarget, TabDrag};
use crate::config::keybinds::Action as Shortcut;
use crate::workspace::{GroupId, TabId};

const COLLAPSED_WIDTH: f32 = 48.0;
const ROW_HEIGHT: f32 = 30.0;
const FOLDER_HEIGHT: f32 = 28.0;
/// How far a folder's tabs are indented.
const FOLDER_INDENT: f32 = 12.0;
/// ANSI colors offered for folders.
const FOLDER_COLORS: [(&str, u8); 6] = [
    ("Red", 1),
    ("Green", 2),
    ("Yellow", 3),
    ("Blue", 4),
    ("Magenta", 5),
    ("Cyan", 6),
];

enum Action {
    Activate(TabId),
    Close(TabId),
    Duplicate(TabId),
    Minimize(TabId),
    StartRename(RenameTarget),
    CommitRename,
    NewTab(usize),
    ToggleCollapse,
    /// Move a tab to an index in `order`, inside a folder or outside any.
    MoveTo(TabId, usize, Option<GroupId>),
    Move(TabId, bool),
    OpenSettings,
    NewGroup(TabId),
    SetGroup(TabId, Option<GroupId>),
    ToggleGroup(GroupId),
    GroupColor(GroupId, Option<u8>),
    Ungroup(GroupId),
    CloseGroup(GroupId),
    NewTabInGroup(GroupId),
    ToggleFiles,
    /// A file or folder from the files panel dropped on the tab list.
    OpenPath(std::path::PathBuf),
}

/// A line in the tab list.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Row {
    Folder(GroupId),
    Tab(TabId),
}

impl App {
    pub(crate) fn sidebar(&mut self, ui: &mut Ui) {
        let collapsed = self.side.collapsed;
        let width = if collapsed {
            COLLAPSED_WIDTH
        } else {
            self.config.sidebar_width
        };
        let fill = ui.visuals().panel_fill;
        let mut actions = Vec::new();

        let panel = egui::Panel::left("sidebar")
            .exact_size(width)
            .resizable(false)
            .frame(egui::Frame::new().fill(fill).inner_margin(6));
        let strip = panel
            .show(ui, |ui| self.sidebar_contents(ui, !collapsed, &mut actions))
            .response
            .rect;

        if collapsed {
            self.sidebar_peek_overlay(ui.ctx(), strip, &mut actions);
        } else {
            self.side.peek = false;
        }

        self.drag_ghost(ui.ctx());

        self.scroll_to_focused = false;
        for action in actions {
            self.apply(action);
        }
    }

    fn sidebar_contents(&mut self, ui: &mut Ui, expanded: bool, actions: &mut Vec<Action>) {
        ui.spacing_mut().item_spacing = vec2(4.0, 2.0);

        // Header: new tab, profile menu, then settings and the collapse toggle on the right.
        let settings_tip = format!(
            "Open settings{}",
            self.keybinds.hint(Shortcut::OpenSettings)
        );
        let new_tab_tip = format!("New tab{}", self.keybinds.hint(Shortcut::NewTab));
        let files_tip = format!("Files{}", self.keybinds.hint(Shortcut::ToggleFiles));
        let sidebar_hint = self.keybinds.hint(Shortcut::ToggleSidebar);
        let header = |ui: &mut Ui, actions: &mut Vec<Action>| {
            if ui.button(" + ").on_hover_text(&new_tab_tip).clicked() {
                actions.push(Action::NewTab(0));
            }
            ui.menu_button(" ⏷ ", |ui| {
                for (i, p) in self.profiles.iter().enumerate() {
                    if ui.button(format!("{}   {}", p.icon, p.name)).clicked() {
                        actions.push(Action::NewTab(i));
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("New tab with profile…");
        };
        if expanded {
            ui.horizontal(|ui| {
                header(ui, actions);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, tip) = if self.side.collapsed {
                        (" » ", format!("Pin sidebar open{sidebar_hint}"))
                    } else {
                        (" « ", format!("Collapse sidebar{sidebar_hint}"))
                    };
                    if ui.button(label).on_hover_text(tip).clicked() {
                        actions.push(Action::ToggleCollapse);
                    }
                    if ui.button(" ⚙ ").on_hover_text(&settings_tip).clicked() {
                        actions.push(Action::OpenSettings);
                    }
                    let files = ui
                        .selectable_label(self.side.files_open && !self.side.hidden, " 🗀 ")
                        .on_hover_text(&files_tip);
                    if files.clicked() {
                        actions.push(Action::ToggleFiles);
                    }
                });
            });
        } else {
            // The collapsed strip shows only tab icons; its header buttons live in the hover
            // peek. Reserve the header's height so icons line up with the peek's rows.
            ui.allocate_exact_size(
                vec2(ui.available_width(), ui.spacing().interact_size.y),
                Sense::hover(),
            );
        }
        ui.add_space(4.0);
        ui.separator();

        let dragged = egui::DragAndDrop::payload::<TabDrag>(ui.ctx()).map(|p| p.0);
        let rows = self.sidebar_rows();
        let mut row_rects: Vec<(Row, Rect)> = Vec::new();

        let scroll = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt(("tabs", expanded));
        let list_rect = scroll
            .show(ui, |ui| {
                let order = self.ws.order.clone();
                for &row in &rows {
                    let rect = match row {
                        Row::Folder(g) => self.folder_row(ui, g, expanded, actions),
                        Row::Tab(id) => {
                            let i = order.iter().position(|t| *t == id).unwrap_or(0);
                            self.tab_row(ui, id, i, &order, expanded, actions)
                        }
                    };
                    row_rects.push((row, rect));
                }
                // Blank space below the tabs: double-click opens a new tab.
                let rest = ui.available_rect_before_wrap();
                if rest.height() > 0.0 && ui.allocate_rect(rest, Sense::click()).double_clicked() {
                    actions.push(Action::NewTab(0));
                }
            })
            .inner_rect;

        // A file dragged from the files panel onto the list opens as a tab of its own.
        if let Some(file) = egui::DragAndDrop::payload::<crate::ui::FileDrag>(ui.ctx())
            && ui
                .ctx()
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|p| list_rect.contains(p))
        {
            ui.painter().rect_stroke(
                list_rect.shrink(2.0),
                CornerRadius::same(6),
                Stroke::new(2.0, self.chrome.accent),
                StrokeKind::Inside,
            );
            if ui.ctx().input(|i| i.pointer.any_released()) {
                actions.push(Action::OpenPath(file.0.clone()));
            }
        }

        // Reordering: dragging a tab over the list shows an insertion line, or highlights a
        // folder header (dropping there puts the tab in that folder).
        if let (Some(dragged), Some(pos)) = (dragged, ui.ctx().input(|i| i.pointer.hover_pos()))
            && list_rect.contains(pos)
            && !row_rects.is_empty()
        {
            let released = ui.ctx().input(|i| i.pointer.any_released());
            let over_folder = row_rects.iter().find_map(|&(row, r)| match row {
                Row::Folder(g) if r.shrink2(vec2(0.0, r.height() * 0.25)).contains(pos) => {
                    Some((g, r))
                }
                _ => None,
            });
            if let Some((g, r)) = over_folder {
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(6),
                    Stroke::new(2.0, self.folder_color(g)),
                    StrokeKind::Inside,
                );
                if released {
                    actions.push(Action::SetGroup(dragged, Some(g)));
                }
            } else {
                let idx = row_rects
                    .iter()
                    .position(|(_, r)| pos.y < r.center().y)
                    .unwrap_or(row_rects.len());
                let (to, group) = self.insertion_target(row_rects.get(idx).map(|(row, _)| *row));
                let y = row_rects
                    .get(idx)
                    .map(|(_, r)| r.top())
                    .unwrap_or_else(|| row_rects.last().unwrap().1.bottom());
                let x = list_rect.x_range();
                let (x, color) = match group {
                    Some(g) => (
                        egui::Rangef::new(x.min + FOLDER_INDENT, x.max),
                        self.folder_color(g),
                    ),
                    None => (x, self.chrome.accent),
                };
                ui.painter().hline(x, y, Stroke::new(2.0, color));
                if released {
                    actions.push(Action::MoveTo(dragged, to, group));
                }
            }
        }
    }

    /// Rows to show: folder headers, and tabs (a collapsed folder still shows its active tab).
    fn sidebar_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut current: Option<GroupId> = None;
        let active = self.ws.visible();
        for &id in &self.ws.order {
            let group = self.ws.group_of(id);
            if group != current {
                if let Some(g) = group {
                    rows.push(Row::Folder(g));
                }
                current = group;
            }
            let hidden = group
                .and_then(|g| self.ws.group(g))
                .is_some_and(|g| g.collapsed)
                && !active.contains(&id);
            if !hidden {
                rows.push(Row::Tab(id));
            }
        }
        rows
    }

    /// Where a drop just above `next` lands: an index into `order`, and the folder it joins.
    fn insertion_target(&self, next: Option<Row>) -> (usize, Option<GroupId>) {
        let index_of = |id: TabId| self.ws.order.iter().position(|t| *t == id).unwrap_or(0);
        match next {
            // Above a tab: join whatever folder that tab is in.
            Some(Row::Tab(t)) => (index_of(t), self.ws.group_of(t)),
            // Above a folder header: just before the folder, outside it.
            Some(Row::Folder(g)) => (self.ws.members(g).first().map_or(0, |&t| index_of(t)), None),
            None => (self.ws.order.len(), None),
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Activate(id) => self.activate(id),
            Action::Close(id) => self.close_tab(id),
            Action::Duplicate(id) => self.duplicate_tab(id),
            Action::Minimize(id) => self.ws.minimize(id),
            Action::StartRename(RenameTarget::Tab(id)) => self.start_rename(id),
            Action::StartRename(RenameTarget::Group(g)) => {
                if let Some(group) = self.ws.group(g) {
                    self.renaming = Some((RenameTarget::Group(g), group.name.clone()));
                }
            }
            Action::OpenSettings => self.open_settings(),
            Action::Move(id, down) => {
                self.ws.move_tab(id, down);
            }
            Action::CommitRename => match self.renaming.take() {
                Some((RenameTarget::Tab(id), text)) => {
                    if let Some(tab) = self.tabs.get_mut(&id) {
                        let text = text.trim();
                        tab.custom_title = (!text.is_empty()).then(|| text.to_string());
                    }
                }
                Some((RenameTarget::Group(g), text)) => {
                    let text = text.trim();
                    if let Some(group) = self.ws.group_mut(g)
                        && !text.is_empty()
                    {
                        group.name = text.to_string();
                    }
                }
                None => {}
            },
            Action::NewTab(profile) => {
                self.new_tab(profile, None);
            }
            Action::ToggleCollapse => {
                self.side.collapsed = !self.side.collapsed;
                self.side.peek = false;
            }
            Action::MoveTo(id, idx, group) => self.ws.move_to(id, idx, group),
            Action::NewGroup(id) => self.new_group(id),
            Action::SetGroup(id, group) => self.ws.set_group(id, group),
            Action::ToggleGroup(g) => {
                if let Some(group) = self.ws.group_mut(g) {
                    group.collapsed = !group.collapsed;
                }
            }
            Action::GroupColor(g, color) => {
                if let Some(group) = self.ws.group_mut(g) {
                    group.color = color;
                }
            }
            Action::Ungroup(g) => self.ws.ungroup(g),
            Action::CloseGroup(g) => self.close_group(g),
            Action::NewTabInGroup(g) => {
                // New tabs open after the focused one and join its folder.
                if let Some(&last) = self.ws.members(g).last() {
                    self.activate(last);
                    self.new_tab(0, None);
                }
            }
            Action::ToggleFiles => {
                if self.side.hidden {
                    self.zen_toggle_files();
                } else {
                    self.side.files_open = !self.side.files_open;
                }
            }
            Action::OpenPath(path) => self.open_path_tab(path),
        }
    }
}
