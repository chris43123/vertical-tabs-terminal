//! Keyboard and clipboard routing to the focused terminal.

use std::time::Instant;

use alacritty_terminal::grid::Scroll;
use eframe::egui::{self};

use crate::terminal::session::Session;

use super::App;
use super::tab::Tab;

impl App {
    /// Route keyboard/clipboard events to the focused terminal.
    pub(super) fn handle_keyboard(&mut self) {
        if self.renaming.is_some()
            || self.switcher.is_some()
            || self.help.is_some()
            || self.ctx.egui_wants_keyboard_input()
        {
            return;
        }
        let events = self.ctx.input(|i| i.events.clone());
        let mods = self.ctx.input(|i| i.modifiers);
        // A key that triggered a shortcut is followed by its text (e.g. "t" for Ctrl+Shift+T);
        // swallow it so the shell doesn't also receive ESC t.
        let mut swallow_text = false;
        // Indices of swallowed text events, removed from egui's input afterwards so a text field
        // opened by the shortcut (rename, switcher) doesn't receive them either.
        let mut swallowed = Vec::new();
        for (index, event) in events.into_iter().enumerate() {
            match event {
                egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    swallow_text = false;
                    self.v_press_seen |= key == egui::Key::V;
                    if self.handle_shortcut(key, physical_key, modifiers) {
                        swallow_text = true;
                        continue;
                    }
                    // Keys typed after opening the switcher or a rename box belong to that text field.
                    if self.switcher.is_some() || self.renaming.is_some() || self.help.is_some() {
                        break;
                    }
                    let Some(session) = self.focused_session() else {
                        continue;
                    };
                    let mode = *session.term.lock().mode();
                    if let Some(bytes) = crate::terminal::input::encode_key(key, modifiers, mode) {
                        self.send_input(bytes);
                    }
                }
                egui::Event::Text(_) if swallow_text => {
                    swallow_text = false;
                    swallowed.push(index);
                }
                _ if self.switcher.is_some() || self.renaming.is_some() || self.help.is_some() => {
                    break;
                }
                egui::Event::Text(text) => {
                    let bytes = crate::terminal::input::encode_text(&text, mods);
                    self.send_input(bytes);
                }
                egui::Event::Copy => {
                    // Ctrl+C copies when there's a selection (or with Shift / Cmd); otherwise it's SIGINT.
                    let copied = self.copy_selection();
                    if !copied && !mods.shift && !mods.mac_cmd {
                        self.send_input(vec![0x03]);
                    }
                }
                egui::Event::Cut => {
                    if !mods.mac_cmd {
                        self.send_input(vec![0x18]);
                    }
                }
                egui::Event::Paste(text) => {
                    self.paste_seen = true;
                    self.paste(&text);
                }
                // egui-winit swallows Ctrl+V and only emits a paste when the clipboard holds
                // text. A V release with neither a press nor a paste before it means the
                // clipboard had something else (an image): pass ^V on, so programs like
                // Claude Code read the clipboard themselves.
                egui::Event::Key {
                    key: egui::Key::V,
                    pressed: false,
                    modifiers,
                    ..
                } => {
                    if !std::mem::take(&mut self.v_press_seen)
                        && !std::mem::take(&mut self.paste_seen)
                        && !modifiers.shift
                    {
                        self.send_input(vec![0x16]);
                    }
                    self.paste_seen = false;
                }
                _ => {}
            }
        }
        if !swallowed.is_empty() {
            self.ctx.input_mut(|i| {
                let mut index = 0;
                i.events.retain(|_| {
                    index += 1;
                    !swallowed.contains(&(index - 1))
                });
            });
        }
    }

    pub(super) fn focused_session(&self) -> Option<&Session> {
        self.ws
            .focused()
            .and_then(|f| self.tabs.get(&f))
            .and_then(Tab::session)
    }

    /// Write user input to the focused tab, snapping the view back to the bottom.
    pub(super) fn send_input(&mut self, bytes: Vec<u8>) {
        self.blink_epoch = Instant::now();
        if let Some(session) = self.focused_session() {
            {
                let mut term = session.term.lock();
                term.scroll_display(Scroll::Bottom);
                term.selection = None;
            }
            session.write(bytes);
        }
    }

    pub fn paste(&mut self, text: &str) {
        if let Some(session) = self.focused_session() {
            let mode = *session.term.lock().mode();
            let bytes = crate::terminal::input::encode_paste(text, mode);
            self.send_input(bytes);
        }
    }

    /// Copy the focused tab's selection. Returns false if nothing was selected.
    fn copy_selection(&mut self) -> bool {
        let Some(session) = self.focused_session() else {
            return false;
        };
        let text = {
            let mut term = session.term.lock();
            let text = term.selection_to_string().filter(|s| !s.is_empty());
            if text.is_some() {
                term.selection = None;
            }
            text
        };
        match (text, &mut self.clipboard) {
            (Some(text), Some(cb)) => {
                let _ = cb.set_text(text);
                true
            }
            (Some(_), None) => true,
            _ => false,
        }
    }

    /// Copy text to the clipboard (used by right-click copy in panes).
    pub fn set_clipboard(&mut self, text: String) {
        if let Some(cb) = &mut self.clipboard {
            let _ = cb.set_text(text);
        }
    }

    pub fn clipboard_text(&mut self) -> Option<String> {
        self.clipboard.as_mut().and_then(|cb| cb.get_text().ok())
    }
}
