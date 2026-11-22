//! The layered card canvas: every pixel carries print colour plus the physical layers the 3D
//! engine needs (relief height, foil, metallic ink, gloss).

use crate::math::*;
use rayon::prelude::*;

/// Foil pattern ids written to the foil layer (G channel = id * 40).
pub mod foil_id {
    pub const NONE: u8 = 0;
    /// Holo art window: diagonal rainbow bands plus fine glitter.
    pub const AURORA: u8 = 1;
    /// Reverse holo frame: attribute pattern cells that flash independently.
    pub const SHATTER: u8 = 2;
    /// Full-art: fine etched flow lines over the whole card, textured.
    pub const ETCH: u8 = 3;
    /// Gold: metallic gold, hue fixed, brushed grain.
    pub const GILT: u8 = 4;
    /// Silver metallic stamp (crests, logo accents): neutral metal, faint rainbow.
    pub const SILVER: u8 = 5;
    /// Cosmos: nebula-tinted holo with dense star glitter.
    pub const COSMOS: u8 = 6;
    /// Cracked ice: shards with independent phases, crisp crack lines.
    pub const ICE: u8 = 7;
    /// Rainbow: strong pastel spectrum over the whole card.
    pub const RAINBOW: u8 = 8;
    /// Stained glass: per-pane colour that brightens as the pane catches the light.
    pub const GLASS: u8 = 9;
    /// Sakura: rose-gold petals.
    pub const SAKURA: u8 = 10;
    /// Obsidian: black mirror with a faint oil-slick sheen.
    pub const OBSIDIAN: u8 = 11;
    /// G channel scale: G = id * STEP.
    pub const STEP: u8 = 20;
}

#[derive(Clone, Copy, Debug)]
pub struct Px {
    /// Linear print colour (ink on card stock), before baked relief shading.
    pub col: V3,
    /// Design height (relief), arbitrary units, 0 = card surface.
    pub h: f32,
    /// How strongly the printed relief shading applies here (0 flat print, 1 strong).
    pub relief: f32,
    /// Printed metallic look baked into the colour (non-foil silver/gold ink).
    pub env: f32,
    pub foil: f32,
    pub fid: u8,
    pub phase: f32,
    /// Glitter seed (0 = none).
    pub spark: f32,
    /// Metallic ink (spec map R).
    pub metal: f32,
    /// Gloss varnish (spec map G).
    pub gloss: f32,
    /// Layout region id (see design::reg), used by the foil passes.
    pub reg: u8,
    /// Glow source intensity (printed energy glow in the card's glow colour).
    pub glow: f32,
}

