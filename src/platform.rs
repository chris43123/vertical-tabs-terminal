//! Handing files to the operating system.

use std::path::Path;
use std::process::{Command, Stdio};

/// Open a file or folder with the system's default application.
pub fn open_with_default_app(path: &Path) -> Result<(), String> {
    let cmd = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        // `start` opens files with their associated app and folders in Explorer.
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        Command::new("xdg-open")
    };
    spawn_detached(cmd, path).map_err(|e| format!("couldn't open {}: {e}", path.display()))
}

/// Open a file in the system's default *text* editor, whatever its extension is associated with.
pub fn open_in_text_editor(path: &Path) -> Result<(), String> {
    let cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg("-t");
        c
    } else if cfg!(windows) {
        Command::new("notepad.exe")
    } else {
        Command::new("xdg-open")
    };
    spawn_detached(cmd, path).map_err(|e| {
        format!(
            "couldn't open {}: {e} (set `editor` in the config or $EDITOR)",
            path.display()
        )
    })
}

/// Run `cmd path` without tying its stdio to vtt.
fn spawn_detached(mut cmd: Command, path: &Path) -> std::io::Result<()> {
    cmd.arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}
