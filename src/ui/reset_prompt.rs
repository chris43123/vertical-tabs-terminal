//! "Reset <tab>? <process> is still running." — asked before Reset tab kills a running
//! program (with `confirm_reset`). Enter resets, Esc or a click outside cancels.

use eframe::egui::{self, CornerRadius, FontId, Id, Key, Modifiers, RichText, Stroke, vec2};

use crate::app::App;
use crate::workspace::TabId;

/// A pending reset confirmation.
pub struct ResetPrompt {
    pub tab: TabId,
    title: String,
    process: String,
    /// Keys are only read from the frame after it opened, so the press that opened it
    /// can't also answer it.
    initialized: bool,
}

impl ResetPrompt {
    pub fn new(tab: TabId, title: String, process: String) -> Self {
        Self {
            tab,
            title,
            process,
            initialized: false,
        }
    }
}

impl App {
    pub(crate) fn reset_prompt_ui(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.reset_prompt.take() else {
            return;
        };
        if !self.tabs.contains_key(&prompt.tab) {
            return;
        }
        let (mut confirm, mut cancel) = (false, false);
        if prompt.initialized {
            ctx.input_mut(|i| {
                confirm = i.consume_key(Modifiers::NONE, Key::Enter);
                cancel = i.consume_key(Modifiers::NONE, Key::Escape);
            });
        }
        prompt.initialized = true;

        let c = self.chrome.clone();
        let frame = egui::Frame::new()
            .fill(c.raised(0.06))
            .stroke(Stroke::new(1.0, c.raised(0.2)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(16);
        let modal = egui::Modal::new(Id::new("reset_prompt"))
            .frame(frame)
            .backdrop_color(c.bg.gamma_multiply(0.55))
            .show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.label(
                    RichText::new(format!("Reset {}?", prompt.title))
                        .font(FontId::proportional(15.0))
                        .color(c.fg)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{} is still running.", prompt.process)).color(c.fg),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let reset =
                        egui::Button::new(RichText::new("Reset").color(c.readable_on(c.danger)))
                            .fill(c.danger)
                            .min_size(vec2(72.0, 0.0));
                    if ui.add(reset).clicked() {
                        confirm = true;
                    }
                    let cancel_button = egui::Button::new(RichText::new("Cancel").color(c.fg))
                        .fill(c.raised(0.14))
                        .min_size(vec2(72.0, 0.0));
                    if ui.add(cancel_button).clicked() {
                        cancel = true;
                    }
                });
            });
        if modal.backdrop_response.clicked() {
            cancel = true;
        }
        if confirm {
            self.reset_tab_now(prompt.tab);
        } else if !cancel {
            self.reset_prompt = Some(prompt);
        }
    }
}
