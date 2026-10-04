//! A tab: what it shows (a terminal or a file preview), its titles and its unread markers.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::preview::Preview;
use crate::terminal::profiles::Profile;
use crate::terminal::session::Session;
use crate::workspace::{GroupId, TabId};

/// What a tab shows: a shell, or a read-only file preview.
pub enum Content {
    Term(Session),
    Preview(Box<Preview>),
}

/// Bounds of a pane's font size in points.
pub(super) const MIN_FONT: f32 = 6.0;
pub(super) const MAX_FONT: f32 = 48.0;

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

    pub(super) fn new(content: Content, profile: Profile) -> Self {
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
pub(super) struct ClosedTab {
    pub profile: Profile,
    pub cwd: Option<PathBuf>,
    pub custom_title: Option<String>,
    pub index: usize,
    /// Set for a closed preview: the file it showed.
    pub preview: Option<PathBuf>,
}

/// What the sidebar's inline rename box is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameTarget {
    Tab(TabId),
    Group(GroupId),
}

pub(super) const MAX_CLOSED: usize = 20;

/// Payload for dragging a tab out of the sidebar.
#[derive(Clone, Copy, Debug)]
pub struct TabDrag(pub TabId);
