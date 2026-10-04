//! File preview contents: what kind of file it is, its text split into lines, and syntax
//! highlighting computed on a worker thread (so a big file never stalls the UI).

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use syntect::highlighting::Theme;

use crate::highlight::{Highlighter, Span};

/// Larger files aren't loaded.
const MAX_TEXT: u64 = 8 * 1024 * 1024;
/// Larger markdown is shown as source (rendering re-parses every frame).
const MAX_MARKDOWN: usize = 512 * 1024;
/// Larger text is shown without highlighting.
const MAX_HIGHLIGHT: usize = 2 * 1024 * 1024;
const MAX_IMAGE: u64 = 64 * 1024 * 1024;

pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];
const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdx", "mkd", "mdown"];

pub struct Text {
    pub text: String,
    /// Byte range of each line, without its line ending.
    pub lines: Vec<Range<usize>>,
    /// syntect syntax name, if highlighting applies.
    pub syntax: Option<String>,
    /// JSON that was reformatted for reading.
    pub pretty_printed: bool,
}

pub enum Body {
    Markdown(Text),
    Code(Text),
    Image,
    Binary(u64),
    TooLarge(u64),
    Error(String),
}

/// Highlighted lines, tagged with the theme generation they were colored for.
type Highlighted = Arc<Mutex<Option<(u64, Arc<Vec<Vec<Span>>>)>>>;

pub struct Preview {
    pub path: PathBuf,
    pub body: Body,
    mtime: Option<SystemTime>,
    /// Bumped on every reload, so cached textures and layouts can be refreshed.
    pub version: u64,
    /// Show markdown as highlighted source instead of rendered.
    pub show_source: bool,
    /// Markdown split into text runs and tables, for rendering.
    pub markdown: Arc<Vec<crate::markdown::Block>>,
    /// Soft-wrapped rows for the current width, when word wrap is on.
    pub wrapped: Option<Wrapped>,
    highlighted: Highlighted,
    /// Theme generation a highlight job was started for.
    requested: Option<u64>,
}

impl Preview {
    pub fn open(path: PathBuf, hl: &Highlighter) -> Self {
        let mtime = modified(&path);
        let body = load(&path, hl);
        Self {
            markdown: blocks(&body),
            wrapped: None,
            path,
            body,
            mtime,
            version: 0,
            show_source: false,
            highlighted: Arc::default(),
            requested: None,
        }
    }

    /// Reload when the file changed on disk. Returns true if it did.
    pub fn reload_if_changed(&mut self, hl: &Highlighter) -> bool {
        let mtime = modified(&self.path);
        if mtime == self.mtime {
            return false;
        }
        self.mtime = mtime;
        self.body = load(&self.path, hl);
        self.markdown = blocks(&self.body);
        self.wrapped = None;
        self.version += 1;
        self.highlighted = Arc::default();
        self.requested = None;
        true
    }

    pub fn text(&self) -> Option<&Text> {
        match &self.body {
            Body::Markdown(t) | Body::Code(t) => Some(t),
            _ => None,
        }
    }

    /// Highlighted lines for theme generation `generation`, if ready. Starts a worker the
    /// first time they're asked for; `ctx` is repainted when it finishes.
    pub fn highlighted(
        &mut self,
        hl: &Arc<Highlighter>,
        theme: &Arc<Theme>,
        generation: u64,
        ctx: &eframe::egui::Context,
    ) -> Option<Arc<Vec<Vec<Span>>>> {
        let text = self.text()?;
        let syntax = text.syntax.clone()?;
        if text.text.len() > MAX_HIGHLIGHT {
            return None;
        }
        let source = text.text.clone();
        let done = self.highlighted.lock().unwrap().clone();
        match done {
            Some((g, lines)) if g == generation => return Some(lines),
            // Keep showing the old colors until the new theme's are ready.
            _ if self.requested == Some(generation) => return done.map(|(_, l)| l),
            _ => {}
        }
        self.requested = Some(generation);
        let (hl, theme, ctx) = (hl.clone(), theme.clone(), ctx.clone());
        let slot = self.highlighted.clone();
        std::thread::Builder::new()
            .name("vtt highlight".into())
            .spawn(move || {
                let lines = hl.highlight(&source, &syntax, &theme);
                *slot.lock().unwrap() = Some((generation, Arc::new(lines)));
                ctx.request_repaint();
            })
            .ok();
        done.map(|(_, l)| l)
    }

