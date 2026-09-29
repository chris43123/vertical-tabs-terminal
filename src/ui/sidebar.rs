//! The vertical tab sidebar: tab rows, split groups, badges, rename, reorder, drag source,
//! and the collapsed icon strip that peeks open on hover.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Order, Rect, Sense, Stroke, StrokeKind, Ui,
    pos2, vec2,
};

use crate::app::{App, TabDrag, shade};
use crate::keybinds::Action as Shortcut;
use crate::session::TabId;

const COLLAPSED_WIDTH: f32 = 48.0;
const ROW_HEIGHT: f32 = 30.0;
const ACCENT: Color32 = Color32::from_rgb(0x89, 0xb4, 0xfa);
const BELL: Color32 = Color32::from_rgb(0xfa, 0xb3, 0x87);

enum Action {
    Activate(TabId),
    Close(TabId),
    Duplicate(TabId),
    Minimize(TabId),
    StartRename(TabId),
    CommitRename,
    NewTab(usize),
    ToggleCollapse,
    Reorder(TabId, usize),
    Move(TabId, bool),
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

        // Header: new tab, profile menu, collapse toggle.
        let new_tab_tip = format!("New tab{}", self.keybinds.hint(Shortcut::NewTab));
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
                });
            });
        } else {
            ui.vertical_centered(|ui| {
                header(ui, actions);
                if ui
                    .button(" » ")
                    .on_hover_text(format!("Expand sidebar{sidebar_hint}"))
                    .clicked()
                {
                    actions.push(Action::ToggleCollapse);
                }
            });
        }
        ui.add_space(4.0);
        ui.separator();

        let dragged = egui::DragAndDrop::payload::<TabDrag>(ui.ctx()).map(|p| p.0);
        let mut row_rects: Vec<(TabId, Rect)> = Vec::new();

        let scroll = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt(("tabs", expanded));
        let list_rect = scroll
            .show(ui, |ui| {
                let order = self.ws.order.clone();
                for (i, &id) in order.iter().enumerate() {
                    let rect = self.tab_row(ui, id, i, &order, expanded, actions);
                    row_rects.push((id, rect));
                }
                // Blank space below the tabs: double-click opens a new tab.
                let rest = ui.available_rect_before_wrap();
                if rest.height() > 0.0 && ui.allocate_rect(rest, Sense::click()).double_clicked() {
                    actions.push(Action::NewTab(0));
                }
            })
            .inner_rect;

        // Reordering: dragging a tab over the list shows an insertion line.
        if let (Some(dragged), Some(pos)) = (dragged, ui.ctx().input(|i| i.pointer.hover_pos()))
            && list_rect.contains(pos)
            && !row_rects.is_empty()
        {
            let idx = row_rects
                .iter()
                .position(|(_, r)| pos.y < r.center().y)
                .unwrap_or(row_rects.len());
            let y = row_rects
                .get(idx)
                .map(|(_, r)| r.top())
                .unwrap_or_else(|| row_rects.last().unwrap().1.bottom());
            ui.painter()
                .hline(list_rect.x_range(), y, Stroke::new(2.0, ACCENT));
            if ui.ctx().input(|i| i.pointer.any_released()) {
                actions.push(Action::Reorder(dragged, idx));
            }
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
        let (rect, resp) = ui.allocate_exact_size(
            vec2(ui.available_width(), ROW_HEIGHT),
            Sense::click_and_drag(),
        );
        let Some(tab) = self.tabs.get(&id) else {
            return rect;
        };
        let painter = ui.painter_at(rect.expand(1.0));
        let visuals = ui.visuals().clone();
        let base = visuals.panel_fill;

        let focused = self.ws.focused() == Some(id);
        let view = self.ws.view_of(id);
        let in_active = view == Some(self.ws.active);
        let in_split = view.is_some_and(|v| self.ws.views[v].root.is_split());

        // Background.
        let bg = if focused {
            shade(base, 1.9)
        } else if in_active {
            shade(base, 1.45)
        } else if resp.hovered() {
            shade(base, 1.3)
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
                ACCENT
            } else {
                shade(ACCENT, 0.55)
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
            .unwrap_or(shade(base, 2.6));
        painter.rect_filled(icon_rect, CornerRadius::same(5), icon_color);
        painter.text(
            icon_rect.center(),
            Align2::CENTER_CENTER,
            &tab.profile.icon,
            FontId::monospace(12.0),
            readable_on(icon_color),
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
            Some(BELL)
        } else if activity {
            Some(ACCENT)
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

            if self.renaming.as_ref().is_some_and(|(r, _)| *r == id) {
                let edit_rect = Rect::from_min_max(
                    pos2(title_left, rect.top() + 4.0),
                    pos2(title_right + 22.0, rect.bottom() - 4.0),
                );
                let (_, buf) = self.renaming.as_mut().unwrap();
                let edit = ui.put(
                    edit_rect,
                    egui::TextEdit::singleline(buf).desired_width(edit_rect.width()),
                );
                edit.request_focus();
                if edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    actions.push(Action::CommitRename);
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.renaming = None;
                }
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
                    shade(base, 2.6)
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
                Stroke::new(1.5, base),
            );
        }
        if focused && !expanded {
            painter.rect_stroke(
                icon_rect.expand(2.5),
                CornerRadius::same(7),
                Stroke::new(1.5, ACCENT),
                StrokeKind::Outside,
            );
        }

        if (focused || self.renaming.as_ref().is_some_and(|(r, _)| *r == id))
            && self.scroll_to_focused
        {
            resp.scroll_to_me(None);
        }
        let resp = if expanded {
            resp
        } else {
            resp.on_hover_text(&title)
        };
        if resp.clicked() {
            actions.push(Action::Activate(id));
        }
        if resp.middle_clicked() {
            actions.push(Action::Close(id));
        }
        if resp.double_clicked() && expanded {
            actions.push(Action::StartRename(id));
        }
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), TabDrag(id));
        }
        let kb = &self.keybinds;
        resp.context_menu(|ui| {
            let items = [
                ("Rename", Shortcut::RenameTab, Action::StartRename(id)),
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
                }
                let button =
                    egui::Button::new(text).shortcut_text(kb.label(shortcut).unwrap_or_default());
                if ui.add(button).clicked() {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        rect
    }

    /// Floating label that follows the pointer while a tab is being dragged.
    fn drag_ghost(&self, ctx: &egui::Context) {
        let Some(drag) = egui::DragAndDrop::payload::<TabDrag>(ctx) else {
            return;
        };
        let (Some(pos), Some(tab)) = (ctx.input(|i| i.pointer.hover_pos()), self.tabs.get(&drag.0))
        else {
            return;
        };
        let painter = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("drag_ghost")));
        let galley = painter.layout_no_wrap(
            tab.title().to_string(),
            FontId::proportional(13.0),
            Color32::WHITE,
        );
        let rect = Rect::from_min_size(pos + vec2(14.0, 10.0), galley.size() + vec2(16.0, 10.0));
        painter.rect_filled(
            rect,
            CornerRadius::same(6),
            Color32::from_rgba_unmultiplied(0x31, 0x32, 0x44, 235),
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, ACCENT),
            StrokeKind::Inside,
        );
        painter.galley(rect.min + vec2(8.0, 5.0), galley, Color32::WHITE);
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Activate(id) => self.activate(id),
            Action::Close(id) => self.close_tab(id),
            Action::Duplicate(id) => self.duplicate_tab(id),
            Action::Minimize(id) => self.ws.minimize(id),
            Action::StartRename(id) => self.start_rename(id),
            Action::Move(id, down) => {
                self.ws.move_tab(id, down);
            }
            Action::CommitRename => {
                if let Some((id, text)) = self.renaming.take()
                    && let Some(tab) = self.tabs.get_mut(&id)
                {
                    let text = text.trim();
                    tab.custom_title = (!text.is_empty()).then(|| text.to_string());
                }
            }
            Action::NewTab(profile) => {
                self.new_tab(profile, None);
            }
            Action::ToggleCollapse => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.sidebar_peek = false;
            }
            Action::Reorder(id, idx) => {
                self.ws.reorder(id, idx);
            }
        }
    }
}

/// Black or white, whichever reads better on `bg`.
fn readable_on(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 150.0 {
        Color32::from_rgb(0x11, 0x11, 0x1b)
    } else {
        Color32::WHITE
    }
}
