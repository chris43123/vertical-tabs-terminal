//! Application state: tabs, their sessions, the split workspace and event routing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event as TermEvent, WindowSize};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::vte::ansi::Rgb;
use eframe::egui::{self, Color32, Key, Modifiers};

use crate::config::keybinds::{Action, Keybinds};
use crate::config::watch::Watcher;
use crate::config::{BellMode, Config};
use crate::files::FileTree;
use crate::preview::Preview;
use crate::preview::highlight::Highlighter;
use crate::render::{Fonts, Palette};
use crate::terminal::procinfo;
use crate::terminal::profiles::{self, Profile};
use crate::terminal::session::{GridSize, Listener, Session};
use crate::theme::{self, Patch, Theme, UiColors, mix};
use crate::workspace::{Drop, Edge, GroupId, TabId, Workspace};

/// What a tab shows: a shell, or a read-only file preview.
pub enum Content {
    Term(Session),
    Preview(Box<Preview>),
}

/// Bounds of a pane's font size in points.
const MIN_FONT: f32 = 6.0;
const MAX_FONT: f32 = 48.0;

pub struct Tab {
    pub content: Content,
    /// For previews: a synthetic profile carrying the file name and type icon.
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
    /// When a visual bell last rang in this pane (`bell = "flash"`).
    pub bell_flash: Option<Instant>,
    /// Last cwd seen, used to start new tabs in the same directory.
    pub cwd: Option<PathBuf>,
    /// Foreground process name, from the last poll.
    pub process: Option<String>,
    /// Zoom of this pane in points, relative to the base font size (not persisted).
    pub zoom: f32,
}

impl Tab {
    /// Font size in points of this pane, given the base size.
    pub fn font_size(&self, base: f32) -> f32 {
        (base + self.zoom).clamp(MIN_FONT, MAX_FONT)
    }

    fn new(content: Content, profile: Profile) -> Self {
        Self {
            content,
            profile,
            osc_title: None,
            auto_title: None,
            custom_title: None,
            activity: false,
            bell: false,
            bell_flash: None,
            cwd: None,
            process: None,
            zoom: 0.0,
        }
    }

    pub fn session(&self) -> Option<&Session> {
        match &self.content {
            Content::Term(s) => Some(s),
            Content::Preview(_) => None,
        }
    }

    pub fn session_mut(&mut self) -> Option<&mut Session> {
        match &mut self.content {
            Content::Term(s) => Some(s),
            Content::Preview(_) => None,
        }
    }

    pub fn preview(&self) -> Option<&Preview> {
        match &self.content {
            Content::Preview(p) => Some(p),
            Content::Term(_) => None,
        }
    }

    pub fn preview_mut(&mut self) -> Option<&mut Preview> {
        match &mut self.content {
            Content::Preview(p) => Some(p),
            Content::Term(_) => None,
        }
    }

    /// Directory the tab is "in": the shell's cwd, or a previewed file's folder.
    pub fn dir(&self) -> Option<&Path> {
        match &self.content {
            Content::Term(_) => self.cwd.as_deref(),
            Content::Preview(p) => p.path.parent(),
        }
    }

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
    /// Set for a closed preview: the file it showed.
    preview: Option<PathBuf>,
}

