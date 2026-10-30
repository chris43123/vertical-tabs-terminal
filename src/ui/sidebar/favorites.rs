//! The pinned favorites above the tabs: one row per favorite with a status dot, its name
//! (dimmed when the directory is missing), a ↻ reset on hover, a context menu and drag
//! reorder. Collapsed, a row is the name's first grapheme plus the dot.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2,
    vec2,
};

use crate::app::{App, FavStatus};
use crate::config::keybinds::Action as Shortcut;
use crate::favorites::FavId;
use crate::theme::mix;
use crate::workspace::Edge;

use super::{Action, ROW_HEIGHT};

const FAV_HEIGHT: f32 = 26.0;
/// Most of the sidebar's remaining height the favorites take before they scroll.
const MAX_SHARE: f32 = 0.4;
/// Contrast kept by a missing favorite's dimmed name against the sidebar.
const DIM_CONTRAST: f32 = 3.0;

/// Payload for dragging a favorite to reorder it.
#[derive(Clone, Debug)]
pub struct FavDrag(pub FavId);

impl App {
    /// Draw the favorites section, if there are any (followed by a separator). It scrolls on
    /// its own past a share of the sidebar's height, so the tab list always keeps room.
    pub(super) fn favorites_section(&self, ui: &mut Ui, expanded: bool, actions: &mut Vec<Action>) {
        if self.favorites.list.list.is_empty() {
            return;
        }
        let ids: Vec<FavId> = self
            .favorites
            .list
            .list
            .iter()
            .map(|f| f.id.clone())
            .collect();
        let mut rects = Vec::with_capacity(ids.len());
        let visible = egui::ScrollArea::vertical()
            .id_salt(("favorites", expanded))
            .max_height((ui.available_height() * MAX_SHARE).max(FAV_HEIGHT))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (i, id) in ids.iter().enumerate() {
                    rects.push(self.favorite_row(ui, id, i, expanded, actions));
                }
            })
            .inner_rect;

        // Reorder: an insertion line between rows while a favorite is dragged over them.
        // Row rects are where the rows are, scrolled out of view or not.
        let dragged = egui::DragAndDrop::payload::<FavDrag>(ui.ctx()).map(|p| p.0.clone());
        let section = rects
            .iter()
            .copied()
            .reduce(|a, b| a.union(b))
            .unwrap_or(Rect::NOTHING);
        if let (Some(dragged), Some(pos)) = (dragged, ui.ctx().input(|i| i.pointer.hover_pos()))
            && visible.expand2(vec2(0.0, 6.0)).contains(pos)
        {
            let idx = rects
                .iter()
                .position(|r| pos.y < r.center().y)
                .unwrap_or(rects.len());
            let y = rects
                .get(idx)
                .map(|r| r.top() - 1.0)
                .unwrap_or_else(|| section.bottom() + 1.0);
            ui.painter_at(visible.expand(2.0)).hline(
                section.x_range(),
                y,
                Stroke::new(2.0, self.chrome.accent),
            );
            if ui.ctx().input(|i| i.pointer.any_released()) {
                actions.push(Action::MoveFavorite(dragged, idx));
            }
        }
        ui.add_space(2.0);
        ui.separator();
    }

    fn favorite_row(
        &self,
        ui: &mut Ui,
        id: &FavId,
        index: usize,
        expanded: bool,
        actions: &mut Vec<Action>,
    ) -> Rect {
        let height = if expanded { FAV_HEIGHT } else { ROW_HEIGHT };
        let (rect, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click_and_drag());
        let Some(fav) = self.favorites.list.get(id) else {
            return rect;
        };
        let c = &self.chrome;
        let visuals = ui.visuals();
        let status = self.favorites.status(id);
        let missing = self.favorites.is_missing(id);
        let linked = self.favorites.linked_tab(id);
        let focused = linked.is_some() && linked == self.ws.focused();
        let painter = ui.painter_at(rect.expand(1.0));

        // A lighter tint than the focused tab's, so the tab stays the strong highlight.
        let bg = if focused {
            mix(c.sidebar, c.accent, if c.light { 0.07 } else { 0.09 })
        } else if resp.hovered() {
            c.raised_sidebar(0.06)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(6), bg);
        let text_color = if missing {
            // Dimmed, but never below a legible contrast (or the user's minimum).
            crate::theme::ensure_contrast(
                mix(visuals.text_color(), c.sidebar, 0.55),
                c.sidebar,
                DIM_CONTRAST.max(self.config.min_contrast),
            )
        } else if focused {
            visuals.strong_text_color()
        } else {
            visuals.text_color()
        };

        if expanded {
            self.status_dot(&painter, pos2(rect.left() + 11.0, rect.center().y), status);
            let reset_rect = Rect::from_center_size(
                pos2(rect.right() - 13.0, rect.center().y),
                vec2(18.0, 18.0),
            );
            let left = rect.left() + 22.0;
            let right = rect.right() - 28.0;
            let mut job = LayoutJob::simple_singleline(
                fav.name.clone(),
                FontId::proportional(13.0),
                text_color,
            );
            job.wrap = TextWrapping::truncate_at_width(right - left);
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            painter.galley(
                pos2(left, rect.center().y - galley.size().y / 2.0),
                galley,
                text_color,
            );
            if resp.hovered() || ui.rect_contains_pointer(reset_rect) {
                let reset = ui
                    .interact(reset_rect, Id::new(("reset_favorite", id)), Sense::click())
                    .on_hover_text(if status == FavStatus::Running {
                        "Reset: start it again from the favorite"
                    } else {
                        "Open"
                    });
                let fill = if reset.hovered() {
                    c.raised_sidebar(0.28)
                } else {
                    Color32::TRANSPARENT
                };
                painter.rect_filled(reset_rect, CornerRadius::same(4), fill);
                // ↻ resets an open tab; ▶ opens one when there's none.
                let glyph = if status == FavStatus::Running {
                    "↻"
                } else {
                    "▶"
                };
                painter.text(
                    reset_rect.center(),
                    Align2::CENTER_CENTER,
                    glyph,
                    FontId::proportional(13.0),
                    visuals.text_color(),
                );
                if reset.clicked() {
                    actions.push(Action::ResetFavorite(id.clone()));
                }
            }
        } else {
            let icon = Rect::from_center_size(rect.center(), vec2(22.0, 22.0));
            painter.rect_stroke(
                icon,
                CornerRadius::same(5),
                Stroke::new(1.0, c.raised_sidebar(0.2)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                icon.center(),
                Align2::CENTER_CENTER,
                first_grapheme(&fav.name),
                FontId::proportional(13.0),
                text_color,
            );
            self.status_dot(&painter, icon.right_top() + vec2(-1.0, 1.0), status);
        }

        let dim = mix(c.fg, c.bg, 0.4);
        let resp = resp.on_hover_ui(|ui| {
            ui.set_max_width(420.0);
            ui.label(RichText::new(&fav.name).strong());
            ui.label(
                RichText::new(crate::ui::display_path(&fav.path))
                    .font(FontId::monospace(12.0))
                    .color(c.fg),
            );
            if missing {
                ui.label(RichText::new("This directory doesn't exist").color(c.danger));
            }
            if let Some(cmd) = &fav.command {
                ui.label(RichText::new(format!("▶ {cmd}")).color(dim));
            }
            if let Some(p) = &fav.profile {
                ui.label(RichText::new(format!("profile: {p}")).color(dim));
            }
            let shortcut = self.keybinds.hint(Shortcut::OpenFavorite(index as u8 + 1));
            if index < 9 && !shortcut.is_empty() {
                ui.label(RichText::new(format!("Open{shortcut}")).color(dim));
            }
        });
        if resp.clicked() {
            actions.push(Action::OpenFavorite(id.clone(), None));
        }
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), FavDrag(id.clone()));
        }
        resp.context_menu(|ui| {
            let items = [
                ("Open", Action::OpenFavorite(id.clone(), None)),
                (
                    "Open in split",
                    Action::OpenFavorite(id.clone(), Some(Edge::Right)),
                ),
                ("Reveal in files panel", Action::RevealFavorite(id.clone())),
                ("Edit…", Action::EditFavorite(id.clone())),
                ("Remove", Action::RemoveFavorite(id.clone())),
            ];
            for (text, action) in items {
                if matches!(action, Action::RemoveFavorite(_)) {
                    ui.separator();
                }
                if ui.button(text).clicked() {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        rect
    }

    /// Running: filled green. Exited: filled red. No session: a hollow ring.
    fn status_dot(&self, painter: &egui::Painter, center: Pos2, status: FavStatus) {
        let c = &self.chrome;
        let (fill, ring) = match status {
            FavStatus::Running => (Some(c.green), None),
            FavStatus::Exited => (Some(c.danger), None),
            FavStatus::Idle => (None, Some(c.raised_sidebar(0.4))),
        };
        if let Some(fill) = fill {
            painter.circle_filled(center, 3.5, fill);
        }
        if let Some(ring) = ring {
            painter.circle_stroke(center, 3.0, Stroke::new(1.2, ring));
        }
    }
}

/// The first user-perceived character of `name`: a char plus the marks, variation selectors
/// and skin tones that attach to it (ZWJ sequences are out of scope).
pub(super) fn first_grapheme(name: &str) -> String {
    let mut chars = name.trim().chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = first.to_string();
    for c in chars {
        let attaches = matches!(c as u32,
            0x0300..=0x036F | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0x1F3FB..=0x1F3FF);
        if !attaches {
            break;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_grapheme_keeps_attached_marks() {
        assert_eq!(first_grapheme("api"), "a");
        assert_eq!(first_grapheme("  🚀 web"), "🚀");
        assert_eq!(first_grapheme("❤\u{FE0F} love"), "❤\u{FE0F}");
        assert_eq!(first_grapheme("e\u{301}cole"), "e\u{301}");
        assert_eq!(first_grapheme("👍🏽 ok"), "👍🏽");
        assert_eq!(first_grapheme(""), "");
    }

    /// The UI keeps egui's default fonts, whose fallbacks draw monochrome emoji and the
    /// symbols the favorites UI uses.
    #[test]
    fn default_fonts_cover_emoji_and_symbols() {
        let fonts = [
            epaint_default_fonts::UBUNTU_LIGHT,
            epaint_default_fonts::NOTO_EMOJI_REGULAR,
            epaint_default_fonts::EMOJI_ICON,
        ];
        let covered = |ch: char| {
            fonts.iter().any(|data| {
                swash::FontRef::from_index(data, 0).is_some_and(|f| f.charmap().map(ch) != 0)
            })
        };
        for ch in ['↻', '▶', '⚠', '🚀', '🐳', '★', '❤'] {
            assert!(covered(ch), "{ch}");
        }
    }
}