impl Default for Px {
    fn default() -> Self {
        Px { col: V3::splat(0.0), h: 0.0, relief: 0.0, env: 0.0, foil: 0.0, fid: 0, phase: 0.0, spark: 0.0, metal: 0.0, gloss: 0.6, reg: 0, glow: 0.0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum H {
    Keep,
    Set(f32),
    Add(f32),
    Max(f32),
}

/// What a draw call writes. Unset fields are left alone.
#[derive(Clone, Copy, Debug)]
pub struct Paint {
    pub col: Option<V3>,
    pub h: H,
    pub relief: Option<f32>,
    pub env: Option<f32>,
    pub foil: Option<(f32, u8, f32, f32)>,
    pub metal: Option<f32>,
    pub gloss: Option<f32>,
    pub reg: Option<u8>,
    pub glow: Option<f32>,
    /// Multiply the existing colour (tinting) instead of replacing it.
    pub mul: Option<V3>,
}

impl Paint {
    pub fn none() -> Paint {
        Paint { col: None, h: H::Keep, relief: None, env: None, foil: None, metal: None, gloss: None, reg: None, glow: None, mul: None }
    }
    /// Opaque ink: also clears foil underneath (callers re-add foil explicitly).
    pub fn col(c: V3) -> Paint {
        Paint { col: Some(c), foil: Some((0.0, foil_id::NONE, 0.0, 0.0)), glow: Some(0.0), ..Paint::none() }
    }
    pub fn h(mut self, h: H) -> Paint {
        self.h = h;
        self
    }
    pub fn relief(mut self, r: f32) -> Paint {
        self.relief = Some(r);
        self
    }
    pub fn env(mut self, e: f32) -> Paint {
        self.env = Some(e);
        self
    }
    pub fn foil(mut self, s: f32, id: u8, phase: f32, spark: f32) -> Paint {
        self.foil = Some((s, id, phase, spark));
        self
    }
    pub fn metal(mut self, m: f32) -> Paint {
        self.metal = Some(m);
        self
    }
    /// Set colour but leave foil untouched (overprints, tints over foil).
    pub fn col_keep_foil(mut self, c: V3) -> Paint {
        self.col = Some(c);
        self
    }
    pub fn col_mul(mut self, m: V3) -> Paint {
        self.mul = Some(m);
        self
    }
    /// Glow source that only raises the glow (thin lines over other glow).
    pub fn glow_max(mut self, g: f32) -> Paint {
        self.glow = Some(-g);
        self
    }
    pub fn glow(mut self, g: f32) -> Paint {
        self.glow = Some(g);
        self
    }
    pub fn reg(mut self, r: u8) -> Paint {
        self.reg = Some(r);
        self
    }
    pub fn gloss(mut self, g: f32) -> Paint {
        self.gloss = Some(g);
        self
    }
}

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Px>,
    /// Card shape coverage.
    pub alpha: Vec<f32>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { w, h, px: vec![Px::default(); w * h], alpha: vec![0.0; w * h] }
    }

    /// Paint over an axis-aligned pixel box. `f(x, y)` returns coverage and paint for a pixel
    /// centre, or None to skip.
    pub fn paint<F>(&mut self, bbox: (f32, f32, f32, f32), f: F)
    where
        F: Fn(f32, f32) -> Option<(f32, Paint)> + Sync,
    {
        let x0 = (bbox.0.floor().max(0.0)) as usize;
        let y0 = (bbox.1.floor().max(0.0)) as usize;
        let x1 = (bbox.2.ceil().min(self.w as f32)).max(0.0) as usize;
        let y1 = (bbox.3.ceil().min(self.h as f32)).max(0.0) as usize;
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let w = self.w;
        self.px[y0 * w..y1 * w].par_chunks_mut(w).enumerate().for_each(|(j, row)| {
            let y = (y0 + j) as f32 + 0.5;
            for x in x0..x1 {
                if let Some((c, p)) = f(x as f32 + 0.5, y) {
                    if c > 0.0 {
                        blend(&mut row[x], c.min(1.0), &p);
                    }
                }
            }
        });
    }

    pub fn full(&self) -> (f32, f32, f32, f32) {
        (0.0, 0.0, self.w as f32, self.h as f32)
    }

    pub fn at(&self, x: usize, y: usize) -> &Px {
        &self.px[y * self.w + x]
    }
}

#[inline]
fn blend(px: &mut Px, c: f32, p: &Paint) {
    if let Some(col) = p.col {
        px.col = px.col.lerp(col, c);
    }
    if let Some(m) = p.mul {
        px.col = px.col.lerp(px.col * m, c);
    }
    match p.h {
        H::Keep => {}
        H::Set(v) => px.h = mix(px.h, v, c),
        H::Add(v) => px.h += v * c,
        H::Max(v) => px.h = mix(px.h, px.h.max(v), c),
    }
    if let Some(r) = p.relief {
        px.relief = mix(px.relief, r, c);
    }
    if let Some(e) = p.env {
        px.env = mix(px.env, e, c);
    }
    if let Some((s, id, ph, sp)) = p.foil {
        px.foil = mix(px.foil, s, c);
        if c >= 0.5 {
            px.fid = id;
            px.phase = ph;
            px.spark = sp;
        }
    }
    if let Some(m) = p.metal {
        px.metal = mix(px.metal, m, c);
    }
    if let Some(g) = p.gloss {
        px.gloss = mix(px.gloss, g, c);
    }
    if let Some(g) = p.glow {
        px.glow = if g < 0.0 { px.glow.max(-g * c) } else { mix(px.glow, g, c) };
    }
    if let Some(r) = p.reg {
        if c >= 0.5 {
            px.reg = r;
        }
    }
}

/// A single-channel coverage mask placed at (x0, y0) in canvas pixels.
#[derive(Clone)]
pub struct Mask {
    pub x0: i32,
    pub y0: i32,
    pub w: usize,
    pub h: usize,
    pub d: Vec<f32>,
}

impl Mask {
    pub fn new(x0: i32, y0: i32, w: usize, h: usize) -> Mask {
        Mask { x0, y0, w, h, d: vec![0.0; w * h] }
    }
    pub fn get(&self, x: i32, y: i32) -> f32 {
        let lx = x - self.x0;
        let ly = y - self.y0;
        if lx < 0 || ly < 0 || lx >= self.w as i32 || ly >= self.h as i32 {
            0.0
        } else {
            self.d[ly as usize * self.w + lx as usize]
        }
    }
    /// Sample at a pixel centre position (canvas coords).
    pub fn at(&self, x: f32, y: f32) -> f32 {
        self.get(x.floor() as i32, y.floor() as i32)
    }
    /// Bilinear sample at an arbitrary position.
    pub fn bilinear(&self, x: f32, y: f32) -> f32 {
        let x = x - 0.5;
        let y = y - 0.5;
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let fx = x - xi as f32;
        let fy = y - yi as f32;
        let a = self.get(xi, yi);
        let b = self.get(xi + 1, yi);
        let c = self.get(xi, yi + 1);
        let d = self.get(xi + 1, yi + 1);
        mix(mix(a, b, fx), mix(c, d, fx), fy)
    }
    /// Grow the mask by `pad` pixels on every side (for blurs and glows).
    pub fn padded(&self, pad: usize) -> Mask {
        let mut m = Mask::new(self.x0 - pad as i32, self.y0 - pad as i32, self.w + pad * 2, self.h + pad * 2);
        for y in 0..self.h {
            for x in 0..self.w {
                m.d[(y + pad) * m.w + x + pad] = self.d[y * self.w + x];
            }
        }
        m
    }
    /// Approximate gaussian blur (3 box passes).
    pub fn blur(&self, r: f32) -> Mask {
        let mut m = self.padded((r * 2.0).ceil() as usize + 1);
        let b = ((r * 0.9).round() as usize).max(1);
        for _ in 0..3 {
            m = m.box_h(b).box_v(b);
        }
        m
    }
    fn box_h(&self, r: usize) -> Mask {
        let mut o = self.clone();
        let n = (2 * r + 1) as f32;
        o.d.par_chunks_mut(self.w).enumerate().for_each(|(y, row)| {
            let src = &self.d[y * self.w..(y + 1) * self.w];
            let mut acc = 0.0;
            for i in 0..=r.min(self.w - 1) {
                acc += src[i];
            }
            for x in 0..self.w {
                row[x] = acc / n;
                let add = x + r + 1;
                if add < self.w {
                    acc += src[add];
                }
                if x >= r {
                    acc -= src[x - r];
                }
            }
        });
        o
    }
    fn box_v(&self, r: usize) -> Mask {
        // Transpose, blur rows, transpose back.
        let t = self.transpose().box_h(r);
        t.transpose()
    }
    fn transpose(&self) -> Mask {
        let mut m = Mask::new(self.y0, self.x0, self.h, self.w);
        for y in 0..self.h {
            for x in 0..self.w {
                m.d[x * self.h + y] = self.d[y * self.w + x];
            }
        }
        m
    }
    pub fn bbox(&self, pad: f32) -> (f32, f32, f32, f32) {
        (self.x0 as f32 - pad, self.y0 as f32 - pad, (self.x0 + self.w as i32) as f32 + pad, (self.y0 + self.h as i32) as f32 + pad)
    }
    pub fn translate(mut self, dx: i32, dy: i32) -> Mask {
        self.x0 += dx;
        self.y0 += dy;
        self
    }
    pub fn union(&mut self, o: &Mask) {
        for y in 0..self.h {
            for x in 0..self.w {
                let v = o.get(self.x0 + x as i32, self.y0 + y as i32);
                let i = y * self.w + x;
                self.d[i] = self.d[i].max(v);
            }
        }
    }
}

/// Write an 8-bit RGBA PNG.
pub fn write_rgba(path: &std::path::Path, w: usize, h: usize, data: &[u8]) {
    let f = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(data).unwrap();
}

/// Write an 8-bit RGB PNG.
pub fn write_rgb(path: &std::path::Path, w: usize, h: usize, data: &[u8]) {
    let f = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(data).unwrap();
}

/// Write a 16-bit greyscale PNG.
pub fn write_gray16(path: &std::path::Path, w: usize, h: usize, v: &[f32]) {
    let f = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Sixteen);
    enc.set_compression(png::Compression::Fast);
    let mut wr = enc.write_header().unwrap();
    let mut data = Vec::with_capacity(w * h * 2);
    for &x in v {
        let q = (x.clamp(0.0, 1.0) * 65535.0).round() as u16;
        data.extend_from_slice(&q.to_be_bytes());
    }
    wr.write_image_data(&data).unwrap();
}

/// Load a PNG as linear RGB + alpha.
pub fn read_png(path: &std::path::Path) -> Option<(usize, usize, Vec<V3>)> {
    let f = std::fs::File::open(path).ok()?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(f));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let ch = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        _ => return None,
    };
    let mut out = Vec::with_capacity(w * h);
    for i in 0..w * h {
        let p = &buf[i * ch..];
        let c = if ch >= 3 { [p[0], p[1], p[2]] } else { [p[0], p[0], p[0]] };
        out.push(srgb8(c));
    }
    Some((w, h, out))
}
