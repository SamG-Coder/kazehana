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

    /// One glyph at (x, baseline y), scaled; returns its advance.
    fn glyph(&mut self, ch: char, x: f32, y: f32, font: Font, size: f32, c: Rgba) -> f32 {
        let f = &fonts()[font as usize];
        let (m, bitmap) = f.rasterize(ch, size);
        let gx = x + m.xmin as f32;
        let gy = y - m.height as f32 - m.ymin as f32;
        for j in 0..m.height {
            for i in 0..m.width {
                let a = bitmap[j * m.width + i] as f32 / 255.0;
                if a > 0.0 {
                    self.blend((gx + i as f32) as i32, (gy + j as f32) as i32, c, a);
                }
            }
        }
        m.advance_width
    }

    /// Writing by hand (pencil, pen): each character a touch off the line and a touch bigger or
    /// smaller, the same for the same `seed`. Returns the width.
    pub fn hand(&mut self, text: &str, x: f32, y: f32, size: f32, c: Rgba, seed: u32) -> f32 {
        let mut h = seed.wrapping_mul(2654435761) | 1;
        let mut rnd = move || {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            (h % 1000) as f32 / 1000.0 - 0.5
        };
        let mut pen = x;
        for ch in text.chars() {
            let k = 1.0 + rnd() * 0.08;
            let dy = rnd() * size * 0.07;
            pen += self.glyph(ch, pen, y + dy, Font::Hand, size * k, c) * 0.97;
        }
        pen - x
    }

    /// How wide handwriting is (about).
    pub fn hand_width(&self, text: &str, size: f32) -> f32 {
        self.measure(text, Font::Hand, size) * 0.97
    }

    /// Handwriting wrapped to a width; returns the lines' count.
    pub fn hand_wrapped(&mut self, text: &str, x: f32, y: f32, width: f32, size: f32, leading: f32, c: Rgba, seed: u32) -> usize {
        let lines = self.wrap(text, Font::Hand, size, width);
        for (i, l) in lines.iter().enumerate() {
            self.hand(l, x, y + i as f32 * leading, size, c, seed.wrapping_add(i as u32 * 7919));
        }
        lines.len()
    }

    /// Lines of text wrapped to a width: at spaces, or between any two characters where there
    /// are none (Japanese).
    pub fn wrap(&self, text: &str, font: Font, size: f32, width: f32) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        let cjk = |ch: char| ch as u32 >= 0x3000;
        let mut tokens: Vec<String> = Vec::new();
        for word in text.split_inclusive(' ') {
            if word.chars().any(cjk) {
                tokens.extend(word.chars().map(|c| c.to_string()));
            } else {
                tokens.push(word.to_string());
            }
        }
        // (kinsoku: closing punctuation never starts a line)
        let closing = |t: &str| matches!(t, "。" | "、" | "」" | "』" | "）" | "！" | "？" | "ー" | "ゃ" | "ゅ" | "ょ" | "っ" | "." | "," | ")");
        for t in tokens {
            let try_ = format!("{cur}{t}");
            if self.measure(try_.trim_end(), font, size) > width && !cur.trim().is_empty() && !closing(t.trim()) {
                lines.push(cur.trim_end().to_string());
                cur = t.trim_start().to_string();
            } else {
                cur = try_;
            }
        }
        if !cur.trim().is_empty() {
            lines.push(cur.trim_end().to_string());
        }
        lines
    }

    /// Text turned sideways (a quarter turn clockwise), as Latin letters run in vertical writing:
    /// its top at (cx + size/2), starting at y. Returns its length down the page.
    fn sideways(&mut self, text: &str, cx: f32, y: f32, font: Font, size: f32, c: Rgba, hand: Option<u32>) -> f32 {
        let w = (self.measure(text, font, size) * 1.05 + 4.0).ceil() as usize;
        let h = (size * 1.35).ceil() as usize;
        let mut m = Canvas::new(w.max(1), h, [0.0, 0.0, 0.0, 1.0]);
        let white = [1.0, 1.0, 1.0, 1.0];
        let len = match hand {
            Some(seed) => m.hand(text, 0.0, size, size, white, seed),
            None => m.text(text, 0.0, size, font, size, white, 0.0),
        };
        // (temp pixel (u, v) -> page (cx + h/2 - v, y + u): the letters' tops face right)
        let x0 = cx + h as f32 * 0.5 - size * 0.12;
        for v in 0..h {
            for u in 0..w {
                let a = m.px[v * w + u][0];
                if a > 0.0 {
                    self.blend((x0 - v as f32) as i32, (y + u as f32) as i32, c, a);
                }
            }
        }
        len
    }

    /// Vertical writing (tategaki): down a column centred on `cx` from `top` to `bottom`, the next
    /// column to the left (`gap` apart). Brackets and the long-vowel mark turn to their vertical
    /// forms; small punctuation sits at the top right of its cell; runs of Latin letters lie on
    /// their side, and one or two digits stand upright. `hand`: written by hand (with that
    /// seed). Returns the columns used.
    pub fn vertical(&mut self, text: &str, cx: f32, top: f32, bottom: f32, font: Font, size: f32, gap: f32, c: Rgba, hand: Option<u32>) -> usize {
        let mut h = hand.unwrap_or(1).wrapping_mul(2654435761) | 1;
        let mut rnd = move || {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            (h % 1000) as f32 / 1000.0 - 0.5
        };
        // split into single characters and runs of Latin text
        let latin = |ch: char| ch.is_ascii() && !matches!(ch, '(' | ')' | ',' | '.' | '~' | '-');
        let mut tokens: Vec<String> = Vec::new();
        for ch in text.chars() {
            match tokens.last_mut() {
                Some(t) if latin(ch) && t.chars().all(latin) => t.push(ch),
                _ => tokens.push(ch.to_string()),
            }
        }
        let (mut x, mut y, mut cols) = (cx, top, 1);
        let step = size * 1.06;
        let f = if hand.is_some() { Font::Hand } else { font };
        for tok in tokens {
            let first = tok.chars().next().unwrap();
            let run = tok.chars().all(latin) && !(tok.chars().count() <= 2 && tok.chars().all(|c| c.is_ascii_digit()));
            if run {
                let t = tok.trim();
                if t.is_empty() {
                    y += step * 0.4;
                    continue;
                }
                // (a long run breaks at its spaces across columns)
                let mut words: Vec<&str> = t.split_inclusive(' ').collect();
                while !words.is_empty() {
                    let mut piece = String::new();
                    while let Some(w) = words.first() {
                        let try_ = format!("{piece}{w}");
                        if y + self.measure(try_.trim_end(), f, size * 0.92) > bottom && !piece.is_empty() {
                            break;
                        }
                        piece = try_;
                        words.remove(0);
                    }
                    if y + self.measure(piece.trim_end(), f, size * 0.92) > bottom && y > top {
                        x -= gap;
                        y = top;
                        cols += 1;
                    }
                    let len = self.sideways(piece.trim_end(), x, y, f, size * 0.92, c, hand);
                    y += len + size * 0.15;
                    if !words.is_empty() {
                        x -= gap;
                        y = top;
                        cols += 1;
                    }
                }
                continue;
            }
            for ch in tok.chars() {
                if ch == ' ' && y == top {
                    continue;
                }
                // (kinsoku: closing marks stay at the foot of their column, never start one)
                let closing = matches!(ch, '）' | ')' | '」' | '』' | '。' | '、' | '，' | '．' | ',' | '.');
                if y + step > bottom && !closing {
                    x -= gap;
                    y = top;
                    cols += 1;
                }
                let (ch, small) = match ch {
                    'ー' | '－' | '—' | '-' => ('｜', false),
                    '（' | '(' => ('︵', false),
                    '）' | ')' => ('︶', false),
                    '「' => ('﹁', false),
                    '」' => ('﹂', false),
                    '『' => ('﹃', false),
                    '』' => ('﹄', false),
                    '〜' | '~' => ('｜', false),
                    '…' => ('︙', false),
                    '、' | '。' | '，' | '．' | ',' | '.' => (ch, true),
                    ' ' | '　' => {
                        y += step * if ch == ' ' { 0.5 } else { 1.0 };
                        continue;
                    }
                    c => (c, false),
                };
                let k = if hand.is_some() { 1.0 + rnd() * 0.08 } else { 1.0 };
                let jx = if hand.is_some() { rnd() * size * 0.08 } else { 0.0 };
                let adv = fonts()[f as usize].metrics(ch, size * k).advance_width;
                let (ox, oy) = if small { (size * 0.55, -size * 0.55) } else { (0.0, 0.0) };
                self.glyph(ch, x - adv / 2.0 + jx + ox, y + size * 0.88 + oy, f, size * k, c);
                y += step;
            }
            let _ = first;
        }
        cols
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
