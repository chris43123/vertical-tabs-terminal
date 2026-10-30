//! `vtt session`: inspect or clear the saved session, for scripts and agents.
//! Exit codes: 0 ok, 1 error (the message says what to do), 2 usage.

use std::io::Write;

use super::{Session, Store};

const USAGE: &str = "usage: vtt session <command>\n\n  \
     show [--json]   list the saved tabs (position, title, profile, directory, command,\n                  \
     folder, favorite, split)\n  \
     clear           forget the saved session (quit vtt first, or it saves again)\n  \
     path            print the session file's path\n\n\
     vtt saves the session while it runs and restores it at launch (`restore_session`).";

/// What the commands need from the outside world (so tests can supply a temp file).
pub struct Env<'a> {
    pub store: &'a Store,
}

/// `vtt session …`. Returns the exit code, or `None` when the arguments aren't a session
/// command (then the app starts as usual).
pub fn run_cli(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some("session") {
        return None;
    }
    crate::diag::set_quiet(true);
    let Some(path) = Store::default_path() else {
        eprintln!("vtt: no data directory on this system");
        return Some(1);
    };
    let store = Store::new(path);
    Some(run(
        &args[1..],
        &Env { store: &store },
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ))
}

/// Run a subcommand (`args` without the leading `session`).
pub fn run(args: &[String], env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args[..] {
        ["show"] => load(env).map(|s| show_text(&s)),
        ["show", "--json"] => load(env).map(|s| show_json(&s)),
        ["clear"] => env.store.clear().map(|existed| {
            if existed {
                format!("cleared {}", env.store.path.display())
            } else {
                "no saved session".into()
            }
        }),
        ["path"] => Ok(env.store.path.display().to_string()),
        ["--help" | "-h" | "help"] => {
            let _ = writeln!(out, "{USAGE}");
            return 0;
        }
        _ => {
            let _ = writeln!(err, "{USAGE}");
            return 2;
        }
    };
    match result {
        Ok(text) => {
            let _ = writeln!(out, "{text}");
            0
        }
        Err(e) => {
            let _ = writeln!(err, "vtt: {e}");
            1
        }
    }
}

fn load(env: &Env) -> Result<Session, String> {
    env.store.load().map_err(|e| {
        format!(
            "{e}\n(vtt moves it aside at its next launch, or remove it with `vtt session clear`)"
        )
    })
}

/// Per tab, the 1-based number of its split (numbered in sidebar order), if it's in one.
fn split_numbers(s: &Session) -> Vec<Option<usize>> {
    let mut numbered: Vec<usize> = Vec::new();
    (0..s.tabs.len())
        .map(|i| {
            let v = s
                .view_of(i)
                .filter(|v| s.views[*v].layout.tabs().len() > 1)?;
            Some(match numbered.iter().position(|n| *n == v) {
                Some(k) => k + 1,
                None => {
                    numbered.push(v);
                    numbered.len()
                }
            })
        })
        .collect()
}

