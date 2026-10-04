//! egui chrome: the vertical tab sidebar, the files panel, the split pane area (terminals and
//! file previews), the tab switcher and the keyboard shortcuts window.

mod banner;
mod files;
mod help;
pub mod icons;
mod panes;
mod preview;
mod sidebar;
mod switcher;

use std::path::Path;

pub use files::FileDrag;
pub use help::Help;
pub use switcher::Switcher;

/// A path for display: `$HOME` shown as `~`.
pub fn display_path(path: &Path) -> String {
    match dirs::home_dir().and_then(|h| path.strip_prefix(&h).ok().map(Path::to_path_buf)) {
        Some(rel) if rel.as_os_str().is_empty() => "~".into(),
        Some(rel) => format!("~{}{}", std::path::MAIN_SEPARATOR, rel.display()),
        None => path.display().to_string(),
    }
}

/// The URI egui's image loaders use for a local file.
pub fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}
