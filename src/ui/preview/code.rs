//! Text and code previews: only the visible rows are laid out, so huge files scroll smoothly.
//!
//! The body is one click-and-drag area rather than a selectable label per row: the selection
//! lives on the [`Preview`](crate::preview::Preview) in document coordinates, so it survives
//! rows scrolling out of view, re-wrapping and any scroll distance. Clicking line numbers
//! selects whole lines.

use std::ops::Range;
use std::sync::Arc;

use eframe::egui::text::{CCursor, LayoutJob};
use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Galley, Id, Pos2, Rect, Sense, Stroke, TextFormat,
    Ui, pos2, vec2,
};

use crate::preview::highlight::Span;
use crate::preview::selection::{Pos, Selection};
use crate::preview::{Text, Wrapped};
use crate::theme::{SelectionColors, UiColors, ensure_contrast, mix};
use crate::workspace::TabId;

/// Longer lines are cut for display.
const MAX_LINE: usize = 20_000;
/// Space above the first row and below the last.
const PAD_Y: f32 = 6.0;
/// Space left of the line numbers.
const MARGIN: f32 = 8.0;

/// What the body's context menu asked for.
pub(super) enum CodeAction {
    Copy,
    CopyAll,
    /// A `path:lines` reference to the selection, or to this line when nothing is selected.
    CopyReference(usize),
}

/// What a drag started on: text (extends by character) or line numbers (by whole lines,
/// from this anchor line).
#[derive(Clone, Copy)]
enum DragMode {
    Text,
    Lines(usize),
}

/// One visual row: the line it shows, the bytes of that line, whether it is the line's first
/// and last row, and how far its text is indented (continuation rows of wrapped lines).
struct Row {
    line: usize,
    bytes: Range<usize>,
    first: bool,
    last: bool,
    indent: f32,
}

/// Source view with line numbers, optionally soft-wrapped to the pane's width.
pub(super) struct CodeView<'a> {
    pub(super) id: TabId,
    pub(super) version: u64,
    pub(super) text: &'a Text,
    pub(super) highlighted: Option<&'a Vec<Vec<Span>>>,
    pub(super) font: FontId,
    pub(super) colors: &'a UiColors,
    /// The user's minimum contrast, applied to selected text on top of the selection colors.
    pub(super) min_contrast: f32,
}

/// Per-frame geometry shared by layout, hit-testing and painting.
struct Geometry {
    row_height: f32,
    char_width: f32,
    digits: usize,
    /// Number of rows.
    count: usize,
    /// Screen position of the content's top-left corner (moves as it scrolls).
    origin: Pos2,
    /// Visible part of the content, on screen.
    visible: Rect,
}

impl Geometry {
    fn text_x(&self) -> f32 {
        self.origin.x + MARGIN + (self.digits + 2) as f32 * self.char_width
    }

    fn row_top(&self, r: usize) -> f32 {
        self.origin.y + PAD_Y + r as f32 * self.row_height
    }
}

