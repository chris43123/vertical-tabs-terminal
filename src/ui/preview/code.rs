//! Text and code previews: only the visible lines are laid out, so huge files scroll smoothly.

use eframe::egui::text::LayoutJob;
use eframe::egui::{self, Color32, FontId, RichText, Stroke, TextFormat, Ui, vec2};

use crate::preview::Text;
use crate::preview::highlight::Span;
use crate::theme::mix;
use crate::workspace::TabId;

/// Longer lines are cut for display.
const MAX_LINE: usize = 20_000;

/// Source view with line numbers. Only visible rows are laid out, so huge files scroll
/// smoothly either way.
pub(super) struct CodeView<'a> {
    pub(super) id: TabId,
    pub(super) version: u64,
    pub(super) text: &'a Text,
    pub(super) highlighted: Option<&'a Vec<Vec<Span>>>,
    pub(super) font: FontId,
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
    pub(super) fn show(&self, ui: &mut Ui, c: &crate::theme::UiColors) {
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
    pub(super) fn show_wrapped(
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

#[cfg(test)]
mod tests {
    use super::*;

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
