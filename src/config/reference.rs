//! The config reference: every key with its type, default, allowed values and meaning.
//!
//! It's printed by `vtt config reference`, written as `REFERENCE.md` next to the config file,
//! and used by `vtt config check` (and live reloads) to flag unknown keys and out-of-range
//! values. Defaults come from `Config::default()` and the shortcut list from the keymap, so
//! only the descriptions are written by hand; tests keep this list in sync with `Config`.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::config::keybinds::{ACTIONS, Action, Chord, Keybinds};
use crate::config::{Config, edit};

/// One config key. Nested keys are dotted (`font.size`).
pub struct Opt {
    pub key: &'static str,
    pub ty: &'static str,
    /// Allowed values, in words (empty when the type says it all).
    pub values: &'static str,
    /// Values outside this range are clamped by vtt; `check` reports them.
    pub range: Option<(f64, f64)>,
    pub doc: &'static str,
}

const fn opt(
    key: &'static str,
    ty: &'static str,
    values: &'static str,
    range: Option<(f64, f64)>,
    doc: &'static str,
) -> Opt {
    Opt {
        key,
        ty,
        values,
        range,
        doc,
    }
}

pub const OPTIONS: &[Opt] = &[
    opt(
        "scrollback",
        "integer",
        "",
        Some((0.0, 1e7)),
        "Lines of scrollback kept per tab.",
    ),
    opt(
        "sidebar_width",
        "float",
        "",
        Some((80.0, 1000.0)),
        "Sidebar width in logical pixels when expanded.",
    ),
    opt(
        "sidebar_collapsed",
        "bool",
        "",
        None,
        "Start with the sidebar collapsed to icons.",
    ),
    opt(
        "default_profile",
        "string",
        "a profile name (a discovered shell or one from `[[profiles]]`)",
        None,
        "Profile used for new tabs. Unset: the first discovered shell.",
    ),
    opt(
        "theme",
        "string",
        "a built-in theme, or the name of `<name>.toml` / `<name>.conf` (kitty) in the `themes` folder next to the config",
        None,
        "Color theme.",
    ),
    opt(
        "theme_file",
        "path",
        "a vtt `.toml` or kitty theme file; `~` is expanded",
        None,
        "Theme file to use instead of `theme`. Watched: edits recolor live.",
    ),
    opt(
        "adopt_shell_palette",
        "bool",
        "",
        None,
        "When a shell sets colors via escape sequences (pywal, wallust…), apply them to the whole app instead of just that tab.",
    ),
    opt(
        "editor",
        "string",
        "a command, e.g. `\"nvim\"` or `\"code --wait\"`",
        None,
        "Editor for \"Open settings\". Unset: $VISUAL / $EDITOR, then the system's default app. Terminal editors open in a new tab.",
    ),
    opt(
        "ui_scale",
        "float",
        "",
        Some((0.5, 3.0)),
        "Scale of the whole UI. Runtime zoom is remembered until this value changes.",
    ),
    opt(
        "min_contrast",
        "float",
        "1.0 is off; 4.5 is WCAG AA, 7 is AAA",
        Some((1.0, 21.0)),
        "Minimum contrast ratio between terminal text and its background.",
    ),
    opt(
        "reduce_motion",
        "bool",
        "",
        None,
        "Turn off UI animations, smooth scrolling and cursor blinking.",
    ),
    opt(
        "focus_border_width",
        "float",
        "0 for none",
        Some((0.0, 6.0)),
        "Width in points of the accent border around the focused pane in a split.",
    ),
    opt(
        "bell",
        "string",
        "`\"badge\"`, `\"flash\"` or `\"none\"`",
        None,
        "What a terminal bell does: mark the tab, also flash the pane, or nothing.",
    ),
    opt(
        "zen_edge_width",
        "float",
        "",
        Some((1.0, 200.0)),
        "Zen mode: width in points of the strip at the window's left edge that reveals the sidebar.",
    ),
    opt(
        "focus_follows_mouse",
        "bool",
        "",
        None,
        "Focus a pane in a split by moving the pointer over it, without clicking.",
    ),
    opt(
        "hide_pointer_when_typing",
        "bool",
        "",
        None,
        "Hide the mouse pointer while typing into a terminal; it comes back when the mouse moves.",
    ),
    opt(
        "copy_on_select",
        "bool",
        "",
        None,
        "Copy text to the clipboard as soon as you finish selecting it with the mouse (drag, double or triple click).",
    ),
    opt(
        "confirm_reset",
        "bool",
        "",
        None,
        "Ask before \"Reset tab\" kills a program running in the tab (never asked on Windows, where vtt can't tell).",
    ),
    opt(
        "font.family",
        "string",
        "an installed font family",
        None,
        "Terminal font. Unset: the system monospace font.",
    ),
    opt(
        "font.size",
        "float",
        "",
        Some((6.0, 48.0)),
        "Font size in points. Pane zoom adds to it.",
    ),
    opt(
        "font.line_height",
        "float",
        "",
        Some((0.8, 3.0)),
        "Line height as a multiple of the font's own.",
    ),
    opt(
        "font.letter_spacing",
        "float",
        "",
        Some((-2.0, 10.0)),
        "Extra space between characters, in points.",
    ),
    opt(
        "cursor.style",
        "string",
        "`\"block\"`, `\"beam\"` or `\"underline\"`",
        None,
        "Cursor shape when the program doesn't choose one.",
    ),
    opt(
        "cursor.blink",
        "bool",
        "",
        None,
        "Blink the cursor (never with `reduce_motion`).",
    ),
    opt(
        "cursor.thickness",
        "float",
        "",
        Some((0.5, 4.0)),
        "Thickness multiplier for beam, underline and the unfocused outline.",
    ),
    opt(
        "files.open",
        "bool",
        "",
        None,
        "Show the files panel at startup.",
    ),
    opt(
        "files.width",
        "float",
        "",
        Some((80.0, 1000.0)),
        "Initial files panel width in logical pixels.",
    ),
    opt(
        "files.show_hidden",
        "bool",
        "",
        None,
        "List dotfiles (and hidden files on Windows).",
    ),
    opt(
        "files.wrap",
        "bool",
        "",
        None,
        "Wrap long lines in text and code previews.",
    ),
];

