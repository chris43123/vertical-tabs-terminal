//! User configuration, loaded from `<config_dir>/vtt/config.toml`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    /// Lines of scrollback kept per tab.
    pub scrollback: usize,
    /// Sidebar width in logical pixels when expanded.
    pub sidebar_width: f32,
    /// Start with the sidebar collapsed to icons.
    pub sidebar_collapsed: bool,
    /// Name of the profile used for new tabs (falls back to the first discovered).
    pub default_profile: Option<String>,
    /// Extra user profiles, appended to the auto-discovered ones.
    pub profiles: Vec<ProfileConfig>,
    pub colors: ColorConfig,
    /// Shortcut overrides: action name -> chord or list of chords (see `keybinds.rs`).
    pub keybindings: HashMap<String, crate::keybinds::BindingConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// Font family name; falls back to the system monospace font.
    pub family: Option<String>,
    /// Font size in points (logical pixels).
    pub size: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Short text/emoji shown in the collapsed sidebar.
    #[serde(default)]
    pub icon: Option<String>,
    /// Hex accent color, e.g. "#89b4fa".
    #[serde(default)]
    pub color: Option<String>,
}

/// Terminal palette. All values are "#rrggbb".
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection: String,
    /// 16 ANSI colors: normal 0-7 then bright 8-15.
    pub ansi: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            scrollback: 10_000,
            sidebar_width: 240.0,
            sidebar_collapsed: false,
            default_profile: None,
            profiles: Vec::new(),
            colors: ColorConfig::default(),
            keybindings: HashMap::new(),
        }
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: None,
            size: 14.0,
        }
    }
}

impl Default for ColorConfig {
    // Catppuccin Mocha-ish.
    fn default() -> Self {
        let ansi = [
            "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#bac2de",
            "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#a6adc8",
        ];
        Self {
            foreground: "#cdd6f4".into(),
            background: "#1e1e2e".into(),
            cursor: "#f5e0dc".into(),
            selection: "#585b70".into(),
            ansi: ansi.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("vtt").join("config.toml"))
    }

    /// Load the config file, falling back to defaults on any error (reported on stderr).
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|err| {
                eprintln!("vtt: invalid config {}: {err}", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

/// Parse "#rrggbb" into an RGB triple.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex() {
        assert_eq!(parse_hex("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("#fff"), None);
    }

    #[test]
    fn example_config_parses() {
        let cfg: Config = toml::from_str(include_str!("../config.example.toml")).unwrap();
        assert_eq!(cfg.scrollback, 10_000);
        assert!(cfg.keybindings.is_empty());
        assert_eq!(cfg.profiles[0].name, "htop");
    }

    #[test]
    fn parses_partial_config() {
        let cfg: Config = toml::from_str(
            r#"
            scrollback = 500
            [font]
            size = 12
            [[profiles]]
            name = "htop"
            command = "htop"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.scrollback, 500);
        assert_eq!(cfg.font.size, 12.0);
        assert_eq!(cfg.profiles[0].command, "htop");
        assert_eq!(cfg.colors.ansi.len(), 16);
    }
}
