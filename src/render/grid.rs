//! Builds one textured mesh per pane from the terminal grid.

use alacritty_terminal::Term;
use alacritty_terminal::index::Side;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
use eframe::egui::{self, Color32, Mesh, Pos2, Rect, Shape, Vec2, pos2};

use super::font::{Fonts, Style};
use crate::session::{GridSize, Listener};
use crate::theme::Theme;

#[derive(Clone, Debug)]
pub struct Palette {
    /// Full 256-color table (theme overrides already applied).
    table: [Color32; 256],
    fg: Color32,
    bg: Color32,
    cursor: Color32,
    selection: Color32,
}

impl Palette {
    pub fn from_theme(t: &Theme) -> Self {
        let c = |[r, g, b]: [u8; 3]| Color32::from_rgb(r, g, b);
        Self {
            table: t.colors.map(c),
            fg: c(t.foreground),
            bg: c(t.background),
            cursor: c(t.cursor),
            selection: c(t.selection),
        }
    }

    pub fn background(&self) -> Color32 {
        self.bg
    }

    pub fn foreground(&self) -> Color32 {
        self.fg
    }

    /// The 256-color palette entry for `i` (without terminal overrides).
    pub fn indexed(&self, i: u8) -> Color32 {
        self.table[i as usize]
    }

    fn named(&self, n: NamedColor) -> Color32 {
        let idx = n as usize;
        match n {
            _ if idx < 16 => self.table[idx],
            NamedColor::Foreground | NamedColor::BrightForeground => self.fg,
            NamedColor::Background => self.bg,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimForeground => dim(self.fg),
            // DimBlack..DimWhite map to the normal colors, dimmed.
            _ => dim(self.table[(idx - NamedColor::DimBlack as usize) % 8]),
        }
    }
}

fn dim(c: Color32) -> Color32 {
    let f = |v: u8| (v as f32 * 0.66) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// Resolve a cell color: terminal (OSC 4/10/11) overrides first, then the palette.
fn resolve(color: Color, colors: &Colors, palette: &Palette) -> Color32 {
    let rgb = |c: alacritty_terminal::vte::ansi::Rgb| Color32::from_rgb(c.r, c.g, c.b);
    match color {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) => colors[i as usize]
            .map(rgb)
            .unwrap_or_else(|| palette.indexed(i)),
        Color::Named(n) => colors[n].map(rgb).unwrap_or_else(|| palette.named(n)),
    }
}

/// Grid size that fits in `rect`.
pub fn grid_size_for(rect: Rect, fonts: &Fonts) -> GridSize {
    let cell = fonts.cell_size();
    GridSize {
        cols: ((rect.width() / cell.x).floor() as usize).max(2),
        lines: ((rect.height() / cell.y).floor() as usize).max(1),
    }
}

/// Pointer position -> (column, screen line, side of the cell), clamped to the grid.
pub fn cell_at(pos: Pos2, rect: Rect, fonts: &Fonts, size: GridSize) -> (usize, usize, Side) {
    cell_at_size(pos, rect, fonts.cell_size(), size)
}

fn cell_at_size(pos: Pos2, rect: Rect, cell: Vec2, size: GridSize) -> (usize, usize, Side) {
    let rel = pos - rect.min;
    let fx = (rel.x / cell.x).max(0.0);
    let fy = (rel.y / cell.y).max(0.0);
    let col = (fx.floor() as usize).min(size.cols.saturating_sub(1));
    let line = (fy.floor() as usize).min(size.lines.saturating_sub(1));
    let side = if rel.x >= size.cols as f32 * cell.x || fx.fract() >= 0.5 {
        Side::Right
    } else {
        Side::Left
    };
    let side = if rel.x < 0.0 { Side::Left } else { side };
    (col, line, side)
}

/// Mesh builder working in physical pixels relative to a pixel-snapped origin.
struct Builder {
    mesh: Mesh,
    origin: Pos2,
    ppp: f32,
    white: Rect,
}