/// Keys of one `[[profiles]]` entry.
pub const PROFILE_KEYS: &[(&str, &str, &str)] = &[
    (
        "name",
        "string, required",
        "Shown in the palette and new-tab menu. Same name as a discovered shell replaces it.",
    ),
    ("command", "string, required", "Program to run."),
    ("args", "list of strings", "Arguments."),
    ("cwd", "path", "Starting directory."),
    ("env", "table of strings", "Extra environment variables."),
    (
        "icon",
        "string",
        "One or two characters shown in the sidebar.",
    ),
    ("color", "string", "`\"#rrggbb\"` for the icon."),
];

/// Tables whose keys are free-form (checked elsewhere: colors by the theme, keybindings by
/// the keymap, profiles against `PROFILE_KEYS`).
const OPEN_TABLES: &[&str] = &["colors", "keybindings", "profiles"];

pub fn find(key: &str) -> Option<&'static Opt> {
    OPTIONS.iter().find(|o| o.key == key)
}

/// The default of `key` as TOML, or `None` when it's unset by default.
fn default_value(key: &str) -> Option<String> {
    value_at(&Config::default(), key)
}

/// The value of `key` in `config` as TOML, or `None` when it's unset.
fn value_at(config: &Config, key: &str) -> Option<String> {
    let mut v = serde_json::to_value(config).ok()?;
    for part in key.split('.') {
        v = v.get(part)?.clone();
    }
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(format!("{s:?}")),
        serde_json::Value::Number(n) if n.is_f64() => {
            let f = n.as_f64()?;
            Some(if f.fract() == 0.0 {
                format!("{f:.1}")
            } else {
                format!("{f}")
            })
        }
        v => Some(v.to_string()),
    }
}

/// Default chords of `action` on this OS, in config form.
fn default_chords(defaults: &Keybinds, action: Action) -> String {
    let chords: Vec<String> = defaults
        .chords(action)
        .iter()
        .map(|c| format!("`{}`", c.to_config()))
        .collect();
    if chords.is_empty() {
        "—".into()
    } else {
        chords.join(", ")
    }
}