    /// Short label for the file type, used as the tab icon.
    pub fn icon(&self) -> &'static str {
        match &self.body {
            Body::Markdown(_) => "md",
            Body::Image => "img",
            Body::Code(t) if t.syntax.as_deref() == Some("JSON") => "{}",
            Body::Code(t) if t.syntax.is_some() => "</>",
            Body::Code(_) => "txt",
            Body::Binary(_) | Body::TooLarge(_) | Body::Error(_) => "?",
        }
    }
}

/// The visual rows of a text when soft-wrapped at `cols` columns.
pub struct Wrapped {
    /// Text version and column count this was computed for.
    pub key: (u64, usize),
    /// Each row: the line it belongs to and the byte range of that line it shows.
    pub rows: Vec<(usize, Range<usize>)>,
}

impl Wrapped {
    pub fn new(text: &Text, version: u64, cols: usize) -> Self {
        let rows = (0..text.lines.len())
            .flat_map(|i| {
                wrap_line(text.line(i), cols)
                    .into_iter()
                    .map(move |r| (i, r))
            })
            .collect();
        Self {
            key: (version, cols),
            rows,
        }
    }
}

/// How far continuation rows of a wrapped line are indented: as far as the line itself is
/// (so wrapped code and lists stay aligned), up to half the row.
pub fn hanging_indent(line: &str, cols: usize) -> usize {
    let indent = line.chars().take_while(|c| *c == ' ').count();
    indent.min(cols / 2)
}

/// Split a line into rows of at most `cols` characters, breaking after the last space that
/// fits (word wrap), or mid-word when a single word is longer than a row. Rows after the
/// first are narrower by the line's [`hanging_indent`].
pub fn wrap_line(line: &str, cols: usize) -> Vec<Range<usize>> {
    let cols = cols.max(1);
    let indent = hanging_indent(line, cols);
    // Byte offset of every char, plus the end.
    let bounds: Vec<usize> = line
        .char_indices()
        .map(|(i, _)| i)
        .chain([line.len()])
        .collect();
    let chars = bounds.len() - 1;
    let mut rows = Vec::new();
    let mut start = 0; // char index
    let mut width = cols;
    while chars - start > width {
        let limit = start + width;
        // Break after the last whitespace within the row, if there is one.
        let brk = (start + 1..=limit)
            .rev()
            .find(|&c| line[bounds[c - 1]..bounds[c]].starts_with(char::is_whitespace))
            .unwrap_or(limit);
        rows.push(bounds[start]..bounds[brk]);
        start = brk;
        width = cols - indent;
    }
    rows.push(bounds[start]..line.len());
    rows
}

fn blocks(body: &Body) -> Arc<Vec<crate::markdown::Block>> {
    Arc::new(match body {
        Body::Markdown(t) => crate::markdown::split(&t.text),
        _ => Vec::new(),
    })
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn load(path: &Path, hl: &Highlighter) -> Body {
    let size = match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => return Body::Error("This is a folder.".into()),
        Ok(m) => m.len(),
        Err(err) => return Body::Error(err.to_string()),
    };
    let ext = extension(path);
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return if size <= MAX_IMAGE {
            Body::Image
        } else {
            Body::TooLarge(size)
        };
    }
    if size > MAX_TEXT {
        return Body::TooLarge(size);
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(err) => return Body::Error(err.to_string()),
    };
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return Body::Binary(size);
    }
    let text = normalize(&String::from_utf8_lossy(&bytes));
    let first_line = text.lines().next().unwrap_or_default();
    let syntax = hl.syntax_for(path, first_line);

    if MARKDOWN_EXTENSIONS.contains(&ext.as_str()) && text.len() <= MAX_MARKDOWN {
        return Body::Markdown(Text::new(text, syntax.or(Some("Markdown".into()))));
    }
    let (text, pretty_printed) = match syntax.as_deref() {
        Some("JSON") if is_minified(&text) => match pretty_json(&text) {
            Some(pretty) => (pretty, true),
            None => (text, false),
        },
        _ => (text, false),
    };
    let mut t = Text::new(text, syntax);
    t.pretty_printed = pretty_printed;
    Body::Code(t)
}

