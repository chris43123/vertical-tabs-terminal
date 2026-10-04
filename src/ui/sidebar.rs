//! The vertical tab sidebar: tab rows, split groups, folders (Zen-style tab groups), badges,
//! rename, reorder, drag source, a long-hover card with the tab's cwd, and the collapsed icon
//! strip that peeks open on hover.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Id, Order, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};

use crate::app::{App, RenameTarget, TabDrag};
use crate::keybinds::Action as Shortcut;
use crate::layout::GroupId;
use crate::session::TabId;
use crate::theme::mix;

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
        let collapsed = self.sidebar_collapsed;
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
            self.sidebar_peek = false;
        }

        self.drag_ghost(ui.ctx());

        self.scroll_to_focused = false;
        for action in actions {
            self.apply(action);
        }
    }

    /// While collapsed, hovering the strip shows the full sidebar floating over the panes
    /// (so terminals don't resize).
    fn sidebar_peek_overlay(
        &mut self,
        ctx: &egui::Context,
        strip: Rect,
        actions: &mut Vec<Action>,
    ) {
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let dragging = egui::DragAndDrop::has_any_payload(ctx);
        let overlay =
            Rect::from_min_size(strip.min, vec2(self.config.sidebar_width, strip.height()));

        if pointer.is_some_and(|p| strip.contains(p)) && !dragging {
            self.sidebar_peek = true;
        } else if self.sidebar_peek {
            let keep = pointer.is_some_and(|p| overlay.expand(8.0).contains(p))
                || ctx.any_popup_open()
                || self.renaming.is_some();
            if !keep || dragging {
                self.sidebar_peek = false;
            }
        }
        if !self.sidebar_peek {
            return;
        }

        let fill = ctx.global_style().visuals.panel_fill;
        egui::Area::new(Id::new("sidebar_peek"))
            .fixed_pos(strip.min)
            .order(Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(fill)
                    .inner_margin(6)
                    .shadow(egui::Shadow {
                        offset: [4, 0],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(120),
                    })
                    .show(ui, |ui| {
                        ui.set_width(self.config.sidebar_width - 12.0);
                        ui.set_height(strip.height() - 12.0);
                        self.sidebar_contents(ui, true, actions);
                    });
            });
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
                    let (label, tip) = if self.sidebar_collapsed {
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
                        .selectable_label(self.files_open, " 🗀 ")
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

    pub(crate) fn folder_color(&self, g: GroupId) -> Color32 {
        match self.ws.group(g).and_then(|g| g.color) {
            Some(i) => self.palette.indexed(i),
            None => self.chrome.accent,
        }
    }

    /// A folder header: chevron, folder icon, name, tab count when collapsed.
    fn folder_row(
        &mut self,
        ui: &mut Ui,
        g: GroupId,
        expanded: bool,
        actions: &mut Vec<Action>,
    ) -> Rect {
        let height = if expanded { FOLDER_HEIGHT } else { 24.0 };
        let (rect, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        let Some(group) = self.ws.group(g).cloned() else {
            return rect;
        };
        let c = self.chrome.clone();
        let color = self.folder_color(g);
        let painter = ui.painter_at(rect.expand(1.0));
        let members = self.ws.members(g);
        let (activity, bell) = members
            .iter()
            .filter_map(|id| self.tabs.get(id))
            .fold((false, false), |(a, b), t| (a || t.activity, b || t.bell));
        if resp.hovered() {
            painter.rect_filled(rect, CornerRadius::same(6), c.raised_sidebar(0.06));
        }
        let icon = if group.collapsed { "🗀" } else { "🗁" };

        if expanded {
            painter.text(
                pos2(rect.left() + 8.0, rect.center().y),
                Align2::CENTER_CENTER,
                if group.collapsed { "⏵" } else { "⏷" },
                FontId::proportional(12.0),
                mix(c.fg, c.sidebar, 0.4),
            );
            painter.text(
                pos2(rect.left() + 24.0, rect.center().y),
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(15.0),
                color,
            );
            let left = rect.left() + 36.0;
            let right = rect.right() - 26.0;
            let renaming = self
                .renaming
                .as_ref()
                .is_some_and(|(r, _)| *r == RenameTarget::Group(g));
            if renaming {
                self.rename_box(
                    ui,
                    Rect::from_min_max(
                        pos2(left, rect.top() + 3.0),
                        pos2(rect.right() - 4.0, rect.bottom() - 3.0),
                    ),
                    actions,
                );
            } else {
                let mut job = LayoutJob::simple_singleline(
                    group.name.clone(),
                    FontId::proportional(13.0),
                    ui.visuals().strong_text_color(),
                );
                job.wrap = TextWrapping::truncate_at_width(right - left);
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                painter.galley(
                    pos2(left, rect.center().y - galley.size().y / 2.0),
                    galley,
                    c.fg,
                );
            }
            // Collapsed: how many tabs are inside, and whether any of them wants attention.
            let badge_center = pos2(rect.right() - 14.0, rect.center().y);
            if group.collapsed && !renaming {
                let text = members.len().to_string();
                painter.text(
                    badge_center,
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(11.5),
                    mix(c.fg, c.sidebar, 0.35),
                );
                if bell || activity {
                    painter.circle_filled(
                        badge_center + vec2(-12.0, 0.0),
                        3.5,
                        if bell { c.bell } else { c.accent },
                    );
                }
            }
        } else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(15.0),
                color,
            );
            if group.collapsed && (bell || activity) {
                painter.circle_filled(
                    rect.center() + vec2(9.0, -6.0),
                    3.5,
                    if bell { c.bell } else { c.accent },
                );
            }
        }

        let resp = resp.on_hover_text(format!(
            "{} · {} tab{}",
            group.name,
            members.len(),
            if members.len() == 1 { "" } else { "s" }
        ));
        if resp.double_clicked() && expanded {
            // The first click already toggled; undo that and rename instead.
            actions.push(Action::ToggleGroup(g));
            actions.push(Action::StartRename(RenameTarget::Group(g)));
        } else if resp.clicked() {
            actions.push(Action::ToggleGroup(g));
        }
        resp.context_menu(|ui| {
            if ui.button("Rename").clicked() {
                actions.push(Action::StartRename(RenameTarget::Group(g)));
                ui.close();
            }
            ui.menu_button("Color", |ui| {
                if ui.button("Accent").clicked() {
                    actions.push(Action::GroupColor(g, None));
                    ui.close();
                }
                for (name, idx) in FOLDER_COLORS {
                    let swatch = RichText::new("⏺ ").color(self.palette.indexed(idx));
                    let mut job = LayoutJob::default();
                    swatch.append_to(
                        &mut job,
                        ui.style(),
                        egui::FontSelection::Default,
                        Align::Center,
                    );
                    RichText::new(name).append_to(
                        &mut job,
                        ui.style(),
                        egui::FontSelection::Default,
                        Align::Center,
                    );
                    if ui.button(job).clicked() {
                        actions.push(Action::GroupColor(g, Some(idx)));
                        ui.close();
                    }
                }
            });
            if ui.button("New tab in folder").clicked() {
                actions.push(Action::NewTabInGroup(g));
                ui.close();
            }
            ui.separator();
            if ui.button("Ungroup").clicked() {
                actions.push(Action::Ungroup(g));
                ui.close();
            }
            if ui.button("Close all tabs in folder").clicked() {
                actions.push(Action::CloseGroup(g));
                ui.close();
            }
        });
        rect
    }

    /// The inline rename text box (tabs and folders).
    fn rename_box(&mut self, ui: &mut Ui, rect: Rect, actions: &mut Vec<Action>) {
        let Some((_, buf)) = self.renaming.as_mut() else {
            return;
        };
        let edit = ui.put(
            rect,
            egui::TextEdit::singleline(buf).desired_width(rect.width()),
        );
        edit.request_focus();
        if edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            actions.push(Action::CommitRename);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.renaming = None;
        }
    }

    /// One sidebar row. Returns its rect.
    fn tab_row(
        &mut self,
        ui: &mut Ui,
        id: TabId,
        index: usize,
        order: &[TabId],
        expanded: bool,
        actions: &mut Vec<Action>,
    ) -> Rect {
        let (full, resp) = ui.allocate_exact_size(
            vec2(ui.available_width(), ROW_HEIGHT),
            Sense::click_and_drag(),
        );
        let Some(tab) = self.tabs.get(&id) else {
            return full;
        };
        // Tabs in a folder are indented, with the folder's color running down the left.
        let group = self.ws.group_of(id);
        let rect = match group {
            Some(g) => {
                let x = if expanded {
                    full.left() + 8.0
                } else {
                    full.left() + 1.0
                };
                ui.painter().vline(
                    x,
                    full.y_range().expand(1.0),
                    Stroke::new(2.0, mix(self.folder_color(g), self.chrome.sidebar, 0.35)),
                );
                if expanded {
                    full.with_min_x(full.left() + FOLDER_INDENT)
                } else {
                    full
                }
            }
            None => full,
        };
        let painter = ui.painter_at(rect.expand(1.0));
        let visuals = ui.visuals().clone();
        let c = self.chrome.clone();

        let focused = self.ws.focused() == Some(id);
        let view = self.ws.view_of(id);
        let in_active = view == Some(self.ws.active);
        let in_split = view.is_some_and(|v| self.ws.views[v].root.is_split());

        // Background.
        let bg = if focused {
            c.raised_sidebar(0.16)
        } else if in_active {
            c.raised_sidebar(0.09)
        } else if resp.hovered() {
            c.raised_sidebar(0.06)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(6), bg);

        // Split group bracket joining adjacent members.
        if in_split {
            let same = |j: Option<&TabId>| j.is_some_and(|t| self.ws.view_of(*t) == view);
            let top = if same(index.checked_sub(1).and_then(|j| order.get(j))) {
                rect.top() - 2.0
            } else {
                rect.top() + 5.0
            };
            let bottom = if same(order.get(index + 1)) {
                rect.bottom() + 2.0
            } else {
                rect.bottom() - 5.0
            };
            let color = if in_active {
                self.chrome.accent
            } else {
                mix(c.accent, c.sidebar, 0.5)
            };
            painter.rect_filled(
                Rect::from_min_max(pos2(rect.left(), top), pos2(rect.left() + 3.0, bottom)),
                CornerRadius::same(2),
                color,
            );
        }

        // Profile icon.
        let icon_center = if expanded {
            pos2(rect.left() + 18.0, rect.center().y)
        } else {
            rect.center()
        };
        let icon_rect = Rect::from_center_size(icon_center, vec2(22.0, 22.0));
        let icon_color = tab
            .profile
            .color
            .map(|[r, g, b]| Color32::from_rgb(r, g, b))
            .unwrap_or(c.raised_sidebar(0.28));
        painter.rect_filled(icon_rect, CornerRadius::same(5), icon_color);
        painter.text(
            icon_rect.center(),
            Align2::CENTER_CENTER,
            &tab.profile.icon,
            FontId::monospace(12.0),
            c.readable_on(icon_color),
        );

        let text_color = if focused || in_active {
            visuals.strong_text_color()
        } else {
            visuals.text_color()
        };
        let title = tab.title().to_string();
        let (activity, bell) = (tab.activity, tab.bell);

        // Unread / bell badges.
        let badge = if bell {
            Some(c.bell)
        } else if activity {
            Some(self.chrome.accent)
        } else {
            None
        };

        if expanded {
            let close_rect = Rect::from_center_size(
                pos2(rect.right() - 14.0, rect.center().y),
                vec2(18.0, 18.0),
            );
            let title_left = icon_rect.right() + 8.0;
            let title_right = rect.right() - 30.0;

            if self
                .renaming
                .as_ref()
                .is_some_and(|(r, _)| *r == RenameTarget::Tab(id))
            {
                let edit_rect = Rect::from_min_max(
                    pos2(title_left, rect.top() + 4.0),
                    pos2(title_right + 22.0, rect.bottom() - 4.0),
                );
                self.rename_box(ui, edit_rect, actions);
            } else {
                let mut job = LayoutJob::simple_singleline(
                    title.clone(),
                    FontId::proportional(13.0),
                    text_color,
                );
                job.wrap = TextWrapping::truncate_at_width(title_right - title_left);
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                painter.galley(
                    pos2(title_left, rect.center().y - galley.size().y / 2.0),
                    galley,
                    text_color,
                );
            }

            if let Some(color) = badge.filter(|_| !resp.hovered()) {
                painter.circle_filled(close_rect.center(), 4.0, color);
            }
            if resp.hovered() || ui.rect_contains_pointer(close_rect) {
                let close = ui.interact(close_rect, Id::new(("close_tab", id)), Sense::click());
                let fill = if close.hovered() {
                    c.raised_sidebar(0.28)
                } else {
                    Color32::TRANSPARENT
                };
                painter.rect_filled(close_rect, CornerRadius::same(4), fill);
                painter.text(
                    close_rect.center(),
                    Align2::CENTER_CENTER,
                    "×",
                    FontId::proportional(15.0),
                    visuals.text_color(),
                );
                if close.clicked() {
                    actions.push(Action::Close(id));
                }
            }
        } else if let Some(color) = badge {
            painter.circle_filled(icon_rect.right_top() + vec2(-1.0, 1.0), 4.5, color);
            painter.circle_stroke(
                icon_rect.right_top() + vec2(-1.0, 1.0),
                4.5,
                Stroke::new(1.5, c.sidebar),
            );
        }
        if focused && !expanded {
            painter.rect_stroke(
                icon_rect.expand(2.5),
                CornerRadius::same(7),
                Stroke::new(1.5, self.chrome.accent),
                StrokeKind::Outside,
            );
        }

        if (focused
            || self
                .renaming
                .as_ref()
                .is_some_and(|(r, _)| *r == RenameTarget::Tab(id)))
            && self.scroll_to_focused
        {
            resp.scroll_to_me(None);
        }
        let resp = self.tab_hover_card(resp, id);
        if resp.clicked() {
            actions.push(Action::Activate(id));
        }
        if resp.middle_clicked() {
            actions.push(Action::Close(id));
        }
        if resp.double_clicked() && expanded {
            actions.push(Action::StartRename(RenameTarget::Tab(id)));
        }
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), TabDrag(id));
        }
        let kb = &self.keybinds;
        let groups: Vec<(GroupId, String)> = self
            .ws
            .groups
            .iter()
            .filter(|g| Some(g.id) != group)
            .map(|g| (g.id, g.name.clone()))
            .collect();
        resp.context_menu(|ui| {
            let items = [
                (
                    "Rename",
                    Shortcut::RenameTab,
                    Action::StartRename(RenameTarget::Tab(id)),
                ),
                ("Duplicate", Shortcut::DuplicateTab, Action::Duplicate(id)),
                ("Move up", Shortcut::MoveTabUp, Action::Move(id, false)),
                ("Move down", Shortcut::MoveTabDown, Action::Move(id, true)),
                (
                    "Minimise from split",
                    Shortcut::MinimizePane,
                    Action::Minimize(id),
                ),
                ("Close", Shortcut::CloseTab, Action::Close(id)),
            ];
            for (text, shortcut, action) in items {
                if matches!(action, Action::Minimize(_)) && !in_split {
                    continue;
                }
                if matches!(action, Action::Close(_)) {
                    ui.separator();
                    let button = egui::Button::new("New folder with tab")
                        .shortcut_text(kb.label(Shortcut::NewGroup).unwrap_or_default());
                    if ui.add(button).clicked() {
                        actions.push(Action::NewGroup(id));
                        ui.close();
                    }
                    if !groups.is_empty() {
                        ui.menu_button("Move to folder", |ui| {
                            for (g, name) in &groups {
                                if ui.button(name).clicked() {
                                    actions.push(Action::SetGroup(id, Some(*g)));
                                    ui.close();
                                }
                            }
                        });
                    }
                    if group.is_some() && ui.button("Remove from folder").clicked() {
                        actions.push(Action::SetGroup(id, None));
                        ui.close();
                    }
                    ui.separator();
                }
                let button =
                    egui::Button::new(text).shortcut_text(kb.label(shortcut).unwrap_or_default());
                if ui.add(button).clicked() {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        full
    }

    /// After a long hover, a card with the tab's full title, working directory and process
    /// (or, for a preview, the file's path).
    fn tab_hover_card(&self, resp: egui::Response, id: TabId) -> egui::Response {
        let Some(tab) = self.tabs.get(&id) else {
            return resp;
        };
        let c = &self.chrome;
        let dim = mix(c.fg, c.bg, 0.4);
        resp.on_hover_ui(|ui| {
            ui.set_max_width(420.0);
            ui.label(RichText::new(tab.title()).strong());
            let mono = FontId::monospace(12.0);
            if let Some(p) = tab.preview() {
                ui.label(
                    RichText::new(crate::ui::display_path(&p.path))
                        .font(mono)
                        .color(c.fg),
                );
                return;
            }
            match &tab.cwd {
                Some(cwd) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🗀").color(dim));
                        ui.label(
                            RichText::new(crate::ui::display_path(cwd))
                                .font(mono.clone())
                                .color(c.fg),
                        );
                    });
                }
                None => {
                    ui.label(RichText::new("cwd unknown").color(dim));
                }
            }
            if let Some(process) = &tab.process {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("▶").color(dim));
                    ui.label(RichText::new(process).font(mono).color(dim));
                });
            }
        })
    }

    /// Floating label that follows the pointer while a tab is being dragged.
    fn drag_ghost(&self, ctx: &egui::Context) {
        let label = if let Some(drag) = egui::DragAndDrop::payload::<TabDrag>(ctx) {
            self.tabs.get(&drag.0).map(|t| t.title().to_string())
        } else {
            egui::DragAndDrop::payload::<crate::ui::FileDrag>(ctx).map(|f| {
                f.0.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| f.0.to_string_lossy().into_owned())
            })
        };
        let (Some(pos), Some(label)) = (ctx.input(|i| i.pointer.hover_pos()), label) else {
            return;
        };
        let painter = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("drag_ghost")));
        let galley = painter.layout_no_wrap(label, FontId::proportional(13.0), self.chrome.fg);
        let rect = Rect::from_min_size(pos + vec2(14.0, 10.0), galley.size() + vec2(16.0, 10.0));
        painter.rect_filled(
            rect,
            CornerRadius::same(6),
            self.chrome.raised(0.12).gamma_multiply(0.95),
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, self.chrome.accent),
            StrokeKind::Inside,
        );
        painter.galley(rect.min + vec2(8.0, 5.0), galley, self.chrome.fg);
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
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.sidebar_peek = false;
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
            Action::ToggleFiles => self.files_open = !self.files_open,
            Action::OpenPath(path) => self.open_path_tab(path),
        }
    }
}
