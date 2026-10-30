//! `vtt favorites`: list and edit the favorites file without the UI, for scripts and agents.
//! Exit codes: 0 ok, 1 error (the message says what to do), 2 usage.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{Draft, Favorites, Field, Store, default_name};

const USAGE: &str = "usage: vtt favorites <command>\n\n  \
     list [--json]               list favorites (position, id, name, path, profile, command)\n  \
     add [path] [--name N] [--profile P] [--command C]\n                              \
     pin a directory (default: the current one; name: its folder)\n  \
     set <fav> <field> <value>   change name, path, profile or command\n  \
     unset <fav> <field>         clear profile or command\n  \
     move <fav> <position>       move a favorite to a 1-based position\n  \
     rm <fav>                    remove a favorite\n  \
     path                        print the favorites file's path\n\n\
     <fav> is a position (1, 2, …), an id (f3) or an exact name.\n\
     A command is typed into the shell when the favorite opens; it must be one line.";

/// What the commands need from the outside world (so tests can supply a temp file).
pub struct Env<'a> {
    pub store: &'a Store,
    /// Where a relative or omitted `add` path is resolved.
    pub cwd: &'a Path,
    /// Names of the known shell profiles (only computed when a profile is given).
    pub profiles: &'a dyn Fn() -> Vec<String>,
}

/// `vtt favorites …`. Returns the exit code, or `None` when the arguments aren't a
/// favorites command (then the app starts as usual).
pub fn run_cli(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some("favorites") {
        return None;
    }
    crate::diag::set_quiet(true);
    let Some(path) = Store::default_path() else {
        eprintln!("vtt: no data directory on this system");
        return Some(1);
    };
    let store = Store::new(path);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let profiles = || {
        crate::terminal::profiles::load(&crate::config::Config::load())
            .into_iter()
            .map(|p| p.name)
            .collect()
    };
    let env = Env {
        store: &store,
        cwd: &cwd,
        profiles: &profiles,
    };
    Some(run(
        &args[1..],
        &env,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ))
}

/// Run a subcommand (`args` without the leading `favorites`).
pub fn run(args: &[String], env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args[..] {
        ["list"] => list(env, false),
        ["list", "--json"] => list(env, true),
        ["add", ref rest @ ..] => match parse_add(rest) {
            Some(add) => add_cmd(env, add),
            None => return usage(err),
        },
        ["set", fav, field, value] => match Field::parse(field) {
            Some(field) => set_cmd(env, fav, field, Some(value)),
            None => return bad_field(err, field),
        },
        ["unset", fav, field] => match Field::parse(field) {
            Some(field) => set_cmd(env, fav, field, None),
            None => return bad_field(err, field),
        },
        ["move", fav, pos] => match pos.parse::<usize>() {
            Ok(pos) => env
                .store
                .update(|f| {
                    let i = f.resolve(fav)?;
                    f.move_to(i, pos)?;
                    Ok(f.list[pos - 1].clone())
                })
                .map(|(fav, _)| format!("moved {} `{}` to position {pos}", fav.id, fav.name)),
            Err(_) => return usage(err),
        },
        ["rm", fav] => env
            .store
            .update(|f| f.resolve(fav).and_then(|i| f.remove(i)))
            .map(|(fav, _)| format!("removed {} `{}`", fav.id, fav.name)),
        ["path"] => Ok(env.store.path.display().to_string()),
        _ => return usage(err),
    };
    match result {
        Ok(text) => {
            if !text.is_empty() {
                let _ = writeln!(out, "{text}");
            }
            0
        }
        Err(e) => {
            let _ = writeln!(err, "vtt: {e}");
            1
        }
    }
}

fn usage(err: &mut dyn Write) -> i32 {
    let _ = writeln!(err, "{USAGE}");
    2
}

fn bad_field(err: &mut dyn Write, field: &str) -> i32 {
    let _ = writeln!(
        err,
        "vtt: unknown field `{field}` (fields: {})\n\n{USAGE}",
        Field::NAMES
    );
    2
}

