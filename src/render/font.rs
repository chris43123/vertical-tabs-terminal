//! Font discovery, glyph rasterization (swash) and the glyph atlas texture.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{self, Color32, ColorImage, Rect, TextureHandle, TextureOptions, Vec2, pos2};
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::Format;
use swash::{CacheKey, FontRef};

/// Monospace families tried (in order) when the config names none or the named one is missing.
const PREFERRED_MONO: &[&str] = &[
    "JetBrains Mono",
    "Cascadia Mono",
    "Cascadia Code",
    "SF Mono",
    "Menlo",
    "Consolas",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
    "Liberation Mono",
    "Ubuntu Mono",
];

/// Fallback families tried for glyphs missing from the primary font.
const FALLBACK_FAMILIES: &[&str] = &[
    "Symbols Nerd Font Mono",
    "Symbols Nerd Font",
    "Noto Sans Mono",
    "DejaVu Sans Mono",
    "Noto Sans Symbols 2",
    "Noto Sans Symbols",
    "Noto Color Emoji",
    "Segoe UI Symbol",
    "Segoe UI Emoji",
    "Apple Color Emoji",
    "Apple Symbols",
    "Noto Sans CJK SC",
    "Noto Sans",
];

const INITIAL_ATLAS: usize = 1024;
const MAX_ATLAS: usize = 4096;

/// A loaded font face (owned data, so swash `FontRef`s can be rebuilt cheaply).
struct Face {
    data: Arc<Vec<u8>>,
    offset: u32,
    key: CacheKey,
}

impl Face {
    fn from_data(data: Arc<Vec<u8>>, index: usize) -> Option<Self> {
        let font = FontRef::from_index(&data, index)?;
        let (offset, key) = (font.offset, font.key);
        Some(Self { data, offset, key })
    }

    fn font(&self) -> FontRef<'_> {
        FontRef {
            data: &self.data,
            offset: self.offset,
            key: self.key,
        }
    }

    fn has_char(&self, c: char) -> bool {
        self.font().charmap().map(c) != 0
    }
}

/// Glyph style variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
}

impl Style {
    fn index(self) -> usize {
        self.bold as usize | ((self.italic as usize) << 1)
    }
}

/// A rasterized glyph in the atlas.
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    /// UV rect in normalized texture coordinates.
    pub uv: Rect,
    /// Offset from the cell's top-left (physical pixels).
    pub offset: Vec2,
    /// Quad size (physical pixels).
    pub size: Vec2,
    /// Color bitmap (emoji): draw untinted.
    pub color: bool,
}

struct Atlas {
    texture: TextureHandle,
    size: usize,
    /// Shelf packer state.
    x: usize,
    y: usize,
    row_h: usize,
}

impl Atlas {
    fn new(ctx: &egui::Context, size: usize) -> Self {
        let mut image = ColorImage::filled([size, size], Color32::TRANSPARENT);
        // 2x2 white block at the origin, sampled for solid rectangles.
        for y in 0..2 {
            for x in 0..2 {
                image.pixels[y * size + x] = Color32::WHITE;
            }
        }
        let texture = ctx.load_texture("vtt-glyph-atlas", image, TextureOptions::LINEAR);
        Self {
            texture,
            size,
            x: 3,
            y: 0,
            row_h: 3,
        }
    }

    /// Reserve space; `None` when the atlas is full.
    fn alloc(&mut self, w: usize, h: usize) -> Option<[usize; 2]> {
        let (pw, ph) = (w + 1, h + 1); // 1px padding against bleeding.
        if pw > self.size || ph > self.size {
            return None;
        }
        if self.x + pw > self.size {
            self.x = 0;
            self.y += self.row_h;
            self.row_h = 0;
        }
        if self.y + ph > self.size {
            return None;
        }
        let pos = [self.x, self.y];
        self.x += pw;
        self.row_h = self.row_h.max(ph);
        Some(pos)
    }

    fn uv(&self, pos: [usize; 2], w: usize, h: usize) -> Rect {
        let s = self.size as f32;
        Rect::from_min_max(
            pos2(pos[0] as f32 / s, pos[1] as f32 / s),
            pos2((pos[0] + w) as f32 / s, (pos[1] + h) as f32 / s),
        )
    }
}

/// More distinct sizes than this clears the metrics cache and atlas.
const MAX_SIZES: usize = 16;

