//! Application state: tabs, their sessions, the split workspace and event routing.
//!
//! [`App`] is split by concern across the submodules; this file holds the state itself, its
//! construction and the per-frame loop.

mod actions;
mod appearance;
mod events;
mod files;
mod input;
mod previews;
mod tab;
mod tabs;

pub use tab::{Content, RenameTarget, Tab, TabDrag};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use alacritty_terminal::event::Event as TermEvent;
use eframe::egui;

use crate::config::Config;
use crate::config::keybinds::Keybinds;
use crate::config::watch::Watcher;
use crate::files::FileTree;
use crate::preview::highlight::Highlighter;
use crate::render::{Fonts, Palette};
use crate::terminal::profiles::{self, Profile};
use crate::theme::{self, Patch, Theme, UiColors};
use crate::workspace::{TabId, Workspace};

use appearance::{apply_motion, apply_style, watch_paths};
use tab::ClosedTab;

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

    /// Pull newly reported problems into the banner (deduplicated).
    fn collect_problems(&mut self) {
        for p in crate::diag::take() {
            if !self.problems.contains(&p) {
                self.problems.push(p);
                self.problems_dismissed = false;
            }
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

    /// Font size in points of pane `id` (the base size plus its own zoom).
    pub fn pane_font_size(&self, id: TabId) -> f32 {
        self.tabs
            .get(&id)
            .map_or(self.font_size, |t| t.font_size(self.font_size))
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
