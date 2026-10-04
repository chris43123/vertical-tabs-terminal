//! Tab icons from what a tab is running: a small dictionary of well-known CLI tools, a folder
//! with the current directory's initial for plain shells, and the process's first letter for
//! anything else. Only glyphs egui's bundled fonts have (no Nerd Font needed).

use std::path::Path;

/// What to draw in a tab's icon square.
#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    /// A folder holding the working directory's initial (`~` for home).
    Folder(String),
    /// A short label, optionally on a brand color.
    Label(String, Option<[u8; 3]>),
}

const SHELLS: &[&str] = &[
    "fish",
    "bash",
    "zsh",
    "sh",
    "dash",
    "ksh",
    "tcsh",
    "csh",
    "nu",
    "elvish",
    "xonsh",
    "pwsh",
    "powershell",
    "cmd",
    "ash",
];

/// Known tools: (process names, label, brand color).
type Tool = (&'static [&'static str], &'static str, Option<[u8; 3]>);

const TOOLS: &[Tool] = &[
    (&["claude"], "*", Some([0xD9, 0x77, 0x57])),
    (&["codex"], "Cx", Some([0x10, 0xA3, 0x7F])),
    (&["gemini"], "G", Some([0x42, 0x85, 0xF4])),
    (&["aider"], "Ai", Some([0x14, 0xB8, 0x14])),
    (
        &["vim", "vi", "nvim", "view"],
        "V",
        Some([0x01, 0x9A, 0x33]),
    ),
    (&["emacs"], "E", Some([0x7F, 0x5A, 0xB6])),
    (
        &["git", "lazygit", "tig", "gitui"],
        "g",
        Some([0xF0, 0x50, 0x32]),
    ),
    (
        &["ssh", "mosh", "mosh-client"],
        "ssh",
        Some([0x2E, 0x7D, 0x6B]),
    ),
    (
        &["docker", "podman", "lazydocker"],
        "Dk",
        Some([0x24, 0x96, 0xED]),
    ),
    (
        &["kubectl", "k9s", "helm", "minikube", "kind"],
        "K8",
        Some([0x32, 0x6C, 0xE5]),
    ),
    (
        &["terraform", "tofu", "terragrunt"],
        "Tf",
        Some([0x84, 0x4F, 0xBA]),
    ),
    (
        &["ansible", "ansible-playbook"],
        "An",
        Some([0xEE, 0x00, 0x00]),
    ),
    (
        &["cargo", "rustc", "rustup"],
        "Rs",
        Some([0xCE, 0x42, 0x2B]),
    ),
    (
        &["python", "python3", "ipython", "uv", "pip", "pip3"],
        "Py",
        Some([0x37, 0x76, 0xAB]),
    ),
    (
        &["node", "npm", "npx", "pnpm", "yarn", "bun", "deno"],
        "JS",
        Some([0x68, 0x9F, 0x38]),
    ),
    (&["go"], "Go", Some([0x00, 0xAD, 0xD8])),
    (
        &["psql", "mysql", "sqlite3", "redis-cli", "mongosh"],
        "DB",
        Some([0x33, 0x67, 0x91]),
    ),
    (&["tmux", "screen", "zellij"], "T", Some([0x1B, 0xB9, 0x1F])),
];

/// The icon for a tab running `process` in `cwd`.
pub fn icon_for(process: Option<&str>, cwd: Option<&Path>, home: Option<&Path>) -> Option<Icon> {
    let name = normalize(process?);
    if let Some((_, label, color)) = TOOLS
        .iter()
        .find(|(names, ..)| names.contains(&name.as_str()))
    {
        return Some(Icon::Label((*label).into(), *color));
    }
    if SHELLS.contains(&name.as_str()) {
        return Some(Icon::Folder(dir_initial(cwd?, home)));
    }
    let first = name.chars().find(|c| c.is_alphanumeric())?;
    Some(Icon::Label(first.to_uppercase().to_string(), None))
}

fn normalize(process: &str) -> String {
    let lower = process.to_lowercase();
    let lower = lower.strip_suffix(".exe").unwrap_or(&lower);
    // python3.12 -> python3, as versioned names are common for interpreters.
    match lower
        .split_once(['.', '-'])
        .filter(|(a, _)| a.starts_with("python") || a.starts_with("pip"))
    {
        Some((base, _)) => base.to_string(),
        None => lower.to_string(),
    }
}

fn dir_initial(cwd: &Path, home: Option<&Path>) -> String {
    if home == Some(cwd) {
        return "~".into();
    }
    cwd.file_name()
        .and_then(|n| n.to_string_lossy().chars().find(|c| c.is_alphanumeric()))
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "/".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(p: &str, cwd: &str) -> Option<Icon> {
        icon_for(Some(p), Some(Path::new(cwd)), Some(Path::new("/home/u")))
    }

    #[test]
    fn shells_show_the_folder_initial() {
        assert_eq!(
            icon("fish", "/home/u/code/vtt"),
            Some(Icon::Folder("V".into()))
        );
        assert_eq!(icon("zsh", "/home/u"), Some(Icon::Folder("~".into())));
        assert_eq!(icon("bash", "/"), Some(Icon::Folder("/".into())));
        assert_eq!(icon("pwsh.exe", "/x/ops"), Some(Icon::Folder("O".into())));
    }

    #[test]
    fn known_tools_and_fallback() {
        assert!(matches!(icon("claude", "/"), Some(Icon::Label(l, Some(_))) if l == "*"));
        assert!(matches!(icon("python3.12", "/"), Some(Icon::Label(l, _)) if l == "Py"));
        assert_eq!(icon("btop", "/"), Some(Icon::Label("B".into(), None)));
        assert_eq!(icon("weirdtool", "/"), Some(Icon::Label("W".into(), None)));
        assert_eq!(icon_for(None, None, None), None);
    }
}
