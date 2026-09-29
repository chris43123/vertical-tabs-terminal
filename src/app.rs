//! Application state: tabs, their sessions, the split workspace and event routing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event as TermEvent, WindowSize};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::vte::ansi::Rgb;
use eframe::egui::{self, Color32, Key, Modifiers};

use crate::config::Config;
use crate::keybinds::{Action, Keybinds};
use crate::layout::{Drop, Edge, Workspace};
use crate::procinfo;
use crate::profiles::{self, Profile};
use crate::render::{Fonts, Palette};
use crate::session::{GridSize, Session, TabId};

pub struct Tab {
    pub session: Session,
    pub profile: Profile,
    /// Title set by the application via OSC 0/2.
    pub osc_title: Option<String>,
    /// Title derived from the foreground process and cwd.
    pub auto_title: Option<String>,
    /// Title set by the user; overrides everything else.
    pub custom_title: Option<String>,
    /// New output arrived while the tab wasn't visible.
    pub activity: bool,
    /// Bell rang while the tab wasn't focused.
    pub bell: bool,
    /// Last cwd seen, used to start new tabs in the same directory.
    pub cwd: Option<PathBuf>,
}

impl Tab {
    pub fn title(&self) -> &str {
        self.custom_title
            .as_deref()
            .or(self.osc_title.as_deref().filter(|t| !t.trim().is_empty()))
            .or(self.auto_title.as_deref())
            .unwrap_or(&self.profile.name)
    }
}

/// Enough of a closed tab to reopen it.
struct ClosedTab {
    profile: Profile,
    cwd: Option<PathBuf>,
    custom_title: Option<String>,
    index: usize,
}

const MAX_CLOSED: usize = 20;

/// Payload for dragging a tab out of the sidebar.
#[derive(Clone, Copy, Debug)]
pub struct TabDrag(pub TabId);

pub struct App {
    pub config: Config,
    pub profiles: Vec<Profile>,
    pub tabs: HashMap<TabId, Tab>,
    pub ws: Workspace,
    next_id: TabId,

    pub fonts: Fonts,
    pub palette: Palette,
    pub font_size: f32,
    pub keybinds: Keybinds,

    tx: Sender<(TabId, TermEvent)>,
    rx: Receiver<(TabId, TermEvent)>,
    ctx: egui::Context,

    pub sidebar_collapsed: bool,
    /// Collapsed sidebar temporarily shown expanded because the pointer hovers it.
    pub sidebar_peek: bool,
    /// Tab currently being renamed, with the edit buffer.
    pub renaming: Option<(TabId, String)>,
    /// Pane rects of the active view from the last frame (for directional pane focus).
    pub pane_rects: Vec<(TabId, egui::Rect)>,
    /// Sub-line scroll accumulator in points.
    pub scroll_accum: f32,
    /// Scroll the sidebar so the focused tab is visible (after keyboard navigation).
    pub scroll_to_focused: bool,
    /// The tab switcher / command palette, when open.
    pub switcher: Option<crate::ui::Switcher>,
    closed: Vec<ClosedTab>,
    /// Focused tab as of the last frame, and the one before it.
    last_focused: Option<TabId>,
    pub prev_focused: Option<TabId>,

    clipboard: Option<arboard::Clipboard>,
    last_poll: Instant,
    window_title: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        let ctx = cc.egui_ctx.clone();
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let palette = Palette::from_config(&config.colors);
        apply_style(&ctx, &palette);

        let font_size = config.font.size;
        let fonts = Fonts::new(
            &ctx,
            config.font.family.as_deref(),
            font_size,
            ctx.pixels_per_point(),
        );
        let (tx, rx) = channel();
        let keybinds = Keybinds::new(&config.keybindings);

