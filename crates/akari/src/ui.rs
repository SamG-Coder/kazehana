//! Immediate-mode 2D overlay: panels, bars and text, drawn over the final image each frame.
//!
//! The game builds a `Ui` every frame (rects, text and images in pixels, origin top-left, sRGB
//! colours with alpha) and hands it to the renderer in `FrameDesc::ui`. Glyphs are rasterised on
//! demand with fontdue into an RGBA atlas (white, their coverage in alpha); images (controller
//! buttons, icons) are packed into the same atlas from the bottom up when loaded. The renderer
//! re-uploads the atlas when its version changes.

use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

pub const ATLAS: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

#[derive(Clone, Copy)]
struct Glyph {
    /// atlas rect (pixels)
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    /// placement relative to the pen on the baseline
    xmin: f32,
    ymin: f32,
    advance: f32,
}

/// An image in the atlas, at full size and at half (box-filtered: drawn small it stays clean).
#[derive(Clone, Copy)]
struct Image {
    /// (x, y, w, h) in the atlas, full size and half
    full: [u16; 4],
    half: [u16; 4],
}

pub struct Font {
    font: fontdue::Font,
    glyphs: HashMap<(char, u16), Glyph>,
    images: HashMap<String, Image>,
    /// RGBA, ATLAS x ATLAS
    pub(crate) atlas: Vec<u8>,
    /// images fill the atlas from the bottom up: glyphs stay above this row
    image_top: u32,
    image_x: u32,
    image_row_h: u32,
    pub(crate) version: u64,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
}

/// Fonts tried in order (system fonts for now; a bundled OFL font such as Noto Sans JP later).
const FONT_PATHS: &[&str] = &["C:/Windows/Fonts/YuGothM.ttc", "C:/Windows/Fonts/meiryo.ttc", "C:/Windows/Fonts/segoeui.ttf", "C:/Windows/Fonts/arial.ttf"];

impl Font {
    pub fn system() -> anyhow::Result<Self> {
        for p in FONT_PATHS {
            if let Ok(bytes) = std::fs::read(p) {
                let f = fontdue::Font::from_bytes(bytes, fontdue::FontSettings { collection_index: 0, scale: 48.0, ..Default::default() }).map_err(|e| anyhow::anyhow!("{p}: {e}"))?;
                let mut atlas = vec![0u8; (ATLAS * ATLAS * 4) as usize];
                // a solid white 4x4 block at the origin: untextured rects sample it
                for y in 0..4 {
                    for x in 0..4 {
                        atlas[((y * ATLAS + x) * 4) as usize..][..4].fill(255);
                    }
                }
                return Ok(Self { font: f, glyphs: HashMap::new(), images: HashMap::new(), atlas, image_top: ATLAS, image_x: 0, image_row_h: 0, version: 1, shelf_x: 8, shelf_y: 0, shelf_h: 8 });
            }
        }
        anyhow::bail!("no usable system font found")
    }

    fn glyph(&mut self, c: char, px: u16) -> Glyph {
        if let Some(g) = self.glyphs.get(&(c, px)) {
            return *g;
        }
        let (m, bitmap) = self.font.rasterize(c, px as f32);
        let (w, h) = (m.width as u32, m.height as u32);
        if self.shelf_x + w + 1 > ATLAS {
            self.shelf_x = 0;
            self.shelf_y += self.shelf_h + 1;
            self.shelf_h = 0;
        }
        if self.shelf_y + h + 1 > self.image_top {
            // atlas full: start over (rare; text re-rasterises next frame; the images stay)
            self.glyphs.clear();
            self.atlas[(8 * ATLAS * 4) as usize..(self.image_top * ATLAS * 4) as usize].fill(0);
            self.shelf_x = 8;
            self.shelf_y = 8;
            self.shelf_h = 0;
        }
        let (x, y) = (self.shelf_x, self.shelf_y);
        for row in 0..h {
            for col in 0..w {
                let dst = (((y + row) * ATLAS + x + col) * 4) as usize;
                self.atlas[dst..dst + 3].fill(255);
                self.atlas[dst + 3] = bitmap[(row * w + col) as usize];
            }
        }
        self.shelf_x += w + 1;
        self.shelf_h = self.shelf_h.max(h);
        self.version += 1;
        let g = Glyph { x: x as u16, y: y as u16, w: w as u16, h: h as u16, xmin: m.xmin as f32, ymin: m.ymin as f32, advance: m.advance_width };
        self.glyphs.insert((c, px), g);
        g
    }

    /// Room for an image (w x h) in the atlas, from the bottom up.
    fn image_slot(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if self.image_x + w + 1 > ATLAS {
            self.image_x = 0;
            self.image_top -= self.image_row_h + 1;
            self.image_row_h = 0;
        }
        if self.image_row_h < h {
            // (a taller image than the row so far: the row grows upwards)
            let grow = h - self.image_row_h;
            if self.image_top < grow + 512 {
                return None; // (keep the top of the atlas for text)
            }
            if self.image_row_h == 0 {
                self.image_top -= h + 1;
            } else {
                // move the row's top up; images already in it stay where they are
                self.image_top -= grow;
            }
            self.image_row_h = h;
        }
        let at = (self.image_x, self.image_top + self.image_row_h - h);
        self.image_x += w + 1;
        Some(at)
    }

