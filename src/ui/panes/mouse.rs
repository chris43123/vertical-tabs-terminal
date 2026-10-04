//! Mouse input in a terminal pane: selection, mouse reporting to the application, and
//! scrolling.

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use eframe::egui::{self, Id, PointerButton, Rect, Ui};

use crate::app::App;
use crate::render::cell_at;
use crate::terminal::input::{MouseButton, encode_mouse};
use crate::workspace::TabId;

impl App {
    /// Mouse selection, mouse reporting to applications, and wheel scrolling.
    pub(super) fn pane_mouse(&mut self, ui: &Ui, id: TabId, resp: &egui::Response, inner: Rect) {
        let Some(session) = self.tabs.get(&id).and_then(|t| t.session()) else {
            return;
        };
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

        let Some(session) = self.tabs[&id].session() else {
            return;
        };
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
            return;
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
