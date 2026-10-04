//! Rendered markdown: ordinary runs go through egui_commonmark; tables, and lists and quotes
//! holding them, are laid out here so they wrap to the pane width.

use std::path::Path;

use eframe::egui::{self, Align, Color32, FontId, Layout, Sense, Stroke, TextStyle, Ui, vec2};

use crate::app::App;
use crate::preview::highlight;
use crate::preview::markdown::{Block, Table};

/// Readable line length for rendered markdown.
const MARKDOWN_WIDTH: f32 = 820.0;

impl App {
    pub(super) fn markdown(
        &mut self,
        ui: &mut Ui,
        scroll_id: impl std::hash::Hash + std::fmt::Debug,
        blocks: &[Block],
        base: Option<&Path>,
        s: f32,
    ) {
        let cache = self.previews.markdown_cache();
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
