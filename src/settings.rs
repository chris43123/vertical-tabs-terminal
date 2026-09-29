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
    fn seeded_config_is_the_example_and_parses() {
        let cfg: Config = toml::from_str(EXAMPLE).unwrap();
        assert!(cfg.profiles.is_empty(), "the seed must not add profiles");
    }
}
