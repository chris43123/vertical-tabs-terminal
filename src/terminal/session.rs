//! A terminal session: alacritty's `Term` state machine plus a PTY running the shell
//! on alacritty's IO thread.

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use alacritty_terminal::event::{Event, EventListener, Notify, OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg, Notifier};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{self, Term};
use alacritty_terminal::tty;

use crate::config::{CursorConfig, CursorStyle};
use crate::terminal::procinfo::{self, ProcInfo};
use crate::terminal::profiles::Profile;
use crate::workspace::TabId;

/// Which session an event came from: its tab, and the session's generation within the app.
/// A reset gives the tab a new session under the same id; the generation tells the old
/// session's late events (its exit above all) apart from the new one's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    pub tab: TabId,
    pub generation: u64,
}

/// Forwards terminal events from the PTY thread to the UI thread and wakes egui.
#[derive(Clone)]
pub struct Listener {
    source: Source,
    tx: Sender<(Source, Event)>,
    ctx: eframe::egui::Context,
}

impl Listener {
    pub fn new(source: Source, tx: Sender<(Source, Event)>, ctx: eframe::egui::Context) -> Self {
        Self { source, tx, ctx }
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.tx.send((self.source, event));
        // egui coalesces repaint requests, so a flood of output still renders once per frame.
        self.ctx.request_repaint();
    }
}

/// Grid size in cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub cols: usize,
    pub lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct Session {
    pub term: Arc<FairMutex<Term<Listener>>>,
    notifier: Notifier,
    pub size: GridSize,
    /// PID of the shell (used for auto titles).
    child_pid: Option<u32>,
    /// PTY master fd, used to query the foreground process group.
    #[cfg(unix)]
    pty_fd: std::os::fd::RawFd,
    /// Job object holding the shell and everything it starts, so Reset can end them all.
    #[cfg(windows)]
    job: Option<job::Job>,
    generation: u64,
}

impl Session {
    /// Start `profile`'s shell in `cwd` (else the profile's directory, else home). Terminal
    /// events are delivered through `listener`.
    pub fn spawn(
        profile: &Profile,
        cwd: Option<std::path::PathBuf>,
        term_config: term::Config,
        size: GridSize,
        cell_px: (u16, u16),
        listener: Listener,
    ) -> std::io::Result<Self> {
        let mut env = profile.env.clone();
        env.insert("TERM".into(), "xterm-256color".into());
        env.insert("COLORTERM".into(), "truecolor".into());
        env.insert("TERM_PROGRAM".into(), "vtt".into());
        env.insert(
            "TERM_PROGRAM_VERSION".into(),
            env!("CARGO_PKG_VERSION").into(),
        );
        // Apps started from Finder or the Dock get no locale, so shells would fall back to
        // ASCII. Terminal.app and iTerm2 set a UTF-8 one too.
        if cfg!(target_os = "macos")
            && ["LC_ALL", "LC_CTYPE", "LANG"]
                .iter()
                .all(|v| std::env::var_os(v).is_none())
        {
            env.insert("LANG".into(), "en_US.UTF-8".into());
        }

        // `escape_args` only exists on Windows.
        #[allow(clippy::needless_update)]
        let options = tty::Options {
            shell: Some(tty::Shell::new(
                profile.command.clone(),
                profile.args.clone(),
            )),
            working_directory: cwd.or_else(|| profile.cwd.clone()).or_else(dirs::home_dir),
            drain_on_exit: true,
            env,
            ..Default::default()
        };

        let window_size = window_size(size, cell_px);
        let pty = tty::new(&options, window_size, listener.source.tab)?;

        #[cfg(unix)]
        let (child_pid, pty_fd) = {
            use std::os::fd::AsRawFd;
            (Some(pty.child().id()), pty.file().as_raw_fd())
        };
        #[cfg(windows)]
        let child_pid = pty.child_watcher().pid().map(|p| p.get());
        // alacritty has already started the shell, so a grandchild started in this short
        // window could escape the job.
        #[cfg(windows)]
        let job = child_pid.and_then(job::Job::for_process);
        let generation = listener.source.generation;

        let term = Arc::new(FairMutex::new(Term::new(
            term_config,
            &size,
            listener.clone(),
        )));

        let pty = crate::terminal::pty::FilteredPty::new(pty);
        let event_loop = EventLoop::new(term.clone(), listener, pty, true, false)?;
        let notifier = Notifier(event_loop.channel());
        event_loop.spawn();

        Ok(Self {
            term,
            notifier,
            size,
            child_pid,
            #[cfg(unix)]
            pty_fd,
            #[cfg(windows)]
            job,
            generation,
        })
    }