fn show_text(s: &Session) -> String {
    if s.is_empty() {
        return "No saved session.".into();
    }
    let focused = s.focused();
    let splits = split_numbers(s);
    s.tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut line = format!(
                "{}{} {}  {}  {}",
                i + 1,
                if focused == Some(i) { " *" } else { "  " },
                t.title.as_deref().map_or("-".into(), printable),
                printable(&t.profile),
                t.dir
                    .as_ref()
                    .map_or("-".into(), |d| printable(&d.display().to_string())),
            );
            if let Some(c) = &t.command {
                line += &format!("  command: {}", printable(c));
            }
            if let Some(f) = t.folder.and_then(|f| s.folders.get(f)) {
                line += &format!("  folder: {}", printable(&f.name));
            }
            if let Some(f) = &t.favorite {
                line += &format!("  favorite: {}", printable(f));
            }
            if let Some(n) = splits[i] {
                line += &format!("  split: {n}");
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A stable object: `active` (the focused tab's position, or null), `tabs` (position,
/// active, title, profile, dir, command, folder position, favorite id, split number) and
/// `folders` (position, name, color, collapsed). Positions are 1-based.
fn show_json(s: &Session) -> String {
    let focused = s.focused();
    let splits = split_numbers(s);
    let tabs: Vec<serde_json::Value> = s
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            serde_json::json!({
                "position": i + 1,
                "active": focused == Some(i),
                "title": t.title,
                "profile": t.profile,
                "dir": t.dir.as_ref().map(|d| d.display().to_string()),
                "command": t.command,
                "folder": t.folder.filter(|f| *f < s.folders.len()).map(|f| f + 1),
                "favorite": t.favorite,
                "split": splits[i],
            })
        })
        .collect();
    let folders: Vec<serde_json::Value> = s
        .folders
        .iter()
        .enumerate()
        .map(|(i, f)| {
            serde_json::json!({
                "position": i + 1,
                "name": f.name,
                "color": f.color,
                "collapsed": f.collapsed,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "active": focused.map(|f| f + 1),
        "tabs": tabs,
        "folders": folders,
    }))
    .expect("json")
}

/// Text from the file made safe for a terminal (control characters escaped).
fn printable(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{Layout, SavedFolder, SavedTab, SavedView, SplitDir};
    use super::*;
    use crate::favorites::tests::temp_dir;

    struct Run {
        code: i32,
        out: String,
        err: String,
    }

    fn run_with(store: &Store, args: &[&str]) -> Run {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &Env { store }, &mut out, &mut err);
        Run {
            code,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    fn store(name: &str) -> Store {
        Store::new(temp_dir(name).join("session.toml"))
    }

    /// Three tabs: "api" (in folder "work", split with the second, focused), a plain one,
    /// and a favorite's tab with a command.
    fn sample() -> Session {
        Session {
            version: 1,
            active: 0,
            tabs: vec![
                SavedTab {
                    profile: "zsh".into(),
                    dir: Some("/src/api".into()),
                    title: Some("api".into()),
                    folder: Some(0),
                    ..SavedTab::default()
                },
                SavedTab {
                    profile: "zsh".into(),
                    dir: Some("/src/web".into()),
                    folder: Some(0),
                    ..SavedTab::default()
                },
                SavedTab {
                    profile: "bash".into(),
                    dir: Some("/var/log".into()),
                    command: Some("tail -f syslog".into()),
                    favorite: Some("f2".into()),
                    ..SavedTab::default()
                },
            ],
            folders: vec![SavedFolder {
                name: "work".into(),
                color: Some(2),
                collapsed: true,
            }],
            views: vec![
                SavedView {
                    focused: 0,
                    name: None,
                    layout: Layout::Split {
                        dir: SplitDir::Horizontal,
                        ratio: 0.5,
                        a: Box::new(Layout::Tab(0)),
                        b: Box::new(Layout::Tab(1)),
                    },
                },
                SavedView {
                    focused: 2,
                    name: None,
                    layout: Layout::Tab(2),
                },
            ],
        }
    }

    #[test]
    fn show_lists_each_tab() {
        let store = store("cli-show");
        store.save(&sample()).unwrap();
        let r = run_with(&store, &["show"]);
        assert_eq!(r.code, 0, "{}", r.err);
        let lines: Vec<&str> = r.out.lines().collect();
        assert_eq!(
            lines,
            [
                "1 * api  zsh  /src/api  folder: work  split: 1",
                "2   -  zsh  /src/web  folder: work  split: 1",
                "3   -  bash  /var/log  command: tail -f syslog  favorite: f2",
            ]
        );
    }

    #[test]
    fn show_json_is_a_stable_object() {
        let store = store("cli-json");
        store.save(&sample()).unwrap();
        let r = run_with(&store, &["show", "--json"]);
        assert_eq!(r.code, 0, "{}", r.err);
        let v: serde_json::Value = serde_json::from_str(&r.out).unwrap();
        assert_eq!(v["active"], 1);
        assert_eq!(v["tabs"].as_array().unwrap().len(), 3);
        assert_eq!(
            v["tabs"][0],
            serde_json::json!({
                "position": 1, "active": true, "title": "api", "profile": "zsh",
                "dir": "/src/api", "command": null, "folder": 1, "favorite": null, "split": 1,
            })
        );
        assert_eq!(v["tabs"][2]["command"], "tail -f syslog");
        assert_eq!(v["tabs"][2]["favorite"], "f2");
        assert_eq!(v["tabs"][2]["split"], serde_json::Value::Null);
        assert_eq!(
            v["folders"],
            serde_json::json!([{ "position": 1, "name": "work", "color": 2, "collapsed": true }])
        );
    }

    #[test]
    fn no_file_is_an_empty_session() {
        let store = store("cli-empty");
        let r = run_with(&store, &["show", "--json"]);
        assert_eq!(r.code, 0);
        let v: serde_json::Value = serde_json::from_str(&r.out).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "active": null, "tabs": [], "folders": [] })
        );
        let r = run_with(&store, &["show"]);
        assert_eq!((r.code, r.out.trim()), (0, "No saved session."));
    }

    #[test]
    fn clear_deletes_and_is_fine_when_absent() {
        let store = store("cli-clear");
        store.save(&sample()).unwrap();
        let r = run_with(&store, &["clear"]);
        assert_eq!(r.code, 0);
        assert!(!store.path.exists());
        let r = run_with(&store, &["clear"]);
        assert_eq!(r.code, 0);
        assert_eq!(r.out.trim(), "no saved session");
    }

    #[test]
    fn path_prints_the_file() {
        let store = store("cli-path");
        let r = run_with(&store, &["path"]);
        assert_eq!(r.code, 0);
        assert_eq!(r.out.trim(), store.path.display().to_string());
    }

    #[test]
    fn a_corrupt_file_is_an_error_with_advice() {
        let store = store("cli-bad");
        std::fs::write(&store.path, "tab = [").unwrap();
        let r = run_with(&store, &["show"]);
        assert_eq!(r.code, 1);
        assert!(r.err.contains("vtt session clear"), "{}", r.err);
        // Reading never moves the file.
        assert!(store.path.exists());
    }

    #[test]
    fn unknown_arguments_are_usage_errors() {
        let store = store("cli-usage");
        for args in [&[][..], &["list"], &["show", "--yaml"], &["clear", "now"]] {
            let r = run_with(&store, args);
            assert_eq!(r.code, 2, "{args:?}");
            assert!(r.err.starts_with("usage: vtt session"));
        }
        assert_eq!(run_with(&store, &["--help"]).code, 0);
    }
}
