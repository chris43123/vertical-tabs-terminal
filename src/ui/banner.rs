//! A toast in the bottom-right corner listing config/theme problems, so a typo in the config
//! doesn't fail silently. It floats over the panes (no terminal resize) and clears itself once
//! a reload comes back clean.

use eframe::egui::{self, Align2, CornerRadius, FontId, Id, Order, RichText, Stroke, vec2};

use crate::app::App;

const MAX_SHOWN: usize = 4;

impl App {
    pub(crate) fn problems_banner(&mut self, ctx: &egui::Context) {
        if self.problems.is_empty() || self.problems_dismissed {
            return;
        }
        let c = self.chrome.clone();
        let hint = self
            .keybinds
            .hint(crate::config::keybinds::Action::OpenSettings);
        let (mut open, mut dismiss) = (false, false);

        egui::Area::new(Id::new("problems_banner"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-16.0, -16.0))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(c.raised(0.07))
                    .stroke(Stroke::new(1.0, c.danger))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(12)
                    .shadow(egui::Shadow {
                        offset: [0, 6],
                        blur: 20,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(120),
                    })
                    .show(ui, |ui| {
                        ui.set_max_width(460.0);
                        let title = match self.problems.len() {
                            1 => "⚠  Settings problem".to_string(),
                            n => format!("⚠  {n} settings problems"),
                        };
                        ui.label(RichText::new(title).strong().color(c.danger));
                        ui.add_space(4.0);
                        for p in self.problems.iter().take(MAX_SHOWN) {
                            ui.label(RichText::new(p).font(FontId::monospace(11.5)).color(c.fg));
                            ui.add_space(2.0);
                        }
                        if self.problems.len() > MAX_SHOWN {
                            ui.weak(format!(
                                "…and {} more (see stderr)",
                                self.problems.len() - MAX_SHOWN
                            ));
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            open = ui.button(format!("Open settings{hint}")).clicked();
                            dismiss = ui.button("Dismiss").clicked();
                        });
                    });
            });

        if dismiss {
            self.problems_dismissed = true;
        }
        if open {
            self.open_settings();
        }
    }
}
