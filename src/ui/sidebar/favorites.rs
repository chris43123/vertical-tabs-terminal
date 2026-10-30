//! The pinned favorites above the tabs, under a "Favorites" header (with a + on hover). A
//! favorite's row *is* its tab once open: it shows the focused state, unread and bell
//! markers and a close button, and the other tabs of its split view are listed under it
//! rather than among the tabs. Each row has a tile (the name's first grapheme with the
//! status dot, open / no tab / exited, and the unread badge on its corners), its name
//! (dimmed when the directory is missing), ↻ reset or ▶ open on hover, a context menu and
//! drag reorder. A handle under the section resizes it. Collapsed, a row is just the tile.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};

use crate::app::{App, FavStatus};
use crate::config::keybinds::Action as Shortcut;
use crate::favorites::FavId;
use crate::theme::mix;
use crate::workspace::{Edge, TabId};

use super::{Action, DIM_CONTRAST, ROW_HEIGHT};

const FAV_HEIGHT: f32 = 26.0;
const HEADER_HEIGHT: f32 = 20.0;
const HANDLE_HEIGHT: f32 = 8.0;
/// Share of the sidebar's remaining height the favorites take before they scroll, until
/// the user drags the handle.
const DEFAULT_SHARE: f32 = 0.4;
/// The most a dragged height may take, so the tab list never disappears.
const MAX_SHARE: f32 = 0.85;
/// Side of a favorite's tile in the expanded and the collapsed sidebar.
const TILE_EXPANDED: f32 = 20.0;
const TILE_COLLAPSED: f32 = 22.0;

/// Payload for dragging a favorite to reorder it.
#[derive(Clone, Debug)]
pub struct FavDrag(pub FavId);

impl App {
    /// Draw the favorites section: the header, the rows (scrolling past their height) and
    /// the resize handle. Collapsed and empty, nothing.
    pub(super) fn favorites_section(
        &mut self,
        ui: &mut Ui,
        expanded: bool,
        actions: &mut Vec<Action>,
    ) {
        let empty = self.favorites.list.list.is_empty();
        if !expanded && empty {
            return;
        }
        if expanded {
            self.favorites_header(ui, actions);
        }
        if empty {
            ui.separator();
            return;
        }
        let ids: Vec<FavId> = self
            .favorites
            .list
            .list
            .iter()
            .map(|f| f.id.clone())
            .collect();
        let groups = self.favorite_tabs();
        let available = ui.available_height();
        let row = if expanded { FAV_HEIGHT } else { ROW_HEIGHT };
        let max_height = self
            .favorites_height
            .unwrap_or(available * DEFAULT_SHARE)
            .min(available * MAX_SHARE)
            .max(row);

        let mut rects = Vec::with_capacity(ids.len());
        let mut content = Rect::NOTHING;
        let output = egui::ScrollArea::vertical()
            .id_salt(("favorites", expanded))
            .max_height(max_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (i, id) in ids.iter().enumerate() {
                    let group = groups.iter().find(|g| &g.id == id);
                    let rect =
                        self.favorite_row(ui, id, i, group.map(|g| g.tab), expanded, actions);
                    rects.push(rect);
                    content = content.union(rect);
                    // Its split's other tabs, under it (bracketed like any split).
                    if let Some(g) = group.filter(|g| !g.siblings.is_empty()) {
                        let order: Vec<TabId> = std::iter::once(g.tab)
                            .chain(g.siblings.iter().copied())
                            .collect();
                        for (j, &t) in order.iter().enumerate().skip(1) {
                            content =
                                content.union(self.tab_row(ui, t, j, &order, expanded, actions));
                        }
                    }
                }
            });
        let visible = output.inner_rect;