impl CodeView<'_> {
    fn spans(&self, line: usize) -> Option<&[Span]> {
        self.highlighted
            .and_then(|h| h.get(line))
            .map(Vec::as_slice)
    }

    /// Show the text; with `wrap`, long lines wrap to the pane's width and continuation rows
    /// have no line number, otherwise they scroll sideways. Returns a context-menu request.
    pub(super) fn show(
        &self,
        ui: &mut Ui,
        selection: &mut Option<Selection>,
        wrapped: &mut Option<Wrapped>,
        wrap: bool,
    ) -> Option<CodeAction> {
        let (row_height, char_width) = ui.fonts_mut(|f| {
            (
                f.row_height(&self.font),
                f.glyph_width(&self.font, 'M').max(1.0),
            )
        });
        let digits = self.text.lines.len().to_string().len().max(3);
        let rows: Option<&[(usize, Range<usize>)]> = if wrap {
            // Left margin, gutter, and room for the scrollbar.
            let used = MARGIN + (digits + 2) as f32 * char_width + 16.0;
            let cols = ((ui.available_width() - used) / char_width)
                .floor()
                .max(8.0) as usize;
            if wrapped.as_ref().map(|w| w.key) != Some((self.version, cols)) {
                *wrapped = Some(Wrapped::new(self.text, self.version, cols));
            }
            let rows = &wrapped.as_ref().unwrap().rows;
            Some(rows.as_slice())
        } else {
            None
        };
        let count = rows.map_or(self.text.lines.len(), <[_]>::len);
        let cols = wrapped.as_ref().map_or(0, |w| w.key.1);
        let row = |r: usize| -> Row {
            match rows {
                Some(rows) => {
                    let (line, ref bytes) = rows[r];
                    let first = r == 0 || rows[r - 1].0 != line;
                    let indent = if first {
                        0.0
                    } else {
                        crate::preview::hanging_indent(self.text.line(line), cols) as f32
                            * char_width
                    };
                    Row {
                        line,
                        bytes: bytes.clone(),
                        first,
                        last: rows.get(r + 1).is_none_or(|n| n.0 != line),
                        indent,
                    }
                }
                None => Row {
                    line: r,
                    bytes: 0..self.text.line(r).len(),
                    first: true,
                    last: true,
                    indent: 0.0,
                },
            }
        };

        let area = if wrap {
            egui::ScrollArea::vertical().id_salt(("preview_code_wrapped", self.id, self.version))
        } else {
            egui::ScrollArea::both().id_salt(("preview_code", self.id, self.version))
        };
        let output = area
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let origin = ui.max_rect().min;
                let g = Geometry {
                    row_height,
                    char_width,
                    digits,
                    count,
                    origin,
                    visible: viewport.translate(origin.to_vec2()),
                };
                self.body(ui, &g, &row, selection)
            });
        let (action, dragging) = output.inner;
        if dragging {
            super::autoscroll(
                ui.ctx(),
                output.id,
                &output.state,
                output.inner_rect,
                output.content_size,
                !wrap,
            );
        }
        action
    }

    /// Lay out, hit-test and paint the visible rows. Returns a context-menu request and
    /// whether a selection drag is in progress.
    fn body(
        &self,
        ui: &mut Ui,
        g: &Geometry,
        row: &dyn Fn(usize) -> Row,
        selection: &mut Option<Selection>,
    ) -> (Option<CodeAction>, bool) {
        let count = g.count;
        let c = self.colors;
        let rh = g.row_height;
        let top = g.visible.top() - g.origin.y;
        let bottom = g.visible.bottom() - g.origin.y;
        let last = (((bottom - PAD_Y) / rh).ceil().max(0.0) as usize + 1).min(count);
        let first = (((top - PAD_Y) / rh).floor().max(0.0) as usize).min(last);
        let visible: Vec<(usize, Row, Arc<Galley>)> = (first..last)
            .map(|r| {
                let row = row(r);
                let galley = self.galley(ui, &row, None);
                (r, row, galley)
            })
            .collect();

        let width = visible
            .iter()
            .map(|(_, row, gal)| row.indent + gal.size().x)
            .fold(0.0, f32::max);
        ui.set_min_size(vec2(
            g.text_x() - g.origin.x + width + 16.0,
            2.0 * PAD_Y + count as f32 * rh,
        ));

        let response = ui.interact(
            g.visible,
            Id::new(("preview_text", self.id)),
            Sense::CLICK | Sense::DRAG,
        );
        let action = self.interact(ui, g, row, &visible, &response, selection);

        let sel = selection.filter(|s| !s.is_empty()).map(|s| s.range());
        let painter = ui.painter();
        let number_color = mix(c.fg, c.bg, 0.6);
        for (r, row, galley) in visible {
            let y = g.row_top(r);
            let selected = sel.and_then(|s| row_selection(s, &row));
            if row.first {
                let in_selection =
                    sel.is_some_and(|(a, b)| a.line <= row.line && row.line <= b.line);
                painter.text(
                    pos2(g.origin.x + MARGIN, y),
                    Align2::LEFT_TOP,
                    format!("{:>w$}", row.line + 1, w = g.digits),
                    self.font.clone(),
                    if in_selection { c.fg } else { number_color },
                );
            }
            let x = g.text_x() + row.indent;
            let galley = match selected {
                Some((ref bytes, newline)) => {
                    let line = self.text.line(row.line);
                    let x0 = x + x_of(&galley, line, &row, bytes.start);
                    let mut x1 = x + x_of(&galley, line, &row, bytes.end);
                    if newline {
                        x1 += g.char_width * 0.6;
                    }
                    painter.rect_filled(
                        Rect::from_x_y_ranges(x0..=x1, y..=y + rh),
                        0.0,
                        c.selection.bg,
                    );
                    self.galley(ui, &row, Some(bytes.clone()))
                }
                None => galley,
            };
            painter.galley(pos2(x, y), galley, c.fg);
        }
        (action, response.dragged())
    }

    /// Update the selection from this frame's pointer input; open the context menu.
    fn interact(
        &self,
        ui: &Ui,
        g: &Geometry,
        row: &dyn Fn(usize) -> Row,
        visible: &[(usize, Row, Arc<Galley>)],
        response: &egui::Response,
        selection: &mut Option<Selection>,
    ) -> Option<CodeAction> {
        let count = g.count;
        if count == 0 {
            return None;
        }
        let (pointer, pressed, down, shift) = ui.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.modifiers.shift,
            )
        });
        let mode_id = Id::new(("preview_drag", self.id));
        // Pointer → (position, over the line numbers?), clamped to the visible rows so a
        // drag past the edge extends to the edge while autoscroll brings more in.
        let hit = |p: Pos2| -> (Pos, bool) {
            let p = p.clamp(g.visible.min, g.visible.max - vec2(0.0, 1.0));
            let r = ((p.y - g.origin.y - PAD_Y) / g.row_height)
                .floor()
                .clamp(0.0, (count - 1) as f32) as usize;
            let in_gutter = p.x < g.text_x();
            let row = row(r);
            let galley = visible
                .iter()
                .find(|(i, ..)| *i == r)
                .map(|(.., gal)| gal.clone())
                .unwrap_or_else(|| self.galley(ui, &row, None));
            let x = p.x - g.text_x() - row.indent;
            let cursor = galley.cursor_from_pos(vec2(x, g.row_height / 2.0));
            let line = self.text.line(row.line);
            let shown = shown(line, &row.bytes);
            let byte = line[shown.clone()]
                .char_indices()
                .nth(cursor.index.0)
                .map_or(shown.end, |(i, _)| shown.start + i);
            (Pos::new(row.line, byte), in_gutter)
        };

        if let Some(p) = pointer {
            if response.hovered() {
                let icon = if p.x < g.text_x() {
                    CursorIcon::Default
                } else {
                    CursorIcon::Text
                };
                ui.ctx().set_cursor_icon(icon);
            }
            if pressed && response.hovered() {
                let (pos, in_gutter) = hit(p);
                let anchor = selection.filter(|_| shift);
                let mode = if in_gutter {
                    let from = anchor.map_or(pos.line, |s| s.anchor.line);
                    *selection = Some(Selection::lines(self.text, from, pos.line));
                    DragMode::Lines(from)
                } else {
                    *selection = Some(match anchor {
                        Some(s) => Selection { head: pos, ..s },
                        None => Selection::caret(pos),
                    });
                    DragMode::Text
                };
                ui.data_mut(|d| d.insert_temp(mode_id, mode));
            } else if down && response.is_pointer_button_down_on() {
                let (pos, _) = hit(p);
                match ui.data(|d| d.get_temp(mode_id)) {
                    Some(DragMode::Lines(from)) => {
                        *selection = Some(Selection::lines(self.text, from, pos.line));
                    }
                    Some(DragMode::Text) => {
                        if let Some(s) = selection {
                            s.head = pos;
                        }
                    }
                    None => {}
                }
            }
            if response.triple_clicked() {
                let (pos, _) = hit(p);
                *selection = Some(Selection::lines(self.text, pos.line, pos.line));
            } else if response.double_clicked() {
                let (pos, in_gutter) = hit(p);
                if !in_gutter {
                    *selection = Some(Selection::word(self.text, pos));
                }
            }
            if response.secondary_clicked() {
                let (pos, _) = hit(p);
                ui.data_mut(|d| d.insert_temp(mode_id.with("menu"), pos.line));
            }
        }
        // A plain click on text selects nothing. (A click on an empty line's number keeps its
        // empty selection, so Shift+click can still extend from that line.)
        let text_mode = matches!(ui.data(|d| d.get_temp(mode_id)), Some(DragMode::Text));
        if !down && text_mode && selection.is_some_and(|s| s.is_empty()) {
            *selection = None;
        }

        let mut action = None;
        let has_selection = selection.is_some_and(|s| !s.is_empty());
        response.context_menu(|ui| {
            if ui
                .add_enabled(has_selection, egui::Button::new("Copy"))
                .clicked()
            {
                action = Some(CodeAction::Copy);
            }
            if ui.button("Copy all").clicked() {
                action = Some(CodeAction::CopyAll);
            }
            let line = ui.data(|d| d.get_temp(mode_id.with("menu"))).unwrap_or(0);
            if ui
                .button("Copy reference")
                .on_hover_text("path:lines, for editors and agents")
                .clicked()
            {
                action = Some(CodeAction::CopyReference(line));
            }
        });
        action
    }

    /// The galley for `row`; text in `selected` (bytes of the line) is recolored to read on
    /// the selection.
    fn galley(&self, ui: &Ui, row: &Row, selected: Option<Range<usize>>) -> Arc<Galley> {
        let line = self.text.line(row.line);
        let style = RowStyle {
            font: &self.font,
            fg: self.colors.fg,
            selected: selected.map(|s| (s, self.colors.selection, self.min_contrast)),
        };
        let job = row_job(line, self.spans(row.line), row.bytes.clone(), &style);
        ui.fonts_mut(|f| f.layout_job(job))
    }
}