impl Text {
    fn new(text: String, syntax: Option<String>) -> Self {
        let mut lines = Vec::new();
        let mut start = 0;
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                lines.push(start..i);
                start = i + 1;
            }
        }
        if start < text.len() || lines.is_empty() {
            lines.push(start..text.len());
        }
        Self {
            text,
            lines,
            syntax,
            pretty_printed: false,
        }
    }

    pub fn line(&self, i: usize) -> &str {
        &self.text[self.lines[i].clone()]
    }
}

/// Unix line endings, tabs expanded to 4-column stops.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let mut col = 0;
        for c in line.chars() {
            if c == '\t' {
                let n = 4 - col % 4;
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            } else {
                out.push(c);
                col += 1;
            }
        }
        out.push('\n');
    }
    if !text.ends_with('\n') {
        out.pop();
    }
    out
}

/// One or two very long lines: generated or minified JSON.
fn is_minified(text: &str) -> bool {
    let lines = text.lines().count().max(1);
    lines <= 3 && text.len() > 200
}

fn pretty_json(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    serde_json::to_string_pretty(&value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, contents: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("vtt-preview-{}-{name}", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn wraps_at_word_boundaries() {
        let rows = |s: &str, cols| -> Vec<String> {
            wrap_line(s, cols)
                .into_iter()
                .map(|r| s[r].to_string())
                .collect()
        };
        assert_eq!(rows("short", 10), ["short"]);
        assert_eq!(rows("", 10), [""]);
        assert_eq!(rows("hello wide world", 10), ["hello ", "wide world"]);
        // A word longer than a row is split.
        assert_eq!(rows("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        // Multi-byte characters count once.
        assert_eq!(rows("ñññ ñññ", 4), ["ñññ ", "ñññ"]);
        // Continuation rows leave room for the line's indent.
        assert_eq!(rows("    aa bb cc dd", 10), ["    aa bb ", "cc dd"]);
        assert_eq!(hanging_indent("    x", 10), 4);
        assert_eq!(hanging_indent("          x", 10), 5);

        let text = Text::new("one two three\nx".into(), None);
        let w = Wrapped::new(&text, 7, 8);
        let lines: Vec<usize> = w.rows.iter().map(|(l, _)| *l).collect();
        assert_eq!(lines, [0, 0, 1]);
        assert_eq!(w.key, (7, 8));
    }

    #[test]
    fn splits_lines_and_expands_tabs() {
        let t = Text::new(normalize("a\tb\r\n\nlast"), None);
        assert_eq!(t.lines.len(), 3);
        assert_eq!(t.line(0), "a   b");
        assert_eq!(t.line(1), "");
        assert_eq!(t.line(2), "last");
        let t = Text::new(normalize("x\n"), None);
        assert_eq!(t.lines.len(), 1);
    }

    #[test]
    fn detects_kinds() {
        let hl = Highlighter::new();
        let md = write("a.md", b"# Title\n");
        assert!(matches!(
            Preview::open(md.clone(), &hl).body,
            Body::Markdown(_)
        ));
        let bin = write("a.bin", b"\x7fELF\0\0\x01");
        assert!(matches!(
            Preview::open(bin.clone(), &hl).body,
            Body::Binary(_)
        ));
        let png = write("a.png", b"not really");
        assert!(matches!(Preview::open(png.clone(), &hl).body, Body::Image));
        let missing = Preview::open(PathBuf::from("/no/such/file"), &hl);
        assert!(matches!(missing.body, Body::Error(_)));
        for p in [md, bin, png] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn pretty_prints_minified_json_keeping_key_order() {
        let hl = Highlighter::new();
        let body = format!(
            r#"{{"zeta":1,"alpha":[1,2,3],"pad":"{}"}}"#,
            "x".repeat(200)
        );
        let path = write("min.json", body.as_bytes());
        let preview = Preview::open(path.clone(), &hl);
        let Body::Code(t) = &preview.body else {
            panic!("expected code");
        };
        assert!(t.pretty_printed);
        assert_eq!(t.line(1).trim(), r#""zeta": 1,"#);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reloads_when_changed() {
        let hl = Highlighter::new();
        let path = write("live.txt", b"one");
        let mut p = Preview::open(path.clone(), &hl);
        assert!(!p.reload_if_changed(&hl));
        std::fs::write(&path, b"one\ntwo").unwrap();
        p.mtime = None; // mtime granularity can hide a same-second change
        assert!(p.reload_if_changed(&hl));
        assert_eq!(p.text().unwrap().lines.len(), 2);
        let _ = std::fs::remove_file(path);
    }
}
