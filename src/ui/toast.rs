//! A short note at the bottom of a pane confirming a copy ("Copied 3,000 lines"), so copying
//! a whole file or a long selection doesn't happen silently. It sits on one scheduled repaint
//! while shown (no polling), fades out unless `reduce_motion` is on, and never takes clicks.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, CornerRadius, Id, Order, Pos2, RichText, Stroke, pos2};

use crate::app::App;
use crate::workspace::TabId;

/// How long a toast stays fully visible.
const HOLD: Duration = Duration::from_millis(1600);
/// Then how long it fades for (skipped with `reduce_motion`).
const FADE: Duration = Duration::from_millis(250);

pub struct Toast {
    pub message: String,
    /// The pane it belongs to; shown at the bottom of the window when it isn't on screen.
    pub pane: Option<TabId>,
    /// Shown as a problem (the clipboard couldn't be written).
    pub failed: bool,
    pub shown: Instant,
}

impl Toast {
    pub fn new(message: String, pane: Option<TabId>, failed: bool) -> Self {
        Self {
            message,
            pane,
            failed,
            shown: Instant::now(),
        }
    }
}

/// "Copied 3,000 lines" or "Copied 42 characters", for `text` that was just copied.
pub fn copied_summary(text: &str) -> String {
    let (n, unit) = if text.contains('\n') {
        (text.split('\n').count(), "line")
    } else {
        (text.chars().count(), "character")
    };
    format!("Copied {}", count(n, unit))
}

/// `n` with thousands separators and `unit`, pluralized: "1 line", "3,000 lines".
pub fn count(n: usize, unit: &str) -> String {
    let digits = n.to_string();
    let mut grouped = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(d);
    }
    let s = if n == 1 { "" } else { "s" };
    format!("{grouped} {unit}{s}")
}

/// Opacity `elapsed` after a toast appeared, or `None` once it's gone; and when to repaint
/// next (`Duration::ZERO`: every frame, while fading).
fn phase(elapsed: Duration, reduce_motion: bool) -> Option<(f32, Duration)> {
    if elapsed < HOLD {
        return Some((1.0, HOLD - elapsed));
    }
    let fade = if reduce_motion { Duration::ZERO } else { FADE };
    let into = elapsed - HOLD;
    (into < fade).then(|| {
        (
            1.0 - into.as_secs_f32() / fade.as_secs_f32(),
            Duration::ZERO,
        )
    })
}

impl App {
    /// Copy `text` and confirm it with a toast saying `message` over `pane`.
    pub fn copy_with_toast(&mut self, text: String, message: String, pane: Option<TabId>) {
        let ok = self.try_set_clipboard(text);
        self.show_toast(message, pane, !ok);
    }

    /// Confirm a copy that egui made itself (rendered markdown's own selection).
    pub fn show_toast(&mut self, message: String, pane: Option<TabId>, failed: bool) {
        let message = if failed {
            "Couldn't copy: the clipboard isn't available".to_string()
        } else {
            message
        };
        // Copies happen during a frame, before toast_ui draws it: no extra repaint needed.
        self.toast = Some(Toast::new(message, pane, failed));
    }

    pub(crate) fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some(toast) = &self.toast else {
            return;
        };
        let Some((opacity, next)) = phase(toast.shown.elapsed(), self.config.reduce_motion) else {
            self.toast = None;
            return;
        };
        if next.is_zero() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(next);
        }
        let screen = ctx.content_rect();
        let area = toast
            .pane
            .and_then(|p| self.pane_rects.iter().find(|(id, _)| *id == p))
            .map_or(screen, |(_, r)| *r);
        let anchor: Pos2 = pos2(area.center().x, area.bottom() - 16.0);
        let c = &self.chrome;
        let (stroke, text) = if toast.failed {
            (c.danger, c.danger)
        } else {
            (c.raised(0.25), c.fg)
        };
        egui::Area::new(Id::new("copy_toast"))
            .order(Order::Foreground)
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(anchor)
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                egui::Frame::new()
                    .fill(c.raised(0.1))
                    .stroke(Stroke::new(1.0, stroke))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.label(RichText::new(&toast.message).color(text));
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_what_was_copied() {
        assert_eq!(copied_summary("hello"), "Copied 5 characters");
        assert_eq!(copied_summary("ñ"), "Copied 1 character");
        assert_eq!(copied_summary("a\nb"), "Copied 2 lines");
        let big = vec!["x"; 3000].join("\n");
        assert_eq!(copied_summary(&big), "Copied 3,000 lines");
        assert_eq!(count(1_234_567, "line"), "1,234,567 lines");
        assert_eq!(count(999, "line"), "999 lines");
        assert_eq!(count(0, "character"), "0 characters");
    }

    #[test]
    fn holds_then_fades_then_goes() {
        let ms = Duration::from_millis;
        assert_eq!(phase(ms(0), false), Some((1.0, HOLD)));
        // While held, the only repaint is scheduled for when the fade starts.
        assert_eq!(phase(ms(600), false), Some((1.0, HOLD - ms(600))));
        let (opacity, next) = phase(HOLD + FADE / 2, false).unwrap();
        assert!((opacity - 0.5).abs() < 0.01);
        assert!(next.is_zero());
        assert_eq!(phase(HOLD + FADE, false), None);
        // reduce_motion: no fade, it just goes.
        assert_eq!(phase(HOLD, true), None);
        assert!(phase(HOLD - ms(1), true).is_some());
    }
}
