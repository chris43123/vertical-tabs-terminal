//! Mouse input in a terminal pane: selection, mouse reporting to the application,
//! scrolling, and Cmd/Ctrl+click on links.

use std::path::PathBuf;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use eframe::egui::{self, CursorIcon, Id, PointerButton, Rect, Stroke, Ui};

use crate::app::App;
use crate::render::cell_at;
use crate::terminal::input::{MouseButton, encode_mouse};
use crate::terminal::links::{self, Link, Target};
use crate::terminal::session::GridSize;
use crate::workspace::TabId;

impl App {
    /// Mouse selection, mouse reporting to applications, wheel scrolling and links. Returns a
    /// file to preview when one was Cmd/Ctrl+clicked.
    pub(super) fn pane_mouse(
        &mut self,
        ui: &Ui,
        id: TabId,
        resp: &egui::Response,
        inner: Rect,
    ) -> Option<PathBuf> {
        if self.hovered_link.as_ref().is_some_and(|(h, _)| *h == id) {
            self.hovered_link = None;
        }
        let session = self.tabs.get(&id).and_then(|t| t.session())?;
        let (mode, display_offset) = {
            let term = session.term.lock();
            (*term.mode(), term.grid().display_offset())
        };
        let size = session.size;
        let mods = ui.input(|i| i.modifiers);
        // Shift bypasses mouse reporting so you can always select text.
        let reporting = mode.intersects(TermMode::MOUSE_MODE) && !mods.shift;
        let pointer = ui.input(|i| i.pointer.hover_pos());

        // Wheel.
        if resp.hovered() {
            let cell_h = self.fonts.cell_size().y;
            // Sum raw wheel events in points; mouse notches (line units) scroll 3 lines each.
            let dy: f32 = ui.input(|i| {
                i.events
                    .iter()
                    .map(|e| match e {
                        egui::Event::MouseWheel { unit, delta, .. } => match unit {
                            egui::MouseWheelUnit::Point => delta.y,
                            egui::MouseWheelUnit::Line => delta.y * 3.0 * cell_h,
                            egui::MouseWheelUnit::Page => delta.y * inner.height(),
                        },
                        _ => 0.0,
                    })
                    .sum()
            });
            if dy != 0.0 {
                self.scroll_accum += dy;
                let lines = (self.scroll_accum / cell_h).trunc() as i32;
                self.scroll_accum -= lines as f32 * cell_h;
                if lines != 0 {
                    self.scroll_lines(
                        id,
                        lines,
                        mode,
                        pointer.map(|p| cell_at(p, inner, &self.fonts, size)),
                        mods,
                        reporting,
                    );
                }
            }
        }

        // Cmd+click (Ctrl+click on Linux/Windows) opens the link under the pointer, as in
        // iTerm2, Ghostty and VS Code: URLs in the browser, files in a preview (returned so the
        // caller opens it after drawing the panes). It wins over mouse reporting and selection.
        if mods.command
            && resp.hovered()
            && let Some(pos) = pointer.filter(|p| inner.contains(*p))
            && let Some(link) = self.link_at(id, pos, inner)
        {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            // Underlined by the caller, on top of the text painted after this.
            self.hovered_link = Some((id, link.clone()));
            if resp.clicked() {
                match link.target {
                    Target::Url(url) => {
                        if let Err(err) = crate::platform::open_url(&url) {
                            crate::diag::warn(err);
                        }
                    }
                    Target::File(path) => return Some(path),
                }
            }
            return None;
        }

        let session = self.tabs[&id].session()?;
        let cell = |pos| cell_at(pos, inner, &self.fonts, size);

        if reporting {
            let last_key = Id::new(("last_mouse_cell", id));
            let events = ui.input(|i| i.events.clone());
            for event in events {
                if let egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } = event
                {
                    if !inner.contains(pos) && pressed {
                        continue;
                    }
                    let button = match button {
                        PointerButton::Primary => MouseButton::Left,
                        PointerButton::Middle => MouseButton::Middle,
                        PointerButton::Secondary => MouseButton::Right,
                        _ => continue,
                    };
                    let (col, line, _) = cell(pos);
                    if let Some(bytes) =
                        encode_mouse(button, pressed, false, col, line, modifiers, mode)
                    {
                        session.write(bytes);
                    }
                }
            }
            if resp.dragged()
                && let Some(pos) = pointer
            {
                let (col, line, _) = cell(pos);
                let last = ui.ctx().data(|d| d.get_temp::<(usize, usize)>(last_key));
                if last != Some((col, line)) {
                    ui.ctx().data_mut(|d| d.insert_temp(last_key, (col, line)));
                    let button = if resp.dragged_by(PointerButton::Secondary) {
                        MouseButton::Right
                    } else {
                        MouseButton::Left
                    };
                    if let Some(bytes) = encode_mouse(button, true, true, col, line, mods, mode) {
                        session.write(bytes);
                    }
                }
            }
            return None;
        }