/// The part of `selection` (start, end in document order) that `row` shows, as bytes of
/// its line, and whether the selection continues past the end of the line (so the row
/// shows its line break as selected).
fn row_selection((a, b): (Pos, Pos), row: &Row) -> Option<(Range<usize>, bool)> {
    if row.line < a.line || row.line > b.line {
        return None;
    }
    let from = if row.line == a.line { a.byte } else { 0 };
    let to = if row.line == b.line {
        b.byte
    } else {
        usize::MAX
    };
    let newline = row.line < b.line && row.last;
    let (start, end) = (from.max(row.bytes.start), to.min(row.bytes.end));
    if start > end || (start == end && !newline) {
        return None;
    }
    Some((start..end, newline))
}

/// X offset of byte `byte` of `line` within `row`'s galley.
fn x_of(galley: &Galley, line: &str, row: &Row, byte: usize) -> f32 {
    let shown = shown(line, &row.bytes);
    if byte > shown.end {
        return galley.size().x;
    }
    let chars = line[shown.start..byte.max(shown.start)].chars().count();
    galley.pos_from_cursor(CCursor::new(chars)).min.x
}

/// The bytes of `range` that are displayed: absurdly long rows (only reachable without
/// wrapping) are cut.
fn shown(line: &str, range: &Range<usize>) -> Range<usize> {
    let mut end = range.end.min(range.start + MAX_LINE).min(line.len());
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    range.start.min(end)..end
}

