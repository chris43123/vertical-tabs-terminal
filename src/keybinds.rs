//! App shortcuts: per-OS defaults, overridable from `[keybindings]` in the config.
//!
//! Chords are written like `"alt+t"`, `"ctrl+shift+tab"` or `"cmd+1"`. On Linux/Windows the
//! defaults use Alt (the key where Cmd sits on a Mac). They avoid Alt+B/D/F, which shells use
//! for word movement and deletion.

use std::collections::HashMap;

use eframe::egui::{Key, Modifiers};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    NewTab,
    CloseTab,
    ReopenClosedTab,
    DuplicateTab,
    RenameTab,
    SplitRight,
    SplitDown,
    /// Pull the focused pane out of its split into its own tab.
    MinimizePane,
    ToggleSidebar,
    NextTab,
    PrevTab,
    /// Move the focused tab (or its whole split group) one step up/down the sidebar.
    MoveTabUp,
    MoveTabDown,
    /// Jump to the Nth tab in the sidebar (1-based).
    GotoTab(u8),
    LastTab,
    /// Jump to the next tab with unread output or a bell.
    NextActivity,
    CommandPalette,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ScrollPageUp,
    ScrollPageDown,
}

/// Config name and human label of every action (except `GotoTab`, which is numbered).
pub const ACTIONS: &[(&str, &str, Action)] = &[
    ("new_tab", "New tab", Action::NewTab),
    ("close_tab", "Close tab", Action::CloseTab),
    (
        "reopen_closed_tab",
        "Reopen closed tab",
        Action::ReopenClosedTab,
    ),
    ("duplicate_tab", "Duplicate tab", Action::DuplicateTab),
    ("rename_tab", "Rename tab", Action::RenameTab),
    (
        "split_right",
        "Split right with new tab",
        Action::SplitRight,
    ),
    ("split_down", "Split down with new tab", Action::SplitDown),
    (
        "minimize_pane",
        "Minimise pane to its own tab",
        Action::MinimizePane,
    ),
    ("toggle_sidebar", "Toggle sidebar", Action::ToggleSidebar),
    ("next_tab", "Next tab", Action::NextTab),
    ("prev_tab", "Previous tab", Action::PrevTab),
    ("move_tab_up", "Move tab up", Action::MoveTabUp),
    ("move_tab_down", "Move tab down", Action::MoveTabDown),
    ("last_tab", "Go to last tab", Action::LastTab),
    (
        "next_activity",
        "Go to next tab with new output",
        Action::NextActivity,
    ),
    (
        "command_palette",
        "Switch tab / command palette",
        Action::CommandPalette,
    ),
    ("focus_left", "Focus pane left", Action::FocusLeft),
    ("focus_right", "Focus pane right", Action::FocusRight),
    ("focus_up", "Focus pane above", Action::FocusUp),
    ("focus_down", "Focus pane below", Action::FocusDown),
    ("zoom_in", "Zoom in", Action::ZoomIn),
    ("zoom_out", "Zoom out", Action::ZoomOut),
    ("zoom_reset", "Reset zoom", Action::ZoomReset),
    ("scroll_page_up", "Scroll up one page", Action::ScrollPageUp),
    (
        "scroll_page_down",
        "Scroll down one page",
        Action::ScrollPageDown,
    ),
];

impl Action {
    fn from_name(name: &str) -> Option<Self> {
        if let Some(&(_, _, a)) = ACTIONS.iter().find(|(n, _, _)| *n == name) {
            return Some(a);
        }
        let n: u8 = name.strip_prefix("goto_tab_")?.parse().ok()?;
        (1..=9).contains(&n).then_some(Self::GotoTab(n))
    }
}

/// A key plus the exact set of modifiers that must be held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Cmd on macOS. Not available elsewhere.
    pub cmd: bool,
}