        // Reorder: an insertion line between favorites while one is dragged over them.
        // Row rects are where the rows are, scrolled out of view or not.
        let dragged = egui::DragAndDrop::payload::<FavDrag>(ui.ctx()).map(|p| p.0.clone());
        // Near the section's top or bottom edge, scroll it so the drop can land anywhere.
        // Only while a favorite is dragged over it: the helper repaints only as it scrolls.
        if dragged.is_some()
            && ui
                .ctx()
                .input(|i| i.pointer.latest_pos())
                .is_some_and(|p| visible.contains(p))
        {
            crate::ui::preview::autoscroll(
                ui.ctx(),
                output.id,
                &output.state,
                visible,
                output.content_size,
                false,
            );
        }
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
                .unwrap_or_else(|| content.bottom() + 1.0);
            ui.painter_at(visible.expand(2.0)).hline(
                content.x_range(),
                y,
                Stroke::new(2.0, self.chrome.accent),
            );
            if ui.ctx().input(|i| i.pointer.any_released()) {
                actions.push(Action::MoveFavorite(dragged, idx));
            }
        }
        self.favorites_handle(ui, visible.height(), content.height(), row);
    }

    /// "Favorites", with a + that shows on hover and opens the add dialog.
    fn favorites_header(&self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let (rect, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), HEADER_HEIGHT), Sense::hover());
        let c = &self.chrome;
        let dim = mix(c.fg, c.sidebar, 0.4);
        ui.painter().text(
            pos2(rect.left() + 6.0, rect.center().y),
            Align2::LEFT_CENTER,
            "Favorites",
            FontId::proportional(11.5),
            dim,
        );
        let plus_rect =
            Rect::from_center_size(pos2(rect.right() - 13.0, rect.center().y), vec2(18.0, 18.0));
        if resp.hovered() || ui.rect_contains_pointer(plus_rect) {
            let tip = format!(
                "Add directory to favorites{}",
                self.keybinds.hint(Shortcut::AddFavorite)
            );
            let plus = ui
                .interact(plus_rect, Id::new("add_favorite"), Sense::click())
                .on_hover_text(tip);
            if plus.hovered() {
                ui.painter()
                    .rect_filled(plus_rect, CornerRadius::same(4), c.raised_sidebar(0.28));
            }
            ui.painter().text(
                plus_rect.center(),
                Align2::CENTER_CENTER,
                "+",
                FontId::proportional(15.0),
                ui.visuals().text_color(),
            );
            if plus.clicked() {
                actions.push(Action::AddFavorite(None));
            }
        }
    }

    /// The line under the favorites: drag it to make the section taller or shorter (kept
    /// for the session and across restarts).
    fn favorites_handle(&mut self, ui: &mut Ui, shown: f32, content: f32, row: f32) {
        // Click sense too: a drag-only response never reports the double-click that resets it.
        let (rect, resp) = ui.allocate_exact_size(
            vec2(ui.available_width(), HANDLE_HEIGHT),
            Sense::click_and_drag(),
        );
        let resp = resp.on_hover_cursor(egui::CursorIcon::ResizeVertical);
        let active = resp.hovered() || resp.dragged();
        let color = if active {
            self.chrome.accent
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        };
        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            Stroke::new(if active { 2.0 } else { 1.0 }, color),
        );
        if resp.dragged() {
            let delta = resp.drag_delta().y;
            if delta != 0.0 {
                // Never taller than the rows themselves, never shorter than one.
                self.favorites_height = Some((shown + delta).clamp(row, content.max(row)));
            }
        }
        if resp.double_clicked() {
            self.favorites_height = None;
        }
    }

    fn favorite_row(
        &self,
        ui: &mut Ui,
        id: &FavId,
        index: usize,
        tab: Option<TabId>,
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
        let status = self.favorite_status(id);
        let missing = self.favorites.is_missing(id);
        let open = tab.and_then(|t| self.tabs.get(&t));
        let focused = tab.is_some() && tab == self.ws.focused();
        let view = tab.and_then(|t| self.ws.view_of(t));
        let in_active = view.is_some() && view == Some(self.ws.active);
        let in_split = view.is_some_and(|v| self.ws.views[v].root.is_split());
        let painter = ui.painter_at(rect.expand(1.0));

        // The same highlight as a tab row: the favorite's row is its tab.
        let bg = if focused {
            mix(c.sidebar, c.accent, if c.light { 0.16 } else { 0.22 })
        } else if in_active || resp.hovered() {
            c.raised_sidebar(0.06)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(6), bg);
        if in_split {
            let color = view.map_or(c.accent, |v| self.view_color(v));
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(rect.left(), rect.top() + 5.0),
                    pos2(rect.left() + 3.0, rect.bottom() + 2.0),
                ),
                CornerRadius::same(2),
                color,
            );
        }
        let text_color = if missing {
            // Dimmed, but never below a legible contrast (or the user's minimum).
            crate::theme::ensure_contrast(
                mix(visuals.text_color(), c.sidebar, 0.55),
                c.sidebar,
                DIM_CONTRAST.max(self.config.min_contrast),
            )
        } else if focused || in_active {
            visuals.strong_text_color()
        } else {
            visuals.text_color()
        };
        // Unread output and bells, as on tab rows.
        let badge = match open {
            Some(t) if t.bell => Some(c.bell),
            Some(t) if t.activity => Some(c.accent),
            _ => None,
        };

        if expanded {
            let icon = Rect::from_center_size(
                pos2(rect.left() + 15.0, rect.center().y),
                vec2(TILE_EXPANDED, TILE_EXPANDED),
            );
            self.favorite_tile(&painter, icon, &fav.name, text_color, status, badge);
            let right_button = Rect::from_center_size(
                pos2(rect.right() - 13.0, rect.center().y),
                vec2(18.0, 18.0),
            );
            let left_button = right_button.translate(vec2(-20.0, 0.0));
            let left = icon.right() + 7.0;
            let right = rect.right() - if tab.is_some() { 48.0 } else { 28.0 };
            let mut job = LayoutJob::simple_singleline(
                row_label(&fav.name).to_string(),
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
            let hovered = resp.hovered()
                || ui.rect_contains_pointer(right_button)
                || ui.rect_contains_pointer(left_button);
            if hovered {
                // Open: ↻ reset and × close. No tab: ▶ open.
                let buttons: Vec<(Rect, &str, &str, Action)> = match tab {
                    Some(t) => vec![
                        (
                            left_button,
                            "↻",
                            "Reset: start it again from the favorite",
                            Action::ResetFavorite(id.clone()),
                        ),
                        (right_button, "×", "Close its tab", Action::Close(t)),
                    ],
                    None => vec![(
                        right_button,
                        "▶",
                        "Open",
                        Action::OpenFavorite(id.clone(), None),
                    )],
                };
                for (r, glyph, tip, action) in buttons {
                    let button = ui
                        .interact(r, Id::new(("favorite_button", id, glyph)), Sense::click())
                        .on_hover_text(tip);
                    if button.hovered() {
                        painter.rect_filled(r, CornerRadius::same(4), c.raised_sidebar(0.28));
                    }
                    let size = if glyph == "×" { 15.0 } else { 13.0 };
                    painter.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        glyph,
                        FontId::proportional(size),
                        visuals.text_color(),
                    );
                    if button.clicked() {
                        actions.push(action);
                    }
                }
            }
        } else {
            let icon = Rect::from_center_size(rect.center(), vec2(TILE_COLLAPSED, TILE_COLLAPSED));
            self.favorite_tile(&painter, icon, &fav.name, text_color, status, badge);
            if focused {
                painter.rect_stroke(
                    icon.expand(2.5),
                    CornerRadius::same(7),
                    Stroke::new(1.5, c.accent),
                    StrokeKind::Outside,
                );
            }
        }
        if focused && self.scroll_to_focused {
            resp.scroll_to_me(None);
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
            if let Some(process) = open.and_then(|t| t.process.as_ref()) {
                ui.label(RichText::new(format!("running: {process}")).color(dim));
            }
            let shortcut = self.keybinds.hint(Shortcut::OpenFavorite(index as u8 + 1));
            if index < 9 && !shortcut.is_empty() {
                ui.label(RichText::new(format!("Open{shortcut}")).color(dim));
            }
        });
        if resp.clicked() {
            actions.push(Action::OpenFavorite(id.clone(), None));
        }
        if resp.middle_clicked()
            && let Some(t) = tab
        {
            actions.push(Action::Close(t));
        }
        if resp.double_clicked() && expanded {
            actions.push(Action::EditFavorite(id.clone()));
        }
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), FavDrag(id.clone()));
        }
        let kb = &self.keybinds;
        resp.context_menu(|ui| {
            let mut item = |ui: &mut Ui, text: &str, shortcut: Option<Shortcut>, action: Action| {
                let label = shortcut.and_then(|s| kb.label(s)).unwrap_or_default();
                if ui
                    .add(egui::Button::new(text).shortcut_text(label))
                    .clicked()
                {
                    actions.push(action);
                    ui.close();
                }
            };
            match tab {
                Some(t) => {
                    item(
                        ui,
                        "Reset",
                        Some(Shortcut::ResetTab),
                        Action::ResetFavorite(id.clone()),
                    );
                    item(ui, "Split right with new tab", None, Action::SplitFrom(t));
                    if in_split {
                        item(
                            ui,
                            "Minimise from split",
                            Some(Shortcut::MinimizePane),
                            Action::Minimize(t),
                        );
                    }
                }
                None => {
                    item(ui, "Open", None, Action::OpenFavorite(id.clone(), None));
                    item(
                        ui,
                        "Open in split",
                        None,
                        Action::OpenFavorite(id.clone(), Some(Edge::Right)),
                    );
                }
            }
            item(
                ui,
                "Reveal in files panel",
                None,
                Action::RevealFavorite(id.clone()),
            );
            item(ui, "Rename / edit…", None, Action::EditFavorite(id.clone()));
            ui.separator();
            if let Some(t) = tab {
                item(ui, "Close tab", Some(Shortcut::CloseTab), Action::Close(t));
            }
            item(
                ui,
                "Remove favorite",
                None,
                Action::RemoveFavorite(id.clone()),
            );
        });
        rect
    }

    /// A favorite's tile, the same in both sidebar widths: the name's first grapheme in a
    /// rounded square, the status dot on its bottom-right corner and the unread or bell
    /// badge on its top-right one.
    fn favorite_tile(
        &self,
        painter: &egui::Painter,
        icon: Rect,
        name: &str,
        text_color: Color32,
        status: FavStatus,
        badge: Option<Color32>,
    ) {
        let c = &self.chrome;
        painter.rect_stroke(
            icon,
            CornerRadius::same(5),
            Stroke::new(1.0, c.raised_sidebar(0.2)),
            StrokeKind::Inside,
        );
        painter.text(
            icon.center(),
            Align2::CENTER_CENTER,
            first_grapheme(name),
            FontId::proportional(icon.height() * 0.6),
            text_color,
        );
        self.status_dot(painter, icon.right_bottom() + vec2(-1.0, -1.0), status);
        if let Some(color) = badge {
            let at = icon.right_top() + vec2(-1.0, 1.0);
            painter.circle_filled(at, 4.5, color);
            painter.circle_stroke(at, 4.5, Stroke::new(1.5, c.sidebar));
        }
    }

    /// Open: filled green. Exited: filled red. No tab: a hollow ring.
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

/// The name next to a favorite's tile: without a leading emoji or symbol, which the tile
/// already shows. A name that starts with a letter or digit stays whole.
pub(super) fn row_label(name: &str) -> &str {
    let name = name.trim();
    let first = first_grapheme(name);
    let rest = name[first.len()..].trim_start();
    let symbol = first.chars().next().is_some_and(|c| !c.is_alphanumeric());
    if symbol && !rest.is_empty() {
        rest
    } else {
        name
    }
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

    #[test]
    fn row_label_drops_the_emoji_the_tile_shows() {
        assert_eq!(row_label("🚀 tmp"), "tmp");
        assert_eq!(row_label("❤\u{FE0F}  love"), "love");
        assert_eq!(row_label("api"), "api");
        assert_eq!(row_label("1 web"), "1 web");
        assert_eq!(row_label("🚀"), "🚀");
        assert_eq!(row_label(" é "), "é");
        assert_eq!(row_label(""), "");
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