impl Builder {
    fn to_points(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.origin.x + x / self.ppp, self.origin.y + y / self.ppp)
    }

    fn rect_px(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let r = Rect::from_min_max(self.to_points(x, y), self.to_points(x + w, y + h));
        self.mesh.add_rect_with_uv(r, self.white, color);
    }

    fn quad_px(&mut self, x: f32, y: f32, size: Vec2, uv: Rect, color: Color32) {
        let r = Rect::from_min_max(self.to_points(x, y), self.to_points(x + size.x, y + size.y));
        self.mesh.add_rect_with_uv(r, uv, color);
    }
}

/// Paint the visible terminal grid into `rect` as a single mesh.
pub fn paint_terminal(
    painter: &egui::Painter,
    rect: Rect,
    term: &Term<Listener>,
    fonts: &mut Fonts,
    palette: &Palette,
    focused: bool,
) {
    let ctx = painter.ctx().clone();
    let ppp = fonts.ppp();
    let (cw, ch) = fonts.cell_px();
    let (cw, ch) = (cw as f32, ch as f32);
    let baseline = fonts.baseline_px();
    let has_bold = fonts.has_bold();

    let content = term.renderable_content();
    let colors = content.colors;
    let default_bg = colors[NamedColor::Background]
        .map(|c| Color32::from_rgb(c.r, c.g, c.b))
        .unwrap_or(palette.bg);

    let origin = pos2(
        (rect.min.x * ppp).round() / ppp,
        (rect.min.y * ppp).round() / ppp,
    );
    let mut bg = Builder {
        mesh: Mesh::with_texture(fonts.texture_id()),
        origin,
        ppp,
        white: fonts.white_uv(),
    };
    let mut fg = Builder {
        mesh: Mesh::with_texture(fonts.texture_id()),
        origin,
        ppp,
        white: fonts.white_uv(),
    };

    // Whole pane background (covers the leftover margin below/right of the grid).
    painter.rect_filled(rect, 0.0, default_bg);

    let display_offset = content.display_offset as i32;
    let cursor_color = colors[NamedColor::Cursor]
        .map(|c| Color32::from_rgb(c.r, c.g, c.b))
        .unwrap_or(palette.cursor);
    let cursor_line = content.cursor.point.line.0 + display_offset;
    let cursor_col = content.cursor.point.column.0;
    let cursor_shape = content.cursor.shape;
    let block_cursor = focused && cursor_shape == CursorShape::Block;
    let mut cursor_wide = false;

    let selection = content.selection;
    let rows_limit = term_screen_lines(rect, ch, ppp);

    // Run-length merge of background quads per row.
    let mut run: Option<(i32, f32, f32, Color32)> = None; // (line, x, width, color)
    let flush = |run: &mut Option<(i32, f32, f32, Color32)>, bg: &mut Builder| {
        if let Some((line, x, w, color)) = run.take() {
            bg.rect_px(x, line as f32 * ch, w, ch, color);
        }
    };

    for indexed in content.display_iter {
        let point = indexed.point;
        let cell = indexed.cell;
        let line = point.line.0 + display_offset;
        if line < 0 || line as usize >= rows_limit {
            continue;
        }
        let flags = cell.flags;
        if flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let col = point.column.0;
        let span = if flags.contains(Flags::WIDE_CHAR) {
            2.0
        } else {
            1.0
        };
        let x = col as f32 * cw;
        let y = line as f32 * ch;

        // Colors.
        let mut fg_color = cell.fg;
        if flags.contains(Flags::BOLD)
            && !has_bold
            && let Color::Named(n) = fg_color
            && (n as usize) < 8
        {
            fg_color = Color::Indexed(n as u8 + 8);
        }
        let mut fgc = resolve(fg_color, colors, palette);
        let mut bgc = resolve(cell.bg, colors, palette);
        if flags.contains(Flags::DIM) {
            fgc = dim(fgc);
        }
        if flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fgc, &mut bgc);
        }
        if selection.is_some_and(|s| s.contains(point)) {
            bgc = palette.selection;
        }
        let is_cursor =
            line == cursor_line && col == cursor_col && cursor_shape != CursorShape::Hidden;
        if is_cursor {
            cursor_wide = span > 1.0;
            if block_cursor {
                // Invert the cell under a focused block cursor.
                fgc = if bgc == default_bg { default_bg } else { bgc };
                bgc = cursor_color;
            }
        }
        if flags.contains(Flags::HIDDEN) {
            fgc = bgc;
        }

        // Background run.
        if bgc != default_bg {
            match &mut run {
                Some((l, rx, rw, c)) if *l == line && *c == bgc && (*rx + *rw - x).abs() < 0.5 => {
                    *rw += cw * span
                }
                _ => {
                    flush(&mut run, &mut bg);
                    run = Some((line, x, cw * span, bgc));
                }
            }
        } else {
            flush(&mut run, &mut bg);
        }

        // Glyph.
        let c = cell.c;
        if c != ' ' && c != '\t' && !flags.contains(Flags::HIDDEN) {
            if !draw_box_char(&mut fg, c, x, y, cw, ch, fgc) {
                let style = Style {
                    bold: flags.contains(Flags::BOLD),
                    italic: flags.contains(Flags::ITALIC),
                };
                if let Some(g) = fonts.glyph(&ctx, c, style, span as u32)
                    && g.size.x > 0.0
                {
                    let tint = if g.color { Color32::WHITE } else { fgc };
                    fg.quad_px(x + g.offset.x, y + g.offset.y, g.size, g.uv, tint);
                }
            }
            // Combining characters are drawn on top of the base glyph.
            for &zw in cell.zerowidth().unwrap_or(&[]) {
                let style = Style {
                    bold: flags.contains(Flags::BOLD),
                    italic: flags.contains(Flags::ITALIC),
                };
                if let Some(g) = fonts.glyph(&ctx, zw, style, 1)
                    && g.size.x > 0.0
                {
                    fg.quad_px(x + g.offset.x, y + g.offset.y, g.size, g.uv, fgc);
                }
            }
        }

        // Decorations.
        if flags.intersects(Flags::ALL_UNDERLINES) {
            let uc = cell
                .underline_color()
                .map(|c| resolve(c, colors, palette))
                .unwrap_or(fgc);
            let uy = y + baseline + fonts.underline_pos;
            fg.rect_px(x, uy, cw * span, fonts.stroke, uc);
            if flags.contains(Flags::DOUBLE_UNDERLINE) {
                fg.rect_px(
                    x,
                    (uy + fonts.stroke * 2.0).min(y + ch - fonts.stroke),
                    cw * span,
                    fonts.stroke,
                    uc,
                );
            }
        }
        if flags.contains(Flags::STRIKEOUT) {
            fg.rect_px(
                x,
                y + baseline - fonts.strike_pos,
                cw * span,
                fonts.stroke,
                fgc,
            );
        }
    }
    flush(&mut run, &mut bg);

    // Non-block cursors are drawn on top of the text.
    if cursor_line >= 0 && (cursor_line as usize) < rows_limit {
        let x = cursor_col as f32 * cw;
        let y = cursor_line as f32 * ch;
        let w = if cursor_wide { cw * 2.0 } else { cw };
        let t = (ppp.round()).max(1.0) * fonts.stroke.max(1.0);
        let shape = if !focused && cursor_shape != CursorShape::Hidden {
            CursorShape::HollowBlock
        } else {
            cursor_shape
        };
        match shape {
            CursorShape::Beam => fg.rect_px(x, y, t.max(2.0), ch, cursor_color),
            CursorShape::Underline => {
                fg.rect_px(x, y + ch - t.max(2.0), w, t.max(2.0), cursor_color)
            }
            CursorShape::HollowBlock => {
                fg.rect_px(x, y, w, t, cursor_color);
                fg.rect_px(x, y + ch - t, w, t, cursor_color);
                fg.rect_px(x, y, t, ch, cursor_color);
                fg.rect_px(x + w - t, y, t, ch, cursor_color);
            }
            CursorShape::Block | CursorShape::Hidden => {}
        }
    }

    bg.mesh.append(fg.mesh);
    painter.with_clip_rect(rect).add(Shape::mesh(bg.mesh));

    if std::mem::take(&mut fonts.atlas_reset) {
        // Earlier quads this frame may reference the old atlas.
        ctx.request_repaint();
    }
}

