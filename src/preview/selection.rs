//! A text selection in a preview, kept in document coordinates (line, byte offset) rather
//! than on widgets, so it survives virtualized rows, re-wrapping and any scroll distance.
//! Pure bookkeeping: no egui.

use super::Text;

/// A position in a [`Text`]: a line index and a byte offset into that line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub line: usize,
    pub byte: usize,
}

impl Pos {
    pub fn new(line: usize, byte: usize) -> Self {
        Self { line, byte }
    }

    /// The nearest valid position in `t`: an existing line, on a char boundary.
    pub fn clamp(self, t: &Text) -> Self {
        let line = self.line.min(t.lines.len().saturating_sub(1));
        let s = t.line(line);
        let mut byte = self.byte.min(s.len());
        while !s.is_char_boundary(byte) {
            byte -= 1;
        }
        Self { line, byte }
    }
}

/// From `anchor` (where the selection started) to `head` (where it was extended to); `head`
/// may come first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Pos,
    pub head: Pos,
}

impl Selection {
    /// An empty selection at `p`, the start of a drag.
    pub fn caret(p: Pos) -> Self {
        Self { anchor: p, head: p }
    }

    /// Start and end, in document order.
    pub fn range(&self) -> (Pos, Pos) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The selected text, lines joined with `\n`.
    pub fn text(&self, t: &Text) -> String {
        let (a, b) = self.range();
        let (a, b) = (a.clamp(t), b.clamp(t));
        let offset = |p: Pos| t.lines[p.line].start + p.byte;
        t.text[offset(a)..offset(b)].to_string()
    }

    /// The word, run of spaces or run of punctuation at `p` (a double-click).
    pub fn word(t: &Text, p: Pos) -> Self {
        let p = p.clamp(t);
        let s = t.line(p.line);
        // The char under the pointer, or the last one when past the end of the line.
        let Some((at, c)) = s[p.byte..]
            .chars()
            .next()
            .map(|c| (p.byte, c))
            .or_else(|| s[..p.byte].char_indices().next_back())
        else {
            return Self::caret(p);
        };
        let class = char_class(c);
        let start = s[..at]
            .char_indices()
            .rev()
            .take_while(|&(_, c)| char_class(c) == class)
            .last()
            .map_or(at, |(i, _)| i);
        let end = s[at..]
            .char_indices()
            .find(|&(_, c)| char_class(c) != class)
            .map_or(s.len(), |(i, _)| at + i);
        Self {
            anchor: Pos::new(p.line, start),
            head: Pos::new(p.line, end),
        }
    }

    /// Whole lines `a` through `b` (either order; a triple-click when equal). The anchor stays
    /// on line `a`, so a later Shift+click extends from it.
    pub fn lines(t: &Text, a: usize, b: usize) -> Self {
        let last = t.lines.len().saturating_sub(1);
        let (a, b) = (a.min(last), b.min(last));
        let start = |l: usize| Pos::new(l, 0);
        let end = |l: usize| Pos::new(l, t.line(l).len());
        if a <= b {
            Self {
                anchor: start(a),
                head: end(b),
            }
        } else {
            Self {
                anchor: end(a),
                head: start(b),
            }
        }
    }

    /// The whole text.
    pub fn all(t: &Text) -> Self {
        Self::lines(t, 0, t.lines.len().saturating_sub(1))
    }

    /// First and last line the selection covers (0-based, inclusive). A selection ending at
    /// the very start of a line doesn't count that line.
    pub fn line_span(&self, t: &Text) -> (usize, usize) {
        let (a, b) = self.range();
        let (a, b) = (a.clamp(t), b.clamp(t));
        let last = if b.byte == 0 && b.line > a.line {
            b.line - 1
        } else {
            b.line
        };
        (a.line, last)
    }

    /// `path:12` or `path:12-40` (1-based lines), the form editors and agents understand.
    pub fn reference(&self, path: &str, t: &Text) -> String {
        match self.line_span(t) {
            (a, b) if a == b => format!("{path}:{}", a + 1),
            (a, b) => format!("{path}:{}-{}", a + 1, b + 1),
        }
    }
}

/// Words, whitespace and punctuation each select as a run of their own kind.
fn char_class(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0
    } else if c.is_whitespace() {
        1
    } else {
        2
    }
}

/// How close to the body's edge (in points) a selection drag starts scrolling.
pub const AUTOSCROLL_EDGE: f32 = 24.0;
/// Scroll speed, in points per second, per point of overshoot.
const AUTOSCROLL_GAIN: f32 = 18.0;
/// Speed right at the start of the edge zone.
const AUTOSCROLL_BASE: f32 = 60.0;
/// Speed cap, in points per second.
pub const AUTOSCROLL_MAX: f32 = 3000.0;

/// How far to scroll this frame while a selection drag is `overshoot_px` into the edge zone
/// (0 or less is inside the body: no scrolling). Faster the further past the edge, capped.
/// `dt` is the frame time in seconds; long stalls count as 0.1 s so the view doesn't jump.
pub fn autoscroll_step(overshoot_px: f32, dt: f32) -> f32 {
    if overshoot_px <= 0.0 || overshoot_px.is_nan() {
        return 0.0;
    }
    let speed = (AUTOSCROLL_BASE + overshoot_px * AUTOSCROLL_GAIN).min(AUTOSCROLL_MAX);
    speed * dt.clamp(0.0, 0.1)
}

