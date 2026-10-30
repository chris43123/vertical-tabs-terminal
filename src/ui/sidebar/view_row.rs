//! The header above a split view's tabs: its name (from its folder, or set by the user), a
//! close button, and in the collapsed strip a two-letter monogram. Each view gets its own
//! color, picked from its oldest tab and kept readable on the sidebar.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Rect, Sense, Stroke, StrokeKind, Ui, pos2,
    vec2,
};

use crate::app::{App, RenameTarget};
use crate::theme::contrast_ratio;
use crate::theme::mix;
use crate::ui::view_names::monogram;
use crate::workspace::TabId;

use super::{Action, FOLDER_INDENT, VIEW_HEIGHT};

/// Minimum contrast of a view's color against the sidebar (WCAG's for UI components).
const MIN_CONTRAST: f32 = 3.0;

impl App {
    /// Color of view `v`: the same for the view's whole life, dimmed unless it's on screen.
    pub(super) fn view_color(&self, v: usize) -> Color32 {
        let seed = self.ws.views[v]
            .root
            .leaves()
            .into_iter()
            .min()
            .unwrap_or(0);
        let color = view_hue(seed, self.chrome.sidebar, self.chrome.light);
        if v == self.ws.active {
            color
        } else {
            mix(color, self.chrome.sidebar, 0.35)
        }
    }

    /// Header of the split view holding `id`. Returns its rect.
    pub(super) fn view_row(
        &mut self,
        ui: &mut Ui,
        id: TabId,
        expanded: bool,
        actions: &mut Vec<Action>,
    ) -> Rect {
        let (full, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), VIEW_HEIGHT), Sense::click());
        let Some(v) = self.ws.view_of(id) else {
            return full;
        };
        let c = self.chrome.clone();
        // Inside a folder: indented like the folder's tabs, with its color down the left.
        let rect = match self.ws.group_of(id) {
            Some(g) => {
                let x = if expanded {
                    full.left() + 8.0
                } else {
                    full.left() + 1.0
                };
                ui.painter().vline(
                    x,
                    full.y_range().expand(1.0),
                    Stroke::new(2.0, mix(self.folder_color(g), c.sidebar, 0.35)),
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
        let name = self.view_name(v);
        let custom = self.ws.views[v].name.is_some();
        let auto = self.view_names.get(v).cloned().flatten();
        let color = self.view_color(v);

        if resp.hovered() {
            painter.rect_filled(rect, CornerRadius::same(6), c.raised_sidebar(0.06));
        }
        // The split bracket starts here and runs down the view's tabs.
        painter.rect_filled(
            Rect::from_min_max(
                pos2(rect.left(), rect.center().y),
                pos2(rect.left() + 3.0, rect.bottom() + 2.0),
            ),
            CornerRadius::same(2),
            color,
        );

        if expanded {
            // Split icon in the tab icons' column.
            let icon =
                Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(13.0, 11.0));
            painter.rect_stroke(
                icon,
                CornerRadius::same(2),
                Stroke::new(1.3, color),
                StrokeKind::Inside,
            );
            painter.vline(icon.center().x, icon.y_range(), Stroke::new(1.3, color));

            let left = rect.left() + 37.0;
            let mut right = rect.right() - 8.0;
            let close_rect = Rect::from_center_size(
                pos2(rect.right() - 14.0, rect.center().y),
                vec2(18.0, 18.0),
            );
            let hovered = resp.hovered() || ui.rect_contains_pointer(close_rect);
            let elsewhere = auto.as_ref().filter(|_| !custom).map_or(0, |n| n.elsewhere);
            if hovered {
                let close = ui.interact(close_rect, Id::new(("close_view", id)), Sense::click());
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
                    ui.visuals().text_color(),
                );
                if close.clicked() {
                    actions.push(Action::CloseView(id));
                }
                close.on_hover_text("Close all tabs in this split");
                right = close_rect.left() - 4.0;
            } else if elsewhere > 0 {
                let galley = painter.layout_no_wrap(
                    format!("+{elsewhere}"),
                    FontId::proportional(11.0),
                    mix(c.fg, c.sidebar, 0.4),
                );
                let pill = Rect::from_min_size(
                    pos2(right - galley.size().x - 10.0, rect.center().y - 8.0),
                    vec2(galley.size().x + 10.0, 16.0),
                );
                painter.rect_stroke(
                    pill,
                    CornerRadius::same(8),
                    Stroke::new(1.0, c.raised_sidebar(0.2)),
                    StrokeKind::Inside,
                );
                painter.galley(
                    pill.center() - galley.size() / 2.0,
                    galley,
                    mix(c.fg, c.sidebar, 0.4),
                );
                right = pill.left() - 6.0;
            }

            if self
                .renaming
                .as_ref()
                .is_some_and(|(r, _)| *r == RenameTarget::View(id))
            {
                self.rename_box(
                    ui,
                    Rect::from_min_max(
                        pos2(left, rect.top() + 2.0),
                        pos2(rect.right() - 4.0, rect.bottom() - 2.0),
                    ),
                    actions,
                );
            } else {
                let strong = ui.visuals().strong_text_color();
                let mut job =
                    LayoutJob::simple_singleline(name.clone(), FontId::proportional(12.5), strong);
                job.wrap = TextWrapping::truncate_at_width(right - left);
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                painter.galley(
                    pos2(left, rect.center().y - galley.size().y / 2.0),
                    galley,
                    strong,
                );
            }
        } else {
            let tile = Rect::from_center_size(rect.center(), vec2(32.0, 17.0));
            painter.rect_filled(tile, CornerRadius::same(5), color);
            painter.text(
                tile.center(),
                Align2::CENTER_CENTER,
                monogram(&name),
                FontId::proportional(11.0),
                c.readable_on(color),
            );
        }

        let mut tip = name;
        if let Some(n) = &auto {
            tip.push('\n');
            tip.push_str(&crate::ui::display_path(&n.dir));
            if n.elsewhere > 0 && !custom {
                let s = if n.elsewhere == 1 { "" } else { "s" };
                tip.push_str(&format!("\n{} pane{s} in another folder", n.elsewhere));
            }
        }
        if custom {
            tip.push_str("\nRenamed (right-click to use the folder's name)");
        }
        let resp = resp.on_hover_text(tip);
        if resp.double_clicked() && expanded {
            actions.push(Action::StartRename(RenameTarget::View(id)));
        } else if resp.clicked() {
            actions.push(Action::Activate(self.ws.views[v].focused));
        }
        resp.context_menu(|ui| {
            if ui.button("Rename split").clicked() {
                actions.push(Action::StartRename(RenameTarget::View(id)));
                ui.close();
            }
            if custom && ui.button("Use the folder's name").clicked() {
                actions.push(Action::ResetViewName(id));
                ui.close();
            }
            ui.separator();
            if ui.button("Close split").clicked() {
                actions.push(Action::CloseView(id));
                ui.close();
            }
        });
        full
    }
}