/// The whole reference as Markdown.
pub fn markdown() -> String {
    let mut out = String::new();
    let path = Config::path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(no config directory)".into());
    let _ = writeln!(
        out,
        "# vtt configuration reference\n\n\
         Generated by `vtt config reference` (vtt {}).\n\n\
         - Config file: `{path}`\n\
         - Format: TOML. Every key is optional; omitted keys use the defaults below.\n\
         - Edits apply live while vtt runs; problems show in a banner.\n\
         - From a shell: `vtt config get <key>`, `vtt config set <key> <value>`, \
         `vtt config unset <key>` (keys as below, dotted for tables: `font.size`, \
         `keybindings.new_tab`). `set` keeps comments and refuses invalid values.\n\
         - Validate after editing by hand: `vtt config check` (exit code 1 on problems).\n\
         - Values outside a range are clamped.\n",
        env!("CARGO_PKG_VERSION")
    );

    let mut section = None;
    for o in OPTIONS {
        let table = o.key.split_once('.').map_or("", |(t, _)| t);
        if section != Some(table) {
            if section.is_some() {
                out.push('\n');
            }
            section = Some(table);
            let title = if table.is_empty() {
                "## Top level".to_string()
            } else {
                format!("## `[{table}]`")
            };
            let _ = writeln!(
                out,
                "{title}\n\n| Key | Type | Default | Values | Description |\n|---|---|---|---|---|"
            );
        }
        let leaf = o.key.rsplit('.').next().unwrap_or(o.key);
        let default = default_value(o.key).map_or("unset".into(), |d| format!("`{d}`"));
        let mut values = o.values.to_string();
        if let Some((lo, hi)) = o.range {
            if !values.is_empty() {
                values.push_str("; ");
            }
            let _ = write!(values, "{lo} to {hi}");
        }
        if o.key == "theme" {
            let names: Vec<String> = crate::theme::BUILTIN
                .iter()
                .map(|(n, _)| format!("`{n}`"))
                .collect();
            let _ = write!(values, ". Built-in: {}", names.join(", "));
        }
        let _ = writeln!(
            out,
            "| `{leaf}` | {} | {default} | {values} | {} |",
            o.ty, o.doc
        );
    }
    out.push('\n');

    out.push_str(
        "## `[colors]`\n\n\
         Color overrides applied on top of the theme. Colors are `\"#rrggbb\"`. Theme files \
         (`themes/<name>.toml`) take the same keys.\n\n\
         | Key | Description |\n|---|---|\n\
         | `foreground` | Default text color. |\n\
         | `background` | Terminal background. |\n\
         | `cursor` | Cursor color. |\n\
         | `selection` | Selection background. |\n\
         | `ansi` | List of 16 colors: the ANSI palette 0–15. |\n\
         | `color0` … `color255` | One palette entry. |\n\
         | `ui.accent` (in `[colors.ui]`) | Accent for focus borders, badges and highlights. |\n\
         | `ui.sidebar` (in `[colors.ui]`) | Sidebar background. |\n\n\
         Kitty theme files (`.conf`) are read with their usual keys (`foreground`, \
         `background`, `cursor`, `selection_background`, `colorN`, `active_border_color`).\n\n",
    );

    out.push_str(
        "## `[[profiles]]`\n\n\
         Extra shells or programs, added after the auto-discovered shells. Repeat the table \
         for each profile.\n\n| Key | Type | Description |\n|---|---|---|\n",
    );
    for (key, ty, doc) in PROFILE_KEYS {
        let _ = writeln!(out, "| `{key}` | {ty} | {doc} |");
    }
    out.push_str(
        "\n```toml\n[[profiles]]\nname = \"htop\"\ncommand = \"htop\"\nicon = \"H\"\n\
         color = \"#a6e3a1\"\n[profiles.env]\nFOO = \"bar\"\n```\n\n",
    );

    let os = if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux/Windows"
    };
    let _ = writeln!(
        out,
        "## `[keybindings]`\n\n\
         `action = \"chord\"` or `action = [\"chord\", …]`. Setting an action replaces all of \
         its defaults; `[]` unbinds it.\n\n\
         Chords are modifiers and a key joined by `+`, e.g. `ctrl+shift+t`, `alt+1`, \
         `cmd+shift+]`. Modifiers: `ctrl`, `shift`, `alt` (or `option`), `cmd` (macOS only). \
         Keys: letters, digits, `f1`–`f20`, `tab`, `enter`, `esc`, `space`, `backspace`, \
         `delete`, `insert`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, \
         `right`, `plus`, `minus`, `equals`, `comma`, `period`, `slash`, `backslash`, `[`, `]`, \
         and other punctuation by name. Avoid chords without a modifier: they'd swallow \
         typing.\n\n\
         Defaults shown are for {os}.\n\n\
         | Action | Description | Default |\n|---|---|---|"
    );
    let defaults = Keybinds::new(&Default::default());
    for (name, label, action) in ACTIONS {
        let _ = writeln!(
            out,
            "| `{name}` | {label} | {} |",
            default_chords(&defaults, *action)
        );
    }
    for n in 1..=9 {
        let action = Action::GotoTab(n);
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            action.config_name(),
            action.label(),
            default_chords(&defaults, action)
        );
    }
    out
}

