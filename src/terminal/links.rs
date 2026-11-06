//! Finding the link under the pointer: an OSC 8 hyperlink set by the program, a URL printed
//! as plain text, or the path of a file that exists. Text links are followed across soft
//! wraps, and across the hard wraps TUIs like Claude Code make themselves.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Direction, Line, Point};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::search::{RegexIter, RegexSearch};

/// URL schemes vtt hands to the OS. Anything else (e.g. `x-man-page:` or an app's custom
/// scheme from an OSC 8 link) could launch arbitrary apps, so it isn't opened.
const SCHEMES: &[&str] = &["https://", "http://", "mailto:", "file://", "ftp://"];

/// Plain-text URLs: a known scheme, then anything that isn't whitespace, a control char or a
/// delimiter that rarely appears unescaped in URLs (same idea as Alacritty's hint regex).
const URL_PATTERN: &str =
    "(https?://|mailto:|file://|ftp://)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>\"\\s{}\\^⟨⟩`\\\\]+";

/// Path-like words: `~/x`, `./x`, `../x`, `/abs/x`, `dir/file.md`, `file.md`, optionally
/// followed by `:line` or `:line:col` (compiler and agent style). Only paths of files that
/// exist count, so ordinary words that match are dropped when resolved.
const PATH_PATTERN: &str = "(~|\\.\\.?)?/?[A-Za-z0-9._@+%~-]+(/[A-Za-z0-9._@+%~-]+)*(:[0-9]+){0,2}";

/// What a link points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Opened by the OS (browser, mail client).
    Url(String),
    /// A local file, shown in a vtt preview.
    File(PathBuf),
}

/// A link and the cells it covers (grid coordinates): one range, or several when the link
/// continues on the next line after a hard wrap (see [`extend_hard_wrap`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub target: Target,
    pub segments: Vec<RangeInclusive<Point>>,
}

impl Link {
    pub fn contains(&self, point: Point) -> bool {
        self.segments.iter().any(|s| s.contains(&point))
    }
}

/// The regexes used to find links in text. Building them is not free; keep one around.
#[derive(Clone, Debug)]
pub struct Matchers {
    url: RegexSearch,
    path: RegexSearch,
}

impl Default for Matchers {
    fn default() -> Self {
        Self {
            url: RegexSearch::new(URL_PATTERN).expect("valid URL regex"),
            path: RegexSearch::new(PATH_PATTERN).expect("valid path regex"),
        }
    }
}

/// Whether vtt should hand `uri` to the OS (one of the allowed schemes).
fn is_openable(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    SCHEMES.iter().any(|s| lower.starts_with(s))
}

/// How many rows a hard-wrapped link is followed across.
const MAX_WRAPPED_ROWS: usize = 8;

/// The link at `point`, if any. OSC 8 hyperlinks win over text that looks like a URL, and
/// URLs over paths. Relative paths are resolved against `cwd`.
pub fn link_at<T>(
    term: &Term<T>,
    point: Point,
    matchers: &mut Matchers,
    cwd: Option<&Path>,
) -> Option<Link> {
    if point.line < term.topmost_line() || point.line > term.bottommost_line() {
        return None;
    }
    continued_from_above(term, point, matchers, cwd, MAX_WRAPPED_ROWS)
        .or_else(|| link_from(term, point, matchers, cwd))
}

/// The link at `point` that starts on its row or, wrapped, on the rows above.
fn link_from<T>(
    term: &Term<T>,
    point: Point,
    matchers: &mut Matchers,
    cwd: Option<&Path>,
) -> Option<Link> {
    if let Some((uri, range)) = hyperlink_at(term, point) {
        return Some(Link {
            target: uri_target(&uri)?,
            segments: vec![range],
        });
    }
    if let Some(found) = match_at(term, point, &mut matchers.url) {
        let (uri, segments) = extend_hard_wrap(term, found);
        return Some(Link {
            target: uri_target(&uri)?,
            segments,
        });
    }
    let found = match_at(term, point, &mut matchers.path)?;
    let (text, segments) = extend_hard_wrap(term, found);
    Some(Link {
        target: Target::File(resolve_path(&text, cwd)?),
        segments,
    })
}