#[derive(Debug, Default, PartialEq)]
struct Add {
    path: Option<String>,
    name: Option<String>,
    profile: Option<String>,
    command: Option<String>,
}

/// `[path] [--name N] [--profile P] [--command C]`, also as `--flag=value`. `None` on bad usage.
fn parse_add(args: &[&str]) -> Option<Add> {
    let mut add = Add::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v)),
            _ => (arg, None),
        };
        let slot = match flag {
            "--name" => &mut add.name,
            "--profile" => &mut add.profile,
            "--command" => &mut add.command,
            _ if flag.starts_with("--") => return None,
            _ => {
                if add.path.replace(arg.to_string()).is_some() {
                    return None;
                }
                i += 1;
                continue;
            }
        };
        let value = match inline {
            Some(v) => v,
            None => {
                i += 1;
                *args.get(i)?
            }
        };
        if slot.replace(value.to_string()).is_some() {
            return None;
        }
        i += 1;
    }
    Some(add)
}

fn add_cmd(env: &Env, add: Add) -> Result<String, String> {
    let path = match &add.path {
        Some(p) => env.cwd.join(super::expand_tilde(Path::new(p))),
        None => env.cwd.to_path_buf(),
    };
    let name = add.name.unwrap_or_else(|| {
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        default_name(&canonical)
    });
    let profiles = profiles_for(env, add.profile.is_some());
    let draft = Draft {
        name,
        path,
        profile: add.profile,
        command: add.command,
    };
    let (i, favs) = env.store.update(|f| f.add(draft, &profiles))?;
    let fav = &favs.list[i];
    Ok(format!(
        "added {} `{}` at {} (position {})",
        fav.id,
        fav.name,
        fav.path.display(),
        i + 1
    ))
}

fn set_cmd(env: &Env, key: &str, field: Field, value: Option<&str>) -> Result<String, String> {
    // A relative path is relative to where the command runs, like `add`.
    let joined;
    let value = match (field, value) {
        (Field::Path, Some(v)) => {
            joined = env
                .cwd
                .join(super::expand_tilde(Path::new(v)))
                .display()
                .to_string();
            Some(joined.as_str())
        }
        _ => value,
    };
    let profiles = profiles_for(env, field == Field::Profile && value.is_some());
    let (fav, _) = env.store.update(|f| {
        let i = f.resolve(key)?;
        f.set(i, field, value, &profiles)?;
        Ok(f.list[i].clone())
    })?;
    Ok(format!("updated {} `{}`", fav.id, fav.name))
}

fn profiles_for(env: &Env, needed: bool) -> Vec<String> {
    if needed { (env.profiles)() } else { Vec::new() }
}

fn list(env: &Env, json: bool) -> Result<String, String> {
    let favs = env.store.load()?;
    Ok(if json {
        list_json(&favs)
    } else {
        list_text(&favs)
    })
}