        // Selection.
        let to_point = |col: usize, line: usize| {
            Point::new(Line(line as i32 - display_offset as i32), Column(col))
        };
        if let Some(pos) = resp.interact_pointer_pos().or(pointer) {
            let (col, line, side) = cell(pos);
            let point = to_point(col, line);
            let mut term = session.term.lock();
            if resp.triple_clicked() {
                term.selection = Some(Selection::new(SelectionType::Lines, point, side));
            } else if resp.double_clicked() {
                term.selection = Some(Selection::new(SelectionType::Semantic, point, side));
            } else if resp.clicked() {
                term.selection = None;
            } else if resp.drag_started_by(PointerButton::Primary) {
                let ty = if mods.alt {
                    SelectionType::Block
                } else {
                    SelectionType::Simple
                };
                term.selection = Some(Selection::new(ty, point, side));
            } else if resp.dragged_by(PointerButton::Primary) {
                if let Some(sel) = term.selection.as_mut() {
                    sel.update(point, side);
                }
                // Auto-scroll when dragging past the top/bottom edge.
                if pos.y < inner.top() {
                    term.scroll_display(Scroll::Delta(1));
                } else if pos.y > inner.bottom() {
                    term.scroll_display(Scroll::Delta(-1));
                }
            }
        }

        // copy_on_select: copy once a mouse selection is finished, keeping it highlighted.
        let finished = resp.drag_stopped_by(PointerButton::Primary)
            || resp.double_clicked()
            || resp.triple_clicked();
        let selected = (self.config.copy_on_select && finished)
            .then(|| session.term.lock().selection_to_string())
            .flatten()
            .filter(|s| !s.is_empty());

        // Right click: copy the selection if any, otherwise paste (Windows Terminal style).
        if resp.secondary_clicked() {
            let text = {
                let mut term = session.term.lock();
                let text = term.selection_to_string().filter(|s| !s.is_empty());
                if text.is_some() {
                    term.selection = None;
                }
                text
            };
            match text {
                Some(text) => self.set_clipboard(text),
                None => {
                    if let Some(text) = self.clipboard_text() {
                        self.activate(id);
                        self.paste(&text);
                    }
                }
            }
        }
        if let Some(text) = selected {
            self.set_clipboard(text);
        }
        None
    }

    /// The openable link under `pos` in pane `id`, if any (relative paths are under its cwd).
    fn link_at(&mut self, id: TabId, pos: egui::Pos2, inner: Rect) -> Option<Link> {
        let session = self.tabs.get(&id).and_then(|t| t.session())?;
        let (col, line, _) = cell_at(pos, inner, &self.fonts, session.size);
        let term = session.term.lock();
        let offset = term.grid().display_offset() as i32;
        let point = Point::new(Line(line as i32 - offset), Column(col));
        let cwd = self.tabs[&id].cwd.as_deref();
        links::link_at(&term, point, &mut self.link_matchers, cwd)
    }

    /// Underline the cells of a hovered link. `offset` is the grid's display offset.
    pub(super) fn underline_link(
        &self,
        painter: &egui::Painter,
        link: &Link,
        inner: Rect,
        size: GridSize,
        offset: usize,
    ) {
        let cell = self.fonts.cell_size();
        let (start, end) = (*link.range.start(), *link.range.end());
        let stroke = Stroke::new(1.0, self.chrome.accent);
        for grid_line in start.line.0..=end.line.0 {
            // Grid line -> row on screen; skip rows scrolled out of view.
            let row = grid_line + offset as i32;
            if row < 0 || row as usize >= size.lines {
                continue;
            }
            let first = if grid_line == start.line.0 {
                start.column.0
            } else {
                0
            };
            let last = if grid_line == end.line.0 {
                end.column.0
            } else {
                size.cols.saturating_sub(1)
            };
            let y = inner.min.y + (row + 1) as f32 * cell.y - 1.0;
            let x0 = inner.min.x + first as f32 * cell.x;
            let x1 = inner.min.x + (last + 1) as f32 * cell.x;
            painter.hline(x0..=x1, y, stroke);
        }
    }

    fn scroll_lines(
        &mut self,
        id: TabId,
        lines: i32,
        mode: TermMode,
        cell: Option<(usize, usize, alacritty_terminal::index::Side)>,
        mods: egui::Modifiers,
        reporting: bool,
    ) {
        let Some(session) = self.tabs.get(&id).and_then(|t| t.session()) else {
            return;
        };
        let count = lines.unsigned_abs() as usize;
        if reporting {
            let (col, line, _) = cell.unwrap_or((0, 0, alacritty_terminal::index::Side::Left));
            let button = if lines > 0 {
                MouseButton::WheelUp
            } else {
                MouseButton::WheelDown
            };
            for _ in 0..count {
                if let Some(bytes) = encode_mouse(button, true, false, col, line, mods, mode) {
                    session.write(bytes);
                }
            }
        } else if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            // Full-screen apps without mouse support (less, man) get arrow keys.
            let seq: &[u8] = match (lines > 0, mode.contains(TermMode::APP_CURSOR)) {
                (true, true) => b"\x1bOA",
                (true, false) => b"\x1b[A",
                (false, true) => b"\x1bOB",
                (false, false) => b"\x1b[B",
            };
            session.write(seq.repeat(count));
        } else {
            session.term.lock().scroll_display(Scroll::Delta(lines));
        }
    }
}