/// Cell metrics for one font size (physical pixels).
#[derive(Clone, Copy, Debug)]
struct Metrics {
    px: f32,
    cell_w: u32,
    cell_h: u32,
    baseline: f32,
    /// Horizontal glyph shift centering glyphs in the letter-spacing extra, per cell.
    x_pad: f32,
    /// Underline offset below the baseline and stroke thickness.
    underline_pos: f32,
    stroke: f32,
    /// Strikeout offset above the baseline.
    strike_pos: f32,
}

impl Metrics {
    const EMPTY: Self = Self {
        px: 0.0,
        cell_w: 1,
        cell_h: 1,
        baseline: 0.0,
        x_pad: 0.0,
        underline_pos: 0.0,
        stroke: 1.0,
        strike_pos: 0.0,
    };
}

fn size_key(size_pt: f32) -> u32 {
    (size_pt * 64.0).round().max(0.0) as u32
}

pub struct Fonts {
    db: fontdb::Database,
    /// Primary faces indexed by `Style::index` (regular, bold, italic, bold-italic).
    primary: [Option<usize>; 4],
    faces: Vec<Face>,
    /// Char -> face index for chars missing in the primary font (None = no font has it).
    fallback: HashMap<char, Option<usize>>,
    fallback_families_loaded: bool,
    full_scans_left: u32,
    scale_ctx: ScaleContext,
    atlas: Atlas,
    /// Keyed by char, style and size key; all sizes share one atlas.
    glyphs: HashMap<(char, Style, u32), Option<Glyph>>,
    /// Set when the atlas had to be reset mid-frame; the caller should repaint.
    pub(crate) atlas_reset: bool,

    ppp: f32,
    line_height: f32,
    letter_spacing: f32,
    /// Metrics per font size (`size_key`); cleared on DPI or spacing changes.
    metrics: HashMap<u32, Metrics>,
    /// Size key and metrics of the size selected by the last `update`.
    active_key: u32,
    active: Metrics,
}

impl Fonts {
    pub fn new(ctx: &egui::Context, family: Option<&str>, size_pt: f32, ppp: f32) -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();

        let mut faces = Vec::new();
        let mut primary = [None; 4];

