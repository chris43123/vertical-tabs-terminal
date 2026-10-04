//! The main area: renders the active view's split tree, pane headers (─ minimise, × close),
//! drag-to-split drop zones, splitter resizing, and mouse input (selection, reporting, scroll).
//! Preview tabs render through `ui/preview.rs`; files dragged from the tree split a pane to show
//! them, or type their path into a terminal.

mod drop_zones;
mod mouse;

use std::time::Duration;

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Rect, Sense, Stroke, StrokeKind,
    Ui, pos2, vec2,
};

use crate::app::{App, TabDrag};
use crate::render::{grid_size_for, paint_terminal};
use crate::workspace::{Dir, Drop, TabId};

const HEADER_HEIGHT: f32 = 24.0;
const PADDING: f32 = 4.0;

/// Cursor blink half-period: on for this long, then off for this long.
const BLINK_HALF: Duration = Duration::from_millis(530);
/// Visual bell flash length: a fade, or a constant overlay with `reduce_motion`.
const FLASH_FADE: f32 = 0.2;
const FLASH_STEADY: f32 = 0.15;

/// Whether a blinking cursor is in its visible phase `elapsed` after the last reset.
fn blink_on(elapsed: Duration) -> bool {
    (elapsed.as_millis() / BLINK_HALF.as_millis()).is_multiple_of(2)
}

/// Time until the cursor next changes phase.
fn until_toggle(elapsed: Duration) -> Duration {
    let half = BLINK_HALF.as_millis();
    Duration::from_millis((half - elapsed.as_millis() % half) as u64)
}

enum PaneAction {
    Focus(TabId),
    Minimize(TabId),
    Close(TabId),
    Drop(TabId, TabId, Drop),
    DropPath(TabId, std::path::PathBuf, Drop),
}

impl App {
    pub(crate) fn panes(&mut self, ui: &mut Ui) {
        let Some(view) = self.ws.active_view().cloned() else {
            return;
        };
        let split = view.root.is_split();
        // Split gaps recede; a single pane fills with its own (possibly OSC-overridden) background.
        let fill = if split {
            self.chrome.recessed(0.3)
        } else {
            self.tab_background(view.focused)
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(fill))
            .show(ui, |ui| {
                let area = ui.max_rect();
                let area = if split { area.shrink(PADDING) } else { area };
                let (mut panes, mut splitters) = (Vec::new(), Vec::new());
                view.root.layout(area, &mut panes, &mut splitters);
                self.pane_rects = panes.clone();

                // Splitters.
                for s in &splitters {
                    let resp = ui.interact(
                        s.rect.expand(2.0),
                        Id::new(("splitter", &s.path)),
                        Sense::drag(),
                    );
                    let cursor = match s.dir {
                        Dir::Horizontal => CursorIcon::ResizeHorizontal,
                        Dir::Vertical => CursorIcon::ResizeVertical,
                    };
                    let resp = resp.on_hover_cursor(cursor);
                    if let (true, Some(pos)) = (resp.dragged(), resp.interact_pointer_pos()) {
                        let ratio = match s.dir {
                            Dir::Horizontal => (pos.x - s.parent.left()) / s.parent.width(),
                            Dir::Vertical => (pos.y - s.parent.top()) / s.parent.height(),
                        };
                        let active = self.ws.active;
                        self.ws.views[active].root.set_ratio(&s.path, ratio);
                    }
                    if resp.hovered() || resp.dragged() {
                        ui.painter().rect_filled(
                            s.rect.shrink(1.0),
                            CornerRadius::same(2),
                            self.chrome.accent,
                        );
                    }
                }

                let mut actions = Vec::new();
                for &(id, rect) in &panes {
                    self.pane(ui, id, rect, split, view.focused == id, &mut actions);
                }

                for action in actions {
                    match action {
                        PaneAction::Focus(id) => self.activate(id),
                        PaneAction::Minimize(id) => self.ws.minimize(id),
                        PaneAction::Close(id) => self.close_tab(id),
                        PaneAction::Drop(dragged, target, drop) => {
                            self.ws.drop_on(dragged, target, drop);
                            self.activate(dragged);
                        }
                        PaneAction::DropPath(id, path, drop) => self.drop_path(id, path, drop),
                    }
                }
            });
    }

