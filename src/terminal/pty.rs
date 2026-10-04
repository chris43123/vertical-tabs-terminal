//! The shell's PTY, with output passed through a small filter before alacritty parses it.
//!
//! Color scripts written for urxvt and foot (pywal, end-4's quickshell, ...) send the background
//! as `OSC 11 ; [alpha]#rrggbb`. alacritty rejects the `[alpha]` prefix and drops the whole
//! sequence, so the palette changes but the background doesn't. The filter strips the prefix.

use std::io::{self, Read};
use std::sync::Arc;

use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
use polling::{Event, PollMode, Poller};

pub struct FilteredPty {
    inner: Pty,
    filter: AlphaFilter,
}

impl FilteredPty {
    pub fn new(inner: Pty) -> Self {
        Self {
            inner,
            filter: AlphaFilter::default(),
        }
    }
}

impl EventedReadWrite for FilteredPty {
    type Reader = Self;
    type Writer = <Pty as EventedReadWrite>::Writer;

    unsafe fn register(
        &mut self,
        poll: &Arc<Poller>,
        event: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        unsafe { self.inner.register(poll, event, mode) }
    }
    fn reregister(&mut self, poll: &Arc<Poller>, event: Event, mode: PollMode) -> io::Result<()> {
        self.inner.reregister(poll, event, mode)
    }
    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()> {
        self.inner.deregister(poll)
    }
    fn reader(&mut self) -> &mut Self {
        self
    }
    fn writer(&mut self) -> &mut Self::Writer {
        self.inner.writer()
    }
}

impl EventedPty for FilteredPty {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        self.inner.next_child_event()
    }
}

impl OnResize for FilteredPty {
    fn on_resize(&mut self, size: WindowSize) {
        self.inner.on_resize(size);
    }
}

impl Read for FilteredPty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if let Some(n) = self.filter.drain(buf) {
                return Ok(n);
            }
            let n = self.inner.reader().read(buf)?;
            if n == 0 {
                return Ok(self.filter.flush(buf));
            }
            if self.filter.is_idle() && !buf[..n].contains(&0x1b) {
                return Ok(n);
            }
            // Everything filtered out (e.g. a chunk ending inside `[100`): read on.
            self.filter.feed(&buf[..n]);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    Esc,
    /// After `ESC ]`, `ESC ] 1`, `ESC ] 1 1`.
    Osc(u8),
    /// After `ESC ] 1 1 ;`.
    Param,
    /// Inside `[digits`, held back until we see whether it closes with `]`.
    Alpha,
}

/// Removes `[digits]` right after `ESC ] 11 ;`, across read boundaries.
#[derive(Default)]
struct AlphaFilter {
    state: State,
    held: Vec<u8>,
    out: Vec<u8>,
    pos: usize,
}

impl AlphaFilter {
    fn is_idle(&self) -> bool {
        self.state == State::Ground && self.pos >= self.out.len()
    }

    /// Copy pending output into `buf`; `None` when there is none.
    fn drain(&mut self, buf: &mut [u8]) -> Option<usize> {
        if self.pos >= self.out.len() {
            self.out.clear();
            self.pos = 0;
            return None;
        }
        let n = (self.out.len() - self.pos).min(buf.len());
        buf[..n].copy_from_slice(&self.out[self.pos..self.pos + n]);
        self.pos += n;
        Some(n)
    }

    /// End of input: release anything held back.
    fn flush(&mut self, buf: &mut [u8]) -> usize {
        self.out.append(&mut self.held);
        self.state = State::Ground;
        self.drain(buf).unwrap_or(0)
    }

    fn feed(&mut self, input: &[u8]) {
        for &b in input {
            self.byte(b);
        }
    }

    fn byte(&mut self, b: u8) {
        const ESC: u8 = 0x1b;
        self.state = match (self.state, b) {
            (State::Param, b'[') => {
                self.held.push(b);
                return self.state = State::Alpha;
            }
            (State::Alpha, b'0'..=b'9') if self.held.len() < 4 => {
                self.held.push(b);
                return;
            }
            (State::Alpha, b']') => {
                self.held.clear();
                return self.state = State::Ground;
            }
            (State::Alpha, _) => {
                self.out.append(&mut self.held);
                State::Ground
            }
            (s, _) => s,
        };
        self.out.push(b);
        self.state = match (self.state, b) {
            (State::Esc, b']') => State::Osc(0),
            (State::Osc(0), b'1') => State::Osc(1),
            (State::Osc(1), b'1') => State::Osc(2),
            (State::Osc(2), b';') => State::Param,
            (_, ESC) => State::Esc,
            _ => State::Ground,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(chunks: &[&[u8]]) -> Vec<u8> {
        let mut f = AlphaFilter::default();
        for c in chunks {
            f.feed(c);
        }
        let mut out = std::mem::take(&mut f.out);
        out.append(&mut f.held);
        out
    }

    #[test]
    fn strips_alpha_from_osc_11() {
        assert_eq!(
            run(&[b"a\x1b]11;[100]#1A1A1D\x1b\\b"]),
            b"a\x1b]11;#1A1A1D\x1b\\b"
        );
    }

    #[test]
    fn strips_across_chunks() {
        assert_eq!(
            run(&[b"\x1b]1", b"1;[", b"8", b"0]#fff\x07"]),
            b"\x1b]11;#fff\x07"
        );
    }

    #[test]
    fn leaves_everything_else() {
        for s in [
            &b"\x1b]4;1;[100]#fff\x07"[..],
            b"\x1b]11;#fff\x07",
            b"\x1b]11;[abc]\x07",
            b"\x1b]110;[1]\x07",
            b"\x1b[11;[1]m",
            b"plain [100] text",
        ] {
            assert_eq!(run(&[s]), s, "{}", String::from_utf8_lossy(s));
        }
    }

    #[test]
    fn alacritty_applies_the_filtered_background() {
        use alacritty_terminal::event::VoidListener;
        use alacritty_terminal::term::{Config, Term};
        use alacritty_terminal::vte::ansi::{NamedColor, Processor, Rgb};

        let size = crate::terminal::session::GridSize { cols: 10, lines: 2 };
        let mut term = Term::new(Config::default(), &size, VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, &run(&[b"\x1b]11;[100]#1A1A1D\x1b\\"]));
        let bg = term.colors()[NamedColor::Background];
        assert_eq!(
            bg,
            Some(Rgb {
                r: 0x1a,
                g: 0x1a,
                b: 0x1d
            })
        );
    }
}
