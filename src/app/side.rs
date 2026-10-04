//! Which side areas are on screen: the tab sidebar (expanded, collapsed to icons, or peeking
//! open on hover), the files panel, or neither (zen mode, where touching the left window edge
//! shows one of them as an overlay).

pub struct SidePanels {
    /// The sidebar shows only an icon strip.
    pub collapsed: bool,
    /// Collapsed sidebar temporarily shown expanded because the pointer hovers it.
    pub peek: bool,
    /// Sidebar and files panel both hidden (zen mode).
    pub hidden: bool,
    /// Zen mode: the overlay is open.
    pub zen_peek: bool,
    /// What the zen overlay shows: the files panel (true) or the tabs.
    pub zen_files: bool,
    /// The pointer has entered the zen overlay since it opened (a keyboard-opened overlay
    /// stays up until then).
    pub zen_hovered_once: bool,
    /// The files panel between the sidebar and the panes.
    pub files_open: bool,
}

impl SidePanels {
    pub fn new(collapsed: bool, files_open: bool) -> Self {
        Self {
            collapsed,
            peek: false,
            hidden: false,
            zen_peek: false,
            zen_files: false,
            zen_hovered_once: false,
            files_open,
        }
    }

    /// The tab list is on screen: as the sidebar, or as the zen overlay.
    pub fn sidebar_visible(&self) -> bool {
        if self.hidden {
            self.zen_peek && !self.zen_files
        } else {
            !self.collapsed || self.peek
        }
    }

    /// The files tree is on screen: as the side panel, or as the zen overlay.
    pub fn files_visible(&self) -> bool {
        if self.hidden {
            self.zen_peek && self.zen_files
        } else {
            self.files_open
        }
    }

    /// Bring the full sidebar on screen (peeking it open if collapsed), e.g. to rename a tab.
    pub fn reveal_sidebar(&mut self) {
        self.hidden = false;
        if self.collapsed {
            self.peek = true;
        }
    }
}
