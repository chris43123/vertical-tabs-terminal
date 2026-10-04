//! Everything about running a shell: the PTY and alacritty session, profile discovery,
//! keyboard/mouse encoding and foreground-process inspection.

pub mod input;
pub mod procinfo;
pub mod profiles;
pub mod pty;
pub mod session;

use std::path::Path;

/// Quote a path for pasting into a shell.
pub fn shell_quote(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        format!("\"{s}\"")
    } else if s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+~,@%".contains(c))
    {
        s.into_owned()
    } else {
        // Works in sh, bash, zsh and fish: close the quote, add an escaped quote, reopen.
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn quotes_for_shells() {
        assert_eq!(shell_quote(Path::new("/tmp/a-b.txt")), "/tmp/a-b.txt");
        assert_eq!(shell_quote(Path::new("/tmp/my file")), "'/tmp/my file'");
        assert_eq!(shell_quote(Path::new("it's")), r"'it'\''s'");
    }
}
