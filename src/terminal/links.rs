//! Finding the link under the pointer: an OSC 8 hyperlink set by the program, or a URL
//! printed as plain text (followed across soft-wrapped lines).

use std::ops::RangeInclusive;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Direction, Point};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::search::{RegexIter, RegexSearch};

/// URL schemes vtt hands to the OS. Anything else (e.g. `x-man-page:` or an app's custom
/// scheme from an OSC 8 link) could launch arbitrary apps, so it isn't opened.
const SCHEMES: &[&str] = &["https://", "http://", "mailto:", "file://", "ftp://"];

/// Plain-text URLs: a known scheme, then anything that isn't whitespace, a control char or a
/// delimiter that rarely appears unescaped in URLs (same idea as Alacritty's hint regex).
const URL_PATTERN: &str =
    "(https?://|mailto:|file://|ftp://)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>\"\\s{}\\^⟨⟩`\\\\]+";

/// A link and the cells it covers (grid coordinates).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub uri: String,
    pub range: RangeInclusive<Point>,
}

/// The regex used for plain-text URLs. Building it is not free; keep one around.
pub fn url_regex() -> RegexSearch {
    RegexSearch::new(URL_PATTERN).expect("valid URL regex")
}

/// Whether vtt should open `uri` (one of the allowed schemes).
pub fn is_openable(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    SCHEMES.iter().any(|s| lower.starts_with(s))
}

/// The link at `point`, if any. OSC 8 hyperlinks win over text that looks like a URL.
pub fn link_at<T>(term: &Term<T>, point: Point, regex: &mut RegexSearch) -> Option<Link> {
    if point.line < term.topmost_line() || point.line > term.bottommost_line() {
        return None;
    }
    hyperlink_at(term, point).or_else(|| url_at(term, point, regex))
}

fn hyperlink_at<T>(term: &Term<T>, point: Point) -> Option<Link> {
    let grid = term.grid();
    let link = grid[point].hyperlink()?;
    let same = |p: Point| grid[p].hyperlink().is_some_and(|h| h == link);
    // Extend over neighbouring cells with the same link, within the (wrapped) line.
    let (first, last) = (term.line_search_left(point), term.line_search_right(point));
    let mut start = point;
    while start > first {
        let prev = start.sub(term, Boundary::Grid, 1);
        if !same(prev) {
            break;
        }
        start = prev;
    }
    let mut end = point;
    while end < last {
        let next = end.add(term, Boundary::Grid, 1);
        if !same(next) {
            break;
        }
        end = next;
    }
    Some(Link {
        uri: link.uri().to_string(),
        range: start..=end,
    })
}

fn url_at<T>(term: &Term<T>, point: Point, regex: &mut RegexSearch) -> Option<Link> {
    let (first, last) = (term.line_search_left(point), term.line_search_right(point));
    let found =
        RegexIter::new(first, last, Direction::Right, term, regex).find(|m| m.contains(&point))?;
    let (start, end) = (*found.start(), *found.end());
    let text = term.bounds_to_string(start, end);
    let uri = trim_url(&text);
    // Drop the trimmed characters' cells from the end (URLs here are single-width).
    let cut = text.chars().count() - uri.chars().count();
    let end = end.sub(term, Boundary::Grid, cut);
    (point <= end).then(|| Link {
        uri: uri.to_string(),
        range: start..=end,
    })
}

/// Strip what a URL in prose is usually followed by: sentence punctuation, quotes, and
/// closing brackets that have no opening one inside the URL ("(see https://x.org/a)").
fn trim_url(url: &str) -> &str {
    let mut s = url;
    loop {
        let Some(last) = s.chars().next_back() else {
            return s;
        };
        let unbalanced =
            |open, close| last == close && s.matches(open).count() < s.matches(close).count();
        if ".,:;!?'\"*_".contains(last) || unbalanced('(', ')') || unbalanced('[', ']') {
            s = &s[..s.len() - last.len_utf8()];
        } else {
            return s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::index::{Column, Line};
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;

    use crate::terminal::session::GridSize;

    fn term(cols: usize, bytes: &[u8]) -> Term<VoidListener> {
        let size = GridSize { cols, lines: 4 };
        let mut term = Term::new(Config::default(), &size, VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, bytes);
        term
    }

    fn at(line: i32, col: usize) -> Point {
        Point::new(Line(line), Column(col))
    }

    #[test]
    fn trims_prose_punctuation() {
        assert_eq!(trim_url("https://x.org/a."), "https://x.org/a");
        assert_eq!(trim_url("https://x.org/a),"), "https://x.org/a");
        assert_eq!(
            trim_url("https://en.wikipedia.org/wiki/Rust_(language)"),
            "https://en.wikipedia.org/wiki/Rust_(language)"
        );
        assert_eq!(trim_url("https://x.org/?q=1"), "https://x.org/?q=1");
    }

    #[test]
    fn only_safe_schemes_open() {
        assert!(is_openable("https://google.com"));
        assert!(is_openable("HTTP://google.com"));
        assert!(is_openable("mailto:a@b.c"));
        assert!(!is_openable("javascript:alert(1)"));
        assert!(!is_openable("x-man-page://ls"));
        assert!(!is_openable("/etc/passwd"));
    }

    #[test]
    fn finds_a_plain_url_under_the_pointer() {
        let t = term(40, b"see https://google.com/search?q=a. ok");
        let mut re = url_regex();
        let link = link_at(&t, at(0, 10), &mut re).unwrap();
        assert_eq!(link.uri, "https://google.com/search?q=a");
        assert_eq!(link.range, at(0, 4)..=at(0, 32));
        // On the trimmed period and on plain words: nothing.
        assert_eq!(link_at(&t, at(0, 33), &mut re), None);
        assert_eq!(link_at(&t, at(0, 1), &mut re), None);
    }

    #[test]
    fn follows_a_url_across_a_soft_wrap() {
        let t = term(12, b"https://example.org/x y");
        let mut re = url_regex();
        let link = link_at(&t, at(1, 2), &mut re).unwrap();
        assert_eq!(link.uri, "https://example.org/x");
        assert_eq!(link.range, at(0, 0)..=at(1, 8));
    }

    #[test]
    fn osc8_hyperlinks_use_their_target() {
        let t = term(
            40,
            b"go \x1b]8;;https://anthropic.com\x1b\\docs\x1b]8;;\x1b\\ now",
        );
        let mut re = url_regex();
        let link = link_at(&t, at(0, 4), &mut re).unwrap();
        assert_eq!(link.uri, "https://anthropic.com");
        assert_eq!(link.range, at(0, 3)..=at(0, 6));
        assert_eq!(link_at(&t, at(0, 8), &mut re), None);
    }
}
