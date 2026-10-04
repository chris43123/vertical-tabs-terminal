//! Work done every frame or on a timer: draining terminal events from the PTY threads and
//! the once-a-second poll of titles, the files panel and open previews.

use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event as TermEvent, WindowSize};
use alacritty_terminal::vte::ansi::Rgb;

use crate::config::BellMode;
use crate::terminal::procinfo;

use super::App;
use super::tab::Tab;

impl App {
    /// Drain terminal events from all PTY threads.
    pub(super) fn process_events(&mut self) {
        let visible = self.ws.visible();
        let focused = self.ws.focused();
        let mut exited = Vec::new();
        let mut woke = Vec::new();
        let bell_mode = self.config.bell;
        while let Ok((id, event)) = self.rx.try_recv() {
            let Some(tab) = self.tabs.get_mut(&id) else {
                continue;
            };
            match event {
                TermEvent::Wakeup => {
                    if !visible.contains(&id) {
                        tab.activity = true;
                    }
                    if !woke.contains(&id) {
                        woke.push(id);
                    }
                }
                TermEvent::Bell => {
                    if bell_mode != BellMode::None {
                        if focused != Some(id) {
                            tab.bell = true;
                        }
                        // A visual bell shows even in the focused pane.
                        if bell_mode == BellMode::Flash && visible.contains(&id) {
                            tab.bell_flash = Some(Instant::now());
                        }
                    }
                }
                TermEvent::Title(t) => tab.osc_title = Some(t),
                TermEvent::ResetTitle => tab.osc_title = None,
                _ if tab.session().is_none() => {}
                TermEvent::PtyWrite(s) => tab.session().unwrap().write(s.into_bytes()),
                TermEvent::ClipboardStore(_, text) => {
                    if let Some(cb) = &mut self.clipboard {
                        let _ = cb.set_text(text);
                    }
                }
                TermEvent::ClipboardLoad(_, format) => {
                    if let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) {
                        tab.session().unwrap().write(format(&text).into_bytes());
                    }
                }
                TermEvent::ColorRequest(index, format) => {
                    let color = match index {
                        256 => Some(self.palette.foreground()),
                        257 => Some(self.palette.background()),
                        _ => None,
                    };
                    let session = tab.session().unwrap();
                    let from_term = session.term.lock().colors()[index];
                    let rgb = from_term.or(color.map(|c| Rgb {
                        r: c.r(),
                        g: c.g(),
                        b: c.b(),
                    }));
                    if let Some(rgb) = rgb {
                        session.write(format(rgb).into_bytes());
                    }
                }
                TermEvent::TextAreaSizeRequest(format) => {
                    // Report the cell size of this pane's own zoom.
                    let size_pt = tab.font_size(self.font_size);
                    self.fonts
                        .update(&self.ctx, size_pt, self.ctx.pixels_per_point());
                    let (cw, ch) = self.fonts.cell_px();
                    let session = tab.session().unwrap();
                    let size = session.size;
                    let ws = WindowSize {
                        num_lines: size.lines as u16,
                        num_cols: size.cols as u16,
                        cell_width: cw,
                        cell_height: ch,
                    };
                    session.write(format(ws).into_bytes());
                }
                TermEvent::ChildExit(_) | TermEvent::Exit => exited.push(id),
                TermEvent::MouseCursorDirty | TermEvent::CursorBlinkingChange => {}
            }
        }
        for id in exited {
            if self.tabs.contains_key(&id) {
                self.close_tab(id);
            }
        }
        // A `cd` is followed by a new prompt, so the focused shell printing is the moment to
        // check whether its cwd moved. Throttled, since output can arrive every frame.
        if self.files_visible()
            && focused.is_some_and(|f| woke.contains(&f))
            && self.last_cwd_check.elapsed() >= Duration::from_millis(150)
        {
            self.last_cwd_check = Instant::now();
            self.follow_cwd();
            // A command just finished (shell back at its prompt): refresh git right away.
            if let Some(session) = focused
                .and_then(|f| self.tabs.get(&f))
                .and_then(Tab::session)
            {
                let idle = session.shell_idle();
                if idle && !self.shell_was_idle {
                    self.git.tick(&self.ctx, true);
                }
                self.shell_was_idle = idle;
            }
        }
        if self.config.adopt_shell_palette {
            self.adopt_shell_colors(&woke);
        }
    }

    /// About once a second while they're on screen: refresh auto titles, re-list changed
    /// folders in the files panel, and reload previews whose file changed.
    pub(super) fn poll(&mut self) {
        let sidebar_visible = if self.side_hidden {
            self.zen_peek && !self.zen_files
        } else {
            !self.sidebar_collapsed || self.sidebar_peek
        };
        let files_visible = self.files_visible();
        let visible = self.ws.visible();
        let previews_visible = visible
            .iter()
            .any(|id| self.tabs.get(id).is_some_and(|t| t.preview().is_some()));
        if !sidebar_visible && !files_visible && !previews_visible {
            return;
        }
        if self.last_poll.elapsed() >= Duration::from_secs(1) {
            self.last_poll = Instant::now();
            if sidebar_visible {
                for tab in self.tabs.values_mut() {
                    let Some(session) = tab.session() else {
                        continue;
                    };
                    let info = session.proc_info();
                    tab.auto_title = procinfo::format_title(&info);
                    tab.process = info.process;
                    if info.cwd.is_some() {
                        tab.cwd = info.cwd;
                    }
                }
            }
            if files_visible {
                self.follow_cwd();
                self.files.refresh();
                self.git.tick(&self.ctx, false);
            }
            if previews_visible {
                let hl = self.highlighter();
                for id in &visible {
                    if let Some(p) = self.tabs.get_mut(id).and_then(Tab::preview_mut)
                        && p.reload_if_changed(&hl)
                        && matches!(p.body, crate::preview::Body::Image)
                    {
                        self.ctx.forget_image(&crate::ui::file_uri(&p.path));
                    }
                }
            }
        }
        self.ctx.request_repaint_after(Duration::from_secs(1));
    }
}
