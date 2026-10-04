//! Keyboard shortcuts window (Ctrl+Shift+/, i.e. Ctrl+?): every shortcut and mouse gesture,
//! grouped. Clicking a shortcut rebinds it; changes go to `[keybindings]` in the config file.

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Id, Key, Layout, Modifiers, Order, RichText, Sense,
    Stroke, StrokeKind, Ui, vec2,
};

use crate::app::App;
use crate::fuzzy::score as fuzzy_score;
use crate::keybinds::{Action, Chord};
use crate::theme::UiColors;

#[derive(Default)]
pub struct Help {
    query: String,
    /// Waiting for a key press to bind.
    capture: Option<Capture>,
    /// Last problem (e.g. a chord without modifiers), shown under the title.
    notice: Option<String>,
    initialized: bool,
}

#[derive(Clone, Copy, PartialEq)]
struct Capture {
    action: Action,
    /// Which of the action's chords is being replaced; `None` adds one.
    slot: Option<usize>,
}

enum Op {
    Capture(Capture),
    Reset(Action),
}

const SECTIONS: &[(&str, &[Action])] = {
    use Action::*;
    &[
        (
            "Tabs",
            &[
                NewTab,
                CloseTab,
                ReopenClosedTab,
                DuplicateTab,
                RenameTab,
                NextTab,
                PrevTab,
                MoveTabUp,
                MoveTabDown,
                NextActivity,
                LastTab,
                NewGroup,
            ],
        ),
        (
            "Panes",
            &[
                SplitRight,
                SplitDown,
                MinimizePane,
                FocusLeft,
                FocusRight,
                FocusUp,
                FocusDown,
            ],
        ),
        (
            "View",
            &[
                ToggleSideArea,
                ToggleSidebar,
                ToggleFiles,
                SearchFiles,
                ZoomIn,
                ZoomOut,
                ZoomReset,
                UiZoomIn,
                UiZoomOut,
                UiZoomReset,
            ],
        ),
        ("Terminal", &[ScrollPageUp, ScrollPageDown]),
        ("App", &[CommandPalette, OpenSettings, ShowHelp]),
    ]
};

/// Fixed keys and mouse gestures (not rebindable), as (section, [(gesture, what it does)]).
fn gestures() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    let mac = cfg!(target_os = "macos");
    let mut terminal = if mac {
        vec![("⌘C / ⌘V", "Copy / paste")]
    } else {
        vec![
            (
                "Ctrl+C",
                "Copy the selection, or interrupt when nothing is selected",
            ),
            ("Ctrl+Shift+C / Ctrl+Shift+V", "Copy / paste"),
            (
                "Ctrl+V",
                "Paste; with an image, apps like Claude Code read it themselves",
            ),
        ]
    };
    terminal.extend([
        ("Shift+Enter", "New line in agents (sends Esc, Enter)"),
        ("Drag", "Select text"),
        ("Double / triple click", "Select a word / line"),
        ("Alt+drag", "Block selection"),
        ("Shift+drag", "Select even when the app uses the mouse"),
        ("Right click", "Copy the selection, or paste"),
        ("Wheel", "Scroll back (apps that use the mouse get it)"),
    ]);
    vec![
        ("Terminal (mouse and fixed keys)", terminal),
        (
            "Sidebar",
            vec![
                ("Click", "Switch to a tab, or collapse a folder"),
                ("Middle click", "Close a tab"),
                ("Double click", "Rename a tab or folder"),
                ("Double click empty space", "New tab"),
                (
                    "Drag a tab",
                    "Reorder, or drop on a pane to split or swap it",
                ),
                ("Right click", "Tab or folder menu"),
            ],
        ),
        (
            "Panes",
            vec![
                (
                    "Drag a pane header",
                    "Drop on an edge to re-split, in the middle to swap",
                ),
                ("Drag a divider", "Resize"),
            ],
        ),
        (
            "Files panel",
            vec![
                ("Click", "Expand a folder, or preview a file"),
                ("Double click a folder", "cd the shell there"),
                ("Drag onto a terminal", "Type the path"),
                (
                    "Drag onto a pane edge",
                    "Preview beside it (a folder opens a terminal there)",
                ),
                ("Right click", "File menu"),
            ],
        ),
        (
            "Palette",
            vec![
                ("Up / Down, Tab, Ctrl+N / Ctrl+P", "Move"),
                ("Enter / Esc", "Pick / close"),
            ],
        ),
    ]
}