/// Signed autoscroll for one axis: positive toward `max`, negative toward `min`, 0 when the
/// pointer `p` is clear of both edge zones of the span `min..max`.
pub fn autoscroll_axis(p: f32, min: f32, max: f32, dt: f32) -> f32 {
    let edge = AUTOSCROLL_EDGE.min((max - min) / 4.0).max(0.0);
    autoscroll_step(p - (max - edge), dt) - autoscroll_step((min + edge) - p, dt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Text {
        Text::new(s.to_string(), None)
    }

    fn sel(a: (usize, usize), b: (usize, usize)) -> Selection {
        Selection {
            anchor: Pos::new(a.0, a.1),
            head: Pos::new(b.0, b.1),
        }
    }

    #[test]
    fn text_of_single_multi_reversed_and_multibyte_ranges() {
        let t = text("hello world\nsecond line\nthird");
        assert_eq!(sel((0, 6), (0, 11)).text(&t), "world");
        assert_eq!(sel((0, 6), (2, 3)).text(&t), "world\nsecond line\nthi");
        // Head before anchor gives the same text.
        assert_eq!(sel((2, 3), (0, 6)).text(&t), "world\nsecond line\nthi");
        assert_eq!(sel((1, 0), (1, 0)).text(&t), "");
        assert!(sel((1, 2), (1, 2)).is_empty());

        let t = text("ñandú 日本語\n€x");
        // Bytes 0..2 is "ñ"; offsets inside a char snap back to its start.
        assert_eq!(sel((0, 0), (0, 2)).text(&t), "ñ");
        assert_eq!(sel((0, 7), (1, 3)).text(&t), " 日本語\n€");
        assert_eq!(sel((0, 9), (0, 15)).text(&t), "日本"); // 9 is inside 日
        // Out-of-range positions clamp to the text.
        assert_eq!(sel((1, 0), (9, 99)).text(&t), "€x");
    }

    #[test]
    fn word_line_range_all_and_reference() {
        let t = text("let foo_bar = a.b;\n\n  indented\nl3\nl4\nl5\nl6\nl7\nl8\nl9");
        let word = |line, byte| Selection::word(&t, Pos::new(line, byte)).text(&t);
        assert_eq!(word(0, 5), "foo_bar");
        assert_eq!(word(0, 4), "foo_bar");
        assert_eq!(word(0, 0), "let");
        assert_eq!(word(0, 3), " ");
        assert_eq!(word(0, 14), "a");
        assert_eq!(word(0, 15), ".");
        // Past the end of the line: the last run.
        assert_eq!(word(0, 99), ";");
        assert_eq!(word(1, 0), "");
        assert_eq!(word(2, 0), "  ");
        let t2 = text("añb ü");
        assert_eq!(Selection::word(&t2, Pos::new(0, 1)).text(&t2), "añb");

        assert_eq!(Selection::lines(&t, 2, 2).text(&t), "  indented");
        assert_eq!(Selection::lines(&t, 2, 3).text(&t), "  indented\nl3");
        // Reversed keeps the anchor on the first line given.
        let up = Selection::lines(&t, 3, 2);
        assert_eq!(up.text(&t), "  indented\nl3");
        assert_eq!(up.anchor.line, 3);
        assert_eq!(Selection::lines(&t, 0, 99).text(&t), t.text);
        assert_eq!(Selection::all(&t).text(&t), t.text);
        assert_eq!(Selection::all(&text("")).text(&text("")), "");

        assert_eq!(Selection::lines(&t, 2, 2).reference("a.md", &t), "a.md:3");
        assert_eq!(Selection::lines(&t, 2, 8).reference("a.md", &t), "a.md:3-9");
        assert_eq!(Selection::lines(&t, 8, 2).reference("a.md", &t), "a.md:3-9");
        // Ending at the start of a line doesn't include it; a caret names its own line.
        assert_eq!(sel((2, 1), (9, 0)).reference("a.md", &t), "a.md:3-9");
        assert_eq!(sel((2, 1), (2, 1)).reference("a.md", &t), "a.md:3");
    }

    #[test]
    fn autoscroll_is_zero_inside_grows_and_caps() {
        let dt = 1.0 / 60.0;
        assert_eq!(autoscroll_step(0.0, dt), 0.0);
        assert_eq!(autoscroll_step(-50.0, dt), 0.0);
        assert_eq!(autoscroll_step(f32::NAN, dt), 0.0);
        let (a, b, c) = (
            autoscroll_step(1.0, dt),
            autoscroll_step(20.0, dt),
            autoscroll_step(100.0, dt),
        );
        assert!(0.0 < a && a < b && b < c);
        let cap = AUTOSCROLL_MAX * dt;
        assert!((autoscroll_step(1e6, dt) - cap).abs() < 1e-3);
        // A long stall doesn't jump the view.
        assert_eq!(autoscroll_step(1e6, 5.0), AUTOSCROLL_MAX * 0.1);

        // Axis: 0 in the middle, positive near the far edge, negative near the near edge.
        assert_eq!(autoscroll_axis(250.0, 0.0, 500.0, dt), 0.0);
        assert!(autoscroll_axis(490.0, 0.0, 500.0, dt) > 0.0);
        assert!(autoscroll_axis(600.0, 0.0, 500.0, dt) > autoscroll_axis(490.0, 0.0, 500.0, dt));
        assert!(autoscroll_axis(5.0, 0.0, 500.0, dt) < 0.0);
        assert!(autoscroll_axis(-80.0, 0.0, 500.0, dt) < autoscroll_axis(5.0, 0.0, 500.0, dt));
    }
}