    fn blit(&mut self, x: u32, y: u32, w: u32, h: u32, rgba: &[u8]) {
        for row in 0..h {
            let dst = (((y + row) * ATLAS + x) * 4) as usize;
            self.atlas[dst..dst + (w * 4) as usize].copy_from_slice(&rgba[(row * w * 4) as usize..((row + 1) * w * 4) as usize]);
        }
    }

    /// Add an image (straight RGBA, sRGB-encoded) under a name. False if the atlas has no room.
    pub fn add_image(&mut self, name: &str, w: u32, h: u32, rgba: &[u8]) -> bool {
        // its half-size copy: each 2 x 2 averaged, colour weighted by coverage
        let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
        let mut half = vec![0u8; (hw * hh * 4) as usize];
        for y in 0..hh {
            for x in 0..hw {
                let (mut c, mut a) = ([0f32; 3], 0f32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(w - 1), (2 * y + dy).min(h - 1));
                    let p = &rgba[((sy * w + sx) * 4) as usize..][..4];
                    let pa = p[3] as f32;
                    for k in 0..3 {
                        c[k] += p[k] as f32 * pa;
                    }
                    a += pa;
                }
                let o = &mut half[((y * hw + x) * 4) as usize..][..4];
                for k in 0..3 {
                    o[k] = if a > 0.0 { (c[k] / a).round() as u8 } else { 0 };
                }
                o[3] = (a / 4.0).round() as u8;
            }
        }
        let (Some(f), Some(hf)) = (self.image_slot(w, h), self.image_slot(hw, hh)) else { return false };
        self.blit(f.0, f.1, w, h, rgba);
        self.blit(hf.0, hf.1, hw, hh, &half);
        let r = |p: (u32, u32), w: u32, h: u32| [p.0 as u16, p.1 as u16, w as u16, h as u16];
        self.images.insert(name.to_string(), Image { full: r(f, w, h), half: r(hf, hw, hh) });
        self.version += 1;
        true
    }

    /// (ascent, line height) at a size.
    pub fn line(&self, px: f32) -> (f32, f32) {
        match self.font.horizontal_line_metrics(px) {
            Some(l) => (l.ascent, l.new_line_size),
            None => (px * 0.8, px * 1.2),
        }
    }
}

/// sRGB colour with alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const WHITE: Rgba = Rgba(255, 255, 255, 255);
    pub const BLACK: Rgba = Rgba(0, 0, 0, 255);
    pub fn alpha(self, a: f32) -> Rgba {
        Rgba(self.0, self.1, self.2, (self.3 as f32 * a.clamp(0.0, 1.0)) as u8)
    }
    pub fn rgb(c: [u8; 3]) -> Rgba {
        Rgba(c[0], c[1], c[2], 255)
    }
}

pub struct Ui {
    pub font: Font,
    pub(crate) verts: Vec<UiVertex>,
    pub width: f32,
    pub height: f32,
}

