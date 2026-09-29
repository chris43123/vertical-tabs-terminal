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
    /// Built-in theme name, or a file `<config_dir>/vtt/themes/<name>.toml|.conf`.
    pub theme: Option<String>,
    /// Path to a theme file (vtt `.toml` or kitty format); takes precedence over `theme`.
    pub theme_file: Option<PathBuf>,
    /// Inline color overrides on top of the theme (same keys as a vtt theme file).
    pub colors: toml::Table,
    /// When a shell sets colors via OSC 4/10/11/12 (as pywal, wallust or matugen scripts do),
    /// apply them to the whole app instead of just that tab.
    pub adopt_shell_palette: bool,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            scrollback: 10_000,
            sidebar_width: 240.0,
            sidebar_collapsed: false,
            default_profile: None,
            profiles: Vec::new(),
            theme: None,
            theme_file: None,
            colors: toml::Table::new(),
            adopt_shell_palette: false,
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

impl Config {
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("vtt").join("config.toml"))
    }

    /// Load the config file, falling back to defaults on any error (reported on stderr).
    pub fn load() -> Self {
        Self::try_load().unwrap_or_else(|err| {
            eprintln!("vtt: {err}");
            Self::default()
        })
    }

    /// Load the config file. A missing file gives the defaults; a broken one is an error
    /// (so a live reload can keep the previous config while you're mid-edit).
    pub fn try_load() -> Result<Self, String> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text)
                .map_err(|err| format!("invalid config {}: {err}", path.display())),
            Err(_) => Ok(Self::default()),
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
        assert!(cfg.colors.is_empty());
        assert!(!cfg.adopt_shell_palette);
    }
}