impl App {
    pub(crate) fn help_ui(&mut self, ctx: &egui::Context) {
        let Some(mut help) = self.help.take() else {
            return;
        };
        let first_frame = !help.initialized;
        help.initialized = true;

        let mut close = false;
        let mut ops = Vec::new();
        let events = ctx.input(|i| i.events.clone());
        if let Some(cap) = help.capture {
            // Keys belong to the capture, not to the search box or the terminal.
            ctx.input_mut(|i| {
                i.events
                    .retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_)))
            });
            ctx.memory_mut(|m| m.stop_text_input());
            for e in &events {
                let egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    modifiers,
                    ..
                } = e
                else {
                    continue;
                };
                let plain = modifiers.is_none();
                if plain && *key == Key::Escape {
                    help.capture = None;
                } else if plain && matches!(key, Key::Backspace | Key::Delete) {
                    if let Some(slot) = cap.slot {
                        help.notice = self.remove_chord(cap.action, slot).err();
                    }
                    help.capture = None;
                } else {
                    let chord = Chord::from_press(*key, *physical_key, *modifiers);
                    if chord.is_bare() {
                        help.notice = Some(format!(
                            "{} alone would stop you typing it in the terminal; add Ctrl or Alt.",
                            chord.label()
                        ));
                        continue;
                    }
                    help.notice = self.bind_chord(cap, chord).err();
                    help.capture = None;
                }
                break;
            }
        } else {
            close |= ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            // The shortcut that opened it closes it (but not the press that just opened it).
            if !first_frame {
                close |= events.iter().any(|e| {
                    matches!(e, egui::Event::Key { key, physical_key, pressed: true, modifiers, .. }
                        if self.keybinds.lookup(*key, *physical_key, *modifiers) == Some(Action::ShowHelp))
                });
            }
        }

        let screen = ctx.content_rect();
        let width = (screen.width() - 32.0).min(760.0);
        let height = (screen.height() - 64.0).max(160.0);
        let c = self.chrome.clone();

        egui::Area::new(Id::new("help_backdrop"))
            .fixed_pos(screen.min)
            .order(Order::Middle)
            .interactable(false)
            .show(ctx, |ui| {
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(90));
            });

        let area = egui::Area::new(Id::new("help"))
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(c.raised(0.06))
                    .stroke(Stroke::new(1.0, c.raised(0.2)))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(14)
                    .shadow(egui::Shadow {
                        offset: [0, 8],
                        blur: 28,
                        spread: 0,
                        color: Color32::from_black_alpha(140),
                    })
                    .show(ui, |ui| {
                        ui.set_width(width - 28.0);
                        ui.set_max_height(height - 28.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Keyboard shortcuts").size(17.0).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let edit = ui.add(
                                    egui::TextEdit::singleline(&mut help.query)
                                        .hint_text("Filter…")
                                        .desired_width(220.0),
                                );
                                if help.capture.is_none() {
                                    edit.request_focus();
                                }
                            });
                        });
                        let hint = match (&help.notice, help.capture) {
                            (Some(n), _) => RichText::new(n).color(c.danger),
                            (None, Some(_)) => RichText::new(
                                "Press the new shortcut · Esc cancels · Backspace removes it",
                            )
                            .color(c.accent),
                            (None, None) => RichText::new(
                                "Click a shortcut to change it, + to add one. Saved to [keybindings] in the config.",
                            )
                            .weak(),
                        };
                        ui.label(hint.size(12.0));
                        ui.add_space(4.0);
                        ui.separator();

                        egui::ScrollArea::vertical()
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                // Keep the rows clear of the scroll bar.
                                egui::Frame::new()
                                    .inner_margin(egui::Margin {
                                        right: 16,
                                        bottom: 4,
                                        ..Default::default()
                                    })
                                    .show(ui, |ui| {
                                        self.help_sections(ui, &c, &help, &mut ops);
                                        self.help_gestures(ui, &c, &help.query);
                                    });
                            });
                    });
            });

        // Clicking outside closes it (not while waiting for a key).
        if help.capture.is_none()
            && !first_frame
            && ctx.input(|i| i.pointer.any_pressed())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| !area.response.rect.contains(p))
        {
            close = true;
        }

        for op in ops {
            match op {
                Op::Capture(cap) => {
                    help.capture = Some(cap);
                    help.notice = None;
                }
                Op::Reset(action) => {
                    help.notice =
                        crate::settings::set_keybinding(&action.config_name(), None).err();
                    self.reload_config();
                }
            }
        }
        if !close {
            self.help = Some(help);
        }
    }

    fn help_sections(&self, ui: &mut Ui, c: &UiColors, help: &Help, ops: &mut Vec<Op>) {
        let goto: Vec<Action> = (1..=9).map(Action::GotoTab).collect();
        for &(title, actions) in SECTIONS {
            let actions: Vec<Action> = if title == "Tabs" {
                actions
                    .iter()
                    .copied()
                    .chain(goto.iter().copied())
                    .collect()
            } else {
                actions.to_vec()
            };
            let rows: Vec<(Action, Vec<Chord>)> = actions
                .into_iter()
                .map(|a| (a, self.keybinds.chords(a)))
                .filter(|(a, chords)| {
                    let labels: Vec<String> = chords.iter().map(Chord::label).collect();
                    let hay = format!("{} {}", a.label(), labels.join(" "));
                    fuzzy_score(&help.query, &hay).is_some()
                })
                .collect();
            if rows.is_empty() {
                continue;
            }
            section_title(ui, c, title);
            for (action, chords) in rows {
                self.action_row(ui, c, help, action, &chords, ops);
            }
        }
    }

    fn action_row(
        &self,
        ui: &mut Ui,
        c: &UiColors,
        help: &Help,
        action: Action,
        chords: &[Chord],
        ops: &mut Vec<Op>,
    ) {
        let capturing = |slot| help.capture == Some(Capture { action, slot });
        ui.horizontal(|ui| {
            ui.set_min_height(26.0);
            ui.label(action.label());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.keybinds.is_overridden(action)
                    && ui
                        .small_button("Reset")
                        .on_hover_text("Back to the default shortcut")
                        .clicked()
                {
                    ops.push(Op::Reset(action));
                }
                if capturing(None) {
                    chip(ui, c, "press keys…", ChipKind::Capturing);
                } else if chip(ui, c, "+", ChipKind::Add)
                    .on_hover_text("Add a shortcut")
                    .clicked()
                {
                    ops.push(Op::Capture(Capture { action, slot: None }));
                }
                // Right-to-left: add in reverse so they read in order.
                for (i, chord) in chords.iter().enumerate().rev() {
                    if capturing(Some(i)) {
                        chip(ui, c, "press keys…", ChipKind::Capturing);
                        continue;
                    }
                    let others = self.keybinds.conflicts(*chord, action);
                    let kind = if others.is_empty() {
                        ChipKind::Key
                    } else {
                        ChipKind::Conflict
                    };
                    let mut resp = chip(ui, c, &chord.label(), kind);
                    resp = if others.is_empty() {
                        resp.on_hover_text("Click to change")
                    } else {
                        let names: Vec<String> = others.iter().map(|a| a.label()).collect();
                        resp.on_hover_text(format!(
                            "Also bound to: {}. Click to change",
                            names.join(", ")
                        ))
                    };
                    if resp.clicked() {
                        ops.push(Op::Capture(Capture {
                            action,
                            slot: Some(i),
                        }));
                    }
                }
                if chords.is_empty() && !capturing(None) {
                    ui.label(RichText::new("unbound").weak().size(12.0));
                }
            });
        });
    }

    fn help_gestures(&self, ui: &mut Ui, c: &UiColors, query: &str) {
        for (title, rows) in gestures() {
            let rows: Vec<_> = rows
                .into_iter()
                .filter(|(g, what)| fuzzy_score(query, &format!("{what} {g}")).is_some())
                .collect();
            if rows.is_empty() {
                continue;
            }
            section_title(ui, c, title);
            for (gesture, what) in rows {
                ui.horizontal(|ui| {
                    ui.set_min_height(26.0);
                    ui.label(what);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        chip(ui, c, gesture, ChipKind::Fixed);
                    });
                });
            }
        }
    }

    /// Replace (or add) one chord of `cap.action` and save it to the config.
    fn bind_chord(&mut self, cap: Capture, chord: Chord) -> Result<(), String> {
        let mut chords = self.keybinds.chords(cap.action);
        match cap.slot {
            Some(i) if i < chords.len() => chords[i] = chord,
            _ => chords.push(chord),
        }
        let mut seen = Vec::new();
        chords.retain(|c| {
            let new = !seen.contains(c);
            seen.push(*c);
            new
        });
        self.save_chords(cap.action, &chords)
    }

    fn remove_chord(&mut self, action: Action, slot: usize) -> Result<(), String> {
        let mut chords = self.keybinds.chords(action);
        if slot < chords.len() {
            chords.remove(slot);
        }
        self.save_chords(action, &chords)
    }

    fn save_chords(&mut self, action: Action, chords: &[Chord]) -> Result<(), String> {
        let list: Vec<String> = chords.iter().map(Chord::to_config).collect();
        crate::settings::set_keybinding(&action.config_name(), Some(&list))?;
        // Apply now rather than waiting for the file watcher.
        self.reload_config();
        Ok(())
    }
}