impl Ui {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self { font: Font::system()?, verts: Vec::new(), width: 1920.0, height: 1080.0 })
    }

    /// Start a frame for a screen of this size.
    pub fn begin(&mut self, width: u32, height: u32) {
        self.verts.clear();
        self.width = width as f32;
        self.height = height as f32;
    }

    /// UI scale: 1 at 1080p.
    pub fn scale(&self) -> f32 {
        self.height / 1080.0
    }

    fn quad(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, u0: f32, v0: f32, u1: f32, v1: f32, c: Rgba) {
        let col = [c.0, c.1, c.2, c.3];
        let v = |x, y, u, w| UiVertex { pos: [x, y], uv: [u, w], color: col };
        self.verts.extend([v(x0, y0, u0, v0), v(x1, y0, u1, v0), v(x1, y1, u1, v1), v(x0, y0, u0, v0), v(x1, y1, u1, v1), v(x0, y1, u0, v1)]);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
        let t = 2.0 / ATLAS as f32;
        self.quad(x, y, x + w, y + h, t, t, t, t, c);
    }

    /// A rect with a vertical gradient (top colour to bottom colour), as two half-quads.
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, top: Rgba, bottom: Rgba) {
        let t = 2.0 / ATLAS as f32;
        let (a, b) = ([top.0, top.1, top.2, top.3], [bottom.0, bottom.1, bottom.2, bottom.3]);
        let v = |x, y, c| UiVertex { pos: [x, y], uv: [t, t], color: c };
        self.verts.extend([v(x, y, a), v(x + w, y, a), v(x + w, y + h, b), v(x, y, a), v(x + w, y + h, b), v(x, y + h, b)]);
    }

    pub fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, c: Rgba) {
        self.rect(x, y, w, t, c);
        self.rect(x, y + h - t, w, t, c);
        self.rect(x, y + t, t, h - 2.0 * t, c);
        self.rect(x + w - t, y + t, t, h - 2.0 * t, c);
    }

    /// Load every PNG in a folder as an image named by its file stem (e.g. assets/ui/buttons:
    /// "playstation_south"). Returns how many.
    pub fn load_images(&mut self, dir: &std::path::Path) -> anyhow::Result<usize> {
        let mut n = 0;
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            if p.extension().and_then(|x| x.to_str()) != Some("png") {
                continue;
            }
            let (w, h, rgba) = crate::textures::decode_png(&std::fs::read(&p)?)?;
            let name = p.file_stem().unwrap().to_string_lossy();
            if !self.font.add_image(&name, w, h, &rgba) {
                anyhow::bail!("UI atlas full at {}", p.display());
            }
            n += 1;
        }
        Ok(n)
    }

    /// The width an image takes drawn `h` high (0 if there is no such image).
    pub fn image_width(&self, name: &str, h: f32) -> f32 {
        self.font.images.get(name).map_or(0.0, |i| h * i.full[2] as f32 / i.full[3] as f32)
    }

    /// Draw an image `h` high with its top-left at (x, y), tinted (WHITE: as it is). Returns the
    /// width drawn (0 if there is no such image).
    pub fn image(&mut self, name: &str, x: f32, y: f32, h: f32, tint: Rgba) -> f32 {
        let Some(img) = self.font.images.get(name).copied() else { return 0.0 };
        let r = if h <= img.half[3] as f32 * 1.2 { img.half } else { img.full };
        let w = h * r[2] as f32 / r[3] as f32;
        let inv = 1.0 / ATLAS as f32;
        // (half a texel in: no bleeding from the neighbours)
        let (u0, v0) = ((r[0] as f32 + 0.5) * inv, (r[1] as f32 + 0.5) * inv);
        let (u1, v1) = ((r[0] + r[2]) as f32 * inv - 0.5 * inv, (r[1] + r[3]) as f32 * inv - 0.5 * inv);
        self.quad(x, y, x + w, y + h, u0, v0, u1, v1, tint);
        w
    }

    /// Width of a single line of text.
    pub fn measure(&mut self, px: f32, s: &str) -> f32 {
        let size = px.round().max(4.0) as u16;
        s.chars().map(|c| self.font.glyph(c, size).advance).sum()
    }

    /// One line of text with its top-left at (x, y). Returns the width.
    pub fn text(&mut self, x: f32, y: f32, px: f32, c: Rgba, s: &str) -> f32 {
        let size = px.round().max(4.0) as u16;
        let (ascent, _) = self.font.line(size as f32);
        let base = (y + ascent).round();
        let mut pen = x.round();
        let inv = 1.0 / ATLAS as f32;
        for ch in s.chars() {
            let g = self.font.glyph(ch, size);
            if g.w > 0 && g.h > 0 {
                let gx = pen + g.xmin;
                let gy = base - g.ymin - g.h as f32;
                self.quad(gx, gy, gx + g.w as f32, gy + g.h as f32, g.x as f32 * inv, g.y as f32 * inv, (g.x + g.w) as f32 * inv, (g.y + g.h) as f32 * inv, c);
            }
            pen += g.advance;
        }
        pen - x
    }

    /// Text with a soft drop shadow (readable over the 3D view).
    pub fn text_shadow(&mut self, x: f32, y: f32, px: f32, c: Rgba, s: &str) -> f32 {
        let o = (px / 18.0).max(1.0);
        self.text(x + o, y + o, px, Rgba(0, 0, 0, (c.3 as f32 * 0.7) as u8), s);
        self.text(x, y, px, c, s)
    }

    pub fn text_centered(&mut self, cx: f32, y: f32, px: f32, c: Rgba, s: &str) {
        let w = self.measure(px, s);
        self.text(cx - w * 0.5, y, px, c, s);
    }

    pub fn text_right(&mut self, rx: f32, y: f32, px: f32, c: Rgba, s: &str) {
        let w = self.measure(px, s);
        self.text(rx - w, y, px, c, s);
    }

    /// Word-wrapped text in a column of width `w`. Returns the height used.
    pub fn text_wrapped(&mut self, x: f32, y: f32, w: f32, px: f32, c: Rgba, s: &str) -> f32 {
        let (_, lh) = self.font.line(px.round());
        let mut yy = y;
        for para in s.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let trial = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if self.measure(px, &trial) > w && !line.is_empty() {
                    self.text(x, yy, px, c, &line);
                    yy += lh;
                    line = word.to_string();
                } else {
                    line = trial;
                }
            }
            self.text(x, yy, px, c, &line);
            yy += lh;
        }
        yy - y
    }

    pub fn line_height(&self, px: f32) -> f32 {
        self.font.line(px.round()).1
    }
}
