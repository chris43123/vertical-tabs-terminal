#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod config;
mod diag;
mod files;
mod fuzzy;
mod git;
mod highlight;
mod icons;
mod input;
mod keybinds;
mod layout;
mod markdown;
mod preview;
mod procinfo;
mod profiles;
mod pty;
mod render;
mod search;
mod session;
mod settings;
mod theme;
mod ui;
mod watch;

fn main() -> eframe::Result {
    let config = config::Config::load();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("vtt")
            .with_app_id("vtt")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([400.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "vtt",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, config)))),
    )
}
