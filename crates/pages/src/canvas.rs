//! A page to draw on: rectangles, lines, circles and text (Japanese and English).

use std::sync::OnceLock;

pub type Rgba = [f32; 4];

/// Which typeface: printed (bold and regular) or the light one used for things written by hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Bold,
    Regular,
    Hand,
}

fn fonts() -> &'static [fontdue::Font; 3] {
    static F: OnceLock<[fontdue::Font; 3]> = OnceLock::new();
    F.get_or_init(|| {
        let load = |names: &[&str]| -> fontdue::Font {
            for n in names {
                if let Ok(b) = std::fs::read(n) {
                    if let Ok(f) = fontdue::Font::from_bytes(b, fontdue::FontSettings { collection_index: 0, scale: 64.0, ..Default::default() }) {
                        return f;
                    }
                }
            }
            panic!("no font found (tried {names:?})");
        };
        [
            load(&["C:/Windows/Fonts/YuGothB.ttc", "C:/Windows/Fonts/meiryob.ttc", "C:/Windows/Fonts/arialbd.ttf"]),
            load(&["C:/Windows/Fonts/YuGothM.ttc", "C:/Windows/Fonts/meiryo.ttc", "C:/Windows/Fonts/arial.ttf"]),
            // (writing by hand: the regular weight; a proper handwriting font can replace it)
            load(&["C:/Windows/Fonts/YuGothR.ttc", "C:/Windows/Fonts/YuGothL.ttc", "C:/Windows/Fonts/meiryo.ttc", "C:/Windows/Fonts/arial.ttf"]),
        ]
    })
}

/// An RGBA page (linear 0..1, straight alpha).
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Rgba>,
}

impl Canvas {
    pub fn new(w: usize, h: usize, paper: Rgba) -> Self {
        Canvas { w, h, px: vec![paper; w * h] }
    }

    fn blend(&mut self, x: i32, y: i32, c: Rgba, a: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let a = (a * c[3]).clamp(0.0, 1.0);
        let p = &mut self.px[y as usize * self.w + x as usize];
        for i in 0..3 {
            p[i] = p[i] * (1.0 - a) + c[i] * a;
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
        for yy in y.max(0.0) as i32..(y + h).min(self.h as f32) as i32 {
            for xx in x.max(0.0) as i32..(x + w).min(self.w as f32) as i32 {
                self.blend(xx, yy, c, 1.0);
            }
        }
    }

    /// A line `t` pixels thick (antialiased).
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, t: f32, c: Rgba) {
        let (minx, maxx) = (x0.min(x1) - t, x0.max(x1) + t);
        let (miny, maxy) = (y0.min(y1) - t, y0.max(y1) + t);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        for y in miny.floor() as i32..=maxy.ceil() as i32 {
            for x in minx.floor() as i32..=maxx.ceil() as i32 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let k = (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0);
                let d = ((px - x0 - k * dx).powi(2) + (py - y0 - k * dy).powi(2)).sqrt();
                let a = (t * 0.5 + 0.5 - d).clamp(0.0, 1.0);
                if a > 0.0 {
                    self.blend(x, y, c, a);
                }
            }
        }
    }

    /// A ring (a circle drawn round something), slightly uneven like a pencil.
    pub fn ring(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, t: f32, c: Rgba) {
        let n = 64;
        let mut prev: Option<(f32, f32)> = None;
        for i in 0..=n + 6 {
            let a = i as f32 / n as f32 * std::f32::consts::TAU + 0.3;
            let wob = 1.0 + 0.04 * (a * 3.0).sin() + 0.015 * i as f32 / n as f32;
            let p = (cx + a.cos() * rx * wob, cy + a.sin() * ry * wob);
            if let Some(q) = prev {
                self.line(q.0, q.1, p.0, p.1, t, c);
            }
            prev = Some(p);
        }
    }

    /// How wide a line of text is, in pixels.
    pub fn measure(&self, text: &str, font: Font, size: f32) -> f32 {
        let f = &fonts()[font as usize];
        text.chars().map(|ch| f.metrics(ch, size).advance_width).sum()
    }

    /// Text with its baseline at y; `align` 0 left, 0.5 centred, 1 right. Returns the width.
    pub fn text(&mut self, text: &str, x: f32, y: f32, font: Font, size: f32, c: Rgba, align: f32) -> f32 {
        let f = &fonts()[font as usize];
        let width = self.measure(text, font, size);
        let mut pen = x - width * align;
        for ch in text.chars() {
            let (m, bitmap) = f.rasterize(ch, size);
            let gx = pen + m.xmin as f32;
            let gy = y - m.height as f32 - m.ymin as f32;
            for j in 0..m.height {
                for i in 0..m.width {
                    let a = bitmap[j * m.width + i] as f32 / 255.0;
                    if a > 0.0 {
                        self.blend((gx + i as f32) as i32, (gy + j as f32) as i32, c, a);
                    }
                }
            }
            pen += m.advance_width;
        }
        width
    }

    /// Text wrapped to a width (at spaces, or anywhere for Japanese); returns the lines' count.
    pub fn wrapped(&mut self, text: &str, x: f32, y: f32, width: f32, font: Font, size: f32, leading: f32, c: Rgba) -> usize {
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in text.split_inclusive(' ') {
            let try_ = format!("{cur}{word}");
            if self.measure(try_.trim_end(), font, size) > width && !cur.is_empty() {
                lines.push(cur.trim_end().to_string());
                cur = word.to_string();
            } else {
                cur = try_;
            }
        }
        if !cur.trim().is_empty() {
            lines.push(cur.trim_end().to_string());
        }
        for (i, l) in lines.iter().enumerate() {
            self.text(l, x, y + i as f32 * leading, font, size, c, 0.0);
        }
        lines.len()
    }

    /// Light paper grain, so a page isn't flat.
    pub fn grain(&mut self, seed: u32, amount: f32) {
        let mut h = seed.wrapping_mul(2654435761);
        for p in self.px.iter_mut() {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            let n = (h % 1000) as f32 / 1000.0 - 0.5;
            for c in p.iter_mut().take(3) {
                *c = (*c + n * amount).clamp(0.0, 1.0);
            }
        }
    }

    /// As 8-bit sRGB RGBA.
    pub fn to_srgb8(&self) -> Vec<u8> {
        let enc = |v: f32| -> u8 {
            let v = v.clamp(0.0, 1.0);
            let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
            (s * 255.0 + 0.5) as u8
        };
        let mut out = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.px {
            out.extend_from_slice(&[enc(p[0]), enc(p[1]), enc(p[2]), (p[3].clamp(0.0, 1.0) * 255.0) as u8]);
        }
        out
    }

    pub fn save_png(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let f = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut enc = png::Encoder::new(f, self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(&self.to_srgb8())?;
        Ok(())
    }
}

/// sRGB 0-255 to a linear colour.
pub fn srgb(r: u8, g: u8, b: u8) -> Rgba {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    [f(r), f(g), f(b), 1.0]
}