    /// Generation of this session (see [`Source`]).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// End everything running in this session: the shell, the foreground program and
    /// background jobs. Unix: every process group in the shell's session gets SIGHUP, and
    /// SIGKILL a moment later if it ignored that (from a one-shot thread, so the UI never
    /// waits). Processes that left the session with `setsid` survive by design. Windows: the
    /// job object is terminated. The session itself still has to be dropped.
    pub fn kill_tree(&self) {
        #[cfg(unix)]
        if let Some(pid) = self.child_pid {
            kill::session(pid as libc::pid_t, self.pty_fd);
        }
        #[cfg(windows)]
        if let Some(job) = &self.job {
            job.terminate();
        }
    }

    /// The foreground process and the shell's cwd.
    pub fn proc_info(&self) -> ProcInfo {
        procinfo::query(self.child_pid, self.raw_pty_fd())
    }

    /// The shell itself is in the foreground, i.e. it's at its prompt rather than running
    /// a program.
    pub fn shell_idle(&self) -> bool {
        procinfo::shell_in_foreground(self.child_pid, self.raw_pty_fd())
    }

    fn raw_pty_fd(&self) -> i64 {
        #[cfg(unix)]
        return self.pty_fd as i64;
        #[cfg(not(unix))]
        -1
    }

    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        self.notifier.notify(bytes);
    }

    pub fn resize(&mut self, size: GridSize, cell_px: (u16, u16)) {
        if size == self.size || size.cols == 0 || size.lines == 0 {
            return;
        }
        self.size = size;
        self.term.lock().resize(size);
        self.notifier.on_resize(window_size(size, cell_px));
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Stops the IO thread, which drops the PTY and hangs up the child.
        let _ = self.notifier.0.send(Msg::Shutdown);
    }
}

#[cfg(unix)]
mod kill {
    use std::time::Duration;

    use libc::pid_t;

    /// Time a process gets to exit after SIGHUP before it's killed.
    const GRACE: Duration = Duration::from_millis(500);

    /// Hang up every process group in session `sid` (the shell's pid: alacritty starts it with
    /// `setsid`), plus the terminal's foreground group, then kill what's left after [`GRACE`].
    pub fn session(sid: pid_t, pty_fd: std::os::fd::RawFd) {
        // Never signal init's session, or (killpg with 0) our own group.
        if sid <= 1 {
            return;
        }
        let groups = groups(sid, pty_fd);
        signal(&groups, libc::SIGHUP);
        // SIGCONT so stopped jobs (Ctrl+Z) act on the hangup.
        signal(&groups, libc::SIGCONT);
        std::thread::spawn(move || {
            std::thread::sleep(GRACE);
            // Re-scan: the session id stays reserved while any member lives, so whatever is
            // still in it now is what ignored the hangup.
            signal(&groups_in_session(sid), libc::SIGKILL);
        });
    }

    fn signal(groups: &[pid_t], sig: libc::c_int) {
        for &g in groups {
            // SAFETY: killpg only sends a signal; a stale group just returns ESRCH.
            unsafe { libc::killpg(g, sig) };
        }
    }

    fn groups(sid: pid_t, pty_fd: std::os::fd::RawFd) -> Vec<pid_t> {
        let mut groups = groups_in_session(sid);
        // SAFETY: tcgetpgrp only reads the fd; an invalid fd returns -1.
        let fg = unsafe { libc::tcgetpgrp(pty_fd) };
        groups.extend([fg, sid].into_iter().filter(|g| *g > 0));
        groups.sort_unstable();
        groups.dedup();
        groups
    }