/// Number of whole lines that fit (guards against painting past the pane on odd sizes).
fn term_screen_lines(rect: Rect, ch_px: f32, ppp: f32) -> usize {
    ((rect.height() * ppp / ch_px).ceil() as usize).max(1)
}

/// Draw box-drawing / block-element chars as solid rects so they tile seamlessly.
/// Returns false when the char isn't handled here (fall back to the font).
fn draw_box_char(
    b: &mut Builder,
    c: char,
    x: f32,
    y: f32,
    cw: f32,
    ch: f32,
    color: Color32,
) -> bool {
    let code = c as u32;
    if let Some([l, r, u, d]) = line_arms(code) {
        let light = (cw / 8.0).round().max(1.0);
        let heavy = (light * 2.0).max(light + 1.0);
        let t = |w: u8| if w == 2 { heavy } else { light };
        let tmax = t(l.max(r).max(u).max(d));
        let cx = x + ((cw - light) / 2.0).floor();
        let cy = y + ((ch - light) / 2.0).floor();
        // Horizontal arms, centered on the light-line position.
        let hy = |w: u8| cy - ((t(w) - light) / 2.0).floor();
        let vx = |w: u8| cx - ((t(w) - light) / 2.0).floor();
        if l > 0 {
            b.rect_px(x, hy(l), cx - x + tmax.max(t(l)), t(l), color);
        }
        if r > 0 {
            let start = vx(u.max(d).max(r)).min(cx);
            b.rect_px(start, hy(r), x + cw - start, t(r), color);
        }
        if u > 0 {
            b.rect_px(vx(u), y, t(u), cy - y + tmax.max(t(u)), color);
        }
        if d > 0 {
            let start = hy(l.max(r).max(d)).min(cy);
            b.rect_px(vx(d), start, t(d), y + ch - start, color);
        }
        return true;
    }

    let eighth_h = |n: f32| (ch * n / 8.0).round();
    let eighth_w = |n: f32| (cw * n / 8.0).round();
    match code {
        // ▀ upper half
        0x2580 => b.rect_px(x, y, cw, eighth_h(4.0), color),
        // ▁..▇ lower n/8, █ full
        0x2581..=0x2588 => {
            let h = eighth_h((code - 0x2580) as f32);
            b.rect_px(x, y + ch - h, cw, h, color);
        }
        // ▉..▏ left 7/8..1/8
        0x2589..=0x258F => b.rect_px(x, y, eighth_w((0x2590 - code) as f32), ch, color),
        // ▐ right half
        0x2590 => {
            let w = eighth_w(4.0);
            b.rect_px(x + cw - w, y, w, ch, color);
        }
        // ░ ▒ ▓ shades
        0x2591..=0x2593 => {
            let alpha = [0.25, 0.5, 0.75][(code - 0x2591) as usize];
            b.rect_px(x, y, cw, ch, color.gamma_multiply(alpha));
        }
        // ▔ upper 1/8
        0x2594 => b.rect_px(x, y, cw, eighth_h(1.0), color),
        // ▕ right 1/8
        0x2595 => {
            let w = eighth_w(1.0);
            b.rect_px(x + cw - w, y, w, ch, color);
        }
        // Quadrants ▖▗▘▙▚▛▜▝▞▟
        0x2596..=0x259F => {
            // Bits: upper-left, upper-right, lower-left, lower-right.
            let q: u8 = match code {
                0x2596 => 0b0010,
                0x2597 => 0b0001,
                0x2598 => 0b1000,
                0x2599 => 0b1011,
                0x259A => 0b1001,
                0x259B => 0b1110,
                0x259C => 0b1101,
                0x259D => 0b0100,
                0x259E => 0b0110,
                _ => 0b0111,
            };
            let (hw, hh) = (eighth_w(4.0), eighth_h(4.0));
            if q & 0b1000 != 0 {
                b.rect_px(x, y, hw, hh, color);
            }
            if q & 0b0100 != 0 {
                b.rect_px(x + hw, y, cw - hw, hh, color);
            }
            if q & 0b0010 != 0 {
                b.rect_px(x, y + hh, hw, ch - hh, color);
            }
            if q & 0b0001 != 0 {
                b.rect_px(x + hw, y + hh, cw - hw, ch - hh, color);
            }
        }
        _ => return false,
    }
    true
}

