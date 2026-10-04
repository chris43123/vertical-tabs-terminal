//! Theme, fonts, zoom and the live config reload.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Color32};

use crate::config::Config;
use crate::config::keybinds::Keybinds;
use crate::render::{Fonts, Palette};
use crate::terminal::profiles::{self, Profile};
use crate::theme::{self, Patch, Theme, UiColors, mix};
use crate::workspace::TabId;

use super::App;
use super::tab::Tab;

impl App {
    /// With `adopt_shell_palette`: if a shell set colors via OSC (e.g. a pywal/matugen script
    /// writing escape sequences to every terminal), make them the app-wide theme. The tab's own
    /// overrides are then cleared, so every tab and the chrome render from one palette.
    pub(super) fn adopt_shell_colors(&mut self, woke: &[TabId]) {
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
    pub(super) fn set_ui_zoom(&mut self, zoom: f32) {
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
}

pub(super) fn apply_style(ctx: &egui::Context, c: &UiColors) {
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
pub(super) fn apply_motion(ctx: &egui::Context, reduce: bool) {
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
pub(super) fn watch_paths(theme_file: Option<PathBuf>) -> Vec<PathBuf> {
    Config::path().into_iter().chain(theme_file).collect()
}