/// How a row's text is drawn.
struct RowStyle<'a> {
    font: &'a FontId,
    fg: Color32,
    /// Selected bytes of the line, the selection colors, and the user's minimum contrast.
    selected: Option<(Range<usize>, SelectionColors, f32)>,
}

/// The bytes `range` of `line`, colored by the line's highlight `spans` (or plain).
fn row_job(line: &str, spans: Option<&[Span]>, range: Range<usize>, style: &RowStyle) -> LayoutJob {
    let Range { start, end } = shown(line, &range);
    let format = |color: Color32, italics: bool, underline: bool| TextFormat {
        font_id: style.font.clone(),
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
    // Append `line[a..b]`, splitting off the selected part to adjust its color.
    let mut append = |a: usize, b: usize, color: Color32, italics: bool, underline: bool| {
        let mut cuts = vec![a, b];
        if let Some((sel, ..)) = &style.selected {
            cuts.extend([sel.start, sel.end].into_iter().filter(|&x| a < x && x < b));
        }
        cuts.sort_unstable();
        for w in cuts.windows(2) {
            let (x, y) = (w[0], w[1]);
            if x == y {
                continue;
            }
            let color = match &style.selected {
                Some((sel, colors, ratio)) if sel.start <= x && y <= sel.end => {
                    ensure_contrast(colors.text(color), colors.bg, *ratio)
                }
                _ => color,
            };
            job.append(&line[x..y], 0.0, format(color, italics, underline));
        }
    };
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
        append(a, b, span.color, span.italic, span.underline);
        pos = b;
    }
    if pos < end {
        append(pos, end, style.fg, false, false);
    }
    if job.is_empty() {
        // Keep empty lines one row tall.
        job.append(" ", 0.0, format(style.fg, false, false));
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(font: &FontId, fg: Color32) -> RowStyle<'_> {
        RowStyle {
            font,
            fg,
            selected: None,
        }
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
        let style = plain(&font, white);
        let job = row_job("let x", Some(&spans), 0..5, &style);
        assert_eq!(job.text, "let x");
        assert_eq!(job.sections.len(), 2);
        assert_eq!(job.sections[0].format.color, red);
        // Spans that overrun the line (stale highlighting) are clipped.
        let job = row_job("le", Some(&spans), 0..2, &style);
        assert_eq!(job.text, "le");
        assert_eq!(row_job("", None, 0..0, &style).text, " ");
        // A wrapped row keeps the colors of the part of the line it shows.
        let job = row_job("let x", Some(&spans), 2..5, &style);
        assert_eq!(job.text, "t x");
        assert_eq!(job.sections[0].format.color, red);
        assert_eq!(job.sections[1].format.color, white);
    }

    #[test]
    fn selected_text_reads_on_the_selection() {
        use crate::theme::{Theme, contrast_ratio};
        let font = FontId::monospace(12.0);
        // Light text on a light selection (a matugen-style theme).
        let theme = Theme {
            foreground: [0xef, 0xd2, 0xc5],
            background: [0x23, 0x18, 0x16],
            selection: [0xff, 0xda, 0xd5],
            ..Theme::default()
        };
        let light = Color32::from_rgb(0xef, 0xd2, 0xc5);
        let colors = SelectionColors::from_theme(&theme);
        let style = RowStyle {
            font: &font,
            fg: light,
            selected: Some((2..4, colors, 1.0)),
        };
        let job = row_job("abcdef", None, 0..6, &style);
        assert_eq!(job.text, "abcdef");
        let c: Vec<Color32> = job.sections.iter().map(|s| s.format.color).collect();
        assert_eq!(c.len(), 3);
        assert_eq!((c[0], c[2]), (light, light));
        assert!(contrast_ratio(c[1], colors.bg) >= 4.5);
        // A higher min_contrast still applies on top.
        let style = RowStyle {
            selected: Some((0..6, colors, 12.0)),
            ..style
        };
        let job = row_job("abcdef", None, 0..6, &style);
        assert!(contrast_ratio(job.sections[0].format.color, colors.bg) >= 12.0);
    }

    #[test]
    fn row_selection_covers_the_rows_it_spans() {
        let row = |line, bytes: Range<usize>, last| Row {
            line,
            bytes,
            first: true,
            last,
            indent: 0.0,
        };
        let sel = (Pos::new(1, 2), Pos::new(3, 4));
        assert_eq!(row_selection(sel, &row(0, 0..10, true)), None);
        assert_eq!(
            row_selection(sel, &row(1, 0..10, true)),
            Some((2..10, true))
        );
        assert_eq!(row_selection(sel, &row(2, 0..0, true)), Some((0..0, true)));
        assert_eq!(
            row_selection(sel, &row(3, 0..10, true)),
            Some((0..4, false))
        );
        assert_eq!(row_selection(sel, &row(4, 0..10, true)), None);
        // Wrapped: only the last row of a line shows its line break.
        assert_eq!(
            row_selection(sel, &row(1, 0..5, false)),
            Some((2..5, false))
        );
        assert_eq!(
            row_selection(sel, &row(1, 5..10, true)),
            Some((5..10, true))
        );
        // A row entirely before the start, on the start line.
        let sel = (Pos::new(1, 7), Pos::new(1, 9));
        assert_eq!(row_selection(sel, &row(1, 0..5, false)), None);
        assert_eq!(
            row_selection(sel, &row(1, 5..10, true)),
            Some((7..9, false))
        );
    }
}
