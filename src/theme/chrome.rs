//! Colors for the app chrome (sidebar, headers, palette), derived from the terminal theme.

use eframe::egui::Color32;

use super::{Rgb, Theme};

/// Colors for the app chrome, derived from the theme so every theme (light or dark) works.
#[derive(Clone, Debug)]
pub struct UiColors {
    pub bg: Color32,
    pub fg: Color32,
    pub accent: Color32,
    pub bell: Color32,
    pub danger: Color32,
    /// Secondary accents (palette labels).
    pub purple: Color32,
    pub green: Color32,
    pub sidebar: Color32,
    /// Background of selected text in previews (the theme's selection color).
    pub selection: Color32,
    pub light: bool,
}

fn c32([r, g, b]: Rgb) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Linear blend from `a` to `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// The theme's selection color, unless it is too close to the background to see (some
/// themes set it equal); then a blend toward the foreground. Text on it is kept readable
/// separately, against `min_contrast`.
fn selection_color(selection: Color32, bg: Color32, fg: Color32) -> Color32 {
    let distance = |a: Color32, b: Color32| {
        let d = |x: u8, y: u8| (x as i32 - y as i32).abs();
        d(a.r(), b.r()) + d(a.g(), b.g()) + d(a.b(), b.b())
    };
    if distance(selection, bg) < 24 {
        mix(bg, fg, 0.3)
    } else {
        selection
    }
}

impl UiColors {
    pub fn from_theme(t: &Theme) -> Self {
        let bg = c32(t.background);
        let light = t.is_light();
        Self {
            bg,
            fg: c32(t.foreground),
            accent: c32(t.accent.unwrap_or(t.colors[4])),
            bell: c32(t.colors[3]),
            danger: c32(t.colors[1]),
            purple: c32(t.colors[5]),
            green: c32(t.colors[2]),
            sidebar: t
                .sidebar
                .map(c32)
                .unwrap_or_else(|| mix(bg, Color32::BLACK, if light { 0.06 } else { 0.18 })),
            selection: selection_color(c32(t.selection), bg, c32(t.foreground)),
            light,
        }
    }

    /// A surface raised above the terminal background by `t` (0 = background, 1 = foreground).
    pub fn raised(&self, t: f32) -> Color32 {
        mix(self.bg, self.fg, t)
    }

    /// A surface raised above the sidebar by `t`.
    pub fn raised_sidebar(&self, t: f32) -> Color32 {
        mix(self.sidebar, self.fg, t)
    }

    /// Something receding below the background (gaps between panes).
    pub fn recessed(&self, t: f32) -> Color32 {
        mix(self.bg, Color32::BLACK, t)
    }

    /// Black or white-ish text, whichever reads better on `bg`.
    pub fn readable_on(&self, bg: Color32) -> Color32 {
        let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
        if lum > 150.0 {
            mix(Color32::BLACK, self.bg, 0.1)
        } else {
            Color32::WHITE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_comes_from_the_theme_and_stays_visible() {
        let mut t = Theme {
            selection: [0x40, 0x50, 0x90],
            ..Theme::default()
        };
        assert_eq!(
            UiColors::from_theme(&t).selection,
            Color32::from_rgb(0x40, 0x50, 0x90)
        );
        t.selection = t.background;
        let c = UiColors::from_theme(&t);
        assert_ne!(c.selection, c.bg);
    }
}