/// Problems with the keys of a parsed config file: unknown keys, wrong tables and values
/// outside their range. (Type errors are caught when deserializing.)
pub fn lint(table: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    let mut check = |key: String, value: &toml::Value| match find(&key) {
        None => problems.push(format!("unknown key `{key}`")),
        Some(o) => {
            let n = value
                .as_float()
                .or_else(|| value.as_integer().map(|i| i as f64));
            if let (Some((lo, hi)), Some(n)) = (o.range, n)
                && !(lo..=hi).contains(&n)
            {
                problems.push(format!(
                    "`{key} = {n}` is outside {lo} to {hi} and will be clamped"
                ));
            }
        }
    };
    for (key, value) in table {
        if OPEN_TABLES.contains(&key.as_str()) {
            continue;
        }
        match value.as_table() {
            Some(sub)
                if OPTIONS
                    .iter()
                    .any(|o| o.key.starts_with(&format!("{key}."))) =>
            {
                for (k, v) in sub {
                    check(format!("{key}.{k}"), v);
                }
            }
            _ => check(key.clone(), value),
        }
    }
    if let Some(profiles) = table.get("profiles").and_then(|p| p.as_array()) {
        for (i, p) in profiles.iter().enumerate() {
            for k in p.as_table().into_iter().flat_map(|t| t.keys()) {
                if !PROFILE_KEYS.iter().any(|(name, _, _)| name == k) {
                    problems.push(format!("unknown key `{k}` in profile {}", i + 1));
                }
            }
        }
    }
    problems
}

/// Everything wrong with the config file, as `vtt config check` reports it.
pub fn check() -> Result<Vec<String>, String> {
    let path = Config::path().ok_or("no config directory on this system")?;
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Ok(Vec::new()),
    };
    problems_in(&text).map_err(|e| format!("invalid config {}: {e}", path.display()))
}

/// Everything wrong with a config document: syntax and type errors (`Err`), then unknown
/// keys, out-of-range values, bad shortcuts and missing themes.
fn problems_in(text: &str) -> Result<Vec<String>, String> {
    let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let config: Config = toml::from_str(text).map_err(|e| e.to_string())?;
    crate::diag::take();
    Keybinds::new(&config.keybindings);
    crate::theme::resolve(&config);
    let mut problems = lint(&table);
    problems.extend(crate::diag::take());
    Ok(problems)
}

/// `vtt config get`: the current value of `key` as TOML (empty when it's unset).
fn get(key: &str) -> Result<String, String> {
    let config = Config::try_load()?;
    if let Some(name) = key.strip_prefix("keybindings.") {
        let action = Action::from_name(name).ok_or_else(|| unknown(key))?;
        let chords: Vec<String> = Keybinds::new(&config.keybindings)
            .chords(action)
            .iter()
            .map(|c| c.to_config())
            .collect();
        return Ok(edit::chords_value(&chords));
    }
    find(key).ok_or_else(|| unknown(key))?;
    Ok(value_at(&config, key).unwrap_or_default())
}