        let mut app = Self {
            profiles: profiles::load(&config),
            sidebar_collapsed: config.sidebar_collapsed,
            config,
            tabs: HashMap::new(),
            ws: Workspace::default(),
            next_id: 1,
            fonts,
            palette,
            font_size,
            keybinds,
            tx,
            rx,
            ctx,
            sidebar_peek: false,
            renaming: None,
            pane_rects: Vec::new(),
            scroll_accum: 0.0,
            scroll_to_focused: false,
            switcher: None,
            closed: Vec::new(),
            last_focused: None,
            prev_focused: None,
            clipboard: arboard::Clipboard::new().ok(),
            last_poll: Instant::now() - Duration::from_secs(10),
            window_title: String::new(),
        };
        app.new_tab(0, None);
        app
    }

    /// Open a tab with profile `profile_idx` in the focused tab's directory. Returns its id.
    /// `split`: split the focused pane of the active view on that edge instead of opening a standalone tab.
    pub fn new_tab(&mut self, profile_idx: usize, split: Option<Edge>) -> Option<TabId> {
        let profile = self
            .profiles
            .get(profile_idx)
            .or(self.profiles.first())?
            .clone();
        // Start in the focused tab's directory, like most terminals do.
        let cwd = self
            .ws
            .focused()
            .and_then(|f| self.tabs.get_mut(&f))
            .and_then(|t| {
                t.cwd = query_proc(&t.session).cwd.or(t.cwd.take());
                t.cwd.clone()
            });
        self.spawn_tab(profile, cwd, split)
    }

    fn spawn_tab(
        &mut self,
        profile: Profile,
        cwd: Option<PathBuf>,
        split: Option<Edge>,
    ) -> Option<TabId> {
        let id = self.next_id;
        self.next_id += 1;
        let focused = self.ws.focused();

        // Real size is applied on the first layout pass.
        let size = GridSize {
            cols: 80,
            lines: 24,
        };
        let session = match Session::spawn(
            id,
            &profile,
            self.config.scrollback,
            size,
            self.fonts.cell_px(),
            cwd,
            self.tx.clone(),
            self.ctx.clone(),
        ) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("vtt: failed to start {}: {err}", profile.command);
                return None;
            }
        };
        self.tabs.insert(
            id,
            Tab {
                session,
                profile,
                osc_title: None,
                auto_title: None,
                custom_title: None,
                activity: false,
                bell: false,
                cwd: None,
            },
        );
        self.ws.add(id, focused);
        if let (Some(edge), Some(target)) = (split, focused) {
            self.ws.drop_on(id, target, Drop::Edge(edge));
        }
        self.scroll_to_focused = true;
        Some(id)
    }

    pub fn close_tab(&mut self, id: TabId) {
        if let Some(tab) = self.tabs.get(&id) {
            self.closed.push(ClosedTab {
                profile: tab.profile.clone(),
                cwd: query_proc(&tab.session).cwd.or(tab.cwd.clone()),
                custom_title: tab.custom_title.clone(),
                index: self.ws.order.iter().position(|t| *t == id).unwrap_or(0),
            });
            if self.closed.len() > MAX_CLOSED {
                self.closed.remove(0);
            }
        }
        self.ws.close(id);
        self.tabs.remove(&id);
        if self.renaming.as_ref().is_some_and(|(r, _)| *r == id) {
            self.renaming = None;
        }
        self.scroll_to_focused = true;
    }

    /// Reopen the most recently closed tab: same profile, directory, name and sidebar position.
    /// (The old process is gone, so this starts a fresh shell.)
    pub fn reopen_closed_tab(&mut self) {
        let Some(closed) = self.closed.pop() else {
            return;
        };
        if let Some(id) = self.spawn_tab(closed.profile, closed.cwd, None) {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.custom_title = closed.custom_title;
            }
            self.ws.reorder(id, closed.index);
        }
    }

    pub fn duplicate_tab(&mut self, id: TabId) {
        let Some(tab) = self.tabs.get(&id) else {
            return;
        };
        let idx = self
            .profiles
            .iter()
            .position(|p| p.name == tab.profile.name)
            .unwrap_or(0);
        self.ws.activate(id);
        self.new_tab(idx, None);
    }

    /// Start renaming a tab in the sidebar (peeking it open if collapsed).
    pub fn start_rename(&mut self, id: TabId) {
        let current = self
            .tabs
            .get(&id)
            .map(|t| t.title().to_string())
            .unwrap_or_default();
        self.renaming = Some((id, current));
        self.scroll_to_focused = true;
        if self.sidebar_collapsed {
            self.sidebar_peek = true;
        }
    }

    /// Jump to the next tab (after the focused one, wrapping) that has a bell or unread output.
    fn next_activity(&mut self) {
        let n = self.ws.order.len();
        let start = self
            .ws
            .focused()
            .and_then(|f| self.ws.order.iter().position(|t| *t == f))
            .unwrap_or(0);
        let next = (1..=n)
            .map(|i| self.ws.order[(start + i) % n])
            .find(|id| self.tabs.get(id).is_some_and(|t| t.bell || t.activity));
        if let Some(id) = next {
            self.activate(id);
        }
    }

    /// Show a tab and clear its unread markers.
    pub fn activate(&mut self, id: TabId) {
        self.ws.activate(id);
        self.mark_seen();
    }

    fn mark_seen(&mut self) {
        for id in self.ws.visible() {
            if let Some(t) = self.tabs.get_mut(&id) {
                t.activity = false;
                if Some(id) == self.ws.focused() {
                    t.bell = false;
                }
            }
        }
    }

    /// Drain terminal events from all PTY threads.
    fn process_events(&mut self) {
        let visible = self.ws.visible();
        let focused = self.ws.focused();
        let mut exited = Vec::new();
        while let Ok((id, event)) = self.rx.try_recv() {
            let Some(tab) = self.tabs.get_mut(&id) else {
                continue;
            };
            match event {
                TermEvent::Wakeup => {
                    if !visible.contains(&id) {
                        tab.activity = true;
                    }
                }
                TermEvent::Bell => {
                    if focused != Some(id) {
                        tab.bell = true;
                    }
                }
                TermEvent::Title(t) => tab.osc_title = Some(t),
                TermEvent::ResetTitle => tab.osc_title = None,
                TermEvent::PtyWrite(s) => tab.session.write(s.into_bytes()),
                TermEvent::ClipboardStore(_, text) => {
                    if let Some(cb) = &mut self.clipboard {
                        let _ = cb.set_text(text);
                    }
                }
                TermEvent::ClipboardLoad(_, format) => {
                    if let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) {
                        tab.session.write(format(&text).into_bytes());
                    }
                }
                TermEvent::ColorRequest(index, format) => {
                    let color = match index {
                        256 => Some(self.palette.foreground()),
                        257 => Some(self.palette.background()),
                        _ => None,
                    };
                    let from_term = tab.session.term.lock().colors()[index];
                    let rgb = from_term.or(color.map(|c| Rgb {
                        r: c.r(),
                        g: c.g(),
                        b: c.b(),
                    }));
                    if let Some(rgb) = rgb {
                        tab.session.write(format(rgb).into_bytes());
                    }
                }
                TermEvent::TextAreaSizeRequest(format) => {
                    let (cw, ch) = self.fonts.cell_px();
                    let size = tab.session.size;
                    let ws = WindowSize {
                        num_lines: size.lines as u16,
                        num_cols: size.cols as u16,
                        cell_width: cw,
                        cell_height: ch,
                    };
                    tab.session.write(format(ws).into_bytes());
                }
                TermEvent::ChildExit(_) | TermEvent::Exit => exited.push(id),
                TermEvent::MouseCursorDirty | TermEvent::CursorBlinkingChange => {}
            }
        }
        for id in exited {
            if self.tabs.contains_key(&id) {
                self.close_tab(id);
            }
        }
    }

    /// Refresh auto titles about once a second while titles are visible.
    fn poll_titles(&mut self) {
        let sidebar_visible = !self.sidebar_collapsed || self.sidebar_peek;
        if !sidebar_visible {
            return;
        }
        if self.last_poll.elapsed() >= Duration::from_secs(1) {
            self.last_poll = Instant::now();
            for tab in self.tabs.values_mut() {
                let info = query_proc(&tab.session);
                tab.auto_title = procinfo::format_title(&info);
                if info.cwd.is_some() {
                    tab.cwd = info.cwd;
                }
            }
        }
        self.ctx.request_repaint_after(Duration::from_secs(1));
    }

    /// Remember the previously focused tab (the switcher preselects it).
    fn track_focus(&mut self) {
        let focused = self.ws.focused();
        if focused != self.last_focused {
            if self
                .last_focused
                .is_some_and(|t| self.tabs.contains_key(&t))
            {
                self.prev_focused = self.last_focused;
            }
            self.last_focused = focused;
        }
    }

    fn update_window_title(&mut self) {
        let title = match self.ws.focused().and_then(|f| self.tabs.get(&f)) {
            Some(t) => format!("{} — vtt", t.title()),
            None => "vtt".to_string(),
        };
        if title != self.window_title {
            self.ctx
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    /// App-level shortcuts. Returns true if the key was consumed.
    fn handle_shortcut(&mut self, key: Key, physical: Option<Key>, m: Modifiers) -> bool {
        match self.keybinds.lookup(key, physical, m) {
            Some(action) => self.run_action(action),
            None => false,
        }
    }

    /// Perform an app action. Returns false if it didn't apply, so the key should reach the shell.
    pub fn run_action(&mut self, action: Action) -> bool {
        match action {
            Action::NewTab => {
                self.new_tab(0, None);
            }
            Action::CloseTab => {
                if let Some(f) = self.ws.focused() {
                    self.close_tab(f);
                }
            }
            Action::SplitRight => {
                self.new_tab(0, Some(Edge::Right));
            }
            Action::SplitDown => {
                self.new_tab(0, Some(Edge::Bottom));
            }
            Action::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                self.sidebar_peek = false;
            }
            Action::ReopenClosedTab => self.reopen_closed_tab(),
            Action::DuplicateTab => {
                if let Some(f) = self.ws.focused() {
                    self.duplicate_tab(f);
                }
            }
            Action::RenameTab => {
                if let Some(f) = self.ws.focused() {
                    self.start_rename(f);
                }
            }
            Action::MinimizePane => {
                if let Some(f) = self.ws.focused() {
                    self.ws.minimize(f);
                }
            }
            Action::NextTab | Action::PrevTab => {
                // With a single tab, let Alt+Up/Down through to the shell.
                if self.ws.order.len() < 2 {
                    return false;
                }
                self.ws.cycle(action == Action::NextTab);
            }
            Action::MoveTabUp | Action::MoveTabDown => {
                if let Some(f) = self.ws.focused() {
                    self.ws.move_tab(f, action == Action::MoveTabDown);
                }
            }
            Action::GotoTab(n) => {
                if let Some(&id) = self.ws.order.get(n as usize - 1) {
                    self.ws.activate(id);
                }
            }
            Action::LastTab => {
                if let Some(&id) = self.ws.order.last() {
                    self.ws.activate(id);
                }
            }
            Action::NextActivity => self.next_activity(),
            Action::CommandPalette => self.switcher = Some(crate::ui::Switcher::default()),
            Action::FocusLeft | Action::FocusRight | Action::FocusUp | Action::FocusDown => {
                let dir = match action {
                    Action::FocusLeft => egui::vec2(-1.0, 0.0),
                    Action::FocusRight => egui::vec2(1.0, 0.0),
                    Action::FocusUp => egui::vec2(0.0, -1.0),
                    _ => egui::vec2(0.0, 1.0),
                };
                // With no pane in that direction, let the key through (shells use Alt+Arrow for word movement).
                if !self.focus_neighbor(dir) {
                    return false;
                }
            }
            Action::ZoomIn => self.font_size = (self.font_size + 1.0).min(48.0),
            Action::ZoomOut => self.font_size = (self.font_size - 1.0).max(6.0),
            Action::ZoomReset => self.font_size = self.config.font.size,
            Action::ScrollPageUp | Action::ScrollPageDown => {
                if let Some(tab) = self.focused_tab() {
                    let scroll = if action == Action::ScrollPageUp {
                        Scroll::PageUp
                    } else {
                        Scroll::PageDown
                    };
                    tab.session.term.lock().scroll_display(scroll);
                }
            }
        }
        self.mark_seen();
        self.scroll_to_focused = true;
        true
    }

    /// Move focus to the closest pane in direction `dir`. Returns false if there is none.
    fn focus_neighbor(&mut self, dir: egui::Vec2) -> bool {
        let Some(cur) = self.ws.focused() else {
            return false;
        };
        let Some(&(_, from)) = self.pane_rects.iter().find(|(t, _)| *t == cur) else {
            return false;
        };
        let best = self
            .pane_rects
            .iter()
            .filter(|(t, _)| *t != cur)
            .filter_map(|&(t, r)| {
                let d = r.center() - from.center();
                let along = d.dot(dir);
                (along > 0.0).then(|| (t, along + (d - dir * along).length() * 2.0))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((t, _)) => {
                self.activate(t);
                true
            }
            None => false,
        }
    }

    /// Route keyboard/clipboard events to the focused terminal.
    fn handle_keyboard(&mut self) {
        if self.renaming.is_some()
            || self.switcher.is_some()
            || self.ctx.egui_wants_keyboard_input()
        {
            return;
        }
        let events = self.ctx.input(|i| i.events.clone());
        let mods = self.ctx.input(|i| i.modifiers);
        // A key that triggered a shortcut is followed by its text (e.g. "t" for Alt+T);
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
                    if self.handle_shortcut(key, physical_key, modifiers) {
                        swallow_text = true;
                        continue;
                    }
                    // Keys typed after opening the switcher or a rename box belong to that text field.
                    if self.switcher.is_some() || self.renaming.is_some() {
                        break;
                    }
                    let Some(tab) = self.focused_tab() else {
                        continue;
                    };
                    let mode = *tab.session.term.lock().mode();
                    if let Some(bytes) = crate::input::encode_key(key, modifiers, mode) {
                        self.send_input(bytes);
                    }
                }
                egui::Event::Text(_) if swallow_text => {
                    swallow_text = false;
                    swallowed.push(index);
                }
                _ if self.switcher.is_some() || self.renaming.is_some() => break,
                egui::Event::Text(text) => {
                    let bytes = crate::input::encode_text(&text, mods);
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
                egui::Event::Paste(text) => self.paste(&text),
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

    fn focused_tab(&self) -> Option<&Tab> {
        self.ws.focused().and_then(|f| self.tabs.get(&f))
    }

    /// Write user input to the focused tab, snapping the view back to the bottom.
    fn send_input(&mut self, bytes: Vec<u8>) {
        if let Some(tab) = self.focused_tab() {
            {
                let mut term = tab.session.term.lock();
                term.scroll_display(Scroll::Bottom);
                term.selection = None;
            }
            tab.session.write(bytes);
        }
    }

    pub fn paste(&mut self, text: &str) {
        if let Some(tab) = self.focused_tab() {
            let mode = *tab.session.term.lock().mode();
            let bytes = crate::input::encode_paste(text, mode);
            self.send_input(bytes);
        }
    }

    /// Copy the focused tab's selection. Returns false if nothing was selected.
    fn copy_selection(&mut self) -> bool {
        let Some(tab) = self.focused_tab() else {
            return false;
        };
        let text = {
            let mut term = tab.session.term.lock();
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Buttons must never keep keyboard focus: Space/Enter belong to the terminal.
        if self.renaming.is_none() && self.switcher.is_none() {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        self.fonts
            .update(&ctx, self.font_size, ctx.pixels_per_point());
        self.process_events();
        self.handle_keyboard();
        self.poll_titles();

        if self.tabs.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        self.sidebar(ui);
        self.panes(ui);
        self.switcher_ui(&ctx);
        self.track_focus();
        self.update_window_title();
    }
}

fn query_proc(session: &Session) -> procinfo::ProcInfo {
    #[cfg(unix)]
    let fd = session.pty_fd as i64;
    #[cfg(not(unix))]
    let fd = -1;
    procinfo::query(session.child_pid, fd)
}

fn apply_style(ctx: &egui::Context, palette: &Palette) {
    let bg = palette.background();
    let mut visuals = egui::Visuals::dark();
    let sidebar_bg = shade(bg, 0.82);
    visuals.panel_fill = sidebar_bg;
    visuals.window_fill = shade(bg, 1.15);
    visuals.extreme_bg_color = shade(bg, 0.7);
    visuals.widgets.noninteractive.bg_stroke.color = shade(bg, 1.4);
    visuals.selection.bg_fill = Color32::from_rgb(0x58, 0x5b, 0x70);
    ctx.set_visuals(visuals);
}

/// Scale a color's brightness.
pub fn shade(c: Color32, f: f32) -> Color32 {
    let s = |v: u8| ((v as f32 * f).round().clamp(0.0, 255.0)) as u8;
    Color32::from_rgb(s(c.r()), s(c.g()), s(c.b()))
}
