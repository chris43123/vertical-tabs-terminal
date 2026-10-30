//! Small file helpers shared by the files vtt writes itself (favorites, session).

use std::path::{Path, PathBuf};

/// Write `contents` to `path` atomically: a temp file in the same directory, then a rename
/// over the old one, so readers see the old file or the new one, never half of it. The
/// directory is created if needed. (`rename` replaces an existing file on Windows too.)
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    use std::io::Write as _;
    let fail = |e: std::io::Error| format!("can't write {}: {e}", path.display());
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(fail)?;
    let tmp = temp_path(path);
    let written = std::fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(contents.as_bytes())?;
        f.sync_all()
    });
    if let Err(e) = written.and_then(|()| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(fail(e));
    }
    Ok(())
}

/// The temp file [`write_atomic`] writes before renaming it over `path`.
pub fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}
