//! Text set with fontdue into coverage masks.

use crate::canvas::Mask;
use fontdue::{Font, FontSettings};

pub struct Fonts {
    /// Card names: Palatino Linotype Bold.
    pub name: Font,
    /// Type line, labels: Franklin Gothic Medium.
    pub label: Font,
    /// Flavour text: Palatino Linotype Italic.
    pub italic: Font,
    /// Fine print: Bahnschrift (default instance).
    pub fine: Font,
    /// Logo: Palatino Linotype Bold (tracked caps).
    pub logo: Font,
    /// Japanese: Yu Gothic Bold.
    pub jp: Font,
}

fn load(path: &str, index: u32) -> Font {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("font {path}: {e}"));
    Font::from_bytes(bytes, FontSettings { collection_index: index, scale: 64.0, ..Default::default() })
        .unwrap_or_else(|e| panic!("font {path}: {e}"))
}

impl Fonts {
    pub fn load() -> Fonts {
        let dir = "C:/Windows/Fonts";
        Fonts {
            name: load(&format!("{dir}/palab.ttf"), 0),
            label: load(&format!("{dir}/framd.ttf"), 0),
            italic: load(&format!("{dir}/palai.ttf"), 0),
            fine: load(&format!("{dir}/bahnschrift.ttf"), 0),
            logo: load(&format!("{dir}/palab.ttf"), 0),
            jp: load(&format!("{dir}/YuGothB.ttc"), 0),
        }
    }
}

/// Measure the advance width of a string.
pub fn measure(font: &Font, size: f32, text: &str, tracking: f32) -> f32 {
    let mut w = 0.0;
    let mut prev: Option<char> = None;
    let n = text.chars().count();
    for (i, c) in text.chars().enumerate() {
        if let Some(p) = prev {
            w += font.horizontal_kern(p, c, size).unwrap_or(0.0);
        }
        w += font.metrics(c, size).advance_width;
        if i + 1 < n {
            w += tracking;
        }
        prev = Some(c);
    }
    w
}

/// Render a single line into a mask whose origin is the pen start on the baseline at (x, y).
/// `sx` squeezes the text horizontally (1 = normal).
pub fn line(font: &Font, size: f32, text: &str, tracking: f32, x: f32, y: f32, sx: f32) -> Mask {
    let width = measure(font, size, text, tracking) * sx;
    let lm = font.horizontal_line_metrics(size);
    let (asc, desc) = lm.map(|l| (l.ascent, l.descent)).unwrap_or((size, -size * 0.3));
    let pad = 4;
    let x0 = x.floor() as i32 - pad;
    let y0 = (y - asc).floor() as i32 - pad;
    let w = (width + size * 0.6).ceil() as usize + pad as usize * 2;
    let h = (asc - desc).ceil() as usize + pad as usize * 2 + 2;
    let mut m = Mask::new(x0, y0, w, h);
    let mut pen = x;
    let mut prev: Option<char> = None;
    for c in text.chars() {
        if let Some(p) = prev {
            pen += font.horizontal_kern(p, c, size).unwrap_or(0.0) * sx;
        }
        let (met, bmp) = font.rasterize(c, size);
        // Glyph placement; squeeze by resampling columns.
        let gx = pen + met.xmin as f32 * sx;
        let gy = y - met.height as f32 - met.ymin as f32;
        if met.width > 0 {
            let out_w = (met.width as f32 * sx).ceil() as usize + 1;
            for row in 0..met.height {
                for ox in 0..out_w {
                    // fractional source position
                    let sxp = (ox as f32 + 0.5) / sx - 0.5;
                    let a = sxp.floor();
                    let f = sxp - a;
                    let get = |i: i32| -> f32 {
                        if i < 0 || i >= met.width as i32 { 0.0 } else { bmp[row * met.width + i as usize] as f32 / 255.0 }
                    };
                    let v = get(a as i32) * (1.0 - f) + get(a as i32 + 1) * f;
                    if v <= 0.0 {
                        continue;
                    }
                    let px = (gx.round() as i32) + ox as i32 - m.x0;
                    let py = (gy.round() as i32) + row as i32 - m.y0;
                    if px >= 0 && py >= 0 && (px as usize) < m.w && (py as usize) < m.h {
                        let i = py as usize * m.w + px as usize;
                        m.d[i] = (m.d[i] + v).min(1.0);
                    }
                }
            }
        }
        pen += (met.advance_width + tracking) * sx;
        prev = Some(c);
    }
    m
}

/// Fit text into `max_w`: shrink the size down to `min_size`, then squeeze horizontally.
/// Returns (size, squeeze).
pub fn fit(font: &Font, size: f32, min_size: f32, text: &str, tracking: f32, max_w: f32) -> (f32, f32) {
    let mut s = size;
    while s > min_size && measure(font, s, text, tracking) > max_w {
        s -= 1.0;
    }
    let w = measure(font, s, text, tracking);
    let sx = if w > max_w { (max_w / w).max(0.72) } else { 1.0 };
    (s, sx)
}

/// Greedy word wrap.
pub fn wrap(font: &Font, size: f32, text: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if measure(font, size, &trial, 0.0) > max_w && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = trial;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
