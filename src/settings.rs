//! "Open settings": create the config file from the documented example if needed, then open it.
//! Terminal editors run in a new vtt tab; without one configured, the OS default app is used.
//! Edits apply live through the config watcher.

use std::path::PathBuf;

use crate::config::Config;

const EXAMPLE: &str = include_str!("../config.example.toml");

/// Make sure the config file exists (seeded with the commented example). Returns its path.
pub fn ensure_config() -> Result<PathBuf, String> {
    let path = Config::path().ok_or("no config directory on this system")?;
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        std::fs::write(&path, EXAMPLE).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    Ok(path)
}

/// Set (`Some`) or remove (`None`, back to the defaults) one action in `[keybindings]` of the
/// config file, keeping everything else as written. The watcher then reloads it.
pub fn set_keybinding(name: &str, chords: Option<&[String]>) -> Result<(), String> {
    let path = ensure_config()?;
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let value = chords.map(|c| match c {
        [] => "[]".to_string(),
        [one] => format!("{one:?}"),
        many => format!(
            "[{}]",
            many.iter()
                .map(|c| format!("{c:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    });
    let new = edit_keybinding(&text, name, value.as_deref());
    // Never write a file the config loader would reject.
    toml::from_str::<Config>(&new).map_err(|e| format!("couldn't update the config: {e}"))?;
    std::fs::write(&path, new).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// Replace, insert or remove `name = value` in the `[keybindings]` table of a TOML document.
/// Line based so comments and layout survive; multi-line arrays are replaced whole.
fn edit_keybinding(text: &str, name: &str, value: Option<&str>) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let is_header = |l: &str| l.trim_start().starts_with('[');
    let section = lines.iter().position(|l| l.trim() == "[keybindings]");
    let Some(start) = section else {
        if let Some(value) = value {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[keybindings]".into());
            lines.push(format!("{name} = {value}"));
        }
        return join_lines(lines, text);
    };
    let end = lines[start + 1..]
        .iter()
        .position(|l| is_header(l))
        .map_or(lines.len(), |i| start + 1 + i);
    let key_of = |l: &str| {
        let (k, _) = l.split_once('=')?;
        let k = k.trim().trim_matches('"');
        (!l.trim_start().starts_with('#')).then_some(k.to_string())
    };
    let found = (start + 1..end).find(|&i| key_of(&lines[i]).as_deref() == Some(name));
    match (found, value) {
        (Some(i), value) => {
            // A multi-line array runs until its closing bracket.
            let mut last = i;
            let rhs = lines[i]
                .split_once('=')
                .map_or("", |(_, v)| v)
                .trim()
                .to_string();
            if rhs.starts_with('[') && !rhs.contains(']') {
                while last + 1 < end && !lines[last].contains(']') {
                    last += 1;
                }
            }
            let replacement: Vec<String> =
                value.map(|v| format!("{name} = {v}")).into_iter().collect();
            lines.splice(i..=last, replacement);
        }
        (None, Some(value)) => {
            // After the last active key, or right below the header.
            let at = (start + 1..end)
                .rev()
                .find(|&i| key_of(&lines[i]).is_some())
                .map_or(start + 1, |i| i + 1);
            lines.insert(at, format!("{name} = {value}"));
        }
        (None, None) => {}
    }
    join_lines(lines, text)
}

fn join_lines(lines: Vec<String>, original: &str) -> String {
    let mut out = lines.join("\n");
    if original.ends_with('\n') || original.is_empty() {
        out.push('\n');
    }
    out
}

/// Editor command line: the `editor` setting, else `$VISUAL`, else `$EDITOR`.
pub fn editor(config: &Config) -> Option<Vec<String>> {
    let cmd = config
        .editor
        .clone()
        .or_else(|| std::env::var("VISUAL").ok())
        .or_else(|| std::env::var("EDITOR").ok())?;
    let parts = split_command(&cmd);
    (!parts.is_empty()).then_some(parts)
}

/// Split a command line on whitespace, honouring simple double/single quotes.
fn split_command(cmd: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for c in cmd.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                has = true;
            }
            (None, c) if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    parts.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if has || !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

/// Open a file with the system's default handler, detached from vtt.
pub fn open_external(path: &std::path::Path) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-t"); // default *text* editor, regardless of the .toml association
        c
    } else if cfg!(windows) {
        std::process::Command::new("notepad.exe")
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| {
            format!(
                "couldn't open {}: {e} (set `editor` in the config or $EDITOR)",
                path.display()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_editor_commands() {
        assert_eq!(split_command("nvim"), vec!["nvim"]);
        assert_eq!(split_command("code --wait"), vec!["code", "--wait"]);
        assert_eq!(
            split_command(r#""C:\Program Files\Editor\ed.exe" -n"#),
            vec![r"C:\Program Files\Editor\ed.exe", "-n"]
        );
        assert_eq!(split_command("  "), Vec::<String>::new());
    }

    #[test]
    fn editor_setting_wins() {
        let cfg = Config {
            editor: Some("micro -syntax on".into()),
            ..Default::default()
        };
        assert_eq!(editor(&cfg).unwrap(), vec!["micro", "-syntax", "on"]);
    }

    #[test]
    fn edits_keybindings_in_place() {
        let text = "# top\nscrollback = 5\n\n[keybindings]\n# new_tab = \"ctrl+t\"\nclose_tab = []\n\n[[profiles]]\nname = \"x\"\n";
        // Insert after the last active key, leaving comments alone.
        let t = edit_keybinding(text, "new_tab", Some("\"alt+t\""));
        assert!(t.contains("close_tab = []\nnew_tab = \"alt+t\"\n"), "{t}");
        assert!(t.contains("# new_tab = \"ctrl+t\""));
        // Replace.
        let t = edit_keybinding(&t, "new_tab", Some("[\"alt+t\", \"f2\"]"));
        assert!(t.contains("new_tab = [\"alt+t\", \"f2\"]\n"), "{t}");
        // Remove (back to defaults).
        let t = edit_keybinding(&t, "new_tab", None);
        assert_eq!(t, text);
        // Keys in other tables aren't touched.
        let t = edit_keybinding(text, "name", Some("\"y\""));
        assert!(t.contains("name = \"x\""));
        assert!(t.contains("[keybindings]\n# new_tab = \"ctrl+t\"\nclose_tab = []\nname = \"y\""));
    }

    #[test]
    fn edits_multiline_arrays_and_adds_the_table() {
        let text =
            "[keybindings]\nnext_tab = [\n  \"ctrl+tab\",\n  \"alt+j\",\n]\nprev_tab = \"alt+k\"\n";
        let t = edit_keybinding(text, "next_tab", Some("\"f1\""));
        assert_eq!(
            t,
            "[keybindings]\nnext_tab = \"f1\"\nprev_tab = \"alt+k\"\n"
        );
        let t = edit_keybinding("scrollback = 5\n", "new_tab", Some("\"f1\""));
        assert_eq!(t, "scrollback = 5\n\n[keybindings]\nnew_tab = \"f1\"\n");
        let cfg: Config = toml::from_str(&t).unwrap();
        assert_eq!(cfg.keybindings.len(), 1);
    }

    #[test]
    fn seeded_config_is_the_example_and_parses() {
        let cfg: Config = toml::from_str(EXAMPLE).unwrap();
        assert!(cfg.profiles.is_empty(), "the seed must not add profiles");
    }
}