impl Chord {
    pub fn parse(s: &str) -> Result<Self, String> {
        Self::parse_for(s, cfg!(target_os = "macos"))
    }

    /// Parse as if running on macOS (`cmd` allowed) or not.
    fn parse_for(s: &str, macos: bool) -> Result<Self, String> {
        let s = s.trim().to_ascii_lowercase();
        // Allow "ctrl++" by treating a trailing '+' as the key.
        let (mods, key) = match s.strip_suffix("++") {
            Some(rest) => (rest, "plus"),
            None => s.rsplit_once('+').unwrap_or(("", &s)),
        };
        let mut chord = Chord {
            key: parse_key(key).ok_or_else(|| format!("unknown key `{key}`"))?,
            ctrl: false,
            shift: false,
            alt: false,
            cmd: false,
        };
        for m in mods.split('+').filter(|m| !m.is_empty()) {
            match m {
                "ctrl" | "control" => chord.ctrl = true,
                "shift" => chord.shift = true,
                "alt" | "option" | "opt" => chord.alt = true,
                "cmd" | "command" | "super" => {
                    if !macos {
                        return Err("`cmd` is only available on macOS".into());
                    }
                    chord.cmd = true;
                }
                other => return Err(format!("unknown modifier `{other}`")),
            }
        }
        Ok(chord)
    }

    fn matches(&self, key: Key, m: Modifiers) -> bool {
        self.key == key
            && self.ctrl == m.ctrl
            && self.shift == m.shift
            && self.alt == m.alt
            && self.cmd == m.mac_cmd
    }

    /// Human-readable form for tooltips, e.g. "Alt+Shift+D" or "⌘T".
    pub fn label(&self) -> String {
        let key = match self.key {
            Key::ArrowLeft => "←".to_string(),
            Key::ArrowRight => "→".to_string(),
            Key::ArrowUp => "↑".to_string(),
            Key::ArrowDown => "↓".to_string(),
            k => k.symbol_or_name().to_string(),
        };
        if cfg!(target_os = "macos") {
            let mut out = String::new();
            for (on, sym) in [
                (self.ctrl, "⌃"),
                (self.alt, "⌥"),
                (self.shift, "⇧"),
                (self.cmd, "⌘"),
            ] {
                if on {
                    out.push_str(sym);
                }
            }
            return out + &key;
        }
        let mut parts = Vec::new();
        for (on, name) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
        ] {
            if on {
                parts.push(name.to_string());
            }
        }
        parts.push(key);
        parts.join("+")
    }
}

fn parse_key(name: &str) -> Option<Key> {
    let key = match name {
        "tab" => Key::Tab,
        "enter" | "return" => Key::Enter,
        "esc" | "escape" => Key::Escape,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "insert" | "ins" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "left" | "arrowleft" => Key::ArrowLeft,
        "right" | "arrowright" => Key::ArrowRight,
        "up" | "arrowup" => Key::ArrowUp,
        "down" | "arrowdown" => Key::ArrowDown,
        "plus" => Key::Plus,
        "minus" | "-" => Key::Minus,
        "equals" | "equal" | "=" => Key::Equals,
        "comma" | "," => Key::Comma,
        "period" | "." => Key::Period,
        "slash" | "/" => Key::Slash,
        "backslash" | "\\" => Key::Backslash,
        "bracketleft" | "[" => Key::OpenBracket,
        "bracketright" | "]" => Key::CloseBracket,
        "backtick" | "grave" | "`" => Key::Backtick,
        "semicolon" | ";" => Key::Semicolon,
        "quote" | "'" => Key::Quote,
        _ => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => {
                    return Key::from_name(&c.to_ascii_uppercase().to_string());
                }
                _ => {}
            }
            // F1-F24 and anything else egui knows by name (e.g. "F5").
            return Key::from_name(&name.to_ascii_uppercase()).or_else(|| Key::from_name(name));
        }
    };
    Some(key)
}

