//! What is running in a tab (foreground process + cwd), used for automatic tab titles.

use std::path::{Path, PathBuf};

/// Info about what's running in a tab, for auto titles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProcInfo {
    pub process: Option<String>,
    pub cwd: Option<PathBuf>,
}

/// `child_pid`: the shell's pid. `pty_fd`: PTY master fd (unix only, ignored on Windows).
/// The foreground process group of the PTY is preferred, falling back to `child_pid`.
#[cfg(unix)]
pub fn query(child_pid: Option<u32>, pty_fd: i64) -> ProcInfo {
    let fg = if pty_fd >= 0 {
        // SAFETY: tcgetpgrp only reads the fd; an invalid fd returns -1.
        let pgrp = unsafe { libc::tcgetpgrp(pty_fd as libc::c_int) };
        (pgrp > 0).then_some(pgrp as u32)
    } else {
        None
    };
    let Some(pid) = fg.or(child_pid) else {
        return ProcInfo::default();
    };
    ProcInfo {
        process: process_name(pid),
        cwd: process_cwd(pid),
    }
}

#[cfg(windows)]
pub fn query(_child_pid: Option<u32>, _pty_fd: i64) -> ProcInfo {
    // Windows titles come from OSC sequences set by the shell.
    ProcInfo::default()
}

/// True when the shell itself is in the foreground (nothing running in it), so typing a
/// command line into it is safe.
#[cfg(unix)]
pub fn shell_in_foreground(child_pid: Option<u32>, pty_fd: i64) -> bool {
    if pty_fd < 0 {
        return false;
    }
    // SAFETY: tcgetpgrp only reads the fd; an invalid fd returns -1.
    let pgrp = unsafe { libc::tcgetpgrp(pty_fd as libc::c_int) };
    child_pid.is_some_and(|pid| pgrp > 0 && pgrp as u32 == pid)
}

#[cfg(windows)]
pub fn shell_in_foreground(_child_pid: Option<u32>, _pty_fd: i64) -> bool {
    // No cheap way to tell on Windows; assume the shell is at its prompt.
    true
}

#[cfg(target_os = "linux")]
fn process_name(pid: u32) -> Option<String> {
    let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(target_os = "linux")]
fn process_cwd(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(target_os = "macos")]
fn process_name(pid: u32) -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: buffer pointer and size are valid for the call.
    let len = unsafe {
        libc::proc_name(
            pid as libc::c_int,
            buf.as_mut_ptr().cast(),
            buf.len() as u32,
        )
    };
    (len > 0).then(|| String::from_utf8_lossy(&buf[..len as usize]).into_owned())
}

#[cfg(target_os = "macos")]
fn process_cwd(pid: u32) -> Option<PathBuf> {
    use std::ffi::CStr;
    use std::mem::{MaybeUninit, size_of};
    use std::os::unix::ffi::OsStrExt;

    let mut info = MaybeUninit::<libc::proc_vnodepathinfo>::zeroed();
    let size = size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    // SAFETY: `info` is a correctly sized, writable buffer for PROC_PIDVNODEPATHINFO.
    let ret = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if ret != size {
        return None;
    }
    // SAFETY: the kernel filled the struct; vip_path is a NUL-terminated MAXPATHLEN buffer.
    let info = unsafe { info.assume_init() };
    let path = unsafe { CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast()) };
    let bytes = path.to_bytes();
    (!bytes.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn process_name(_pid: u32) -> Option<String> {
    None
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn process_cwd(_pid: u32) -> Option<PathBuf> {
    None
}

/// Format a title: "<process> · <cwd>", with `$HOME` shown as `~` and at most the last two
/// path components. Missing parts are skipped.
pub fn format_title(info: &ProcInfo) -> Option<String> {
    format_with_home(info, dirs::home_dir().as_deref())
}

fn format_with_home(info: &ProcInfo, home: Option<&Path>) -> Option<String> {
    let cwd = info.cwd.as_deref().map(|cwd| short_path(cwd, home));
    match (info.process.as_deref(), cwd) {
        (Some(p), Some(c)) => Some(format!("{p} · {c}")),
        (Some(p), None) => Some(p.to_string()),
        (None, Some(c)) => Some(c),
        (None, None) => None,
    }
}

fn short_path(path: &Path, home: Option<&Path>) -> String {
    let (prefix, rel) = match home.and_then(|h| path.strip_prefix(h).ok()) {
        Some(rel) => ("~", rel),
        None => ("", path),
    };
    let parts: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    if parts.is_empty() {
        return if prefix.is_empty() {
            path.to_string_lossy().into_owned()
        } else {
            prefix.to_string()
        };
    }
    let sep = std::path::MAIN_SEPARATOR;
    let tail = parts[parts.len().saturating_sub(2)..].join(&sep.to_string());
    if parts.len() > 2 {
        format!("…{sep}{tail}")
    } else if prefix.is_empty() {
        // Absolute path with at most two components: keep it as is.
        path.to_string_lossy().into_owned()
    } else {
        format!("{prefix}{sep}{tail}")
    }
}

// Every test here needs a Unix process to query.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn info(process: Option<&str>, cwd: Option<&str>) -> ProcInfo {
        ProcInfo {
            process: process.map(str::to_string),
            cwd: cwd.map(PathBuf::from),
        }
    }

    #[cfg(unix)]
    #[test]
    fn formats_titles() {
        let home = Some(Path::new("/home/u"));
        assert_eq!(
            format_with_home(&info(Some("vim"), Some("/home/u")), home).unwrap(),
            "vim · ~"
        );
        assert_eq!(
            format_with_home(&info(Some("fish"), Some("/home/u/src")), home).unwrap(),
            "fish · ~/src"
        );
        assert_eq!(
            format_with_home(&info(None, Some("/home/u/a/b/c")), home).unwrap(),
            "…/b/c"
        );
        assert_eq!(
            format_with_home(&info(Some("zsh"), Some("/etc")), home).unwrap(),
            "zsh · /etc"
        );
        assert_eq!(
            format_with_home(&info(None, Some("/usr/share/doc/x")), home).unwrap(),
            "…/doc/x"
        );
        assert_eq!(
            format_with_home(&info(Some("htop"), None), home).unwrap(),
            "htop"
        );
        assert_eq!(format_with_home(&info(None, None), home), None);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn queries_current_process() {
        let info = query(Some(std::process::id()), -1);
        assert!(info.process.is_some_and(|p| !p.is_empty()));
        assert_eq!(info.cwd, std::env::current_dir().ok());
    }
}