fn section_title(ui: &mut Ui, c: &UiColors, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).strong().size(12.5).color(c.accent));
    ui.add_space(2.0);
}

#[derive(Clone, Copy, PartialEq)]
enum ChipKind {
    Key,
    Conflict,
    Capturing,
    Add,
    Fixed,
}

/// A key cap: monospace label in a rounded box.
fn chip(ui: &mut Ui, c: &UiColors, text: &str, kind: ChipKind) -> egui::Response {
    let visuals = ui.visuals();
    let color = match kind {
        ChipKind::Conflict => c.danger,
        ChipKind::Capturing => c.accent,
        ChipKind::Fixed | ChipKind::Add => visuals.weak_text_color(),
        ChipKind::Key => visuals.strong_text_color(),
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::monospace(12.0), color);
    let size = galley.size() + vec2(14.0, 6.0);
    let sense = if kind == ChipKind::Fixed {
        Sense::hover()
    } else {
        Sense::click()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let fill = match kind {
        ChipKind::Fixed => c.raised(0.1),
        _ if resp.hovered() => c.raised(0.24),
        _ => c.raised(0.15),
    };
    let stroke = match kind {
        ChipKind::Capturing => Stroke::new(1.5, c.accent),
        ChipKind::Conflict => Stroke::new(1.0, c.danger),
        _ => Stroke::new(1.0, c.raised(0.28)),
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(5),
        fill,
        stroke,
        StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    if kind == ChipKind::Fixed {
        resp
    } else {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinds::ACTIONS;

    #[test]
    fn every_action_is_listed() {
        for &(name, _, action) in ACTIONS {
            assert!(
                SECTIONS.iter().any(|(_, list)| list.contains(&action)),
                "`{name}` is missing from the shortcuts window"
            );
        }
    }
}
