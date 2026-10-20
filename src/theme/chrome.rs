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
    /// Selected text in previews (the theme's selection colors).
    pub selection: SelectionColors,
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

/// Contrast at which text counts as readable (WCAG AA for normal text).
const READABLE: f32 = 4.5;

/// WCAG relative luminance (sRGB linearized).
fn wcag_luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio (1..=21) between two colors.
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (wcag_luminance(a), wcag_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Move `fg` toward white or black until it reaches `ratio` against `bg`.
/// Returns `fg` unchanged when it already qualifies or `ratio <= 1`.
pub fn ensure_contrast(fg: Color32, bg: Color32, ratio: f32) -> Color32 {
    let ratio = ratio.clamp(1.0, 21.0);
    if ratio <= 1.0 || contrast_ratio(fg, bg) >= ratio {
        return fg;
    }
    let mix = |t: f32, to: u8| {
        let f = |v: u8| (v as f32 + (to as f32 - v as f32) * t).round() as u8;
        Color32::from_rgb(f(fg.r()), f(fg.g()), f(fg.b()))
    };
    // Prefer the direction away from the background; fall back to the other
    // one if it cannot reach the target.
    let first = if wcag_luminance(bg) > 0.179 { 0 } else { 255 };
    let reach = |to: u8| contrast_ratio(mix(1.0, to), bg);
    let to = if reach(first) >= ratio || reach(first) >= reach(255 - first) {
        first
    } else {
        255 - first
    };
    if reach(to) < ratio {
        return mix(1.0, to);
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..8 {
        let m = (lo + hi) / 2.0;
        if contrast_ratio(mix(m, to), bg) >= ratio {
            hi = m;
        } else {
            lo = m;
        }
    }
    mix(hi, to)
}

/// Selected text: the selection background, and how text on it stays readable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionColors {
    pub bg: Color32,
    /// The theme's `selection_foreground` when it reads well on `bg`, else whichever of the
    /// theme's foreground and background (or black and white) reads best.
    fg: Color32,
    /// The theme chose `fg` itself: all selected text uses it, as in kitty.
    forced: bool,
}

impl SelectionColors {
    pub fn from_theme(t: &Theme) -> Self {
        let (fg, theme_bg) = (c32(t.foreground), c32(t.background));
        let bg = selection_color(c32(t.selection), theme_bg, fg);
        if let Some(sel_fg) = t.selection_foreground.map(c32)
            && contrast_ratio(sel_fg, bg) >= 3.0
        {
            return Self {
                bg,
                fg: sel_fg,
                forced: true,
            };
        }
        let best = |a: Color32, b: Color32| {
            if contrast_ratio(a, bg) >= contrast_ratio(b, bg) {
                a
            } else {
                b
            }
        };
        let mut text = best(fg, theme_bg);
        if contrast_ratio(text, bg) < READABLE {
            text = best(Color32::BLACK, Color32::WHITE);
        }
        Self {
            bg,
            fg: text,
            forced: false,
        }
    }

    /// The color to draw text of `color` in while it is selected: its own color if that is
    /// readable on the selection (so syntax colors survive), otherwise the selection's text
    /// color. A theme's `selection_foreground` always wins.
    pub fn text(&self, color: Color32) -> Color32 {
        if !self.forced && contrast_ratio(color, self.bg) >= READABLE {
            color
        } else {
            self.fg
        }
    }
}

/// The theme's selection color, unless it is too close to the background to see (some
/// themes set it equal); then a blend toward the foreground.
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
            selection: SelectionColors::from_theme(t),
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
            UiColors::from_theme(&t).selection.bg,
            Color32::from_rgb(0x40, 0x50, 0x90)
        );
        t.selection = t.background;
        let c = UiColors::from_theme(&t);
        assert_ne!(c.selection.bg, c.bg);
    }

    /// A light selection under light text, like a matugen theme's `secondary_container`.
    fn light_selection() -> Theme {
        Theme {
            foreground: [0xef, 0xd2, 0xc5],
            background: [0x23, 0x18, 0x16],
            selection: [0xff, 0xda, 0xd5],
            ..Theme::default()
        }
    }

    #[test]
    fn selected_text_is_readable_without_a_theme_color() {
        let t = light_selection();
        let s = SelectionColors::from_theme(&t);
        let fg = c32(t.foreground);
        // Light text on the light selection swaps to the dark background color.
        assert!(contrast_ratio(fg, s.bg) < 2.0);
        assert_eq!(s.text(fg), c32(t.background));
        assert!(contrast_ratio(s.text(Color32::from_rgb(0xff, 0xbf, 0x87)), s.bg) >= READABLE);
        // Text that already reads well keeps its own color.
        let dark_red = Color32::from_rgb(0x73, 0x10, 0x10);
        assert_eq!(s.text(dark_red), dark_red);
        // A selection neither theme color reads on falls back to black or white.
        let t = Theme {
            foreground: [0x80, 0x80, 0x80],
            background: [0x90, 0x90, 0x90],
            selection: [0x60, 0x60, 0x60],
            ..Theme::default()
        };
        let s = SelectionColors::from_theme(&t);
        assert!(contrast_ratio(s.text(c32(t.foreground)), s.bg) >= READABLE);
    }

    #[test]
    fn theme_selection_foreground_wins_when_readable() {
        let t = Theme {
            selection_foreground: Some([0x5d, 0x3f, 0x3b]),
            ..light_selection()
        };
        let s = SelectionColors::from_theme(&t);
        let theirs = Color32::from_rgb(0x5d, 0x3f, 0x3b);
        assert_eq!(s.text(c32(t.foreground)), theirs);
        // Even for text that would have been readable.
        assert_eq!(s.text(Color32::BLACK), theirs);
        // An unreadable choice is ignored.
        let t = Theme {
            selection_foreground: Some([0xff, 0xff, 0xff]),
            ..light_selection()
        };
        let s = SelectionColors::from_theme(&t);
        assert!(contrast_ratio(s.text(c32(t.foreground)), s.bg) >= READABLE);
    }

    #[test]
    fn contrast_gray_on_gray_reaches_target() {
        let bg = Color32::from_rgb(110, 110, 110);
        let fg = Color32::from_rgb(120, 120, 120);
        let out = ensure_contrast(fg, bg, 4.5);
        assert!(contrast_ratio(out, bg) >= 4.5);
    }

    #[test]
    fn contrast_sufficient_unchanged_and_off_is_noop() {
        let (w, k) = (Color32::WHITE, Color32::BLACK);
        assert_eq!(ensure_contrast(w, k, 7.0), w);
        let a = Color32::from_rgb(100, 100, 100);
        assert_eq!(ensure_contrast(a, a, 1.0), a);
        assert!((contrast_ratio(w, k) - 21.0).abs() < 0.01);
    }

    #[test]
    fn contrast_flips_direction_when_needed() {
        // Mid-gray background: pushing toward the brighter side can't reach 7.
        let bg = Color32::from_rgb(130, 130, 130);
        let out = ensure_contrast(Color32::from_rgb(140, 140, 140), bg, 4.5);
        assert!(contrast_ratio(out, bg) >= 4.5);
        let out = ensure_contrast(Color32::from_rgb(10, 20, 30), Color32::BLACK, 21.0);
        assert_eq!(out, Color32::WHITE);
    }
}