/// What the sidebar's inline rename box is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameTarget {
    Tab(TabId),
    Group(GroupId),
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
    /// Colors for the app chrome (sidebar, headers, palette), derived from the theme.
    pub chrome: UiColors,
    /// Theme from the config/theme file, before any adopted shell palette.
    base_theme: Theme,
    /// Colors adopted from shells via OSC (with `adopt_shell_palette`).
    shell_patch: Patch,
    watcher: Watcher,
    pub font_size: f32,
    /// Whole-UI zoom factor (egui's zoom), separate from the terminal `font_size`.
    pub ui_zoom: f32,
    pub keybinds: Keybinds,

    tx: Sender<(TabId, TermEvent)>,
    rx: Receiver<(TabId, TermEvent)>,
    ctx: egui::Context,

    pub sidebar_collapsed: bool,
    /// Collapsed sidebar temporarily shown expanded because the pointer hovers it.
    pub sidebar_peek: bool,
    /// Sidebar and files panel both hidden (Ctrl+B).
    pub side_hidden: bool,
    /// Zen mode: the sidebar is revealed by touching the left window edge.
    pub zen_peek: bool,
    /// What the zen overlay shows: the files panel (true) or the tabs.
    pub zen_files: bool,
    /// The pointer has entered the zen overlay since it opened (a keyboard-opened overlay
    /// stays up until then).
    pub zen_hovered_once: bool,
    /// Tab or folder currently being renamed, with the edit buffer.
    pub renaming: Option<(RenameTarget, String)>,
    /// Pane rects of the active view from the last frame (for directional pane focus).
    pub pane_rects: Vec<(TabId, egui::Rect)>,
    /// Sub-line scroll accumulator in points.
    pub scroll_accum: f32,
    /// Scroll the sidebar so the focused tab is visible (after keyboard navigation).
    pub scroll_to_focused: bool,
    /// The tab switcher / command palette, when open.
    pub switcher: Option<crate::ui::Switcher>,
    /// The keyboard shortcuts window.
    pub help: Option<crate::ui::Help>,
    closed: Vec<ClosedTab>,
    /// Focused tab as of the last frame, and the one before it.
    last_focused: Option<TabId>,
    pub prev_focused: Option<TabId>,

    clipboard: Option<arboard::Clipboard>,
    /// A V key press reached us as a key event (i.e. it wasn't eaten as a paste shortcut).
    v_press_seen: bool,
    /// A paste event arrived since the last V release.
    paste_seen: bool,
    last_poll: Instant,
    /// Start of the cursor blink cycle; reset on input and focus so the cursor shows while typing.
    pub blink_epoch: Instant,
    window_focused: bool,
    window_title: String,
    /// Config/theme problems shown in the banner until fixed or dismissed.
    pub problems: Vec<String>,
    pub problems_dismissed: bool,

    /// The files panel between the sidebar and the panes.
    pub files_open: bool,
    pub files: FileTree,
    /// The cwd the tree last followed; the user may browse elsewhere until it changes.
    pub(crate) files_followed: Option<PathBuf>,
    /// Soft-wrap long lines in text and code previews.
    pub preview_wrap: bool,
    /// Git state of the repository the files panel shows.
    pub git: crate::files::git::Watcher,
    /// The files panel's search box.
    pub file_search: crate::files::search::Search,
    /// The search box had keyboard focus last frame (so it keeps it).
    pub search_focused: bool,
    /// Give the search box keyboard focus next frame.
    pub focus_search: bool,
    /// The focused shell was at its prompt at the last check (a command finishing is when
    /// git state is most likely to have changed).
    shell_was_idle: bool,
    last_cwd_check: Instant,
    /// Loaded on first use: syntax definitions are a few MB.
    highlighter: Option<Arc<Highlighter>>,
    /// Code colors generated from the palette, and a counter bumped when they change.
    pub syntax_theme: Arc<syntect::highlighting::Theme>,
    pub syntax_theme_xml: String,
    pub theme_generation: u64,
    pub md_cache: Option<egui_commonmark::CommonMarkCache>,
    /// Theme generation the markdown cache's code theme was registered for.
    pub md_theme_generation: u64,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        let ctx = cc.egui_ctx.clone();
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let resolved = theme::resolve(&config);
        let chrome = UiColors::from_theme(&resolved.theme);
        let palette = Palette::from_theme(&resolved.theme);
        apply_style(&ctx, &chrome);
        apply_motion(&ctx, config.reduce_motion);
        let watcher = Watcher::spawn(watch_paths(resolved.file), ctx.clone());

        let ui_zoom = restored_ui_zoom(cc.storage, config.ui_scale);
        ctx.set_zoom_factor(ui_zoom);
        let font_size = config.font.size;
        let mut fonts = Fonts::new(
            &ctx,
            config.font.family.as_deref(),
            font_size,
            ctx.pixels_per_point(),
        );
        fonts.set_spacing(&ctx, config.font.line_height, config.font.letter_spacing);
        let (tx, rx) = channel();
        let keybinds = Keybinds::new(&config.keybindings);
        egui_extras::install_image_loaders(&ctx);
        let syntax_theme_xml = crate::preview::highlight::tm_theme(&palette);
        let mut files = FileTree::default();
        files.show_hidden = config.files.show_hidden;
        let files_open = config.files.open;
        let preview_wrap = config.files.wrap;

        let mut app = Self {
            profiles: profiles::load(&config),
            sidebar_collapsed: config.sidebar_collapsed,
            config,
            tabs: HashMap::new(),
            ws: Workspace::default(),
            next_id: 1,
            fonts,
            palette,
            chrome,
            base_theme: resolved.theme,
            shell_patch: Patch::default(),
            watcher,
            font_size,
            ui_zoom,
            keybinds,
            tx,
            rx,
            ctx,
            sidebar_peek: false,
            side_hidden: false,
            zen_peek: false,
            zen_files: false,
            zen_hovered_once: false,
            renaming: None,
            pane_rects: Vec::new(),
            scroll_accum: 0.0,
            scroll_to_focused: false,
            switcher: None,
            help: None,
            closed: Vec::new(),
            last_focused: None,
            prev_focused: None,
            clipboard: arboard::Clipboard::new().ok(),
            v_press_seen: false,
            paste_seen: false,
            last_poll: Instant::now() - Duration::from_secs(10),
            blink_epoch: Instant::now(),
            window_focused: true,
            window_title: String::new(),
            problems: Vec::new(),
            problems_dismissed: false,
            files_open,
            preview_wrap,
            git: Default::default(),
            file_search: Default::default(),
            search_focused: false,
            focus_search: false,
            shell_was_idle: true,
            files,
            files_followed: None,
            last_cwd_check: Instant::now(),
            highlighter: None,
            syntax_theme: Arc::new(
                crate::preview::highlight::load_theme(&syntax_theme_xml).unwrap_or_default(),
            ),
            syntax_theme_xml,
            theme_generation: 1,
            md_cache: None,
            md_theme_generation: 0,
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
        let cwd = self.ws.focused().and_then(|f| self.focused_dir(f));
        self.spawn_tab(profile, cwd, split)
    }

    /// Fresh directory of tab `id` (re-queried for a shell).
    fn focused_dir(&mut self, id: TabId) -> Option<PathBuf> {
        let tab = self.tabs.get_mut(&id)?;
        if let Some(session) = tab.session() {
            let cwd = session.proc_info().cwd;
            if cwd.is_some() {
                tab.cwd = cwd;
            }
        }
        tab.dir().map(Path::to_path_buf)
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
            &profile,
            cwd,
            crate::terminal::session::term_config(self.config.scrollback, &self.config.cursor),
            size,
            self.fonts.cell_px(),
            Listener::new(id, self.tx.clone(), self.ctx.clone()),
        ) {
            Ok(s) => s,
            Err(err) => {
                crate::diag::warn(format!("failed to start {}: {err}", profile.command));
                return None;
            }
        };
        self.tabs
            .insert(id, Tab::new(Content::Term(session), profile));
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
                cwd: tab
                    .session()
                    .and_then(|s| s.proc_info().cwd)
                    .or(tab.cwd.clone()),
                custom_title: tab.custom_title.clone(),
                index: self.ws.order.iter().position(|t| *t == id).unwrap_or(0),
                preview: tab.preview().map(|p| p.path.clone()),
            });
            if self.closed.len() > MAX_CLOSED {
                self.closed.remove(0);
            }
        }
        self.ws.close(id);
        self.tabs.remove(&id);
        if self
            .renaming
            .as_ref()
            .is_some_and(|(r, _)| *r == RenameTarget::Tab(id))
        {
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
        let id = match closed.preview {
            Some(path) => self.spawn_preview(path, None),
            None => self.spawn_tab(closed.profile, closed.cwd, None),
        };
        if let Some(id) = id {
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
        if let Some(p) = tab.preview() {
            let path = p.path.clone();
            self.spawn_preview(path, None);
            return;
        }
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
        self.renaming = Some((RenameTarget::Tab(id), current));
        self.scroll_to_focused = true;
        self.side_hidden = false;
        if self.sidebar_collapsed {
            self.sidebar_peek = true;
        }
    }

    /// Put tab `id` into a new folder and start naming it.
    pub fn new_group(&mut self, id: TabId) {
        let name = format!("Group {}", self.ws.groups.len() + 1);
        if let Some(g) = self.ws.new_group(id, name.clone()) {
            self.renaming = Some((RenameTarget::Group(g), name));
            self.side_hidden = false;
            if self.sidebar_collapsed {
                self.sidebar_peek = true;
            }
        }
    }

    /// Close every tab in a folder.
    pub fn close_group(&mut self, group: GroupId) {
        for id in self.ws.members(group) {
            self.close_tab(id);
        }
    }

    /// Show `path` in a preview pane next to the focused terminal. A preview already in the
    /// active view is reused, so clicking through files doesn't pile up panes. Keyboard focus
    /// stays where it was.
    pub fn open_preview(&mut self, path: PathBuf) {
        let visible = self.ws.visible();
        let existing = visible
            .iter()
            .copied()
            .find(|id| self.tabs.get(id).is_some_and(|t| t.preview().is_some()));
        if let Some(id) = existing {
            self.replace_preview(id, path);
            return;
        }
        let focused = self.ws.focused();
        let edge = self.preview_edge();
        if let Some(id) = self.spawn_preview(path, focused.map(|f| (f, edge)))
            && let Some(f) = focused
        {
            // The new pane is on screen; keep typing into the terminal.
            if self.ws.view_of(id) == self.ws.view_of(f) {
                self.ws.activate(f);
            }
        }
    }

    /// Show `path` in preview tab `id` instead of the file it shows now.
    fn replace_preview(&mut self, id: TabId, path: PathBuf) {
        let preview = Preview::open(path, &self.highlighter());
        if let Some(tab) = self.tabs.get_mut(&id)
            && tab.preview().is_some()
        {
            tab.profile = preview_profile(&preview);
            tab.custom_title = None;
            tab.content = Content::Preview(Box::new(preview));
        }
    }

    /// A file or folder from the files panel dropped on pane `target`. On an edge it opens in a
    /// new split there: a preview for a file, a terminal for a folder. In the center it types
    /// the path into a terminal, or replaces what a preview shows.
    pub fn drop_path(&mut self, target: TabId, path: PathBuf, drop: Drop) {
        let target_is_preview = self
            .tabs
            .get(&target)
            .is_some_and(|t| t.preview().is_some());
        match drop {
            Drop::Edge(edge) if path.is_dir() => {
                self.activate(target);
                if let Some(profile) = self.profiles.first().cloned() {
                    self.spawn_tab(profile, Some(path), Some(edge));
                }
            }
            Drop::Edge(edge) => {
                let focused = self.ws.focused();
                if let Some(id) = self.spawn_preview(path, Some((target, edge))) {
                    // Like clicking a file: the new pane shows up, focus stays put.
                    match focused.filter(|f| self.ws.view_of(*f) == self.ws.view_of(id)) {
                        Some(f) => self.ws.activate(f),
                        None => self.ws.activate(id),
                    }
                }
            }
            Drop::Center if target_is_preview => {
                if !path.is_dir() {
                    self.replace_preview(target, path);
                }
            }
            Drop::Center => {
                self.activate(target);
                self.insert_path(&path);
            }
        }
        self.scroll_to_focused = true;
    }

    /// Open `path` as a tab of its own: a preview, or a terminal for a folder.
    pub fn open_path_tab(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.new_tab_in(path);
        } else {
            self.spawn_preview(path, None);
        }
    }

    /// Split right, or down when the focused pane is too narrow for two columns.
    fn preview_edge(&self) -> Edge {
        let focused = self.ws.focused();
        match self.pane_rects.iter().find(|(t, _)| Some(*t) == focused) {
            Some((_, r)) if r.width() < 700.0 && r.height() > r.width() * 0.8 => Edge::Bottom,
            _ => Edge::Right,
        }
    }

    /// Open a preview tab, standalone or split next to `split.0` on edge `split.1`.
    fn spawn_preview(&mut self, path: PathBuf, split: Option<(TabId, Edge)>) -> Option<TabId> {
        let preview = Preview::open(path, &self.highlighter());
        let id = self.next_id;
        self.next_id += 1;
        let after = split.map(|(t, _)| t).or(self.ws.focused());
        let profile = preview_profile(&preview);
        self.tabs
            .insert(id, Tab::new(Content::Preview(Box::new(preview)), profile));
        self.ws.add(id, after);
        if let Some((target, edge)) = split {
            self.ws.drop_on(id, target, Drop::Edge(edge));
        }
        self.scroll_to_focused = true;
        Some(id)
    }

    pub fn highlighter(&mut self) -> Arc<Highlighter> {
        self.highlighter
            .get_or_insert_with(|| Arc::new(Highlighter::new()))
            .clone()
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
        let mut woke = Vec::new();
        let bell_mode = self.config.bell;
        while let Ok((id, event)) = self.rx.try_recv() {
            let Some(tab) = self.tabs.get_mut(&id) else {
                continue;
            };
            match event {
                TermEvent::Wakeup => {
                    if !visible.contains(&id) {
                        tab.activity = true;
                    }
                    if !woke.contains(&id) {
                        woke.push(id);
                    }
                }
                TermEvent::Bell => {
                    if bell_mode != BellMode::None {
                        if focused != Some(id) {
                            tab.bell = true;
                        }
                        // A visual bell shows even in the focused pane.
                        if bell_mode == BellMode::Flash && visible.contains(&id) {
                            tab.bell_flash = Some(Instant::now());
                        }
                    }
                }
                TermEvent::Title(t) => tab.osc_title = Some(t),
                TermEvent::ResetTitle => tab.osc_title = None,
                _ if tab.session().is_none() => {}
                TermEvent::PtyWrite(s) => tab.session().unwrap().write(s.into_bytes()),
                TermEvent::ClipboardStore(_, text) => {
                    if let Some(cb) = &mut self.clipboard {
                        let _ = cb.set_text(text);
                    }
                }
                TermEvent::ClipboardLoad(_, format) => {
                    if let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) {
                        tab.session().unwrap().write(format(&text).into_bytes());
                    }
                }
                TermEvent::ColorRequest(index, format) => {
                    let color = match index {
                        256 => Some(self.palette.foreground()),
                        257 => Some(self.palette.background()),
                        _ => None,
                    };
                    let session = tab.session().unwrap();
                    let from_term = session.term.lock().colors()[index];
                    let rgb = from_term.or(color.map(|c| Rgb {
                        r: c.r(),
                        g: c.g(),
                        b: c.b(),
                    }));
                    if let Some(rgb) = rgb {
                        session.write(format(rgb).into_bytes());
                    }
                }
                TermEvent::TextAreaSizeRequest(format) => {
                    // Report the cell size of this pane's own zoom.
                    let size_pt = tab.font_size(self.font_size);
                    self.fonts
                        .update(&self.ctx, size_pt, self.ctx.pixels_per_point());
                    let (cw, ch) = self.fonts.cell_px();
                    let session = tab.session().unwrap();
                    let size = session.size;
                    let ws = WindowSize {
                        num_lines: size.lines as u16,
                        num_cols: size.cols as u16,
                        cell_width: cw,
                        cell_height: ch,
                    };
                    session.write(format(ws).into_bytes());
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
        // A `cd` is followed by a new prompt, so the focused shell printing is the moment to
        // check whether its cwd moved. Throttled, since output can arrive every frame.
        if self.files_visible()
            && focused.is_some_and(|f| woke.contains(&f))
            && self.last_cwd_check.elapsed() >= Duration::from_millis(150)
        {
            self.last_cwd_check = Instant::now();
            self.follow_cwd();
            // A command just finished (shell back at its prompt): refresh git right away.
            if let Some(session) = focused
                .and_then(|f| self.tabs.get(&f))
                .and_then(Tab::session)
            {
                let idle = session.shell_idle();
                if idle && !self.shell_was_idle {
                    self.git.tick(&self.ctx, true);
                }
                self.shell_was_idle = idle;
            }
        }
        if self.config.adopt_shell_palette {
            self.adopt_shell_colors(&woke);
        }
    }

    /// With `adopt_shell_palette`: if a shell set colors via OSC (e.g. a pywal/matugen script
    /// writing escape sequences to every terminal), make them the app-wide theme. The tab's own
    /// overrides are then cleared, so every tab and the chrome render from one palette.
    fn adopt_shell_colors(&mut self, woke: &[TabId]) {
        let mut adopted = false;
        for id in woke {
            let Some(session) = self.tabs.get(id).and_then(Tab::session) else {
                continue;
            };
            let patch = theme::patch_from_osc(session.term.lock().colors());
            if patch.is_empty() {
                continue;
            }
            self.shell_patch.merge(patch);
            adopted = true;
        }
        if adopted {
            let mut theme = self.base_theme.clone();
            theme.apply(&self.shell_patch);
            self.set_theme(&theme);
            self.reset_tab_colors();
        }
    }

    fn set_theme(&mut self, theme: &Theme) {
        self.palette = Palette::from_theme(theme);
        self.chrome = UiColors::from_theme(theme);
        self.syntax_theme_xml = crate::preview::highlight::tm_theme(&self.palette);
        if let Some(t) = crate::preview::highlight::load_theme(&self.syntax_theme_xml) {
            self.syntax_theme = Arc::new(t);
        }
        self.theme_generation += 1;
        apply_style(&self.ctx, &self.chrome);
        self.ctx.request_repaint();
    }

    /// Clear per-tab OSC color overrides so tabs render from the app theme.
    fn reset_tab_colors(&self) {
        use alacritty_terminal::vte::ansi::Handler;
        for session in self.tabs.values().filter_map(Tab::session) {
            let mut term = session.term.lock();
            for i in 0..alacritty_terminal::term::color::COUNT {
                term.reset_color(i);
            }
        }
    }

    /// Set the whole-UI zoom (clamped, rounded to 2 decimals) and apply it to egui.
    fn set_ui_zoom(&mut self, zoom: f32) {
        self.ui_zoom = ((zoom * 100.0).round() / 100.0).clamp(0.5, 3.0);
        self.ctx.set_zoom_factor(self.ui_zoom);
    }

    /// Re-read the config (and theme) after a file changed. A broken config is reported and
    /// ignored, so a half-saved edit doesn't wipe your settings.
    pub(crate) fn reload_config(&mut self) {
        // Everything gets re-evaluated; problems that still exist are reported again.
        self.problems.clear();
        self.problems_dismissed = false;
        let config = match Config::try_load() {
            Ok(c) => c,
            Err(err) => {
                crate::diag::warn(format!("{err} (keeping previous config)"));
                return;
            }
        };
        let old = std::mem::replace(&mut self.config, config);
        let new = &self.config;

        self.keybinds = Keybinds::new(&new.keybindings);
        self.profiles = profiles::load(new);
        if new.font.family != old.font.family {
            self.fonts = Fonts::new(
                &self.ctx,
                new.font.family.as_deref(),
                self.font_size,
                self.ctx.pixels_per_point(),
            );
        }
        if new.font.family != old.font.family
            || new.font.line_height != old.font.line_height
            || new.font.letter_spacing != old.font.letter_spacing
        {
            self.fonts
                .set_spacing(&self.ctx, new.font.line_height, new.font.letter_spacing);
        }
        if new.reduce_motion != old.reduce_motion {
            apply_motion(&self.ctx, new.reduce_motion);
        }
        if new.cursor != old.cursor || new.scrollback != old.scrollback {
            let options = crate::terminal::session::term_config(new.scrollback, &new.cursor);
            for session in self.tabs.values().filter_map(Tab::session) {
                session.term.lock().set_options(options.clone());
            }
        }
        if new.ui_scale != old.ui_scale {
            self.ui_zoom = new.ui_scale.clamp(0.5, 3.0);
            self.ctx.set_zoom_factor(self.ui_zoom);
        }
        if new.font.size != old.font.size {
            self.font_size = new.font.size;
        }
        if new.sidebar_collapsed != old.sidebar_collapsed {
            self.sidebar_collapsed = new.sidebar_collapsed;
        }
        if new.files.open != old.files.open {
            self.files_open = new.files.open;
        }
        if new.files.wrap != old.files.wrap {
            self.preview_wrap = new.files.wrap;
        }
        if new.files.show_hidden != old.files.show_hidden {
            self.files.show_hidden = new.files.show_hidden;
        }

        let resolved = theme::resolve(&self.config);
        self.watcher.set_paths(watch_paths(resolved.file));
        // An explicit theme change wins over any previously adopted shell palette.
        self.shell_patch = Patch::default();
        if resolved.theme != self.base_theme {
            self.base_theme = resolved.theme;
            let theme = self.base_theme.clone();
            self.set_theme(&theme);
            self.reset_tab_colors();
        }
    }

    /// Create the config if needed and open it: in a new tab when an editor is configured,
    /// otherwise with the system's default app. Saving applies changes live.
    pub fn open_settings(&mut self) {
        let path = match crate::config::edit::ensure_config() {
            Ok(p) => p,
            Err(err) => return crate::diag::warn(err),
        };
        let Some(mut cmd) = crate::config::edit::editor(&self.config) else {
            if let Err(err) = crate::platform::open_in_text_editor(&path) {
                crate::diag::warn(err);
            }
            return;
        };
        let program = cmd.remove(0);
        cmd.push(path.to_string_lossy().into_owned());
        let profile = Profile {
            name: "settings".into(),
            command: program,
            args: cmd,
            cwd: None,
            env: HashMap::new(),
            icon: "⚙".into(),
            color: None,
        };
        let cwd = path.parent().map(PathBuf::from);
        if let Some(id) = self.spawn_tab(profile, cwd, None)
            && let Some(tab) = self.tabs.get_mut(&id)
        {
            tab.custom_title = Some("Settings".into());
        }
    }

    /// Pull newly reported problems into the banner (deduplicated).
    fn collect_problems(&mut self) {
        for p in crate::diag::take() {
            if !self.problems.contains(&p) {
                self.problems.push(p);
                self.problems_dismissed = false;
            }
        }
    }

    /// Background a tab actually renders with (an OSC 11 override, else the theme's).
    pub fn tab_background(&self, id: TabId) -> Color32 {
        use alacritty_terminal::vte::ansi::NamedColor;
        self.tabs
            .get(&id)
            .and_then(Tab::session)
            .and_then(|s| s.term.lock().colors()[NamedColor::Background])
            .map(|c| Color32::from_rgb(c.r, c.g, c.b))
            .unwrap_or(self.palette.background())
    }

    /// About once a second while they're on screen: refresh auto titles, re-list changed
    /// folders in the files panel, and reload previews whose file changed.
    fn poll(&mut self) {
        let sidebar_visible = if self.side_hidden {
            self.zen_peek && !self.zen_files
        } else {
            !self.sidebar_collapsed || self.sidebar_peek
        };
        let files_visible = self.files_visible();
        let visible = self.ws.visible();
        let previews_visible = visible
            .iter()
            .any(|id| self.tabs.get(id).is_some_and(|t| t.preview().is_some()));
        if !sidebar_visible && !files_visible && !previews_visible {
            return;
        }
        if self.last_poll.elapsed() >= Duration::from_secs(1) {
            self.last_poll = Instant::now();
            if sidebar_visible {
                for tab in self.tabs.values_mut() {
                    let Some(session) = tab.session() else {
                        continue;
                    };
                    let info = session.proc_info();
                    tab.auto_title = procinfo::format_title(&info);
                    tab.process = info.process;
                    if info.cwd.is_some() {
                        tab.cwd = info.cwd;
                    }
                }
            }
            if files_visible {
                self.follow_cwd();
                self.files.refresh();
                self.git.tick(&self.ctx, false);
            }
            if previews_visible {
                let hl = self.highlighter();
                for id in &visible {
                    if let Some(p) = self.tabs.get_mut(id).and_then(Tab::preview_mut)
                        && p.reload_if_changed(&hl)
                        && matches!(p.body, crate::preview::Body::Image)
                    {
                        self.ctx.forget_image(&crate::ui::file_uri(&p.path));
                    }
                }
            }
        }
        self.ctx.request_repaint_after(Duration::from_secs(1));
    }

    /// The files tree is on screen: as the side panel, or as the zen overlay.
    fn files_visible(&self) -> bool {
        if self.side_hidden {
            self.zen_peek && self.zen_files
        } else {
            self.files_open
        }
    }

    /// Zen mode: show the overlay with the files panel (`files`) or the tabs, right away and
    /// until the pointer has visited it and left again.
    pub(crate) fn zen_show(&mut self, files: bool) {
        self.zen_files = files;
        self.zen_peek = true;
        self.zen_hovered_once = false;
        if files {
            self.follow_cwd();
        }
    }

    /// Ctrl+\ in zen mode: switch the overlay between tabs and files.
    pub(crate) fn zen_toggle_files(&mut self) {
        self.zen_show(!(self.zen_peek && self.zen_files));
    }

    /// Point the files panel at the focused shell's cwd when it changes (after `cd`).
    /// Previews don't move the tree, so clicking through files keeps your place.
    fn follow_cwd(&mut self) {
        let Some(id) = self.ws.focused() else { return };
        let Some(tab) = self.tabs.get_mut(&id) else {
            return;
        };
        let Some(session) = tab.session() else {
            return;
        };
        if let Some(cwd) = session.proc_info().cwd {
            tab.cwd = Some(cwd);
        }
        let Some(cwd) = tab.cwd.clone() else { return };
        if self.files_followed.as_ref() != Some(&cwd) || self.files.root().is_none() {
            self.files.set_root(cwd.clone());
            self.files_followed = Some(cwd);
        }
    }

    /// Browse the files panel somewhere else (until the shell's cwd changes again).
    pub fn browse_files(&mut self, dir: PathBuf) {
        self.files.set_root(dir);
    }

    /// The terminal that file actions type into: the focused pane, or else another terminal
    /// pane in the active view (when a preview has focus).
    fn target_terminal(&self) -> Option<TabId> {
        let focused = self.ws.focused()?;
        std::iter::once(focused)
            .chain(self.ws.visible())
            .find(|id| self.tabs.get(id).is_some_and(|t| t.session().is_some()))
    }

    /// `cd` the focused shell into `dir`. If a program is running in it, open a new tab there
    /// instead of typing into that program.
    pub fn cd_focused(&mut self, dir: &Path) {
        let idle = self.target_terminal().and_then(|id| {
            let session = self.tabs.get(&id)?.session()?;
            session.shell_idle().then(|| (id, session.proc_info().cwd))
        });
        match idle {
            Some((id, cwd)) => {
                self.activate(id);
                // Below the shell's cwd, type the short relative form (`cd src/ui`).
                let target = cwd
                    .and_then(|cwd| dir.strip_prefix(cwd).ok().map(Path::to_path_buf))
                    .filter(|rel| !rel.as_os_str().is_empty())
                    .unwrap_or_else(|| dir.to_path_buf());
                let line = format!("cd {}\r", crate::terminal::shell_quote(&target));
                self.send_input(line.into_bytes());
            }
            None => self.new_tab_in(dir.to_path_buf()),
        }
    }

    pub fn new_tab_in(&mut self, dir: PathBuf) {
        if let Some(profile) = self.profiles.first().cloned() {
            self.spawn_tab(profile, Some(dir), None);
        }
    }

    /// Type a (quoted) path into the terminal, as if pasted.
    pub fn insert_path(&mut self, path: &Path) {
        if let Some(id) = self.target_terminal() {
            self.activate(id);
            self.paste(&format!("{} ", crate::terminal::shell_quote(path)));
        }
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
            self.blink_epoch = Instant::now();
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
                self.side_hidden = false;
            }
            Action::ToggleSideArea => {
                self.side_hidden = !self.side_hidden;
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
                // With a single tab, let the key through to the shell.
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
            Action::ShowHelp => self.help = Some(crate::ui::Help::default()),
            Action::OpenSettings => self.open_settings(),
            Action::ToggleFiles => {
                if self.side_hidden {
                    self.zen_toggle_files();
                } else {
                    self.files_open = !self.files_open;
                }
            }
            Action::SearchFiles => {
                if self.side_hidden {
                    self.zen_show(true);
                } else {
                    self.files_open = true;
                }
                self.focus_search = true;
            }
            Action::NewGroup => {
                if let Some(f) = self.ws.focused() {
                    self.new_group(f);
                }
            }
            Action::FocusLeft | Action::FocusRight | Action::FocusUp | Action::FocusDown => {
                let dir = match action {
                    Action::FocusLeft => egui::vec2(-1.0, 0.0),
                    Action::FocusRight => egui::vec2(1.0, 0.0),
                    Action::FocusUp => egui::vec2(0.0, -1.0),
                    _ => egui::vec2(0.0, 1.0),
                };
                // With no pane in that direction, let the key through (shells may use it, e.g. Alt+Arrow for word movement).
                if !self.focus_neighbor(dir) {
                    return false;
                }
            }
            Action::ZoomIn | Action::ZoomOut | Action::ZoomReset => {
                let base = self.font_size;
                if let Some(tab) = self.ws.focused().and_then(|f| self.tabs.get_mut(&f)) {
                    tab.zoom = match action {
                        Action::ZoomIn => (tab.zoom + 1.0).min(MAX_FONT - base),
                        Action::ZoomOut => (tab.zoom - 1.0).max(MIN_FONT - base),
                        _ => 0.0,
                    };
                }
            }
            Action::UiZoomIn => self.set_ui_zoom(self.ui_zoom * 1.1),
            Action::UiZoomOut => self.set_ui_zoom(self.ui_zoom / 1.1),
            Action::UiZoomReset => self.set_ui_zoom(self.config.ui_scale),
            Action::ScrollPageUp | Action::ScrollPageDown => {
                if let Some(session) = self.focused_session() {
                    let scroll = if action == Action::ScrollPageUp {
                        Scroll::PageUp
                    } else {
                        Scroll::PageDown
                    };
                    session.term.lock().scroll_display(scroll);
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

    /// Font size in points of pane `id` (the base size plus its own zoom).
    pub fn pane_font_size(&self, id: TabId) -> f32 {
        self.tabs
            .get(&id)
            .map_or(self.font_size, |t| t.font_size(self.font_size))
    }

    fn focused_session(&self) -> Option<&Session> {
        self.ws
            .focused()
            .and_then(|f| self.tabs.get(&f))
            .and_then(Tab::session)
    }

    /// Write user input to the focused tab, snapping the view back to the bottom.
    fn send_input(&mut self, bytes: Vec<u8>) {
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

const UI_ZOOM_KEY: &str = "ui_zoom";

/// The runtime UI zoom (Ctrl+Shift+=/-), saved across restarts together with the
/// `ui_scale` it was based on, so editing `ui_scale` in the config still takes effect.
#[derive(serde::Serialize, serde::Deserialize)]
struct SavedUiZoom {
    zoom: f32,
    ui_scale: f32,
}

fn restored_ui_zoom(storage: Option<&dyn eframe::Storage>, ui_scale: f32) -> f32 {
    let saved = storage.and_then(|s| eframe::get_value::<SavedUiZoom>(s, UI_ZOOM_KEY));
    match saved {
        Some(saved) if saved.ui_scale == ui_scale => saved.zoom,
        _ => ui_scale,
    }
    .clamp(0.5, 3.0)
}

impl eframe::App for App {
    /// Only the window geometry and UI zoom are persisted;
    /// egui's memory would restore stale focus/scroll state.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(
            storage,
            UI_ZOOM_KEY,
            &SavedUiZoom {
                zoom: self.ui_zoom,
                ui_scale: self.config.ui_scale,
            },
        );
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Buttons must never keep keyboard focus: Space/Enter belong to the terminal.
        if self.renaming.is_none()
            && self.switcher.is_none()
            && self.help.is_none()
            && !self.search_focused
        {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        self.fonts
            .update(&ctx, self.font_size, ctx.pixels_per_point());
        let window_focused = ctx.input(|i| i.focused);
        if window_focused && !self.window_focused {
            self.blink_epoch = Instant::now();
        }
        self.window_focused = window_focused;
        if self.watcher.changed() {
            self.reload_config();
        }
        self.collect_problems();
        self.process_events();
        self.handle_keyboard();
        self.poll();

        if self.tabs.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if self.side_hidden {
            let area = ui.available_rect_before_wrap();
            self.zen_sidebar(&ctx, area);
        } else {
            self.zen_peek = false;
            self.sidebar(ui);
            if self.files_open {
                self.files_panel(ui);
            }
        }
        self.panes(ui);
        self.switcher_ui(&ctx);
        self.help_ui(&ctx);
        self.problems_banner(&ctx);
        self.track_focus();
        self.update_window_title();
    }
}

/// The tab profile for a preview: file name as title, file type as icon.
fn preview_profile(p: &Preview) -> Profile {
    Profile {
        name: p
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.path.to_string_lossy().into_owned()),
        command: String::new(),
        args: Vec::new(),
        cwd: p.path.parent().map(Path::to_path_buf),
        env: HashMap::new(),
        icon: p.icon().into(),
        color: None,
    }
}

fn apply_style(ctx: &egui::Context, c: &UiColors) {
    let mut visuals = if c.light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    visuals.panel_fill = c.sidebar;
    visuals.window_fill = c.raised(0.06);
    visuals.extreme_bg_color = c.recessed(0.25);
    visuals.widgets.noninteractive.bg_stroke.color = c.raised(0.15);
    visuals.widgets.noninteractive.fg_stroke.color = mix(c.fg, c.bg, 0.25);
    visuals.widgets.inactive.fg_stroke.color = mix(c.fg, c.bg, 0.1);
    visuals.override_text_color = None;
    visuals.selection.bg_fill = mix(c.accent, c.bg, 0.55);
    visuals.hyperlink_color = c.accent;
    ctx.set_visuals(visuals);
}

/// Animations off (instant transitions and scrolling) or back to egui's defaults.
fn apply_motion(ctx: &egui::Context, reduce: bool) {
    let default = egui::Style::default();
    ctx.global_style_mut(|s| {
        if reduce {
            s.animation_time = 0.0;
            s.scroll_animation = egui::style::ScrollAnimation::none();
        } else {
            s.animation_time = default.animation_time;
            s.scroll_animation = default.scroll_animation;
        }
    });
}

/// Files whose changes trigger a live reload: the config, plus the active theme file.
fn watch_paths(theme_file: Option<PathBuf>) -> Vec<PathBuf> {
    Config::path().into_iter().chain(theme_file).collect()
}
