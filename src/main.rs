//! vtt: a GPU-accelerated terminal with a vertical tab sidebar.
//!
//! Layout, roughly from the bottom up:
//!
//! - [`config`]: the config file, keybindings, settings editing and the file watcher.
//! - [`theme`]: resolving the color theme and deriving the UI's colors from it.
//! - [`terminal`]: a shell in a PTY driven by alacritty_terminal, input encoding, and
//!   inspecting the foreground process.
//! - [`workspace`]: pure tab/split/folder bookkeeping, with no rendering or sessions.
//! - [`files`] and [`preview`]: the files panel's tree, git status and search, and file
//!   previews with syntax highlighting.
//! - [`render`]: glyph atlas and per-pane meshes for terminal grids.
//! - [`app`]: the application state and its behavior; it owns everything above.
//! - [`ui`]: the egui chrome, drawn from `App` each frame.
//!
//! [`diag`], [`fuzzy`] and [`platform`] are small helpers shared across these.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod config;
mod diag;
mod files;
mod fuzzy;
mod platform;
mod preview;
mod render;
mod terminal;
mod theme;
mod ui;
mod workspace;

fn main() -> eframe::Result {
    let config = config::Config::load();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("vtt")
        .with_app_id("vtt")
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([400.0, 240.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "vtt",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, config)))),
    )
}

/// The app icon for the window and taskbar (a bundled app's own icon takes precedence).
fn window_icon() -> Option<eframe::egui::IconData> {
    let png = include_bytes!("../packaging/icons/vtt-256.png");
    let image = image::load_from_memory(png).ok()?.into_rgba8();
    Some(eframe::egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}
