//! Preview panes: rendered markdown, syntax-highlighted code (only the visible lines are laid
//! out, so huge files scroll smoothly), images, and a fallback for everything else.

use std::path::Path;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Align, Color32, FontFamily, FontId, Layout, RichText, Sense, Stroke, TextFormat,
    TextStyle, Ui, UiBuilder, vec2,
};

use crate::app::App;
use crate::preview::highlight::{self, Span};
use crate::preview::markdown::{Block, Table};
use crate::preview::{Body, Preview, Text};
use crate::theme::mix;
use crate::workspace::TabId;

const TOOLBAR_HEIGHT: f32 = 28.0;
/// Readable line length for rendered markdown.
const MARKDOWN_WIDTH: f32 = 820.0;
/// Longer lines are cut for display.
const MAX_LINE: usize = 20_000;

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

        let hl = self.highlighter();
        let (syntax_theme, generation) = (self.syntax_theme.clone(), self.theme_generation);
        // Pane zoom; markdown looks as before at zoom 0 whatever the base size.
        let font_size = self.pane_font_size(id);
        let scale = font_size / self.font_size;
        let ctx = ui.ctx().clone();
        let Some(preview) = self.tabs.get_mut(&id).and_then(|t| t.preview_mut()) else {
            return;
        };
        let highlighted = preview.highlighted(&hl, &syntax_theme, generation, &ctx);

        let wrap = self.preview_wrap;
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
                Action::ToggleWrap => self.preview_wrap = !self.preview_wrap,
            }
        }
    }

    fn markdown(
        &mut self,
        ui: &mut Ui,
        scroll_id: impl std::hash::Hash + std::fmt::Debug,
        blocks: &[Block],
        base: Option<&Path>,
        s: f32,
    ) {
        let cache = self.md_cache.get_or_insert_with(Default::default);
        if self.md_theme_generation != self.theme_generation {
            let _ = cache.add_syntax_theme_from_bytes(
                highlight::THEME_NAME,
                self.syntax_theme_xml.as_bytes(),
            );
            self.md_theme_generation = self.theme_generation;
        }
        // Relative images (`![](docs/shot.png)`) load from the file's folder.
        let base_uri =
            base.map(|b| format!("{}{}", crate::ui::file_uri(b), std::path::MAIN_SEPARATOR));
        // A viewer for content `width` wide (images are scaled down to fit it).
        let viewer = |width: f32| {
            let v = egui_commonmark::CommonMarkViewer::new()
                .syntax_theme_dark(highlight::THEME_NAME)
                .syntax_theme_light(highlight::THEME_NAME)
                .show_alt_text_on_hover(true)
                .max_image_width(Some(width.max(1.0) as usize));
            match &base_uri {
                Some(uri) => v.default_implicit_uri_scheme(uri.clone()),
                None => v,
            }
        };

        let style = ui.style_mut();
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(15.0 * s));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(30.0 * s));
        style
            .text_styles
            .insert(TextStyle::Monospace, FontId::monospace(13.5 * s));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(15.0 * s));
        style.spacing.item_spacing = vec2(8.0, 8.0) * s;
        // Everything wraps to the column; nothing scrolls sideways.
        style.wrap_mode = Some(egui::TextWrapMode::Wrap);
        let c = self.chrome.clone();

        egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // A centered column of readable width, with side padding on narrow panes.
                let full = ui.available_width();
                let padding = if full < 500.0 { 14.0 } else { 28.0 };
                let width = (full - 2.0 * padding).clamp(1.0, MARKDOWN_WIDTH * s);
                let margin = ((full - width) / 2.0).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(margin);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.add_space(20.0 * s);
                        md_blocks(ui, blocks, cache, &viewer, &c, s);
                        ui.add_space(40.0 * s);
                    });
                });
            });
    }
}