/// `vtt config set` (`Some`) or `unset` (`None`): change one key in the config file,
/// keeping its comments and layout. Refuses a change that would add a problem.
fn set(key: &str, values: Option<&[String]>) -> Result<String, String> {
    let (table, name) = key.rsplit_once('.').unwrap_or(("", key));
    let value = if table == "keybindings" {
        Action::from_name(name).ok_or_else(|| unknown(key))?;
        // No chords unbinds the action.
        values
            .map(|chords| {
                for c in chords {
                    Chord::parse(c).map_err(|e| format!("`{c}`: {e}"))?;
                }
                Ok::<_, String>(edit::chords_value(chords))
            })
            .transpose()?
    } else {
        let o = find(key).ok_or_else(|| unknown(key))?;
        values
            .map(|v| match v {
                [one] => parse_value(o, one),
                _ => Err(format!(
                    "`{key}` takes one value (quote it if it has spaces)"
                )),
            })
            .transpose()?
    };

    let path = edit::ensure_config()?;
    let old =
        std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let new = edit::edit_key(&old, table, name, value.as_deref());
    let before = problems_in(&old).unwrap_or_default();
    let added: Vec<String> = problems_in(&new)
        .map_err(|e| format!("`{key}`: {e}"))?
        .into_iter()
        .filter(|p| !before.contains(p))
        .collect();
    if !added.is_empty() {
        return Err(added.join("\n"));
    }
    if new != old {
        std::fs::write(&path, new).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    Ok(match value {
        Some(v) => format!("{key} = {v}"),
        None => format!("{key}: back to the default"),
    })
}

/// A command-line value as TOML for `o`, checked against its type and range.
fn parse_value(o: &Opt, raw: &str) -> Result<String, String> {
    let article = if o.ty.starts_with('i') { "an" } else { "a" };
    let wrong = || format!("`{}` takes {article} {}, not `{raw}`", o.key, o.ty);
    let value = match o.ty {
        "integer" => toml::Value::Integer(raw.parse().map_err(|_| wrong())?),
        "float" => toml::Value::Float(raw.parse().map_err(|_| wrong())?),
        "bool" => toml::Value::Boolean(raw.parse().map_err(|_| wrong())?),
        _ => toml::Value::String(raw.to_string()),
    };
    if let (Some((lo, hi)), Some(n)) = (
        o.range,
        value.as_float().or(value.as_integer().map(|i| i as f64)),
    ) && !(lo..=hi).contains(&n)
    {
        return Err(format!("`{}` must be between {lo} and {hi}", o.key));
    }
    Ok(value.to_string())
}

fn unknown(key: &str) -> String {
    format!("unknown key `{key}` (see `vtt config reference`)")
}

/// Write `REFERENCE.md` next to the config file when it's missing or out of date, so agents
/// and people editing the file find the reference without running vtt.
pub fn write_beside_config() {
    let Some(path) = Config::path().and_then(|p| p.parent().map(|d| d.join("REFERENCE.md"))) else {
        return;
    };
    let text = markdown();
    if std::fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, text);
}

/// `vtt config <command>`. Returns the exit code, or `None` when the arguments aren't a
/// config command (then the app starts as usual).
pub fn run_cli(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some("config") {
        return None;
    }
    crate::diag::set_quiet(true);
    let code = match args.get(1).map(String::as_str) {
        Some("reference") => {
            print!("{}", markdown());
            0
        }
        Some("path") => match Config::path() {
            Some(p) => {
                println!("{}", p.display());
                0
            }
            None => {
                eprintln!("vtt: no config directory on this system");
                1
            }
        },
        Some("get") if args.len() == 3 => report(get(&args[2])),
        Some("set") if args.len() >= 3 => report(set(&args[2], Some(&args[3..]))),
        Some("unset") if args.len() == 3 => report(set(&args[2], None)),
        Some("check") => match check() {
            Ok(problems) if problems.is_empty() => {
                println!("{}: ok", config_display());
                0
            }
            Ok(problems) => {
                for p in problems {
                    println!("{}: {p}", config_display());
                }
                1
            }
            Err(err) => {
                println!("{err}");
                1
            }
        },
        _ => {
            eprintln!(
                "usage: vtt config <command>\n\n  \
                 get <key>              print a key's current value\n  \
                 set <key> <value>      change a key, keeping the file's comments\n  \
                 set keybindings.<action> [<chord>…]\n                         \
                 bind an action to these chords (none: unbind it)\n  \
                 unset <key>            remove a key so it uses the default\n  \
                 reference              print every key, its default and allowed values (Markdown)\n  \
                 path                   print the config file's path\n  \
                 check                  validate the config file (exit code 1 on problems)\n\n\
                 Keys are dotted for tables: font.size, cursor.style, keybindings.new_tab."
            );
            2
        }
    };
    Some(code)
}