/// When `point` is in the first word of its row and the row above ends at the right edge,
/// the link may have started up there (see [`extend_hard_wrap`]); find it from there.
fn continued_from_above<T>(
    term: &Term<T>,
    point: Point,
    matchers: &mut Matchers,
    cwd: Option<&Path>,
    rows: usize,
) -> Option<Link> {
    if rows == 0 || point.line <= term.topmost_line() {
        return None;
    }
    let first = first_non_blank(term, point.line)?;
    let in_first_word = (first..=point.column.0).all(|c| !is_blank(term, point.line, c));
    if point.column.0 < first || !in_first_word {
        return None;
    }
    let above = point.line - 1i32;
    let last = last_non_blank(term, above).filter(|&c| at_right_edge(term, c))?;
    let from = Point::new(above, Column(last));
    continued_from_above(term, from, matchers, cwd, rows - 1)
        .or_else(|| link_from(term, from, matchers, cwd))
        .filter(|link| link.contains(point))
}

/// Follow a link that a program wrapped itself, with a newline and an indent rather than a
/// terminal soft wrap (Claude Code and other TUIs do this): while the match is the last thing
/// on a row that is full up to the right edge, append the first word of the next row.
fn extend_hard_wrap<T>(
    term: &Term<T>,
    (mut text, first): (String, RangeInclusive<Point>),
) -> (String, Vec<RangeInclusive<Point>>) {
    let mut segments = vec![first];
    let cols = term.columns();
    while segments.len() <= MAX_WRAPPED_ROWS {
        let end = *segments.last().expect("one segment").end();
        if end.line >= term.bottommost_line()
            || !at_right_edge(term, end.column.0)
            || last_non_blank(term, end.line) != Some(end.column.0)
        {
            break;
        }
        let next = end.line + 1i32;
        let Some(start) = first_non_blank(term, next) else {
            break;
        };
        let mut col = start;
        while col < cols {
            let c = term.grid()[next][Column(col)].c;
            if !is_link_char(c) {
                break;
            }
            text.push(c);
            col += 1;
        }
        if col == start {
            break;
        }
        segments.push(Point::new(next, Column(start))..=Point::new(next, Column(col - 1)));
    }
    if segments.len() > 1 {
        // Trailing punctuation on the last piece ("…/69.").
        let cut = text.chars().count() - trim_url(&text).chars().count();
        text.truncate(trim_url(&text).len());
        let last = segments.pop().expect("more than one segment");
        let (start, end) = (*last.start(), *last.end());
        if end.column.0 >= start.column.0 + cut {
            segments.push(start..=Point::new(end.line, end.column - cut));
        }
    }
    (text, segments)
}

/// Columns a hard-wrapped row may stop short of the edge (some TUIs keep a margin).
const EDGE_SLACK: usize = 1;

fn at_right_edge<T>(term: &Term<T>, col: usize) -> bool {
    col + 1 + EDGE_SLACK >= term.columns()
}

fn is_blank<T>(term: &Term<T>, line: Line, col: usize) -> bool {
    matches!(term.grid()[line][Column(col)].c, ' ' | '\0')
}

fn first_non_blank<T>(term: &Term<T>, line: Line) -> Option<usize> {
    (0..term.columns()).find(|&c| !is_blank(term, line, c))
}

fn last_non_blank<T>(term: &Term<T>, line: Line) -> Option<usize> {
    (0..term.columns())
        .rev()
        .find(|&c| !is_blank(term, line, c))
}

/// Characters that can continue a wrapped URL or path: not whitespace, and not the delimiters
/// excluded from [`URL_PATTERN`].
fn is_link_char(c: char) -> bool {
    !c.is_whitespace() && !c.is_control() && !"<>\"{}^⟨⟩`\\".contains(c) && c != '\0'
}