/// A hue picked from `seed`, brightened (dark sidebar) or darkened (light sidebar) until it
/// stands out from `sidebar`.
fn view_hue(seed: u64, sidebar: Color32, light: bool) -> Color32 {
    // Golden-ratio steps spread consecutive tab ids around the color wheel.
    let hue = (seed as f64 * 0.618_034).fract() as f32;
    let mut value: f32 = if light { 0.62 } else { 0.82 };
    loop {
        let color: Color32 = egui::ecolor::Hsva::new(hue, 0.62, value, 1.0).into();
        let done = if light { value <= 0.2 } else { value >= 1.0 };
        if done || contrast_ratio(color, sidebar) >= MIN_CONTRAST {
            return color;
        }
        value = if light {
            value - 0.04
        } else {
            (value + 0.04).min(1.0)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_colors_stand_out_from_the_sidebar() {
        for sidebar in [
            Color32::from_rgb(16, 19, 26),
            Color32::from_rgb(240, 240, 236),
        ] {
            let light = sidebar.r() > 128;
            for seed in 0..50 {
                let color = view_hue(seed, sidebar, light);
                assert!(
                    contrast_ratio(color, sidebar) >= MIN_CONTRAST,
                    "{seed} {color:?}"
                );
            }
        }
        assert_eq!(
            view_hue(7, Color32::BLACK, false),
            view_hue(7, Color32::BLACK, false)
        );
    }
}
