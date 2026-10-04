//! Syntax highlighting for file previews, colored from the terminal palette so code looks
//! like it does in your shell tools and follows theme changes.
//!
//! The theme is generated as a `.tmTheme` document: syntect loads it for code previews, and
//! the markdown viewer registers the same document for fenced code blocks.

use std::path::Path;

use eframe::egui::Color32;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::render::Palette;

/// Name the generated theme is registered under.
pub const THEME_NAME: &str = "vtt";

/// A run of `len` bytes drawn in one style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub len: usize,
    pub color: Color32,
    pub italic: bool,
    pub underline: bool,
}

pub struct Highlighter {
    syntaxes: SyntaxSet,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            syntaxes: syntaxes(),
        }
    }

    /// Name of the syntax for `path`, by extension, file name or first line.
    pub fn syntax_for(&self, path: &Path, first_line: &str) -> Option<String> {
        let ss = &self.syntaxes;
        let name = path.file_name()?.to_string_lossy();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        // A few common files the bundled grammars don't cover, mapped to a close relative.
        let alias = match ext.as_str() {
            "jsonc" | "json5" | "geojson" | "ipynb" => Some("json"),
            "ts" | "tsx" | "mjs" | "cjs" | "jsx" => Some("js"),
            "zsh" | "fish" | "envrc" => Some("sh"),
            "kts" => Some("kt"),
            _ => None,
        };
        let by_name = match name.as_ref() {
            "Dockerfile" | "Containerfile" => ss.find_syntax_by_extension("sh"),
            ".bashrc" | ".zshrc" | ".profile" | ".bash_profile" => {
                ss.find_syntax_by_extension("sh")
            }
            "PKGBUILD" => ss.find_syntax_by_extension("sh"),
            _ => None,
        };
        let found: Option<&SyntaxReference> = by_name
            .or_else(|| ss.find_syntax_by_extension(&name))
            .or_else(|| ss.find_syntax_by_extension(&ext))
            .or_else(|| alias.and_then(|a| ss.find_syntax_by_extension(a)))
            .or_else(|| ss.find_syntax_by_first_line(first_line));
        found
            .filter(|s| s.name != "Plain Text")
            .map(|s| s.name.clone())
    }

    /// Highlight `text` line by line. Returns the spans of each line (without line endings).
    pub fn highlight(&self, text: &str, syntax: &str, theme: &Theme) -> Vec<Vec<Span>> {
        let Some(syntax) = self.syntaxes.find_syntax_by_name(syntax) else {
            return Vec::new();
        };
        let mut h = HighlightLines::new(syntax, theme);
        let mut out = Vec::new();
        for line in LinesWithEndings::from(text) {
            let Ok(ranges) = h.highlight_line(line, &self.syntaxes) else {
                return Vec::new();
            };
            let content_len = line.trim_end_matches(['\n', '\r']).len();
            let mut spans = Vec::with_capacity(ranges.len());
            let mut pos = 0;
            for (style, piece) in ranges {
                let len = piece.len().min(content_len.saturating_sub(pos));
                pos += piece.len();
                if len == 0 {
                    continue;
                }
                let c = style.foreground;
                spans.push(Span {
                    len,
                    color: Color32::from_rgb(c.r, c.g, c.b),
                    italic: style.font_style.contains(FontStyle::ITALIC),
                    underline: style.font_style.contains(FontStyle::UNDERLINE),
                });
            }
            out.push(spans);
        }
        out
    }
}

/// The bundled grammars plus a TOML one (syntect ships none).
fn syntaxes() -> SyntaxSet {
    let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
    match syntect::parsing::SyntaxDefinition::load_from_str(
        include_str!("syntaxes/toml.sublime-syntax"),
        true,
        None,
    ) {
        Ok(toml) => builder.add(toml),
        Err(err) => crate::diag::warn(format!("TOML grammar: {err}")),
    }
    builder.build()
}