/// A local `file://` URI that exists becomes a preview; other allowed schemes go to the OS.
fn uri_target(uri: &str) -> Option<Target> {
    if let Some(path) = file_uri_path(uri).filter(|p| p.is_file()) {
        return Some(Target::File(path));
    }
    is_openable(uri).then(|| Target::Url(uri.to_string()))
}

/// The path of a `file:///abs/path` or `file://localhost/abs/path` URI, percent-decoded.
fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri
        .get(..7)?
        .eq_ignore_ascii_case("file://")
        .then(|| &uri[7..])?;
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    path.starts_with('/')
        .then(|| PathBuf::from(percent_decode(path)))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The existing file `text` names: `~/` is the home folder, relative paths are under `cwd`,
/// and a trailing `:line[:col]` is ignored.
fn resolve_path(text: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let name = strip_line_suffix(text);
    let path = match name.strip_prefix("~/") {
        Some(rest) => dirs::home_dir()?.join(rest),
        None if Path::new(name).is_absolute() => PathBuf::from(name),
        None => cwd?.join(name),
    };
    path.is_file().then_some(path)
}

/// `src/main.rs:12:5` -> `src/main.rs`.
fn strip_line_suffix(text: &str) -> &str {
    let mut s = text;
    for _ in 0..2 {
        match s.rsplit_once(':') {
            Some((head, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => s = head,
            _ => break,
        }
    }
    s
}

fn hyperlink_at<T>(term: &Term<T>, point: Point) -> Option<(String, RangeInclusive<Point>)> {
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
    Some((link.uri().to_string(), start..=end))
}

/// The match of `regex` covering `point` in its (wrapped) line, with trailing prose
/// punctuation trimmed off the text and the range.
fn match_at<T>(
    term: &Term<T>,
    point: Point,
    regex: &mut RegexSearch,
) -> Option<(String, RangeInclusive<Point>)> {
    let (first, last) = (term.line_search_left(point), term.line_search_right(point));
    let found =
        RegexIter::new(first, last, Direction::Right, term, regex).find(|m| m.contains(&point))?;
    let (start, end) = (*found.start(), *found.end());
    let text = term.bounds_to_string(start, end);
    let trimmed = trim_url(&text);
    // Drop the trimmed characters' cells from the end (links here are single-width).
    let cut = text.chars().count() - trimmed.chars().count();
    let end = end.sub(term, Boundary::Grid, cut);
    (!trimmed.is_empty() && point <= end).then(|| (trimmed.to_string(), start..=end))
}

/// Strip what a link in prose is usually followed by: sentence punctuation, quotes, and
/// closing brackets that have no opening one inside the link ("(see https://x.org/a)").
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

    fn url(s: &str) -> Target {
        Target::Url(s.to_string())
    }

    /// A fresh directory with `docs/plan.md` in it.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vtt-links-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::write(dir.join("docs/plan.md"), "# plan").unwrap();
        dir
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
        let mut m = Matchers::default();
        let link = link_at(&t, at(0, 10), &mut m, None).unwrap();
        assert_eq!(link.target, url("https://google.com/search?q=a"));
        assert_eq!(link.segments, vec![at(0, 4)..=at(0, 32)]);
        // On the trimmed period and on plain words: nothing.
        assert_eq!(link_at(&t, at(0, 33), &mut m, None), None);
        assert_eq!(link_at(&t, at(0, 1), &mut m, None), None);
    }

    #[test]
    fn follows_a_url_across_a_soft_wrap() {
        let t = term(12, b"https://example.org/x y");
        let mut m = Matchers::default();
        let link = link_at(&t, at(1, 2), &mut m, None).unwrap();
        assert_eq!(link.target, url("https://example.org/x"));
        assert_eq!(link.segments, vec![at(0, 0)..=at(1, 8)]);
    }

    #[test]
    fn follows_a_url_a_program_wrapped_with_an_indent() {
        // A 30-column row filled to the edge, then a newline and an indent (Claude Code style).
        let t = term(30, b"go: https://x.org/aaaaaaaaaaaa\r\n  bbbb/cc. next");
        let mut m = Matchers::default();
        let want = url("https://x.org/aaaaaaaaaaaabbbb/cc");
        let segments = vec![at(0, 4)..=at(0, 29), at(1, 2)..=at(1, 8)];
        for point in [at(0, 10), at(1, 2), at(1, 8)] {
            let link = link_at(&t, point, &mut m, None).unwrap();
            assert_eq!(link.target, want, "{point:?}");
            assert_eq!(link.segments, segments, "{point:?}");
        }
        assert_eq!(link_at(&t, at(1, 9), &mut m, None), None);
        assert_eq!(link_at(&t, at(1, 12), &mut m, None), None);
    }

    #[test]
    fn a_url_short_of_the_edge_does_not_take_the_next_line() {
        let t = term(30, b"go: https://x.org/a\r\n  bbbb");
        let mut m = Matchers::default();
        let link = link_at(&t, at(0, 6), &mut m, None).unwrap();
        assert_eq!(link.target, url("https://x.org/a"));
        assert_eq!(link.segments, vec![at(0, 4)..=at(0, 18)]);
        assert_eq!(link_at(&t, at(1, 3), &mut m, None), None);
    }

    #[test]
    fn osc8_hyperlinks_use_their_target() {
        let t = term(
            40,
            b"go \x1b]8;;https://anthropic.com\x1b\\docs\x1b]8;;\x1b\\ now",
        );
        let mut m = Matchers::default();
        let link = link_at(&t, at(0, 4), &mut m, None).unwrap();
        assert_eq!(link.target, url("https://anthropic.com"));
        assert_eq!(link.segments, vec![at(0, 3)..=at(0, 6)]);
        assert_eq!(link_at(&t, at(0, 8), &mut m, None), None);
    }

    #[test]
    fn strips_line_and_column() {
        assert_eq!(strip_line_suffix("src/main.rs:12:5"), "src/main.rs");
        assert_eq!(strip_line_suffix("src/main.rs:12"), "src/main.rs");
        assert_eq!(strip_line_suffix("src/main.rs"), "src/main.rs");
        assert_eq!(strip_line_suffix("a:b"), "a:b");
    }

    #[test]
    fn decodes_file_uris() {
        assert_eq!(
            file_uri_path("file:///tmp/a%20b.md"),
            Some(PathBuf::from("/tmp/a b.md"))
        );
        assert_eq!(
            file_uri_path("FILE://localhost/tmp/x"),
            Some(PathBuf::from("/tmp/x"))
        );
        assert_eq!(file_uri_path("file://server/share"), None);
        assert_eq!(file_uri_path("https://x.org"), None);
    }

    #[cfg(unix)]
    #[test]
    fn finds_existing_files_by_relative_and_absolute_path() {
        let dir = scratch("paths");
        let mut m = Matchers::default();

        let t = term(60, b"wrote docs/plan.md:3, see notes.md.");
        let link = link_at(&t, at(0, 8), &mut m, Some(&dir)).unwrap();
        assert_eq!(link.target, Target::File(dir.join("docs/plan.md")));
        assert_eq!(link.segments, vec![at(0, 6)..=at(0, 19)]);
        // Words and files that don't exist aren't links.
        assert_eq!(link_at(&t, at(0, 1), &mut m, Some(&dir)), None);
        assert_eq!(link_at(&t, at(0, 28), &mut m, Some(&dir)), None);
        // Relative paths need a directory to resolve against.
        assert_eq!(link_at(&t, at(0, 8), &mut m, None), None);

        let abs = format!("`{}`", dir.join("docs/plan.md").display());
        let t = term(200, abs.as_bytes());
        let link = link_at(&t, at(0, 3), &mut m, None).unwrap();
        assert_eq!(link.target, Target::File(dir.join("docs/plan.md")));
        assert_eq!(link.segments, vec![at(0, 1)..=at(0, abs.len() - 2)]);

        let uri = format!("file://{}", dir.join("docs/plan.md").display());
        let t = term(200, uri.as_bytes());
        let link = link_at(&t, at(0, 3), &mut m, None).unwrap();
        assert_eq!(link.target, Target::File(dir.join("docs/plan.md")));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
