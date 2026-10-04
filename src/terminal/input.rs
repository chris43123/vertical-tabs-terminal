//! Translate egui input into the byte sequences a terminal application expects
//! (xterm-style encoding).

use alacritty_terminal::term::TermMode;
use eframe::egui::{Key, Modifiers};

/// xterm modifier parameter: 1 + shift + 2*alt + 4*ctrl. `None` when no modifiers.
fn modifier_param(m: Modifiers) -> Option<u8> {
    let v = (m.shift as u8) | ((m.alt as u8) << 1) | ((m.ctrl as u8) << 2);
    (v != 0).then_some(v + 1)
}

/// Encode a non-text key press. Returns `None` for keys that produce text
/// (those arrive separately as `Event::Text`).
pub fn encode_key(key: Key, mods: Modifiers, mode: TermMode) -> Option<Vec<u8>> {
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let param = modifier_param(mods);

    // CSI/SS3 cursor-style keys: arrows, Home, End, F1-F4.
    let cursor_key = |c: u8| -> Vec<u8> {
        match param {
            Some(p) => format!("\x1b[1;{p}{}", c as char).into_bytes(),
            None if app_cursor => vec![0x1b, b'O', c],
            None => vec![0x1b, b'[', c],
        }
    };
    let ss3_key = |c: u8| -> Vec<u8> {
        match param {
            Some(p) => format!("\x1b[1;{p}{}", c as char).into_bytes(),
            None => vec![0x1b, b'O', c],
        }
    };
    // `CSI n ~` keys.
    let tilde_key = |n: u8| -> Vec<u8> {
        match param {
            Some(p) => format!("\x1b[{n};{p}~").into_bytes(),
            None => format!("\x1b[{n}~").into_bytes(),
        }
    };
    let esc_prefix = |mut bytes: Vec<u8>| -> Vec<u8> {
        if mods.alt {
            bytes.insert(0, 0x1b);
        }
        bytes
    };

    let bytes = match key {
        Key::ArrowUp => cursor_key(b'A'),
        Key::ArrowDown => cursor_key(b'B'),
        Key::ArrowRight => cursor_key(b'C'),
        Key::ArrowLeft => cursor_key(b'D'),
        Key::Home => cursor_key(b'H'),
        Key::End => cursor_key(b'F'),
        Key::Insert => tilde_key(2),
        Key::Delete => tilde_key(3),
        Key::PageUp => tilde_key(5),
        Key::PageDown => tilde_key(6),
        Key::F1 => ss3_key(b'P'),
        Key::F2 => ss3_key(b'Q'),
        Key::F3 => ss3_key(b'R'),
        Key::F4 => ss3_key(b'S'),
        Key::F5 => tilde_key(15),
        Key::F6 => tilde_key(17),
        Key::F7 => tilde_key(18),
        Key::F8 => tilde_key(19),
        Key::F9 => tilde_key(20),
        Key::F10 => tilde_key(21),
        Key::F11 => tilde_key(23),
        Key::F12 => tilde_key(24),
        // Shift+Enter has no legacy encoding; ESC CR (what Alt+Enter sends) is the de-facto
        // "newline without submitting" for Claude Code, fish, readline and friends.
        Key::Enter if mods.shift => vec![0x1b, b'\r'],
        Key::Enter => esc_prefix(vec![b'\r']),
        Key::Tab if mods.shift => b"\x1b[Z".to_vec(),
        Key::Tab => esc_prefix(vec![b'\t']),
        Key::Backspace if mods.ctrl => esc_prefix(vec![0x08]),
        Key::Backspace => esc_prefix(vec![0x7f]),
        Key::Escape => vec![0x1b],
        _ if mods.ctrl => return ctrl_code(key).map(esc_prefix),
        _ => return None,
    };
    Some(bytes)
}

/// Ctrl+<key> control codes.
fn ctrl_code(key: Key) -> Option<Vec<u8>> {
    let name = key.name();
    let b = match key {
        Key::Space | Key::Num2 => 0x00,
        Key::OpenBracket | Key::Num3 => 0x1b,
        Key::Backslash | Key::Num4 => 0x1c,
        Key::CloseBracket | Key::Num5 => 0x1d,
        Key::Num6 => 0x1e,
        Key::Minus | Key::Slash | Key::Num7 => 0x1f,
        Key::Num8 => 0x7f,
        _ if name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase() => {
            name.as_bytes()[0] & 0x1f
        }
        _ => return None,
    };
    Some(vec![b])
}

/// Encode typed text; Alt sends an ESC prefix (except on macOS where Option composes characters).
pub fn encode_text(text: &str, mods: Modifiers) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 1);
    if mods.alt && !cfg!(target_os = "macos") {
        out.push(0x1b);
    }
    out.extend_from_slice(text.as_bytes());
    out
}

/// Wrap pasted text in bracketed-paste markers when the application asked for them.
pub fn encode_paste(text: &str, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::BRACKETED_PASTE) {
        // Strip ESC so pasted text can't terminate the bracket early.
        let clean = text.replace('\x1b', "");
        format!("\x1b[200~{clean}\x1b[201~").into_bytes()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left = 0,
    Middle = 1,
    Right = 2,
    WheelUp = 64,
    WheelDown = 65,
}