/// Render split markdown into the current column width.
fn md_blocks(
    ui: &mut Ui,
    blocks: &[Block],
    cache: &mut egui_commonmark::CommonMarkCache,
    viewer: &dyn Fn(f32) -> egui_commonmark::CommonMarkViewer<'static>,
    c: &crate::theme::UiColors,
    s: f32,
) {
    for (i, block) in blocks.iter().enumerate() {
        ui.push_id(i, |ui| match block {
            Block::Text(text) => {
                viewer(ui.available_width()).show(ui, cache, text);
            }
            Block::Table(table) => {
                md_table(ui, table, cache, viewer, c, s);
                ui.add_space(8.0 * s);
            }
            Block::List(list) => md_list(ui, list, cache, viewer, c, s),
            Block::Quote(inner) => {
                // A bar down the left, like egui_commonmark's own quotes.
                let response = egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: (12.0 * s).round() as i8,
                        right: 0,
                        top: (2.0 * s).round() as i8,
                        bottom: (2.0 * s).round() as i8,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        md_blocks(ui, inner, cache, viewer, c, s);
                    })
                    .response;
                let r = response.rect;
                ui.painter().vline(
                    r.left() + 2.0 * s,
                    r.y_range(),
                    Stroke::new(3.0 * s, c.raised(0.2)),
                );
                ui.add_space(8.0 * s);
            }
        });
    }
}

/// A list whose items hold code blocks or tables: a marker column, and each item's blocks in
/// the column beside it (so a code block gets the item's full width).
fn md_list(
    ui: &mut Ui,
    list: &crate::preview::markdown::List,
    cache: &mut egui_commonmark::CommonMarkCache,
    viewer: &dyn Fn(f32) -> egui_commonmark::CommonMarkViewer<'static>,
    c: &crate::theme::UiColors,
    s: f32,
) {
    let row = ui.text_style_height(&TextStyle::Body);
    let marker_width = s * if list.start.is_some() { 30.0 } else { 22.0 };
    let text = ui.visuals().text_color();
    for (i, item) in list.items.iter().enumerate() {
        ui.push_id(("item", i), |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let (rect, _) = ui.allocate_exact_size(vec2(marker_width, row), Sense::hover());
                let center = egui::pos2(rect.right() - 10.0 * s, rect.top() + row / 2.0);
                let painter = ui.painter();
                match (item.task, list.start) {
                    (Some(checked), _) => {
                        let b = egui::Rect::from_center_size(center, vec2(12.0, 12.0) * s);
                        painter.rect_stroke(
                            b,
                            3,
                            Stroke::new(1.2 * s, text),
                            egui::StrokeKind::Inside,
                        );
                        if checked {
                            painter.line(
                                vec![
                                    b.left_center() + vec2(2.5, 0.0) * s,
                                    b.center_bottom() + vec2(-1.0, -3.0) * s,
                                    b.right_top() + vec2(-2.5, 3.0) * s,
                                ],
                                Stroke::new(1.6 * s, c.accent),
                            );
                        }
                    }
                    (None, Some(first)) => {
                        painter.text(
                            egui::pos2(rect.right() - 6.0 * s, rect.top() + row / 2.0),
                            egui::Align2::RIGHT_CENTER,
                            format!("{}.", first + i as u64),
                            TextStyle::Body.resolve(ui.style()),
                            text,
                        );
                    }
                    (None, None) => {
                        painter.circle_filled(center, 2.5 * s, text);
                    }
                }
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 4.0 * s;
                    md_blocks(ui, &item.blocks, cache, viewer, c, s);
                });
            });
        });
    }
    ui.add_space(6.0 * s);
}