/// A `.tmTheme` (plist XML) mapping common scopes to the 16 ANSI colors of `palette`.
pub fn tm_theme(palette: &Palette) -> String {
    let hex = |c: Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
    let ansi = |i: u8| hex(palette.indexed(i));
    let fg = hex(palette.foreground());
    let bg = hex(palette.background());

    // (scope selector, color, font style)
    let rules: Vec<(&str, String, &str)> = vec![
        ("comment, punctuation.definition.comment", ansi(8), "italic"),
        ("string", ansi(2), ""),
        ("constant.character.escape, string.regexp", ansi(6), ""),
        (
            "constant.numeric, constant.language, constant.other",
            ansi(3),
            "",
        ),
        ("keyword, storage, keyword.operator.word", ansi(5), ""),
        ("keyword.operator, punctuation.separator", fg.clone(), ""),
        (
            "entity.name.function, support.function, meta.function-call variable.function",
            ansi(4),
            "",
        ),
        (
            "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, support.type, support.class, storage.type.primitive",
            ansi(6),
            "",
        ),
        ("entity.name.tag", ansi(1), ""),
        ("entity.other.attribute-name", ansi(3), ""),
        ("variable.parameter", fg.clone(), "italic"),
        ("variable.language, support.variable", ansi(1), ""),
        ("entity.name.namespace, entity.name.module", ansi(3), ""),
        (
            "meta.attribute, meta.annotation, entity.name.decorator",
            ansi(3),
            "",
        ),
        ("meta.preprocessor, keyword.control.import", ansi(5), ""),
        // JSON/YAML/TOML keys stand out from string values.
        (
            "meta.mapping.key string, meta.structure.dictionary.key string, support.type.property-name, entity.name.tag.yaml, meta.object-literal.key, entity.name.key",
            ansi(4),
            "",
        ),
        ("markup.heading, entity.name.section", ansi(4), "bold"),
        ("markup.bold", fg.clone(), "bold"),
        ("markup.italic", fg.clone(), "italic"),
        (
            "markup.underline.link, markup.underline",
            ansi(4),
            "underline",
        ),
        ("markup.raw, markup.inline.raw", ansi(2), ""),
        ("markup.quote", ansi(8), "italic"),
        ("markup.inserted", ansi(2), ""),
        ("markup.deleted", ansi(1), ""),
        ("markup.changed", ansi(3), ""),
        ("meta.diff.header, meta.diff.range", ansi(4), ""),
        ("invalid", ansi(1), "underline"),
    ];

    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "\n",
        r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">"#,
        "\n<plist version=\"1.0\"><dict><key>name</key><string>vtt</string><key>settings</key><array>\n",
    ));
    let selection = hex(palette.indexed(8));
    xml.push_str(&format!(
        "<dict><key>settings</key><dict>\
         <key>background</key><string>{bg}</string>\
         <key>foreground</key><string>{fg}</string>\
         <key>caret</key><string>{fg}</string>\
         <key>selection</key><string>{selection}</string>\
         </dict></dict>\n"
    ));
    for (scope, color, style) in rules {
        xml.push_str(&format!(
            "<dict><key>scope</key><string>{scope}</string><key>settings</key><dict>\
             <key>foreground</key><string>{color}</string>\
             <key>fontStyle</key><string>{style}</string></dict></dict>\n"
        ));
    }
    xml.push_str("</array></dict></plist>\n");
    xml
}

/// Parse a document made by [`tm_theme`].
pub fn load_theme(xml: &str) -> Option<Theme> {
    ThemeSet::load_from_reader(&mut std::io::Cursor::new(xml.as_bytes())).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme as AppTheme;

    #[test]
    fn generated_theme_loads_and_colors_json_keys() {
        let palette = Palette::from_theme(&AppTheme::default());
        let theme = load_theme(&tm_theme(&palette)).expect("valid tmTheme");
        let h = Highlighter::new();
        let syntax = h
            .syntax_for(Path::new("data.json"), "")
            .expect("json syntax");
        let lines = h.highlight("{\"key\": \"value\", \"n\": 1}\n", &syntax, &theme);
        assert_eq!(lines.len(), 1);
        let total: usize = lines[0].iter().map(|s| s.len).sum();
        assert_eq!(total, "{\"key\": \"value\", \"n\": 1}".len());
        let colors: Vec<Color32> = lines[0].iter().map(|s| s.color).collect();
        assert!(
            colors.contains(&palette.indexed(2)),
            "string values are green"
        );
        assert!(colors.contains(&palette.indexed(4)), "keys are blue");
        assert!(colors.contains(&palette.indexed(3)), "numbers are yellow");
    }

    #[test]
    fn toml_grammar_colors_sections_keys_and_values() {
        let palette = Palette::from_theme(&AppTheme::default());
        let theme = load_theme(&tm_theme(&palette)).unwrap();
        let h = Highlighter::new();
        let text = "[package] # c\nname = \"vtt\"\nopt = { a = true, n = 3 }\n";
        let lines = h.highlight(text, "TOML", &theme);
        let color_of = |line: usize, src: &str, needle: &str| {
            let at = src.find(needle).unwrap();
            let mut pos = 0;
            lines[line]
                .iter()
                .find(|s| {
                    pos += s.len;
                    pos > at
                })
                .unwrap()
                .color
        };
        let l: Vec<&str> = text.lines().collect();
        assert_eq!(color_of(0, l[0], "package"), palette.indexed(4));
        assert_eq!(color_of(0, l[0], "# c"), palette.indexed(8));
        assert_eq!(color_of(1, l[1], "name"), palette.indexed(4));
        assert_eq!(color_of(1, l[1], "vtt"), palette.indexed(2));
        assert_eq!(color_of(2, l[2], "true"), palette.indexed(3));
        assert_eq!(color_of(2, l[2], "3"), palette.indexed(3));
    }

    #[test]
    fn finds_syntaxes() {
        let h = Highlighter::new();
        assert_eq!(
            h.syntax_for(Path::new("main.rs"), "").as_deref(),
            Some("Rust")
        );
        assert_eq!(
            h.syntax_for(Path::new("Cargo.toml"), "").as_deref(),
            Some("TOML")
        );
        assert!(h.syntax_for(Path::new("run"), "#!/bin/bash").is_some());
        assert_eq!(h.syntax_for(Path::new("notes.txt"), "hello"), None);
    }
}
