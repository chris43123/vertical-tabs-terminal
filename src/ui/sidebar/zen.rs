//! The collapsed strip that peeks open on hover, and the zen-mode overlay revealed from the
//! left window edge.

use eframe::egui::{self, Color32, Id, Order, Rect, vec2};

use crate::app::App;

use super::Action;

impl App {
    /// While collapsed, hovering the strip shows the full sidebar floating over the panes
    /// (so terminals don't resize).
    pub(super) fn sidebar_peek_overlay(
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
            self.side.peek = true;
        } else if self.side.peek {
            let keep = pointer.is_some_and(|p| overlay.expand(8.0).contains(p))
                || ctx.any_popup_open()
                || self.renaming.is_some();
            if !keep || dragging {
                self.side.peek = false;
            }
        }
        if !self.side.peek {
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

    /// Zen mode: while the side area is hidden, touching the window's left edge shows the
    /// sidebar at full width, floating over the panes (so terminals don't resize).
    pub(crate) fn zen_sidebar(&mut self, ctx: &egui::Context, area: Rect) {
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let dragging = egui::DragAndDrop::has_any_payload(ctx);
        let width = if self.side.zen_files {
            self.config.files.width
        } else {
            self.config.sidebar_width
        };
        let overlay = Rect::from_min_size(area.min, vec2(width, area.height()));

        if pointer.is_some_and(|p| p.x <= area.min.x + 4.0 && area.contains(p)) && !dragging {
            if !self.side.zen_peek {
                self.side.zen_peek = true;
            }
            self.side.zen_hovered_once = true;
        } else if self.side.zen_peek {
            let inside = pointer.is_some_and(|p| overlay.expand(8.0).contains(p));
            self.side.zen_hovered_once |= inside;
            let escape =
                !self.files.search_focused && ctx.input(|i| i.key_pressed(egui::Key::Escape));
            // Opened by keyboard: stays until the pointer has entered and left again.
            let keep = (inside
                || !self.side.zen_hovered_once
                || self.files.search_focused
                || ctx.any_popup_open()
                || self.renaming.is_some())
                && !escape;
            if !keep || dragging {
                self.side.zen_peek = false;
            }
        }
        if !self.side.zen_peek {
            return;
        }

        let mut actions = Vec::new();
        let c = self.chrome.clone();
        let fill = if self.side.zen_files {
            crate::theme::mix(c.sidebar, c.bg, 0.45)
        } else {
            ctx.global_style().visuals.panel_fill
        };
        egui::Area::new(Id::new(if self.side.zen_files {
            "files_zen"
        } else {
            "sidebar_zen"
        }))
        .fixed_pos(area.min)
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
                    ui.set_width(width - 12.0);
                    ui.set_height(area.height() - 12.0);
                    if self.side.zen_files {
                        self.files_overlay_contents(ui);
                    } else {
                        self.sidebar_contents(ui, true, &mut actions);
                    }
                });
        });
        self.drag_ghost(ctx);
        self.scroll_to_focused = false;
        for action in actions {
            self.apply(action);
        }
    }
}
