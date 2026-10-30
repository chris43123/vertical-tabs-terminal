//! A tab's launch recipe: how it was started, so Reset (and later session restore) can
//! start it again the same way.

use std::path::{Path, PathBuf};

use crate::favorites::FavId;
use crate::terminal::profiles::Profile;

/// How a terminal tab was launched. Pure data.
#[derive(Clone, Debug, PartialEq)]
pub struct LaunchRecipe {
    /// The shell profile (its name and the resolved command, args and env).
    pub profile: Profile,
    /// Where the shell starts. Resolved when the recipe is built, so a reset starts in the
    /// same place even if the profile or home changed meanwhile.
    pub dir: Option<PathBuf>,
    /// Typed into the shell once it's ready, so aliases, PATH and history apply and quitting
    /// the command leaves a shell behind.
    pub command: Option<String>,
    /// Name the tab starts with.
    pub custom_title: Option<String>,
    /// The favorite it was opened from. While that favorite exists, Reset starts from the
    /// favorite's current recipe instead of this one.
    pub favorite: Option<FavId>,
    /// Left out of the saved session (the settings editor: its program isn't a profile).
    pub ephemeral: bool,
}

impl LaunchRecipe {
    /// A recipe for `profile`, starting in `requested`, else the profile's cwd, else home.
    pub fn new(profile: Profile, requested: Option<PathBuf>) -> Self {
        let dir = resolve_dir(requested, &profile, dirs::home_dir().as_deref());
        Self {
            profile,
            dir,
            command: None,
            custom_title: None,
            favorite: None,
            ephemeral: false,
        }
    }

    pub fn with_title(mut self, title: Option<String>) -> Self {
        self.custom_title = title;
        self
    }

    /// What to write to the shell once it's ready: the command and a carriage return
    /// (Enter), or nothing.
    pub fn typed_input(&self) -> Option<Vec<u8>> {
        let command = self.command.as_deref()?.trim_end_matches(['\r', '\n']);
        (!command.trim().is_empty()).then(|| format!("{command}\r").into_bytes())
    }
}

fn resolve_dir(
    requested: Option<PathBuf>,
    profile: &Profile,
    home: Option<&Path>,
) -> Option<PathBuf> {
    requested
        .or_else(|| profile.cwd.clone())
        .or_else(|| home.map(Path::to_path_buf))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn profile(cwd: Option<&str>) -> Profile {
        Profile {
            name: "zsh".into(),
            command: "/bin/zsh".into(),
            args: vec!["-l".into()],
            cwd: cwd.map(PathBuf::from),
            env: HashMap::new(),
            icon: "Z".into(),
            color: None,
        }
    }

    #[test]
    fn dir_prefers_requested_then_profile_then_home() {
        let home = Some(Path::new("/home/u"));
        assert_eq!(
            resolve_dir(Some("/tmp".into()), &profile(Some("/srv")), home),
            Some(PathBuf::from("/tmp"))
        );
        assert_eq!(
            resolve_dir(None, &profile(Some("/srv")), home),
            Some(PathBuf::from("/srv"))
        );
        assert_eq!(
            resolve_dir(None, &profile(None), home),
            Some(PathBuf::from("/home/u"))
        );
        assert_eq!(resolve_dir(None, &profile(None), None), None);
    }

    #[test]
    fn new_keeps_profile_and_resolves_dir() {
        let r = LaunchRecipe::new(profile(None), Some("/tmp".into()));
        assert_eq!(r.profile, profile(None));
        assert_eq!(r.dir, Some(PathBuf::from("/tmp")));
        assert_eq!(r.command, None);
        assert_eq!(r.custom_title, None);
        assert_eq!(r.favorite, None);
        // Without a requested dir it falls back to home, never stays unresolved when home exists.
        let r = LaunchRecipe::new(profile(None), None);
        assert_eq!(r.dir, dirs::home_dir());
    }

    #[test]
    fn keeps_command_and_title_when_cloned() {
        let mut r = LaunchRecipe::new(profile(Some("/srv")), None).with_title(Some("logs".into()));
        r.command = Some("tail -f app.log".into());
        let copy = r.clone();
        assert_eq!(copy.custom_title.as_deref(), Some("logs"));
        assert_eq!(copy.command.as_deref(), Some("tail -f app.log"));
        assert_eq!(copy.dir, Some(PathBuf::from("/srv")));
        assert_eq!(copy, r);
    }

    #[test]
    fn typed_input_is_command_and_carriage_return() {
        let mut r = LaunchRecipe::new(profile(None), None);
        assert_eq!(r.typed_input(), None);
        r.command = Some("claude".into());
        assert_eq!(r.typed_input().as_deref(), Some(&b"claude\r"[..]));
        // A trailing newline isn't doubled into two Enters.
        r.command = Some("k9s\n".into());
        assert_eq!(r.typed_input().as_deref(), Some(&b"k9s\r"[..]));
        r.command = Some("   ".into());
        assert_eq!(r.typed_input(), None);
    }
}