/// Print a command's result and turn it into an exit code.
fn report(result: Result<String, String>) -> i32 {
    match result {
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
            0
        }
        Err(err) => {
            eprintln!("vtt: {err}");
            1
        }
    }
}

fn config_display() -> String {
    Config::path()
        .unwrap_or_else(|| PathBuf::from("config.toml"))
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dotted paths of every scalar field of `Config`, from its serialized defaults.
    fn config_keys() -> Vec<String> {
        let mut keys = Vec::new();
        let v = serde_json::to_value(Config::default()).unwrap();
        for (k, v) in v.as_object().unwrap() {
            if OPEN_TABLES.contains(&k.as_str()) {
                continue;
            }
            match v.as_object() {
                Some(sub) => keys.extend(sub.keys().map(|s| format!("{k}.{s}"))),
                None => keys.push(k.clone()),
            }
        }
        keys
    }

    #[test]
    fn every_config_field_is_documented() {
        let keys = config_keys();
        for k in &keys {
            assert!(find(k).is_some(), "`{k}` is missing from OPTIONS");
        }
        for o in OPTIONS {
            assert!(
                keys.iter().any(|k| k == o.key),
                "`{}` isn't a Config field",
                o.key
            );
        }
    }

    #[test]
    fn every_option_is_in_the_example() {
        let example = include_str!("../../config.example.toml");
        let mut section = String::new();
        let mut found = Vec::new();
        for line in example.lines() {
            let line = line.trim().trim_start_matches('#').trim();
            if let Some(t) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = t.to_string();
            } else if let Some((k, _)) = line.split_once(" = ") {
                found.push(if section.is_empty() {
                    k.to_string()
                } else {
                    format!("{section}.{k}")
                });
            }
        }
        for o in OPTIONS {
            assert!(
                found.iter().any(|k| k == o.key),
                "`{}` is missing from config.example.toml",
                o.key
            );
        }
    }

    #[test]
    fn example_lints_clean() {
        let table: toml::Table = toml::from_str(include_str!("../../config.example.toml")).unwrap();
        assert_eq!(lint(&table), Vec::<String>::new());
    }

    #[test]
    fn lint_flags_unknown_keys_and_ranges() {
        let table: toml::Table = toml::from_str(
            "scrolback = 5\nmin_contrast = 30\n[font]\nsize = 12\nsiz = 3\n\
             [[profiles]]\nname = \"a\"\ncommand = \"b\"\ncolour = \"#fff\"\n",
        )
        .unwrap();
        let problems = lint(&table);
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("scrolback")));
        assert!(problems.iter().any(|p| p.contains("font.siz")));
        assert!(problems.iter().any(|p| p.contains("min_contrast")));
        assert!(problems.iter().any(|p| p.contains("colour")));
    }

    #[test]
    fn parses_values_by_type_and_range() {
        let v = |key: &str, raw: &str| parse_value(find(key).unwrap(), raw);
        assert_eq!(v("scrollback", "500").unwrap(), "500");
        assert_eq!(v("font.size", "16").unwrap(), "16.0");
        assert_eq!(v("reduce_motion", "true").unwrap(), "true");
        assert_eq!(v("editor", "code --wait").unwrap(), "\"code --wait\"");
        assert!(v("scrollback", "lots").is_err());
        assert!(v("font.size", "100").is_err());
        assert!(v("reduce_motion", "yes").is_err());
    }

    #[test]
    fn problems_in_catches_bad_enums_and_chords() {
        assert!(problems_in("bell = \"loud\"\n").is_err());
        let p = problems_in("[keybindings]\nnew_tab = \"ctrl+nope\"\n").unwrap();
        assert!(p.iter().any(|p| p.contains("ctrl+nope")), "{p:?}");
        assert!(
            problems_in(include_str!("../../config.example.toml"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn reference_lists_defaults_and_actions() {
        let md = markdown();
        assert!(md.contains("| `scrollback` | integer | `10000` |"));
        assert!(md.contains("| `size` | float | `14.0` |"));
        assert!(md.contains("| `style` | string | `\"block\"` |"));
        assert!(md.contains("| `default_profile` | string | unset |"));
        assert!(md.contains("`new_tab`"));
        assert!(md.contains("`goto_tab_9`"));
    }
}