    fn pane(
        &mut self,
        ui: &mut Ui,
        id: TabId,
        rect: Rect,
        split: bool,
        focused: bool,
        actions: &mut Vec<PaneAction>,
    ) {
        let term_bg = self.tab_background(id);
        let window_focused = ui.input(|i| i.focused);

        let mut body = rect;
        if split {
            let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_HEIGHT));
            body.min.y = header.bottom();
            self.pane_header(ui, id, header, focused, actions);
            ui.painter().rect_filled(
                body,
                CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 6,
                    se: 6,
                },
                term_bg,
            );
        }
        if self.tabs.get(&id).is_some_and(|t| t.preview().is_some()) {
            // Any click inside a preview focuses it (its widgets still get the click).
            let pressed = ui.input(|i| i.pointer.any_pressed());
            if pressed && !focused && ui.rect_contains_pointer(body) {
                actions.push(PaneAction::Focus(id));
            }
            if !split {
                ui.painter().rect_filled(body, 0.0, term_bg);
            }
            self.preview_pane(ui, id, body);
            self.pane_overlays(ui, id, rect, body, split, focused);
            if split && !focused {
                ui.painter().rect_filled(
                    body,
                    CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 6,
                        se: 6,
                    },
                    Color32::from_black_alpha(25),
                );
            }
            self.drop_zone(ui, id, rect, actions);
            self.file_drop(ui, id, rect, actions);
            return;
        }
        let inner = body.shrink2(vec2(PADDING + 2.0, PADDING));

        // Select this pane's size for everything below (grid, mouse, painting).
        let (pane_size, ppp) = (self.pane_font_size(id), ui.ctx().pixels_per_point());
        self.fonts.update(ui.ctx(), pane_size, ppp);
        let Some(session) = self.tabs.get_mut(&id).and_then(|t| t.session_mut()) else {
            return;
        };
        let size = grid_size_for(inner, &self.fonts);
        session.resize(size, self.fonts.cell_px());

        let resp = ui.interact(body, Id::new(("term", id)), Sense::click_and_drag());
        if (resp.clicked() || resp.drag_started() || resp.secondary_clicked()) && !focused {
            actions.push(PaneAction::Focus(id));
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
        }
        self.pane_mouse(ui, id, &resp, inner);

        let Some(session) = self.tabs[&id].session() else {
            return;
        };
        {
            let term = session.term.lock();
            // Blink only while this pane is the one being typed into, and never with
            // reduce_motion. Repaints are scheduled for the next toggle only, so an idle
            // or unfocused window sleeps.
            let active = focused && window_focused;
            let blinking = active && !self.config.reduce_motion && term.cursor_style().blinking;
            let elapsed = self.blink_epoch.elapsed();
            let cursor_visible = !blinking || blink_on(elapsed);
            if blinking {
                ui.ctx().request_repaint_after(until_toggle(elapsed));
            }
            paint_terminal(
                &ui.painter_at(inner),
                inner,
                &term,
                &mut self.fonts,
                &self.palette,
                focused && window_focused,
                &crate::render::TermOpts {
                    min_contrast: self.config.min_contrast,
                    cursor_thickness: self.config.cursor.thickness,
                    cursor_visible,
                },
            );
        }
        // Dim inactive panes a little so the focused one stands out.
        if split && !focused {
            ui.painter().rect_filled(
                body,
                CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 6,
                    se: 6,
                },
                Color32::from_black_alpha(40),
            );
        }

        self.pane_overlays(ui, id, rect, body, split, focused);
        self.drop_zone(ui, id, rect, actions);
        self.file_drop(ui, id, rect, actions);
    }

    /// Drawn over a finished pane: the visual bell flash and, in splits, the focus border.
    fn pane_overlays(
        &mut self,
        ui: &Ui,
        id: TabId,
        rect: Rect,
        body: Rect,
        split: bool,
        focused: bool,
    ) {
        let radius = if split {
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: 6,
                se: 6,
            }
        } else {
            CornerRadius::ZERO
        };
        let reduce = self.config.reduce_motion;
        if let Some(tab) = self.tabs.get_mut(&id)
            && let Some(start) = tab.bell_flash
        {
            let t = start.elapsed().as_secs_f32();
            let (len, strength) = if reduce {
                (FLASH_STEADY, 1.0)
            } else {
                (FLASH_FADE, 1.0 - t / FLASH_FADE)
            };
            if t >= len {
                tab.bell_flash = None;
            } else {
                ui.painter().rect_filled(
                    body,
                    radius,
                    self.chrome.fg.gamma_multiply(0.2 * strength),
                );
                if reduce {
                    // One repaint to take the overlay off again.
                    ui.ctx()
                        .request_repaint_after(Duration::from_secs_f32(len - t));
                } else {
                    ui.ctx().request_repaint();
                }
            }
        }
        let width = self.config.focus_border_width.clamp(0.0, 6.0);
        if split && focused && width > 0.0 {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(6),
                Stroke::new(width, self.chrome.accent),
                StrokeKind::Inside,
            );
        }
    }

    fn pane_header(
        &self,
        ui: &mut Ui,
        id: TabId,
        header: Rect,
        focused: bool,
        actions: &mut Vec<PaneAction>,
    ) {
        let Some(tab) = self.tabs.get(&id) else {
            return;
        };
        let c = &self.chrome;
        let painter = ui.painter();
        let bg = if focused {
            c.raised(0.1)
        } else {
            c.raised(0.04)
        };
        painter.rect_filled(
            header,
            CornerRadius {
                nw: 6,
                ne: 6,
                sw: 0,
                se: 0,
            },
            bg,
        );
        if focused {
            painter.hline(
                header.x_range().shrink(6.0),
                header.top() + 1.0,
                Stroke::new(2.0, self.chrome.accent),
            );
        }

        // Dragging the header re-drags the pane (to swap or re-split it).
        let resp = ui.interact(
            header,
            Id::new(("pane_header", id)),
            Sense::click_and_drag(),
        );
        if resp.clicked() && !focused {
            actions.push(PaneAction::Focus(id));
        }
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), TabDrag(id));
        }

        let text_color = if focused {
            ui.visuals().strong_text_color()
        } else {
            ui.visuals().text_color()
        };
        let buttons_w = 48.0;
        let mut job = LayoutJob::simple_singleline(
            format!("{}  {}", tab.profile.icon, tab.title()),
            FontId::proportional(12.5),
            text_color,
        );
        job.wrap = TextWrapping::truncate_at_width(header.width() - buttons_w - 16.0);
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        painter.galley(
            pos2(
                header.left() + 10.0,
                header.center().y - galley.size().y / 2.0,
            ),
            galley,
            text_color,
        );

        let close_rect = Rect::from_center_size(
            pos2(header.right() - 14.0, header.center().y),
            vec2(20.0, 18.0),
        );
        let min_rect = close_rect.translate(vec2(-22.0, 0.0));
        let close_tip = format!(
            "Close{}",
            self.keybinds
                .hint(crate::config::keybinds::Action::CloseTab)
        );
        for (r, glyph, tip, is_close) in [
            (min_rect, "—", "Minimise (move back to its own tab)", false),
            (close_rect, "×", close_tip.as_str(), true),
        ] {
            let b = ui
                .interact(r, Id::new((glyph, id)), Sense::click())
                .on_hover_text(tip);
            if b.hovered() {
                let fill = if is_close { c.danger } else { c.raised(0.26) };
                painter.rect_filled(r, CornerRadius::same(4), fill);
            }
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                glyph,
                FontId::proportional(13.0),
                text_color,
            );
            if b.clicked() {
                actions.push(if is_close {
                    PaneAction::Close(id)
                } else {
                    PaneAction::Minimize(id)
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blink_phases() {
        let ms = Duration::from_millis;
        assert!(blink_on(ms(0)));
        assert!(blink_on(ms(529)));
        assert!(!blink_on(ms(530)));
        assert!(!blink_on(ms(1059)));
        assert!(blink_on(ms(1060)));
    }

    #[test]
    fn toggle_countdown() {
        let ms = Duration::from_millis;
        assert_eq!(until_toggle(ms(0)), ms(530));
        assert_eq!(until_toggle(ms(500)), ms(30));
        assert_eq!(until_toggle(ms(530)), ms(530));
    }
}