/// Arm weights [left, right, up, down] (0 none, 1 light, 2 heavy) for simple box-drawing lines.
fn line_arms(code: u32) -> Option<[u8; 4]> {
    Some(match code {
        0x2500 => [1, 1, 0, 0],
        0x2501 => [2, 2, 0, 0],
        0x2502 => [0, 0, 1, 1],
        0x2503 => [0, 0, 2, 2],
        0x250C | 0x256D => [0, 1, 0, 1],
        0x250D => [0, 2, 0, 1],
        0x250E => [0, 1, 0, 2],
        0x250F => [0, 2, 0, 2],
        0x2510 | 0x256E => [1, 0, 0, 1],
        0x2511 => [2, 0, 0, 1],
        0x2512 => [1, 0, 0, 2],
        0x2513 => [2, 0, 0, 2],
        0x2514 | 0x2570 => [0, 1, 1, 0],
        0x2515 => [0, 2, 1, 0],
        0x2516 => [0, 1, 2, 0],
        0x2517 => [0, 2, 2, 0],
        0x2518 | 0x256F => [1, 0, 1, 0],
        0x2519 => [2, 0, 1, 0],
        0x251A => [1, 0, 2, 0],
        0x251B => [2, 0, 2, 0],
        0x251C => [0, 1, 1, 1],
        0x2523 => [0, 2, 2, 2],
        0x2524 => [1, 0, 1, 1],
        0x252B => [2, 0, 2, 2],
        0x252C => [1, 1, 0, 1],
        0x2533 => [2, 2, 0, 2],
        0x2534 => [1, 1, 1, 0],
        0x253B => [2, 2, 2, 0],
        0x253C => [1, 1, 1, 1],
        0x254B => [2, 2, 2, 2],
        0x2574 => [1, 0, 0, 0],
        0x2575 => [0, 0, 1, 0],
        0x2576 => [0, 1, 0, 0],
        0x2577 => [0, 0, 0, 1],
        0x2578 => [2, 0, 0, 0],
        0x2579 => [0, 0, 2, 0],
        0x257A => [0, 2, 0, 0],
        0x257B => [0, 0, 0, 2],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    #[test]
    fn color_cube_and_grayscale() {
        let p = Palette::from_theme(&Theme::default());
        assert_eq!(p.indexed(16), Color32::from_rgb(0, 0, 0));
        assert_eq!(p.indexed(21), Color32::from_rgb(0, 0, 255));
        assert_eq!(p.indexed(196), Color32::from_rgb(255, 0, 0));
        assert_eq!(p.indexed(231), Color32::from_rgb(255, 255, 255));
        assert_eq!(p.indexed(59), Color32::from_rgb(95, 95, 95));
        assert_eq!(p.indexed(232), Color32::from_rgb(8, 8, 8));
        assert_eq!(p.indexed(255), Color32::from_rgb(238, 238, 238));
        assert_eq!(p.indexed(1), Color32::from_rgb(0xf3, 0x8b, 0xa8));
    }

    #[test]
    fn named_colors() {
        let p = Palette::from_theme(&Theme::default());
        assert_eq!(p.named(NamedColor::Background), p.background());
        assert_eq!(p.named(NamedColor::Foreground), p.foreground());
        assert_eq!(p.named(NamedColor::DimRed), dim(p.indexed(1)));
    }

    #[test]
    fn cell_hit_testing() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, 100.0));
        let cell = vec2(10.0, 20.0);
        let size = GridSize { cols: 10, lines: 5 };
        assert_eq!(
            cell_at_size(pos2(10.0, 20.0), rect, cell, size),
            (0, 0, Side::Left)
        );
        assert_eq!(
            cell_at_size(pos2(26.0, 45.0), rect, cell, size),
            (1, 1, Side::Right)
        );
        // Clamped past the edges.
        assert_eq!(
            cell_at_size(pos2(500.0, 500.0), rect, cell, size),
            (9, 4, Side::Right)
        );
        assert_eq!(
            cell_at_size(pos2(0.0, 0.0), rect, cell, size),
            (0, 0, Side::Left)
        );
    }
}
