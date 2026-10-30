//! What the main area shows with no tab open: how to open one, like an editor's empty
//! workbench.

use eframe::egui::{self, Align, FontId, Layout, RichText, Ui, vec2};

use crate::app::App;
use crate::config::keybinds::Action;
use crate::theme::{ensure_contrast, mix};

/// Hints are dimmed, but never below this contrast (or the user's `min_contrast`).
const DIM_CONTRAST: f32 = 4.5;

impl App {
    pub(super) fn empty_area(&mut self, ui: &mut Ui) {
        let c = self.chrome.clone();
        let dim = ensure_contrast(
            mix(c.fg, c.bg, 0.45),
            c.bg,
            DIM_CONTRAST.max(self.config.min_contrast),
        );
        let (mut open, mut reopen) = (false, false);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(c.bg))
            .show(ui, |ui| {
                let block = vec2(ui.available_width().min(360.0), 150.0);
                let top = ((ui.available_height() - block.y) / 2.0).max(0.0);
                ui.add_space(top);
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ui.label(
                        RichText::new("No terminal open")
                            .font(FontId::proportional(18.0))
                            .color(c.fg)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    let hint = match self.keybinds.label(Action::NewTab) {
                        Some(key) => format!("Press {key} to open a terminal, or click below."),
                        None => "Click below to open a terminal.".to_string(),
                    };
                    ui.label(RichText::new(hint).color(dim));
                    ui.add_space(14.0);
                    let button = egui::Button::new(
                        RichText::new("New terminal").color(c.readable_on(c.accent)),
                    )
                    .fill(c.accent)
                    .min_size(vec2(140.0, 28.0));
                    open = ui.add(button).clicked();
                    if self.can_reopen() {
                        ui.add_space(6.0);
                        let label = format!(
                            "Reopen closed tab{}",
                            self.keybinds.hint(Action::ReopenClosedTab)
                        );
                        let button = egui::Button::new(RichText::new(label).color(c.fg))
                            .fill(c.raised(0.14))
                            .min_size(vec2(140.0, 28.0));
                        reopen = ui.add(button).clicked();
                    }
                });
            });
        if open {
            self.new_tab(0, None);
        } else if reopen {
            self.reopen_closed_tab();
        }
    }
}