/// A config value: one chord or a list. An empty string or empty list unbinds the action.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum BindingConfig {
    One(String),
    Many(Vec<String>),
}

impl BindingConfig {
    fn chords(&self) -> Vec<&str> {
        match self {
            BindingConfig::One(s) => vec![s.as_str()],
            BindingConfig::Many(v) => v.iter().map(String::as_str).collect(),
        }
        .into_iter()
        .filter(|s| !s.trim().is_empty() && *s != "none")
        .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Keybinds {
    /// Checked in order; user overrides come first so they win over conflicting defaults.
    bindings: Vec<(Chord, Action)>,
}

impl Keybinds {
    pub fn new(user: &HashMap<String, BindingConfig>) -> Self {
        let mut user_bindings = Vec::new();
        let mut overridden = Vec::new();
        for (name, binding) in user {
            let Some(action) = Action::from_name(name) else {
                eprintln!("vtt: unknown keybinding action `{name}`");
                continue;
            };
            overridden.push(action);
            for s in binding.chords() {
                match Chord::parse(s) {
                    Ok(chord) => user_bindings.push((chord, action)),
                    Err(err) => eprintln!("vtt: keybinding `{name} = \"{s}\"`: {err}"),
                }
            }
        }
        let defaults = defaults()
            .into_iter()
            .filter(|(_, a)| !overridden.contains(a));
        Self {
            bindings: user_bindings.into_iter().chain(defaults).collect(),
        }
    }

    /// Match the logical key first, then the physical key (so e.g. Cmd+Shift+] still matches
    /// when Shift turns the logical key into `}`).
    pub fn lookup(&self, key: Key, physical: Option<Key>, m: Modifiers) -> Option<Action> {
        let find = |k: Key| {
            self.bindings
                .iter()
                .find(|(c, _)| c.matches(k, m))
                .map(|(_, a)| *a)
        };
        find(key).or_else(|| physical.filter(|p| *p != key).and_then(find))
    }

    /// First chord bound to `action`, e.g. "Alt+T".
    pub fn label(&self, action: Action) -> Option<String> {
        self.bindings
            .iter()
            .find(|(_, a)| *a == action)
            .map(|(c, _)| c.label())
    }

    /// Like [`Self::label`], formatted for a tooltip, e.g. " (Alt+T)".
    pub fn hint(&self, action: Action) -> String {
        self.label(action)
            .map(|l| format!(" ({l})"))
            .unwrap_or_default()
    }
}

fn defaults() -> Vec<(Chord, Action)> {
    let macos = cfg!(target_os = "macos");
    default_specs(macos)
        .into_iter()
        .map(|(s, a)| (Chord::parse_for(&s, macos).expect("valid default chord"), a))
        .collect()
}

/// Default chords as strings, for macOS or Linux/Windows.
fn default_specs(macos: bool) -> Vec<(String, Action)> {
    use Action::*;
    let mut list: Vec<(&str, Action)> = if macos {
        vec![
            ("cmd+t", NewTab),
            ("cmd+w", CloseTab),
            ("cmd+shift+t", ReopenClosedTab),
            ("cmd+r", RenameTab),
            ("cmd+d", SplitRight),
            ("cmd+shift+d", SplitDown),
            ("cmd+shift+m", MinimizePane), // Cmd+M minimises the window on macOS.
            ("cmd+b", ToggleSidebar),
            ("cmd+down", NextTab),
            ("cmd+up", PrevTab),
            ("cmd+shift+]", NextTab),
            ("cmd+shift+[", PrevTab),
            ("cmd+shift+down", MoveTabDown),
            ("cmd+shift+up", MoveTabUp),
            ("cmd+9", LastTab),
            ("cmd+shift+a", NextActivity),
            ("cmd+p", CommandPalette),
            ("cmd+shift+p", CommandPalette),
            ("cmd+alt+left", FocusLeft),
            ("cmd+alt+right", FocusRight),
            ("cmd+alt+up", FocusUp),
            ("cmd+alt+down", FocusDown),
            ("cmd+equals", ZoomIn),
            ("cmd+plus", ZoomIn),
            ("cmd+shift+plus", ZoomIn),
            ("cmd+minus", ZoomOut),
            ("cmd+0", ZoomReset),
        ]
    } else {
        vec![
            ("alt+t", NewTab),
            ("alt+w", CloseTab),
            ("alt+shift+t", ReopenClosedTab),
            ("alt+r", RenameTab),
            ("alt+shift+d", SplitRight),
            ("alt+shift+e", SplitDown),
            ("alt+m", MinimizePane),
            ("alt+shift+b", ToggleSidebar),
            ("alt+down", NextTab),
            ("alt+up", PrevTab),
            ("alt+shift+down", MoveTabDown),
            ("alt+shift+up", MoveTabUp),
            ("alt+9", LastTab),
            ("alt+a", NextActivity),
            ("alt+p", CommandPalette),
            // Common terminal shortcuts, kept as alternates since they clash with nothing.
            ("ctrl+shift+t", NewTab),
            ("ctrl+shift+w", CloseTab),
            ("ctrl+shift+p", CommandPalette),
            ("alt+left", FocusLeft),
            ("alt+right", FocusRight),
            ("ctrl+alt+up", FocusUp),
            ("ctrl+alt+down", FocusDown),
            ("ctrl+equals", ZoomIn),
            ("ctrl+plus", ZoomIn),
            ("ctrl+shift+plus", ZoomIn),
            ("ctrl+minus", ZoomOut),
            ("ctrl+0", ZoomReset),
        ]
    };
    list.extend([
        ("ctrl+tab", NextTab),
        ("ctrl+pagedown", NextTab),
        ("ctrl+shift+tab", PrevTab),
        ("ctrl+pageup", PrevTab),
        ("shift+pageup", ScrollPageUp),
        ("shift+pagedown", ScrollPageDown),
    ]);
    // 1-8 jump to that tab; 9 is the last tab (like browsers).
    let goto_mod = if macos { "cmd" } else { "alt" };
    list.iter()
        .map(|(s, a)| (s.to_string(), *a))
        .chain((1..=8).map(|n| (format!("{goto_mod}+{n}"), GotoTab(n))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(ctrl: bool, shift: bool, alt: bool) -> Modifiers {
        Modifiers {
            ctrl,
            shift,
            alt,
            command: ctrl,
            ..Default::default()
        }
    }

    impl Keybinds {
        fn lookup_l(&self, key: Key, m: Modifiers) -> Option<Action> {
            self.lookup(key, None, m)
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn physical_key_fallback() {
        // e.g. on AZERTY, the key in the "0" position yields a different logical key.
        let kb = Keybinds::new(&HashMap::new());
        assert_eq!(
            kb.lookup(Key::Semicolon, Some(Key::Num0), mods(true, false, false)),
            Some(Action::ZoomReset)
        );
        assert_eq!(
            kb.lookup(Key::Semicolon, None, mods(true, false, false)),
            None
        );
    }

    #[test]
    fn default_tables_parse_on_every_os() {
        for macos in [false, true] {
            let specs = default_specs(macos);
            for (s, _) in &specs {
                assert!(Chord::parse_for(s, macos).is_ok(), "macos={macos}: {s}");
            }
            // No chord is bound to two different actions.
            for (i, (a, x)) in specs.iter().enumerate() {
                for (b, y) in &specs[i + 1..] {
                    assert!(
                        Chord::parse_for(a, macos) != Chord::parse_for(b, macos) || x == y,
                        "macos={macos}: {a} bound to {x:?} and {y:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn parses_chords() {
        let c = Chord::parse("Alt+Shift+D").unwrap();
        assert_eq!((c.key, c.alt, c.shift, c.ctrl), (Key::D, true, true, false));
        assert_eq!(Chord::parse("ctrl++").unwrap().key, Key::Plus);
        assert_eq!(Chord::parse("ctrl+pagedown").unwrap().key, Key::PageDown);
        assert_eq!(Chord::parse("alt+1").unwrap().key, Key::Num1);
        assert_eq!(Chord::parse("f5").unwrap().key, Key::F5);
        assert_eq!(Chord::parse("ctrl+[").unwrap().key, Key::OpenBracket);
        assert!(Chord::parse("hyper+x").is_err());
        assert!(Chord::parse("alt+nosuchkey").is_err());
    }

    #[test]
    fn every_action_name_round_trips() {
        for &(name, _, action) in ACTIONS {
            assert_eq!(Action::from_name(name), Some(action), "{name}");
        }
        assert_eq!(Action::from_name("goto_tab_9"), Some(Action::GotoTab(9)));
        assert!(Action::from_name("goto_tab_0").is_none());
        assert!(Action::from_name("goto_tab_10").is_none());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn defaults_and_exact_modifier_matching() {
        let kb = Keybinds::new(&HashMap::new());
        assert_eq!(
            kb.lookup_l(Key::T, mods(false, false, true)),
            Some(Action::NewTab)
        );
        assert_eq!(
            kb.lookup_l(Key::T, mods(true, true, false)),
            Some(Action::NewTab)
        );
        assert_eq!(
            kb.lookup_l(Key::Num3, mods(false, false, true)),
            Some(Action::GotoTab(3))
        );
        assert_eq!(
            kb.lookup_l(Key::Num9, mods(false, false, true)),
            Some(Action::LastTab)
        );
        assert_eq!(
            kb.lookup_l(Key::ArrowDown, mods(false, false, true)),
            Some(Action::NextTab)
        );
        assert_eq!(
            kb.lookup_l(Key::ArrowUp, mods(false, true, true)),
            Some(Action::MoveTabUp)
        );
        // Alt+D / Alt+B stay with the shell (delete word / back word).
        assert_eq!(kb.lookup_l(Key::D, mods(false, false, true)), None);
        assert_eq!(kb.lookup_l(Key::B, mods(false, false, true)), None);
        // Extra modifiers must not match.
        assert_eq!(kb.lookup_l(Key::T, mods(true, false, true)), None);
        assert_eq!(kb.hint(Action::NewTab), " (Alt+T)");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn user_overrides_replace_defaults() {
        let mut user = HashMap::new();
        user.insert("new_tab".to_string(), BindingConfig::One("ctrl+t".into()));
        user.insert("close_tab".to_string(), BindingConfig::Many(vec![]));
        // Rebinding Alt+T to something else wins over the (now removed) default.
        user.insert(
            "toggle_sidebar".to_string(),
            BindingConfig::Many(vec!["alt+t".into(), "bogus+x".into()]),
        );
        let kb = Keybinds::new(&user);
        assert_eq!(
            kb.lookup_l(Key::T, mods(true, false, false)),
            Some(Action::NewTab)
        );
        assert_eq!(
            kb.lookup_l(Key::T, mods(false, false, true)),
            Some(Action::ToggleSidebar)
        );
        assert_eq!(kb.lookup_l(Key::T, mods(true, true, false)), None);
        assert_eq!(kb.lookup_l(Key::W, mods(false, false, true)), None);
        assert_eq!(kb.hint(Action::CloseTab), "");
    }

    #[test]
    fn parses_from_toml() {
        let map: HashMap<String, BindingConfig> = toml::from_str(
            r#"
            new_tab = "ctrl+t"
            next_tab = ["ctrl+tab", "alt+j"]
            "#,
        )
        .unwrap();
        assert_eq!(map["new_tab"].chords(), vec!["ctrl+t"]);
        assert_eq!(map["next_tab"].chords().len(), 2);
    }
}
