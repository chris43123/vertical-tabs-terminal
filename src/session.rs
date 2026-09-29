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

use crate::profiles::Profile;

pub type TabId = u64;

/// Forwards terminal events from the PTY thread to the UI thread and wakes egui.
#[derive(Clone)]
pub struct Listener {
    id: TabId,
    tx: Sender<(TabId, Event)>,
    ctx: eframe::egui::Context,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.tx.send((self.id, event));
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
    pub child_pid: Option<u32>,
    /// PTY master fd, used to query the foreground process group.
    #[cfg(unix)]
    pub pty_fd: std::os::fd::RawFd,
}

impl Session {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        id: TabId,
        profile: &Profile,
        scrollback: usize,
        size: GridSize,
        cell_px: (u16, u16),
        cwd: Option<std::path::PathBuf>,
        tx: Sender<(TabId, Event)>,
        ctx: eframe::egui::Context,
    ) -> std::io::Result<Self> {
        let mut env = profile.env.clone();
        env.insert("TERM".into(), "xterm-256color".into());
        env.insert("COLORTERM".into(), "truecolor".into());
        env.insert("TERM_PROGRAM".into(), "vtt".into());

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
        let pty = tty::new(&options, window_size, id)?;

        #[cfg(unix)]
        let (child_pid, pty_fd) = {
            use std::os::fd::AsRawFd;
            (Some(pty.child().id()), pty.file().as_raw_fd())
        };
        #[cfg(windows)]
        let child_pid = pty.child_watcher().pid().map(|p| p.get());

        let listener = Listener { id, tx, ctx };
        let config = term::Config {
            scrolling_history: scrollback,
            ..Default::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, listener.clone())));

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
        })
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

fn window_size(size: GridSize, cell_px: (u16, u16)) -> WindowSize {
    WindowSize {
        num_lines: size.lines as u16,
        num_cols: size.cols as u16,
        cell_width: cell_px.0,
        cell_height: cell_px.1,
    }
}
