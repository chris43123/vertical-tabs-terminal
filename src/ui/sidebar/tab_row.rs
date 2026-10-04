//! A tab row: icon, title, badges, close button, context menu and drag source, plus the
//! hover card and the ghost drawn while dragging.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Order, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};

use crate::app::{App, Content, RenameTarget, TabDrag};
use crate::config::keybinds::Action as Shortcut;
use crate::theme::mix;
use crate::ui::icons::Icon;
use crate::workspace::{GroupId, TabId};

use super::folder::paint_folder_icon;
use super::{Action, FOLDER_INDENT, ROW_HEIGHT};

impl App {
    /// One sidebar row. Returns its rect.
    pub(super) fn tab_row(
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
        let icon = match &tab.content {
            Content::Term(_) => crate::ui::icons::icon_for(
                tab.process.as_deref(),
                tab.cwd.as_deref(),
                dirs::home_dir().as_deref(),
            ),
            Content::Preview(_) => None,
        };
        let profile_color = tab
            .profile
            .color
            .map(|[r, g, b]| Color32::from_rgb(r, g, b))
            .unwrap_or(c.raised_sidebar(0.28));
        match icon {
            Some(Icon::Folder(initial)) => {
                paint_folder_icon(
                    &painter,
                    icon_rect,
                    &initial,
                    profile_color,
                    c.readable_on(profile_color),
                );
            }
            other => {
                let (label, color) = match other {
                    Some(Icon::Label(label, color)) => {
                        (label, color.map(|[r, g, b]| Color32::from_rgb(r, g, b)))
                    }
                    _ => (tab.profile.icon.clone(), None),
                };
                let fill = color.unwrap_or(profile_color);
                painter.rect_filled(icon_rect, CornerRadius::same(5), fill);
                painter.text(
                    icon_rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::monospace(12.0),
                    c.readable_on(fill),
                );
            }
        }

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
    pub(super) fn drag_ghost(&self, ctx: &egui::Context) {
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
}