    /// Process groups with a member in session `sid`.
    pub(super) fn groups_in_session(sid: pid_t) -> Vec<pid_t> {
        let mut groups: Vec<pid_t> = all_pids()
            .into_iter()
            // SAFETY: getsid only reads process state; a gone pid returns -1.
            .filter(|&pid| pid > 0 && unsafe { libc::getsid(pid) } == sid)
            // SAFETY: getpgid only reads process state; a gone pid returns -1.
            .map(|pid| unsafe { libc::getpgid(pid) })
            .filter(|&g| g > 0)
            .collect();
        groups.sort_unstable();
        groups.dedup();
        groups
    }

    #[cfg(target_os = "linux")]
    fn all_pids() -> Vec<pid_t> {
        std::fs::read_dir("/proc")
            .map(|dir| {
                dir.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[cfg(target_os = "macos")]
    fn all_pids() -> Vec<pid_t> {
        // SAFETY: a null buffer asks for the number of pids.
        let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        if n <= 0 {
            return Vec::new();
        }
        // Room for processes started between the two calls.
        let mut pids: Vec<pid_t> = vec![0; n as usize + 64];
        let size = (pids.len() * std::mem::size_of::<pid_t>()) as libc::c_int;
        // SAFETY: `pids` is writable for `size` bytes.
        let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) };
        pids.truncate(n.max(0) as usize);
        pids
    }

    /// Elsewhere only the foreground group and the shell's own are signalled.
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn all_pids() -> Vec<pid_t> {
        Vec::new()
    }
}

#[cfg(windows)]
mod job {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    /// A job object that kills its processes when terminated or closed.
    pub struct Job(HANDLE);

    impl Job {
        /// Put process `pid` (and so everything it starts later) in a new job.
        pub fn for_process(pid: u32) -> Option<Self> {
            // SAFETY: plain Win32 calls; every handle is checked and closed on failure.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let job = Job(job);
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    return None;
                }
                let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                if process.is_null() {
                    return None;
                }
                let ok = AssignProcessToJobObject(job.0, process);
                CloseHandle(process);
                (ok != 0).then_some(job)
            }
        }

        pub fn terminate(&self) {
            // SAFETY: the handle is valid until drop.
            unsafe { TerminateJobObject(self.0, 1) };
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle is valid and closed only here.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Terminal options from the config. `default_cursor_style` is what the cursor returns to
/// until (or after) a program sets its own, so the configured shape and blink apply exactly.
pub fn term_config(scrollback: usize, cursor: &CursorConfig) -> term::Config {
    use alacritty_terminal::vte::ansi::{CursorShape, CursorStyle as TermCursor};
    let shape = match cursor.style {
        CursorStyle::Block => CursorShape::Block,
        CursorStyle::Beam => CursorShape::Beam,
        CursorStyle::Underline => CursorShape::Underline,
    };
    term::Config {
        scrolling_history: scrollback,
        default_cursor_style: TermCursor {
            shape,
            blinking: cursor.blink,
        },
        ..Default::default()
    }
}

fn window_size(size: GridSize, cell_px: (u16, u16)) -> WindowSize {
    WindowSize {
        num_lines: size.lines as u16,
        num_cols: size.cols as u16,
        cell_width: cell_px.0,
        cell_height: cell_px.1,
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::os::unix::process::CommandExt;
    use std::time::{Duration, Instant};

    use super::kill;

    #[test]
    fn kill_session_ends_background_jobs_and_ignorers() {
        // A session leader like the shell, with a background job and a child that ignores SIGHUP.
        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.args(["-c", "sleep 300 & (trap '' HUP; sleep 300) & wait"]);
        // SAFETY: setsid is async-signal-safe.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let mut child = cmd.spawn().unwrap();
        let sid = child.id() as libc::pid_t;
        let start = Instant::now();
        while kill::groups_in_session(sid).is_empty() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!kill::groups_in_session(sid).is_empty());

        kill::session(sid, -1);
        let _ = child.wait();
        let start = Instant::now();
        while !kill::groups_in_session(sid).is_empty() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            kill::groups_in_session(sid).is_empty(),
            "processes survived"
        );
    }
}