/// A markdown table that fits its column: widths come from the content, wide columns wrap.
/// Cells are rendered as markdown, so inline code, emphasis and links work inside them.
fn md_table(
    ui: &mut Ui,
    table: &Table,
    cache: &mut egui_commonmark::CommonMarkCache,
    viewer: &dyn Fn(f32) -> egui_commonmark::CommonMarkViewer<'static>,
    c: &crate::theme::UiColors,
    s: f32,
) {
    let pad = 8.0 * s;
    let columns = table
        .rows
        .iter()
        .map(Vec::len)
        .chain([table.header.len()])
        .max()
        .unwrap_or(0);
    if columns == 0 {
        return;
    }
    let font = TextStyle::Body.resolve(ui.style());
    let natural: Vec<f32> = (0..columns)
        .map(|col| {
            std::iter::once(&table.header)
                .chain(&table.rows)
                .filter_map(|row| row.get(col))
                .map(|cell| {
                    let text = crate::preview::markdown::plain_text(cell);
                    ui.fonts_mut(|f| f.layout_no_wrap(text, font.clone(), Color32::WHITE))
                        .size()
                        .x
                })
                .fold(0.0, f32::max)
                // Padding, plus slack for bold headers and code-span frames.
                + 2.0 * pad
                + 12.0 * s
        })
        .collect();
    let border = 1.0;
    let widths =
        crate::preview::markdown::fit_columns(&natural, ui.available_width() - 2.0 * border);
    let grid = c.raised(0.14);

    egui::Frame::new()
        .stroke(Stroke::new(border, grid))
        .corner_radius(6)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let rows = std::iter::once((true, &table.header))
                .chain(table.rows.iter().map(|r| (false, r)))
                .filter(|(header, row)| !(*header && row.is_empty()));
            for (r, (header, row)) in rows.enumerate() {
                let background = ui.painter().add(egui::Shape::Noop);
                let line = ui.horizontal_top(|ui| {
                    for (col, &w) in widths.iter().enumerate() {
                        let cell = row.get(col).map(String::as_str).unwrap_or("");
                        let text = if header && !cell.is_empty() {
                            format!("**{cell}**")
                        } else {
                            cell.to_string()
                        };
                        ui.allocate_ui_with_layout(
                            vec2(w, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_width(w);
                                egui::Frame::new()
                                    .inner_margin(egui::Margin::symmetric(
                                        pad.round() as i8,
                                        (5.0 * s).round() as i8,
                                    ))
                                    .show(ui, |ui| {
                                        let inner = (w - 2.0 * pad).max(1.0);
                                        ui.set_width(inner);
                                        ui.spacing_mut().item_spacing = vec2(4.0, 2.0) * s;
                                        ui.push_id((r, col), |ui| {
                                            viewer(inner).show(ui, cache, &text);
                                        });
                                    });
                            },
                        );
                    }
                });
                let rect = line.response.rect;
                let fill = if header {
                    c.raised(0.07)
                } else if r % 2 == 0 {
                    c.raised(0.025)
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter()
                    .set(background, egui::Shape::rect_filled(rect, 0.0, fill));
                if header {
                    ui.painter()
                        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, grid));
                }
            }
        });
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

/// Source view with line numbers. Only visible rows are laid out, so huge files scroll
/// smoothly either way.
struct CodeView<'a> {
    id: TabId,
    version: u64,
    text: &'a Text,
    highlighted: Option<&'a Vec<Vec<Span>>>,
    font: FontId,
}

impl CodeView<'_> {
    fn spans(&self, line: usize) -> Option<&[Span]> {
        self.highlighted
            .and_then(|h| h.get(line))
            .map(Vec::as_slice)
    }

    fn gutter(&self, ui: &mut Ui, number: Option<usize>, c: &crate::theme::UiColors) {
        let digits = self.text.lines.len().to_string().len().max(3);
        let label = match number {
            Some(n) => format!("{n:>digits$}  "),
            None => " ".repeat(digits + 2),
        };
        ui.add(
            egui::Label::new(
                RichText::new(label)
                    .font(self.font.clone())
                    .color(mix(c.fg, c.bg, 0.6)),
            )
            .selectable(false),
        );
    }

    /// One row per line; long lines scroll sideways.
    fn show(&self, ui: &mut Ui, c: &crate::theme::UiColors) {
        let row_height = ui.fonts_mut(|f| f.row_height(&self.font));
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        egui::ScrollArea::both()
            .id_salt(("preview_code", self.id, self.version))
            .auto_shrink([false, false])
            .show_rows(ui, row_height, self.text.lines.len(), |ui, range| {
                ui.add_space(6.0);
                for i in range {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        self.gutter(ui, Some(i + 1), c);
                        let line = self.text.line(i);
                        let job = row_job(line, self.spans(i), 0..line.len(), &self.font, c.fg);
                        ui.add(egui::Label::new(job).extend());
                    });
                }
            });
    }

    /// Long lines wrap to the pane's width; continuation rows have no line number.
    fn show_wrapped(
        &self,
        ui: &mut Ui,
        wrapped: &mut Option<crate::preview::Wrapped>,
        c: &crate::theme::UiColors,
    ) {
        let (row_height, char_width) = ui.fonts_mut(|f| {
            (
                f.row_height(&self.font),
                f.glyph_width(&self.font, 'M').max(1.0),
            )
        });
        let digits = self.text.lines.len().to_string().len().max(3);
        // Left margin, gutter, and room for the scrollbar.
        let used = 8.0 + (digits + 2) as f32 * char_width + 16.0;
        let cols = ((ui.available_width() - used) / char_width)
            .floor()
            .max(8.0) as usize;
        if wrapped.as_ref().map(|w| w.key) != Some((self.version, cols)) {
            *wrapped = Some(crate::preview::Wrapped::new(self.text, self.version, cols));
        }
        let rows = &wrapped.as_ref().unwrap().rows;

        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        egui::ScrollArea::vertical()
            .id_salt(("preview_code_wrapped", self.id, self.version))
            .auto_shrink([false, false])
            .show_rows(ui, row_height, rows.len(), |ui, range| {
                ui.add_space(6.0);
                for r in range {
                    let (line, ref bytes) = rows[r];
                    let first = r == 0 || rows[r - 1].0 != line;
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        self.gutter(ui, first.then_some(line + 1), c);
                        let text = self.text.line(line);
                        if !first {
                            let indent = crate::preview::hanging_indent(text, cols);
                            ui.add_space(indent as f32 * char_width);
                        }
                        let job = row_job(text, self.spans(line), bytes.clone(), &self.font, c.fg);
                        ui.add(egui::Label::new(job).extend());
                    });
                }
            });
    }
}