/// A stable array of objects: `id`, `name`, `path`, `profile`, `command`, `position`, `exists`.
fn list_json(favs: &Favorites) -> String {
    let items: Vec<serde_json::Value> = favs
        .list
        .iter()
        .enumerate()
        .map(|(i, f)| {
            serde_json::json!({
                "id": f.id,
                "name": f.name,
                "path": f.path.display().to_string(),
                "profile": f.profile,
                "command": f.command,
                "position": i + 1,
                "exists": f.path.is_dir(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&items).expect("json")
}

fn list_text(favs: &Favorites) -> String {
    if favs.list.is_empty() {
        return "No favorites yet. Add one with `vtt favorites add [path]`.".into();
    }
    favs.list
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut line = format!("{}  {}  {}  {}", i + 1, f.id, f.name, f.path.display());
            if let Some(p) = &f.profile {
                line.push_str(&format!("  profile: {p}"));
            }
            if let Some(c) = &f.command {
                line.push_str(&format!("  command: {c}"));
            }
            if !f.path.is_dir() {
                line.push_str("  (missing)");
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::super::tests::temp_dir;
    use super::*;

    struct Run {
        code: i32,
        out: String,
        err: String,
    }

    struct Fixture {
        root: PathBuf,
        store: Store,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = temp_dir(name);
            for d in ["api", "web", "tmp"] {
                std::fs::create_dir(root.join(d)).unwrap();
            }
            let store = Store::new(root.join("data").join("favorites.toml"));
            Self { root, store }
        }

        fn run_in(&self, cwd: &Path, args: &[&str]) -> Run {
            let profiles = || vec!["zsh".to_string(), "fish".to_string()];
            let env = Env {
                store: &self.store,
                cwd,
                profiles: &profiles,
            };
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = run(&args, &env, &mut out, &mut err);
            Run {
                code,
                out: String::from_utf8(out).unwrap(),
                err: String::from_utf8(err).unwrap(),
            }
        }

        fn run(&self, args: &[&str]) -> Run {
            self.run_in(&self.root, args)
        }

        fn file(&self) -> String {
            std::fs::read_to_string(&self.store.path).unwrap_or_default()
        }

        fn json(&self) -> serde_json::Value {
            let r = self.run(&["list", "--json"]);
            assert_eq!(r.code, 0, "{}", r.err);
            serde_json::from_str(&r.out).unwrap()
        }
    }

    #[test]
    fn add_defaults_to_the_cwd_and_its_name() {
        let fx = Fixture::new("cli-add");
        let r = fx.run_in(&fx.root.join("api"), &["add"]);
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(r.out.contains("added f1 `api`"), "{}", r.out);
        let r = fx.run(&[
            "add",
            "web",
            "--name",
            "🚀 web",
            "--profile=fish",
            "--command",
            "npm run dev",
        ]);
        assert_eq!(r.code, 0, "{}", r.err);
        let json = fx.json();
        assert_eq!(json[0]["path"], fx.root.join("api").display().to_string());
        assert_eq!(json[1]["name"], "🚀 web");
        assert_eq!(json[1]["profile"], "fish");
        assert_eq!(json[1]["command"], "npm run dev");
    }

    #[test]
    fn add_refuses_duplicates_bad_profiles_and_missing_dirs() {
        let fx = Fixture::new("cli-dup");
        assert_eq!(fx.run(&["add", "api"]).code, 0);
        let before = fx.file();
        let r = fx.run(&["add", "api", "--name", "again"]);
        assert_eq!(r.code, 1);
        assert!(r.err.contains("already favorite 1"), "{}", r.err);
        let r = fx.run(&["add", "web", "--profile", "nosuch"]);
        assert_eq!(r.code, 1);
        assert!(r.err.contains("zsh, fish"), "{}", r.err);
        assert_eq!(fx.run(&["add", "nowhere"]).code, 1);
        assert_eq!(fx.file(), before);
    }

    #[test]
    fn list_shape_is_stable() {
        let fx = Fixture::new("cli-list");
        let r = fx.run(&["list"]);
        assert_eq!(r.code, 0);
        assert!(r.out.contains("No favorites"));
        assert_eq!(fx.json(), serde_json::json!([]));
        fx.run(&["add", "api"]);
        fx.run(&["add", "tmp", "--command", "ls"]);
        std::fs::remove_dir(fx.root.join("tmp")).unwrap();
        let json = fx.json();
        let keys: Vec<&str> = json[0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "id", "name", "path", "profile", "command", "position", "exists"
            ]
        );
        assert_eq!(json[0]["id"], "f1");
        assert_eq!(json[0]["profile"], serde_json::Value::Null);
        assert_eq!(json[0]["position"], 1);
        assert_eq!(json[0]["exists"], true);
        assert_eq!(json[1]["exists"], false);
        let text = fx.run(&["list"]).out;
        assert!(text.contains("1  f1  api"), "{text}");
        assert!(text.contains("command: ls  (missing)"), "{text}");
    }

    #[test]
    fn set_unset_move_and_rm() {
        let fx = Fixture::new("cli-edit");
        for d in ["api", "web", "tmp"] {
            assert_eq!(fx.run(&["add", d]).code, 0);
        }
        assert_eq!(fx.run(&["set", "api", "command", "cargo test"]).code, 0);
        assert_eq!(fx.run(&["set", "f2", "profile", "zsh"]).code, 0);
        assert_eq!(fx.run(&["set", "3", "name", "scratch"]).code, 0);
        let json = fx.json();
        assert_eq!(json[0]["command"], "cargo test");
        assert_eq!(json[1]["profile"], "zsh");
        assert_eq!(json[2]["name"], "scratch");
        assert_eq!(fx.run(&["unset", "1", "command"]).code, 0);
        assert_eq!(fx.json()[0]["command"], serde_json::Value::Null);
        assert_eq!(fx.run(&["unset", "1", "name"]).code, 1);
        assert_eq!(fx.run(&["set", "1", "path", "web"]).code, 1);

        assert_eq!(fx.run(&["move", "scratch", "1"]).code, 0);
        let ids: Vec<String> = fx
            .json()
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, ["f3", "f1", "f2"]);
        assert_eq!(fx.run(&["move", "1", "9"]).code, 1);

        let r = fx.run(&["rm", "f1"]);
        assert_eq!(r.code, 0);
        assert!(r.out.contains("removed f1"));
        assert_eq!(fx.json().as_array().unwrap().len(), 2);
        assert_eq!(fx.run(&["rm", "f1"]).code, 1);
    }

    #[test]
    fn ambiguous_names_need_an_id() {
        let fx = Fixture::new("cli-ambiguous");
        fx.run(&["add", "api", "--name", "dev"]);
        fx.run(&["add", "web", "--name", "dev"]);
        let r = fx.run(&["rm", "dev"]);
        assert_eq!(r.code, 1);
        assert!(r.err.contains("f1, f2"), "{}", r.err);
        assert_eq!(fx.json().as_array().unwrap().len(), 2);
    }

    #[test]
    fn commands_with_control_characters_are_refused() {
        let fx = Fixture::new("cli-command");
        fx.run(&["add", "api", "--command", "ls"]);
        let before = fx.file();
        for bad in ["ls\nrm -rf ~", "ls\r", "echo \u{1b}[2J"] {
            let r = fx.run(&["set", "1", "command", bad]);
            assert_eq!(r.code, 1, "{bad:?}");
            assert_eq!(fx.file(), before, "{bad:?}");
            let r = fx.run(&["add", "web", "--command", bad]);
            assert_eq!(r.code, 1, "{bad:?}");
            assert_eq!(fx.file(), before, "{bad:?}");
        }
    }

    #[test]
    fn bad_usage_exits_2() {
        let fx = Fixture::new("cli-usage");
        for args in [
            &[][..],
            &["bogus"],
            &["list", "--yaml"],
            &["add", "a", "b"],
            &["add", "--name"],
            &["add", "--color", "red"],
            &["add", "--name", "a", "--name", "b"],
            &["set", "1", "command"],
            &["set", "1", "colour", "x"],
            &["unset", "1"],
            &["move", "1", "first"],
            &["rm"],
        ] {
            let r = fx.run(args);
            assert_eq!(r.code, 2, "{args:?}");
            assert!(r.err.contains("usage: vtt favorites"), "{args:?}");
        }
        assert!(!fx.store.path.exists());
        let r = fx.run(&["path"]);
        assert_eq!(r.code, 0);
        assert_eq!(r.out.trim(), fx.store.path.display().to_string());
    }

    #[test]
    fn not_a_favorites_command() {
        assert_eq!(run_cli(&["config".into()]), None);
        assert_eq!(run_cli(&[]), None);
    }
}