        let family_name = family
            .filter(|f| {
                db.faces()
                    .any(|fi| fi.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(f)))
            })
            .map(str::to_owned)
            .or_else(|| {
                if let Some(f) = family {
                    crate::diag::warn(format!(
                        "font family {f:?} not found, using a default monospace font"
                    ));
                }
                PREFERRED_MONO
                    .iter()
                    .find(|f| {
                        db.faces()
                            .any(|fi| fi.families.iter().any(|(n, _)| n == *f))
                    })
                    .map(|f| f.to_string())
            });

        if let Some(name) = &family_name {
            for (i, (weight, style)) in [
                (fontdb::Weight::NORMAL, fontdb::Style::Normal),
                (fontdb::Weight::BOLD, fontdb::Style::Normal),
                (fontdb::Weight::NORMAL, fontdb::Style::Italic),
                (fontdb::Weight::BOLD, fontdb::Style::Italic),
            ]
            .into_iter()
            .enumerate()
            {
                let families = [fontdb::Family::Name(name)];
                let query = fontdb::Query {
                    families: &families,
                    weight,
                    style,
                    ..Default::default()
                };
                let Some(id) = db.query(&query) else { continue };
                let info = db.face(id).expect("queried face exists");
                // fontdb returns the closest match; only accept a true variant for bold/italic.
                let is_variant = match i {
                    0 => true,
                    1 => info.weight.0 >= 600,
                    2 => info.style != fontdb::Style::Normal,
                    _ => info.weight.0 >= 600 && info.style != fontdb::Style::Normal,
                };
                if !is_variant {
                    continue;
                }
                if let Some(face) = load_face(&db, id) {
                    faces.push(face);
                    primary[i] = Some(faces.len() - 1);
                }
            }
        }

        if primary[0].is_none() {
            // Any monospace face, then the font bundled with egui.
            let mono = db.faces().find(|f| f.monospaced).map(|f| f.id);
            let face = mono.and_then(|id| load_face(&db, id)).or_else(|| {
                Face::from_data(Arc::new(epaint_default_fonts::HACK_REGULAR.to_vec()), 0)
            });
            faces.push(face.expect("bundled font parses"));
            primary[0] = Some(faces.len() - 1);
        }

        let mut fonts = Self {
            db,
            primary,
            faces,
            fallback: HashMap::new(),
            fallback_families_loaded: false,
            full_scans_left: 32,
            scale_ctx: ScaleContext::new(),
            atlas: Atlas::new(ctx, INITIAL_ATLAS),
            glyphs: HashMap::new(),
            atlas_reset: false,
            ppp: 0.0,
            line_height: 1.0,
            letter_spacing: 0.0,
            metrics: HashMap::new(),
            active_key: 0,
            active: Metrics::EMPTY,
        };
        fonts.update(ctx, size_pt, ppp);
        fonts
    }

    /// Select the active font size (computing its metrics on first use). Cheap enough to call
    /// per pane; the atlas is only reset when the DPI scale changes or too many sizes pile up.
    pub fn update(&mut self, ctx: &egui::Context, size_pt: f32, ppp: f32) {
        if ppp != self.ppp {
            self.ppp = ppp;
            self.invalidate(ctx);
        }
        let key = size_key(size_pt);
        if let Some(m) = self.metrics.get(&key) {
            self.active_key = key;
            self.active = *m;
            return;
        }
        if self.metrics.len() >= MAX_SIZES {
            self.invalidate(ctx);
        }
        let m = self.compute_metrics(key as f32 / 64.0);
        self.metrics.insert(key, m);
        self.active_key = key;
        self.active = m;
    }

    /// Set line height (multiplier) and letter spacing (points); rebuilds metrics on change.
    pub fn set_spacing(&mut self, ctx: &egui::Context, line_height: f32, letter_spacing: f32) {
        let line_height = line_height.clamp(0.8, 3.0);
        let letter_spacing = letter_spacing.clamp(-2.0, 10.0);
        if line_height == self.line_height && letter_spacing == self.letter_spacing {
            return;
        }
        self.line_height = line_height;
        self.letter_spacing = letter_spacing;
        self.invalidate(ctx);
        // Reselect the active size under the new spacing.
        if self.ppp > 0.0 {
            let size_pt = self.active_key as f32 / 64.0;
            self.update(ctx, size_pt, self.ppp);
        }
    }

    /// Drop all cached metrics and glyphs.
    fn invalidate(&mut self, ctx: &egui::Context) {
        self.metrics.clear();
        self.reset_atlas(ctx, self.atlas.size);
        self.atlas_reset = true;
    }

    fn compute_metrics(&self, size_pt: f32) -> Metrics {
        let px = size_pt * self.ppp;
        let face = &self.faces[self.primary[0].unwrap_or(0)];
        let font = face.font();
        let fm = font.metrics(&[]).scale(px);
        let gid = font.charmap().map('0');
        let advance = font.glyph_metrics(&[]).scale(px).advance_width(gid);

        let ascent = fm.ascent.abs();
        let descent = fm.descent.abs();
        let natural_w = advance.round().max(1.0);
        let cell_w = (advance + self.letter_spacing * self.ppp).round().max(1.0);
        let natural_h = (ascent + descent + fm.leading.max(0.0)).ceil().max(1.0);
        let cell_h = (natural_h * self.line_height).ceil().max(1.0);
        let baseline = (fm.leading.max(0.0) / 2.0 + ascent + (cell_h - natural_h) / 2.0)
            .round()
            .clamp(0.0, cell_h);
        let stroke = (px / 14.0).round().max(1.0);
        let underline_pos =
            ((descent / 2.0).round()).clamp(1.0, (cell_h - baseline - stroke).max(0.0));
        let x_height = if fm.x_height > 0.0 {
            fm.x_height
        } else {
            ascent * 0.5
        };
        Metrics {
            px,
            cell_w: cell_w as u32,
            cell_h: cell_h as u32,
            baseline,
            x_pad: ((cell_w - natural_w) / 2.0).round(),
            underline_pos,
            stroke,
            strike_pos: (x_height / 2.0).round().min(baseline),
        }
    }

    fn reset_atlas(&mut self, ctx: &egui::Context, size: usize) {
        self.atlas = Atlas::new(ctx, size);
        self.glyphs.clear();
    }

    pub fn ppp(&self) -> f32 {
        self.ppp
    }

    /// Cell size in egui points, aligned to whole physical pixels.
    pub fn cell_size(&self) -> Vec2 {
        Vec2::new(
            self.active.cell_w as f32 / self.ppp,
            self.active.cell_h as f32 / self.ppp,
        )
    }

    /// Cell size in physical pixels.
    pub fn cell_px(&self) -> (u16, u16) {
        (self.active.cell_w as u16, self.active.cell_h as u16)
    }

    pub(crate) fn baseline_px(&self) -> f32 {
        self.active.baseline
    }

    /// Underline offset below the baseline (physical px).
    pub(crate) fn underline_pos(&self) -> f32 {
        self.active.underline_pos
    }

    /// Decoration stroke thickness (physical px).
    pub(crate) fn stroke(&self) -> f32 {
        self.active.stroke
    }

    /// Strikeout offset above the baseline (physical px).
    pub(crate) fn strike_pos(&self) -> f32 {
        self.active.strike_pos
    }

    pub(crate) fn texture_id(&self) -> egui::TextureId {
        self.atlas.texture.id()
    }

    /// UV of the white texel block, for solid quads.
    pub(crate) fn white_uv(&self) -> Rect {
        let c = 1.0 / self.atlas.size as f32;
        Rect::from_min_max(pos2(c * 0.5, c * 0.5), pos2(c * 1.5, c * 1.5))
    }

    /// Whether a real bold face is available (otherwise bold is emboldened synthetically).
    pub(crate) fn has_bold(&self) -> bool {
        self.primary[1].is_some()
    }

    /// Get (rasterizing if needed) a glyph. `span` is the number of cells it may occupy (1 or 2).
    pub(crate) fn glyph(
        &mut self,
        ctx: &egui::Context,
        c: char,
        style: Style,
        span: u32,
    ) -> Option<Glyph> {
        let key = (c, style, self.active_key);
        if let Some(g) = self.glyphs.get(&key) {
            return *g;
        }
        let g = self.rasterize(ctx, c, style, span);
        // Rasterizing may have reset the atlas (clearing `glyphs`); inserting is still correct.
        self.glyphs.insert(key, g);
        g
    }

    /// Pick the face for a char: styled primary, regular primary, then fallback fonts.
    fn face_for(&mut self, c: char, style: Style) -> Option<(usize, bool)> {
        let regular = self.primary[0]?;
        let styled = self.primary[style.index()];
        if let Some(i) = styled.filter(|i| self.faces[*i].has_char(c)) {
            return Some((i, false));
        }
        if self.faces[regular].has_char(c) {
            // Synthetic bold when the bold face is missing.
            return Some((regular, style.bold));
        }
        if let Some(r) = self.fallback.get(&c) {
            return r.map(|i| (i, style.bold));
        }
        let found = self.find_fallback(c);
        self.fallback.insert(c, found);
        found.map(|i| (i, style.bold))
    }

    fn find_fallback(&mut self, c: char) -> Option<usize> {
        // Already-loaded fallback faces first.
        if let Some(i) = (0..self.faces.len())
            .find(|i| !self.primary.contains(&Some(*i)) && self.faces[*i].has_char(c))
        {
            return Some(i);
        }
        if !self.fallback_families_loaded {
            self.fallback_families_loaded = true;
            for name in FALLBACK_FAMILIES {
                let families = [fontdb::Family::Name(name)];
                let query = fontdb::Query {
                    families: &families,
                    ..Default::default()
                };
                if let Some(face) = self.db.query(&query).and_then(|id| load_face(&self.db, id)) {
                    self.faces.push(face);
                }
            }
            if let Some(i) = (0..self.faces.len())
                .find(|i| !self.primary.contains(&Some(*i)) && self.faces[*i].has_char(c))
            {
                return Some(i);
            }
        }
        // Last resort: scan every system face for this char. Each scan reads every font file,
        // so cap how many are done per session (garbage output can contain many unknown chars).
        if self.full_scans_left == 0 {
            return None;
        }
        self.full_scans_left -= 1;
        let ids: Vec<_> = self.db.faces().map(|f| f.id).collect();
        for id in ids {
            let hit = self
                .db
                .with_face_data(id, |data, index| {
                    FontRef::from_index(data, index as usize)
                        .is_some_and(|f| f.charmap().map(c) != 0)
                })
                .unwrap_or(false);
            if hit && let Some(face) = load_face(&self.db, id) {
                self.faces.push(face);
                return Some(self.faces.len() - 1);
            }
        }
        None
    }

    fn rasterize(
        &mut self,
        ctx: &egui::Context,
        c: char,
        style: Style,
        span: u32,
    ) -> Option<Glyph> {
        let (face_idx, embolden) = self.face_for(c, style)?;
        let m = self.active;
        let px = m.px;
        let face = &self.faces[face_idx];
        let font = face.font();
        let gid = font.charmap().map(c);
        if gid == 0 {
            return None;
        }

        let mut scaler = self.scale_ctx.builder(font).size(px).hint(true).build();
        let sources = [
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ];
        let mut render = Render::new(&sources);
        render.format(Format::Alpha);
        if embolden {
            render.embolden((px / 24.0).max(0.5));
        }
        let image = render.render(&mut scaler, gid)?;
        let (w, h) = (
            image.placement.width as usize,
            image.placement.height as usize,
        );
        if w == 0 || h == 0 {
            // Whitespace-like glyph: nothing to draw, but it exists.
            return Some(Glyph {
                uv: Rect::NOTHING,
                offset: Vec2::ZERO,
                size: Vec2::ZERO,
                color: false,
            });
        }

        let (mut color_image, mut offset, mut size, is_color) = match image.content {
            Content::Mask => {
                let pixels = image
                    .data
                    .iter()
                    .map(|&a| Color32::from_white_alpha(a))
                    .collect();
                let offset = Vec2::new(
                    image.placement.left as f32 + m.x_pad * span as f32,
                    m.baseline - image.placement.top as f32,
                );
                (
                    ColorImage::new([w, h], pixels),
                    offset,
                    Vec2::new(w as f32, h as f32),
                    false,
                )
            }
            Content::SubpixelMask | Content::Color => {
                let img = ColorImage::from_rgba_unmultiplied([w, h], &image.data);
                let offset = Vec2::new(
                    image.placement.left as f32 + m.x_pad * span as f32,
                    m.baseline - image.placement.top as f32,
                );
                (
                    img,
                    offset,
                    Vec2::new(w as f32, h as f32),
                    image.content == Content::Color,
                )
            }
        };

        // Color bitmaps (emoji strikes) are often far larger than a cell: fit them into `span` cells.
        if is_color {
            let (max_w, max_h) = ((m.cell_w * span) as f32, m.cell_h as f32);
            if size.x > max_w || size.y > max_h {
                let scale = (max_w / size.x).min(max_h / size.y);
                let (nw, nh) = (
                    ((size.x * scale).round() as usize).max(1),
                    ((size.y * scale).round() as usize).max(1),
                );
                color_image = downscale(&color_image, nw, nh);
                size = Vec2::new(nw as f32, nh as f32);
                offset = Vec2::new(
                    ((max_w - size.x) / 2.0).floor(),
                    ((max_h - size.y) / 2.0).floor(),
                );
            }
        }

        let [cw, ch] = color_image.size;
        let pos = match self.atlas.alloc(cw, ch) {
            Some(p) => p,
            None => {
                // Grow once it's full; beyond the max, start over (glyphs re-rasterize lazily).
                let new_size = (self.atlas.size * 2).min(MAX_ATLAS);
                self.reset_atlas(ctx, new_size);
                self.atlas_reset = true;
                self.atlas.alloc(cw, ch)?
            }
        };
        self.atlas
            .texture
            .set_partial(pos, color_image, TextureOptions::LINEAR);
        Some(Glyph {
            uv: self.atlas.uv(pos, cw, ch),
            offset,
            size,
            color: is_color,
        })
    }
}

fn load_face(db: &fontdb::Database, id: fontdb::ID) -> Option<Face> {
    db.with_face_data(id, |data, index| {
        Face::from_data(Arc::new(data.to_vec()), index as usize)
    })
    .flatten()
}

/// Box-filter downscale for color bitmaps.
fn downscale(src: &ColorImage, w: usize, h: usize) -> ColorImage {
    let [sw, sh] = src.size;
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        let (y0, y1) = (y * sh / h, ((y + 1) * sh / h).max(y * sh / h + 1));
        for x in 0..w {
            let (x0, x1) = (x * sw / w, ((x + 1) * sw / w).max(x * sw / w + 1));
            let mut acc = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let p = src.pixels[sy * sw + sx];
                    acc[0] += p.r() as u32;
                    acc[1] += p.g() as u32;
                    acc[2] += p.b() as u32;
                    acc[3] += p.a() as u32;
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            // Pixels are premultiplied already, so averaging channels directly is correct.
            out.push(Color32::from_rgba_premultiplied(
                (acc[0] / n) as u8,
                (acc[1] / n) as u8,
                (acc[2] / n) as u8,
                (acc[3] / n) as u8,
            ));
        }
    }
    ColorImage::new([w, h], out)
}
