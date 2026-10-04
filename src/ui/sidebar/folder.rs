//! Folder rows (named tab groups): header, color, context menu and the inline rename box.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Ui, pos2, vec2,
};

use crate::app::{App, RenameTarget};
use crate::theme::mix;
use crate::workspace::GroupId;

use super::{Action, FOLDER_COLORS, FOLDER_HEIGHT};

impl App {
    pub(crate) fn folder_color(&self, g: GroupId) -> Color32 {
        match self.ws.group(g).and_then(|g| g.color) {
            Some(i) => self.palette.indexed(i),
            None => self.chrome.accent,
        }
    }

    /// A folder header: chevron, folder icon, name, tab count when collapsed.
    pub(super) fn folder_row(
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
    pub(super) fn rename_box(&mut self, ui: &mut Ui, rect: Rect, actions: &mut Vec<Action>) {
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
}

/// A folder shape (body plus tab) holding `initial`, for tabs sitting at a shell prompt.
pub(super) fn paint_folder_icon(
    painter: &egui::Painter,
    rect: Rect,
    initial: &str,
    fill: Color32,
    ink: Color32,
) {
    let tab = Rect::from_min_size(rect.left_top() + vec2(0.0, 2.0), vec2(10.0, 6.0));
    let body = Rect::from_min_max(
        rect.left_top() + vec2(0.0, 5.0),
        rect.right_bottom() - vec2(0.0, 1.0),
    );
    painter.rect_filled(tab, CornerRadius::same(2), fill);
    painter.rect_filled(body, CornerRadius::same(4), fill);
    painter.text(
        body.center() + vec2(0.0, 1.0),
        Align2::CENTER_CENTER,
        initial,
        FontId::monospace(11.0),
        ink,
    );
}