/// The bytes `range` of `line`, colored by the line's highlight `spans` (or plain).
fn row_job(
    line: &str,
    spans: Option<&[Span]>,
    range: std::ops::Range<usize>,
    font: &FontId,
    fg: Color32,
) -> LayoutJob {
    // Cut absurdly long rows (only reachable without wrapping).
    let mut end = range.end.min(range.start + MAX_LINE).min(line.len());
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    let start = range.start.min(end);
    let format = |color: Color32, italics: bool, underline: bool| TextFormat {
        font_id: font.clone(),
        color,
        italics,
        underline: if underline {
            Stroke::new(1.0, color)
        } else {
            Stroke::NONE
        },
        ..Default::default()
    };
    let mut job = LayoutJob::default();
    let mut pos = start;
    let mut span_start = 0;
    for span in spans.unwrap_or_default() {
        let span_end = span_start + span.len;
        let (a, b) = (span_start.max(pos), span_end.min(end));
        span_start = span_end;
        if b <= a {
            if span_end >= end {
                break;
            }
            continue;
        }
        if !line.is_char_boundary(a) || !line.is_char_boundary(b) {
            break;
        }
        job.append(
            &line[a..b],
            0.0,
            format(span.color, span.italic, span.underline),
        );
        pos = b;
    }
    if pos < end {
        job.append(&line[pos..end], 0.0, format(fg, false, false));
    }
    if job.is_empty() {
        // Keep empty lines one row tall.
        job.append(" ", 0.0, format(fg, false, false));
    }
    job
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

    #[test]
    fn row_job_follows_spans_and_falls_back() {
        let font = FontId::monospace(12.0);
        let red = Color32::RED;
        let spans = [Span {
            len: 3,
            color: red,
            italic: false,
            underline: false,
        }];
        let white = Color32::WHITE;
        let job = row_job("let x", Some(&spans), 0..5, &font, white);
        assert_eq!(job.text, "let x");
        assert_eq!(job.sections.len(), 2);
        assert_eq!(job.sections[0].format.color, red);
        // Spans that overrun the line (stale highlighting) are clipped.
        let job = row_job("le", Some(&spans), 0..2, &font, white);
        assert_eq!(job.text, "le");
        assert_eq!(row_job("", None, 0..0, &font, white).text, " ");
        // A wrapped row keeps the colors of the part of the line it shows.
        let job = row_job("let x", Some(&spans), 2..5, &font, white);
        assert_eq!(job.text, "t x");
        assert_eq!(job.sections[0].format.color, red);
        assert_eq!(job.sections[1].format.color, white);
    }
}