/// Encode a mouse report. `col`/`line` are 0-based cell coordinates.
/// `motion` marks a drag/move event. Returns `None` when the terminal isn't tracking the mouse.
pub fn encode_mouse(
    button: MouseButton,
    pressed: bool,
    motion: bool,
    col: usize,
    line: usize,
    mods: Modifiers,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if !mode.intersects(TermMode::MOUSE_MODE) {
        return None;
    }
    if motion && !mode.intersects(TermMode::MOUSE_MOTION | TermMode::MOUSE_DRAG) {
        return None;
    }
    let mut code = button as u8;
    if motion {
        code += 32;
    }
    if mods.shift {
        code += 4;
    }
    if mods.alt {
        code += 8;
    }
    if mods.ctrl {
        code += 16;
    }

    if mode.contains(TermMode::SGR_MOUSE) {
        let suffix = if pressed { 'M' } else { 'm' };
        return Some(format!("\x1b[<{code};{};{}{suffix}", col + 1, line + 1).into_bytes());
    }

    // Legacy X10 encoding: releases are reported as button 3, coords capped at 223.
    if !pressed && !motion {
        code = (code & !0b11) | 3;
    }
    let (x, y) = (col.min(222) as u8 + 33, line.min(222) as u8 + 33);
    Some(vec![0x1b, b'[', b'M', 32 + code, x, y])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(shift: bool, alt: bool, ctrl: bool) -> Modifiers {
        Modifiers {
            shift,
            alt,
            ctrl,
            ..Default::default()
        }
    }

    #[test]
    fn shift_enter_sends_esc_cr() {
        let e = encode_key(Key::Enter, m(true, false, false), TermMode::empty());
        assert_eq!(e, Some(vec![0x1b, b'\r']));
    }

    #[test]
    fn arrows() {
        assert_eq!(
            encode_key(Key::ArrowUp, Modifiers::NONE, TermMode::NONE).unwrap(),
            b"\x1b[A"
        );
        assert_eq!(
            encode_key(Key::ArrowUp, Modifiers::NONE, TermMode::APP_CURSOR).unwrap(),
            b"\x1bOA"
        );
        assert_eq!(
            encode_key(Key::ArrowLeft, m(false, false, true), TermMode::NONE).unwrap(),
            b"\x1b[1;5D"
        );
        assert_eq!(
            encode_key(Key::ArrowRight, m(true, true, false), TermMode::NONE).unwrap(),
            b"\x1b[1;4C"
        );
    }

    #[test]
    fn tilde_and_function_keys() {
        assert_eq!(
            encode_key(Key::Delete, Modifiers::NONE, TermMode::NONE).unwrap(),
            b"\x1b[3~"
        );
        assert_eq!(
            encode_key(Key::PageUp, m(true, false, false), TermMode::NONE).unwrap(),
            b"\x1b[5;2~"
        );
        assert_eq!(
            encode_key(Key::F1, Modifiers::NONE, TermMode::NONE).unwrap(),
            b"\x1bOP"
        );
        assert_eq!(
            encode_key(Key::F5, Modifiers::NONE, TermMode::NONE).unwrap(),
            b"\x1b[15~"
        );
    }

    #[test]
    fn control_codes() {
        assert_eq!(
            encode_key(Key::C, m(false, false, true), TermMode::NONE).unwrap(),
            [0x03]
        );
        assert_eq!(
            encode_key(Key::Space, m(false, false, true), TermMode::NONE).unwrap(),
            [0x00]
        );
        assert_eq!(
            encode_key(Key::A, m(false, true, true), TermMode::NONE).unwrap(),
            [0x1b, 0x01]
        );
        assert_eq!(
            encode_key(Key::Backspace, Modifiers::NONE, TermMode::NONE).unwrap(),
            [0x7f]
        );
        assert_eq!(
            encode_key(Key::Tab, m(true, false, false), TermMode::NONE).unwrap(),
            b"\x1b[Z"
        );
        // Plain letters are text, not key sequences.
        assert!(encode_key(Key::A, Modifiers::NONE, TermMode::NONE).is_none());
    }

    #[test]
    fn paste() {
        assert_eq!(encode_paste("a\nb", TermMode::NONE), b"a\rb");
        assert_eq!(
            encode_paste("x\x1b", TermMode::BRACKETED_PASTE),
            b"\x1b[200~x\x1b[201~"
        );
    }

    #[test]
    fn mouse() {
        let sgr = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        assert_eq!(
            encode_mouse(MouseButton::Left, true, false, 0, 0, Modifiers::NONE, sgr).unwrap(),
            b"\x1b[<0;1;1M"
        );
        assert_eq!(
            encode_mouse(MouseButton::Left, false, false, 4, 2, Modifiers::NONE, sgr).unwrap(),
            b"\x1b[<0;5;3m"
        );
        assert!(
            encode_mouse(
                MouseButton::Left,
                true,
                false,
                0,
                0,
                Modifiers::NONE,
                TermMode::NONE
            )
            .is_none()
        );
        let x10 = TermMode::MOUSE_REPORT_CLICK;
        assert_eq!(
            encode_mouse(MouseButton::Left, false, false, 0, 0, Modifiers::NONE, x10).unwrap(),
            [0x1b, b'[', b'M', 35, 33, 33]
        );
    }
}
