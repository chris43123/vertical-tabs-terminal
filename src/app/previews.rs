//! File preview tabs: opening files next to the focused terminal and handling paths dropped
//! from the files panel.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui_commonmark::CommonMarkCache;

use crate::preview::Preview;
use crate::preview::highlight::{self, Highlighter};
use crate::render::Palette;
use crate::terminal::profiles::Profile;
use crate::workspace::{Drop, Edge, TabId};

use super::App;
use super::tab::{Content, Tab};

/// State shared by all preview panes: the highlighter, code colors generated from the
/// palette, the markdown viewer's cache, and soft wrap.
pub struct Previews {
    /// Loaded on first use: syntax definitions are a few MB.
    highlighter: Option<Arc<Highlighter>>,
    /// Code colors generated from the palette, and a counter bumped when they change.
    pub syntax_theme: Arc<syntect::highlighting::Theme>,
    syntax_theme_xml: String,
    pub theme_generation: u64,
    md_cache: Option<CommonMarkCache>,
    /// Theme generation the markdown cache's code theme was registered for.
    md_theme_generation: u64,
    /// Soft-wrap long lines in text and code previews.
    pub wrap: bool,
}

impl Previews {
    pub fn new(palette: &Palette, wrap: bool) -> Self {
        let mut previews = Self {
            highlighter: None,
            syntax_theme: Arc::default(),
            syntax_theme_xml: String::new(),
            theme_generation: 0,
            md_cache: None,
            md_theme_generation: 0,
            wrap,
        };
        previews.set_palette(palette);
        previews
    }

    pub fn highlighter(&mut self) -> Arc<Highlighter> {
        self.highlighter
            .get_or_insert_with(|| Arc::new(Highlighter::new()))
            .clone()
    }

    /// Regenerate code colors after the terminal palette changed.
    pub fn set_palette(&mut self, palette: &Palette) {
        self.syntax_theme_xml = highlight::tm_theme(palette);
        if let Some(t) = highlight::load_theme(&self.syntax_theme_xml) {
            self.syntax_theme = Arc::new(t);
        }
        self.theme_generation += 1;
    }

    /// The markdown viewer's cache, with the current code colors registered.
    pub fn markdown_cache(&mut self) -> &mut CommonMarkCache {
        let cache = self.md_cache.get_or_insert_with(Default::default);
        if self.md_theme_generation != self.theme_generation {
            let _ = cache.add_syntax_theme_from_bytes(
                highlight::THEME_NAME,
                self.syntax_theme_xml.as_bytes(),
            );
            self.md_theme_generation = self.theme_generation;
        }
        cache
    }
}

impl App {
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

    /// Open `path` as a tab of its own: a preview, or a terminal for a folder.
    pub fn open_path_tab(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.new_tab_in(path);
        } else {
            self.spawn_preview(path, None);
        }
    }

    /// Show `path` in preview tab `id` instead of the file it shows now.
    fn replace_preview(&mut self, id: TabId, path: PathBuf) {
        let preview = Preview::open(path, &self.previews.highlighter());
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

    /// Split right, or down when the focused pane is too narrow for two columns.
    fn preview_edge(&self) -> Edge {
        let focused = self.ws.focused();
        match self.pane_rects.iter().find(|(t, _)| Some(*t) == focused) {
            Some((_, r)) if r.width() < 700.0 && r.height() > r.width() * 0.8 => Edge::Bottom,
            _ => Edge::Right,
        }
    }

    /// Open a preview tab, standalone or split next to `split.0` on edge `split.1`.
    pub(super) fn spawn_preview(
        &mut self,
        path: PathBuf,
        split: Option<(TabId, Edge)>,
    ) -> Option<TabId> {
        let preview = Preview::open(path, &self.previews.highlighter());
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
