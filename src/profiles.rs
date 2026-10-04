//! Shell profile discovery (per OS) plus user-defined profiles from the config.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{Config, ProfileConfig};

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    /// Short glyph shown in the collapsed sidebar.
    pub icon: String,
    pub color: Option<[u8; 3]>,
}

impl Profile {
    fn simple(name: &str, command: impl Into<String>, args: &[&str], icon: &str) -> Self {
        Self {
            name: name.to_string(),
            command: command.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            env: HashMap::new(),
            icon: icon.to_string(),
            color: None,
        }
    }

    fn from_config(p: &ProfileConfig) -> Self {
        Self {
            name: p.name.clone(),
            command: p.command.clone(),
            args: p.args.clone(),
            cwd: p.cwd.clone(),
            env: p.env.clone(),
            icon: p.icon.clone().unwrap_or_else(|| default_icon(&p.name)),
            color: p.color.as_deref().and_then(crate::config::parse_hex),
        }
    }
}

/// Icon derived from the first letter of a name, e.g. "zsh" -> "Z".
fn default_icon(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| ">".into())
}

/// Returns discovered profiles followed by user-configured ones.
/// The default profile (if configured and found) is moved to the front.
pub fn load(config: &Config) -> Vec<Profile> {
    let mut profiles = discover();
    for p in &config.profiles {
        // A user profile with the same name overrides the discovered one.
        profiles.retain(|d| d.name != p.name);
        profiles.push(Profile::from_config(p));
    }
    if let Some(default) = &config.default_profile
        && let Some(i) = profiles.iter().position(|p| &p.name == default)
    {
        let p = profiles.remove(i);
        profiles.insert(0, p);
    }
    if profiles.is_empty() {
        profiles.push(fallback());
    }
    profiles
}

fn fallback() -> Profile {
    if cfg!(windows) {
        Profile::simple("cmd", "cmd.exe", &[], "C")
    } else {
        Profile::simple("sh", "/bin/sh", &[], "$")
    }
}

/// Find an executable on PATH.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

#[cfg(not(windows))]
fn discover() -> Vec<Profile> {
    let mut shells: Vec<String> = Vec::new();
    if let Ok(sh) = std::env::var("SHELL") {
        shells.push(sh);
    }
    if let Ok(text) = std::fs::read_to_string("/etc/shells") {
        shells.extend(parse_etc_shells(&text));
    }

    let mut profiles: Vec<Profile> = Vec::new();
    for path in shells {
        let Some(name) = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        // /etc/shells often lists the same shell under /bin and /usr/bin.
        if profiles.iter().any(|p| p.name == name) || !Path::new(&path).exists() {
            continue;
        }
        // Skip non-interactive helpers.
        if matches!(
            name.as_str(),
            "nologin" | "false" | "git-shell" | "rbash" | "systemd-home-fallback-shell"
        ) {
            continue;
        }
        // Login shell so the user's profile files are sourced (matches other terminals on macOS).
        let args: &[&str] = if cfg!(target_os = "macos") {
            &["-l"]
        } else {
            &[]
        };
        profiles.push(Profile::simple(
            &name,
            path.clone(),
            args,
            &default_icon(&name),
        ));
    }
    if let Some(pwsh) = which("pwsh")
        && !profiles.iter().any(|p| p.name == "pwsh")
    {
        profiles.push(Profile::simple(
            "pwsh",
            pwsh.to_string_lossy(),
            &["-NoLogo"],
            "P",
        ));
    }
    profiles
}

/// Parse `/etc/shells`: one absolute path per line, `#` comments.
#[cfg_attr(windows, allow(dead_code))]
fn parse_etc_shells(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with('/'))
        .map(str::to_owned)
        .collect()
}

#[cfg(windows)]
fn discover() -> Vec<Profile> {
    let mut profiles = Vec::new();

    if let Some(pwsh) = which("pwsh.exe").or_else(|| {
        let pf = std::env::var_os("ProgramFiles")?;
        let p = Path::new(&pf).join("PowerShell").join("7").join("pwsh.exe");
        p.is_file().then_some(p)
    }) {
        profiles.push(Profile::simple(
            "PowerShell",
            pwsh.to_string_lossy(),
            &["-NoLogo"],
            "P",
        ));
    }
    if let Some(ps) = which("powershell.exe") {
        profiles.push(Profile::simple(
            "Windows PowerShell",
            ps.to_string_lossy(),
            &["-NoLogo"],
            "W",
        ));
    }
    profiles.push(Profile::simple("Command Prompt", "cmd.exe", &[], "C"));

    if let Some(bash) = find_git_bash() {
        let mut p = Profile::simple("Git Bash", bash.to_string_lossy(), &["--login", "-i"], "G");
        p.env.insert("CHERE_INVOKING".into(), "1".into()); // Keep the start directory.
        profiles.push(p);
    }

    for distro in wsl_distros() {
        profiles.push(Profile::simple(&distro, "wsl.exe", &["-d", &distro], "L"));
    }
    profiles
}

#[cfg(windows)]
fn find_git_bash() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    // `git.exe` usually lives in <git>\cmd\git.exe; bash is <git>\bin\bash.exe.
    if let Some(git) = which("git.exe")
        && let Some(root) = git.parent().and_then(Path::parent)
    {
        candidates.push(root.join("bin").join("bash.exe"));
    }
    for var in [
        "ProgramFiles",
        "ProgramW6432",
        "ProgramFiles(x86)",
        "LOCALAPPDATA",
    ] {
        if let Some(base) = std::env::var_os(var) {
            candidates.push(Path::new(&base).join("Git").join("bin").join("bash.exe"));
            candidates.push(
                Path::new(&base)
                    .join("Programs")
                    .join("Git")
                    .join("bin")
                    .join("bash.exe"),
            );
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

#[cfg(windows)]
fn wsl_distros() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("wsl.exe")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    // wsl.exe prints UTF-16LE.
    let units: Vec<u16> = out
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&b| u16::from_le_bytes(b))
        .collect();
    String::from_utf16_lossy(&units)
        .lines()
        .map(|l| l.trim().trim_matches('\0').to_string())
        .filter(|l| !l.is_empty() && !l.starts_with("docker-desktop"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etc_shells_parsing() {
        let text = "# comment\n/bin/sh\n\n  /usr/bin/fish \nnot-a-path\n";
        assert_eq!(parse_etc_shells(text), vec!["/bin/sh", "/usr/bin/fish"]);
    }

    #[test]
    fn user_profiles_override_and_default_first() {
        let mut cfg = Config::default();
        cfg.profiles.push(ProfileConfig {
            name: "custom".into(),
            command: "htop".into(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            icon: None,
            color: Some("#ff0000".into()),
        });
        cfg.default_profile = Some("custom".into());
        let profiles = load(&cfg);
        assert_eq!(profiles[0].name, "custom");
        assert_eq!(profiles[0].icon, "C");
        assert_eq!(profiles[0].color, Some([255, 0, 0]));
    }
}
