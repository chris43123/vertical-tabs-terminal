//! Preview panes: rendered markdown, syntax-highlighted code (only the visible lines are laid
//! out, so huge files scroll smoothly), images, and a fallback for everything else.

mod code;
mod markdown;

use std::path::Path;

use eframe::egui::{
    self, Align, FontFamily, FontId, Layout, RichText, Sense, Stroke, Ui, UiBuilder, vec2,
};

use crate::app::App;
use crate::preview::{Body, Preview};
use crate::theme::mix;
use crate::workspace::TabId;

use code::CodeView;

const TOOLBAR_HEIGHT: f32 = 28.0;

enum Action {
    ToggleSource,
    OpenExternal,
    CopyPath,
    ToggleWrap,
}

impl App {
    pub(crate) fn preview_pane(&mut self, ui: &mut Ui, id: TabId, rect: egui::Rect) {
        let c = self.chrome.clone();
        let mut actions = Vec::new();
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Min)),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        let ui = &mut child;

        let hl = self.previews.highlighter();
        let (syntax_theme, generation) = (
            self.previews.syntax_theme.clone(),
            self.previews.theme_generation,
        );
        // Pane zoom; markdown looks as before at zoom 0 whatever the base size.
        let font_size = self.pane_font_size(id);
        let scale = font_size / self.font_size;
        let ctx = ui.ctx().clone();
        let Some(preview) = self.tabs.get_mut(&id).and_then(|t| t.preview_mut()) else {
            return;
        };
        let highlighted = preview.highlighted(&hl, &syntax_theme, generation, &ctx);

        let wrap = self.previews.wrap;
        toolbar(ui, preview, wrap, &c, &mut actions);

        let body = ui.available_rect_before_wrap();
        let mut body_ui = ui.new_child(UiBuilder::new().max_rect(body));
        let ui = &mut body_ui;
        match &preview.body {
            Body::Markdown(_) if !preview.show_source => {
                let base = preview.path.parent().map(Path::to_path_buf);
                let scroll_id = ("preview_md", id, preview.version);
                let blocks = preview.markdown.clone();
                self.markdown(ui, scroll_id, &blocks, base.as_deref(), scale);
            }
            Body::Markdown(text) | Body::Code(text) => {
                let view = CodeView {
                    id,
                    version: preview.version,
                    text,
                    highlighted: highlighted.as_deref(),
                    font: FontId::new((font_size * 0.92).round(), FontFamily::Monospace),
                };
                if wrap {
                    view.show_wrapped(ui, &mut preview.wrapped, &c);
                } else {
                    view.show(ui, &c);
                }
            }
            Body::Image => {
                let uri = crate::ui::file_uri(&preview.path);
                egui::ScrollArea::both()
                    .id_salt(("preview_img", id))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.add(
                                egui::Image::new(uri)
                                    .max_size(body.size() - vec2(24.0, 24.0))
                                    .fit_to_original_size(1.0)
                                    .maintain_aspect_ratio(true),
                            );
                        });
                    });
            }
            Body::Binary(size) => notice(ui, "Binary file", Some(*size), &c, &mut actions),
            Body::TooLarge(size) => {
                notice(ui, "Too large to preview", Some(*size), &c, &mut actions)
            }
            Body::Error(err) => notice(ui, err, None, &c, &mut actions),
        }

        for action in actions {
            let Some(preview) = self.tabs.get_mut(&id).and_then(|t| t.preview_mut()) else {
                return;
            };
            match action {
                Action::ToggleSource => preview.show_source = !preview.show_source,
                Action::OpenExternal => {
                    if let Err(err) = crate::platform::open_with_default_app(&preview.path) {
                        crate::diag::warn(err);
                    }
                }
                Action::CopyPath => {
                    let path = preview.path.to_string_lossy().into_owned();
                    self.set_clipboard(path);
                }
                Action::ToggleWrap => self.previews.wrap = !self.previews.wrap,
            }
        }
    }
}

fn toolbar(
    ui: &mut Ui,
    preview: &Preview,
    wrap: bool,
    c: &crate::theme::UiColors,
    actions: &mut Vec<Action>,
) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), TOOLBAR_HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c.raised(0.05));
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, c.raised(0.12)),
    );
    let mut bar = ui.new_child(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(10.0, 3.0)))
            .layout(Layout::right_to_left(Align::Center)),
    );
    let ui = &mut bar;
    if ui
        .small_button("↗")
        .on_hover_text("Open with default app")
        .clicked()
    {
        actions.push(Action::OpenExternal);
    }
    if ui.small_button("📋").on_hover_text("Copy path").clicked() {
        actions.push(Action::CopyPath);
    }
    let shows_text = match preview.body {
        Body::Code(_) => true,
        Body::Markdown(_) => preview.show_source,
        _ => false,
    };
    if shows_text
        && ui
            .selectable_label(wrap, "Wrap")
            .on_hover_text("Wrap long lines to the pane's width")
            .clicked()
    {
        actions.push(Action::ToggleWrap);
    }
    if matches!(preview.body, Body::Markdown(_)) {
        let label = if preview.show_source {
            "Rendered"
        } else {
            "Source"
        };
        if ui
            .small_button(label)
            .on_hover_text("Switch between rendered markdown and its source")
            .clicked()
        {
            actions.push(Action::ToggleSource);
        }
    }
    if let Some(t) = preview.text() {
        let mut info = format!("{} lines", t.lines.len());
        if let Some(s) = &t.syntax {
            info = format!("{s} · {info}");
        }
        if t.pretty_printed {
            info.push_str(" · reformatted");
        }
        ui.label(RichText::new(info).size(11.5).color(mix(c.fg, c.bg, 0.45)));
    }
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.add(
            egui::Label::new(
                RichText::new(crate::ui::display_path(&preview.path))
                    .size(12.0)
                    .color(mix(c.fg, c.bg, 0.2)),
            )
            .truncate(),
        )
        .on_hover_text(preview.path.to_string_lossy());
    });
}

fn notice(
    ui: &mut Ui,
    message: &str,
    size: Option<u64>,
    c: &crate::theme::UiColors,
    actions: &mut Vec<Action>,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.35);
        ui.label(RichText::new(message).size(16.0).color(c.fg));
        if let Some(size) = size {
            ui.label(RichText::new(human_size(size)).color(mix(c.fg, c.bg, 0.45)));
        }
        ui.add_space(8.0);
        if ui.button("Open with default app").clicked() {
            actions.push(Action::OpenExternal);
        }
    });
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }
}
