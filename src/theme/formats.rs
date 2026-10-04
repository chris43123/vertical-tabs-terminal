//! Reading themes: vtt TOML, kitty's `key value` format, and colors a shell set via OSC.

use std::path::Path;

use crate::config::parse_hex;

use super::{Patch, Rgb};

/// Load a theme file: `.toml` is vtt's format, anything else is parsed as kitty's.
pub(super) fn load_file(path: &Path) -> Result<Patch, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if path.extension().is_some_and(|e| e == "toml") {
        let (patch, warnings) = parse_toml(&text)?;
        for w in warnings {
            crate::diag::warn(format!("{}: {w}", path.display()));
        }
        Ok(patch)
    } else {
        Ok(parse_kitty(&text))
    }
}

/// vtt TOML theme: `foreground`, `background`, `cursor`, `selection`, `ansi = [16 colors]`,
/// `colorN` for any palette index, and `[ui]` with `accent` / `sidebar`.
pub(super) fn parse_toml(text: &str) -> Result<(Patch, Vec<String>), String> {
    let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    patch_from_table(&table)
}

pub(super) fn patch_from_table(table: &toml::Table) -> Result<(Patch, Vec<String>), String> {
    let mut patch = Patch::default();
    let mut warnings = Vec::new();
    let color = |key: &str, v: &toml::Value| -> Result<Rgb, String> {
        v.as_str()
            .and_then(parse_hex)
            .ok_or_else(|| format!("`{key}` must be a \"#rrggbb\" color"))
    };
    for (key, value) in table {
        match key.as_str() {
            "foreground" => patch.foreground = Some(color(key, value)?),
            "background" => patch.background = Some(color(key, value)?),
            "cursor" => patch.cursor = Some(color(key, value)?),
            "selection" | "selection_background" => patch.selection = Some(color(key, value)?),
            "ansi" => {
                let list = value.as_array().ok_or("`ansi` must be a list of colors")?;
                if list.len() > 16 {
                    warnings.push(format!(
                        "`ansi` has {} colors; only the first 16 are used",
                        list.len()
                    ));
                }
                for (i, v) in list.iter().take(16).enumerate() {
                    patch.colors.push((i as u8, color("ansi", v)?));
                }
            }
            "ui" => {
                let ui = value.as_table().ok_or("`[ui]` must be a table")?;
                for (k, v) in ui {
                    match k.as_str() {
                        "accent" => patch.accent = Some(color(k, v)?),
                        "sidebar" => patch.sidebar = Some(color(k, v)?),
                        other => warnings.push(format!("unknown key `ui.{other}`")),
                    }
                }
            }
            other => match other
                .strip_prefix("color")
                .and_then(|n| n.parse::<u8>().ok())
            {
                Some(i) => patch.colors.push((i, color(key, value)?)),
                None => warnings.push(format!("unknown key `{other}`")),
            },
        }
    }
    Ok((patch, warnings))
}

/// kitty theme format: `key value` per line, `#` comments. Unknown keys are ignored,
/// so full kitty configs and generated theme files both work.
fn parse_kitty(text: &str) -> Patch {
    let mut patch = Patch::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(key), Some(value)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Some(rgb) = parse_hex(value) else {
            continue;
        };
        match key {
            "foreground" => patch.foreground = Some(rgb),
            "background" => patch.background = Some(rgb),
            "cursor" => patch.cursor = Some(rgb),
            "selection_background" => patch.selection = Some(rgb),
            "active_border_color" => patch.accent = Some(rgb),
            _ => {
                if let Some(i) = key.strip_prefix("color").and_then(|n| n.parse::<u8>().ok()) {
                    patch.colors.push((i, rgb));
                }
            }
        }
    }
    patch
}

/// Collect the colors a shell set via OSC (the `Some` entries of alacritty's color table).
/// Indices 0-255 are the palette; 256/257/258 are foreground/background/cursor.
pub fn patch_from_osc(colors: &alacritty_terminal::term::color::Colors) -> Patch {
    use alacritty_terminal::vte::ansi::NamedColor;
    let rgb = |c: alacritty_terminal::vte::ansi::Rgb| [c.r, c.g, c.b];
    Patch {
        foreground: colors[NamedColor::Foreground].map(rgb),
        background: colors[NamedColor::Background].map(rgb),
        cursor: colors[NamedColor::Cursor].map(rgb),
        colors: (0..=255u8)
            .filter_map(|i| colors[i as usize].map(|c| (i, rgb(c))))
            .collect(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{BUILTIN, Theme};

    #[test]
    fn builtins_parse_cleanly() {
        for (name, src) in BUILTIN {
            let (patch, warnings) = parse_toml(src).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(warnings.is_empty(), "{name}: {warnings:?}");
            assert_eq!(patch.colors.len(), 16, "{name}");
        }
        assert!(!Theme::default().is_light());
    }

    #[test]
    fn toml_theme_with_color_n_and_ui() {
        let (p, warnings) = parse_toml(
            r##"
            background = "#101010"
            color4 = "#0000ff"
            color232 = "#121212"
            bogus = 1
            [ui]
            accent = "#ff00ff"
            "##,
        )
        .unwrap();
        assert_eq!(p.background, Some([0x10, 0x10, 0x10]));
        assert!(p.colors.contains(&(4, [0, 0, 0xff])));
        assert!(p.colors.contains(&(232, [0x12, 0x12, 0x12])));
        assert_eq!(p.accent, Some([0xff, 0, 0xff]));
        assert_eq!(warnings, vec!["unknown key `bogus`"]);
        assert!(parse_toml("background = \"red\"").is_err());
    }

    #[test]
    fn kitty_theme() {
        let p = parse_kitty(
            "# generated\nbackground            #1D1A18\nforeground #EDD4C2\ncolor1 #EF672C\n\
             color255 #DEC1B1\nselection_background #F4DED4\nfont_family Hack\ninclude other.conf\n",
        );
        assert_eq!(p.background, Some([0x1d, 0x1a, 0x18]));
        assert_eq!(p.foreground, Some([0xed, 0xd4, 0xc2]));
        assert_eq!(p.selection, Some([0xf4, 0xde, 0xd4]));
        assert_eq!(
            p.colors,
            vec![(1, [0xef, 0x67, 0x2c]), (255, [0xde, 0xc1, 0xb1])]
        );
    }

    #[test]
    fn osc_overrides_become_a_patch() {
        use alacritty_terminal::term::color::Colors;
        use alacritty_terminal::vte::ansi::{NamedColor, Rgb as ARgb};
        let mut colors = Colors::default();
        assert!(patch_from_osc(&colors).is_empty());
        colors[4] = Some(ARgb { r: 1, g: 2, b: 3 });
        colors[NamedColor::Background] = Some(ARgb { r: 4, g: 5, b: 6 });
        let p = patch_from_osc(&colors);
        assert_eq!(p.colors, vec![(4, [1, 2, 3])]);
        assert_eq!(p.background, Some([4, 5, 6]));
    }
}
