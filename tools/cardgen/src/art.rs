//! Card illustrations: load `assets/cards/art/Sxx/Sxx-NNN.png` or paint a procedural
//! placeholder landscape in the attribute's palette.

use crate::canvas::read_png;
use crate::math::*;
use rayon::prelude::*;
use std::path::Path;
use tcg::CardDef;

pub struct Art {
    pub w: usize,
    pub h: usize,
    pub px: Vec<V3>,
    pub placeholder: bool,
}

impl Art {
    /// Bilinear sample with UV in 0..1.
    pub fn sample(&self, u: f32, v: f32) -> V3 {
        let x = (u * self.w as f32 - 0.5).clamp(0.0, self.w as f32 - 1.001);
        let y = (v * self.h as f32 - 0.5).clamp(0.0, self.h as f32 - 1.001);
        let (xi, yi) = (x as usize, y as usize);
        let (fx, fy) = (x - xi as f32, y - yi as f32);
        let g = |x: usize, y: usize| self.px[y * self.w + x];
        let a = g(xi, yi).lerp(g(xi + 1, yi), fx);
        let b = g(xi, yi + 1).lerp(g(xi + 1, yi + 1), fx);
        a.lerp(b, fy)
    }
}

/// Load the card's illustration or paint a placeholder of `w` x `h` pixels (cover-cropped).
pub fn load(card: &CardDef, art_root: &Path, w: usize, h: usize) -> Art {
    let path = art_root.join(&card.art);
    if let Some((sw, sh, src)) = read_png(&path) {
        let src = Art { w: sw, h: sh, px: src, placeholder: false };
        // Cover crop: scale so the image fills w x h, centred.
        let s = (w as f32 / sw as f32).max(h as f32 / sh as f32);
        let (cw, ch) = (w as f32 / s / sw as f32, h as f32 / s / sh as f32);
        let (u0, v0) = ((1.0 - cw) * 0.5, (1.0 - ch) * 0.5);
        // Pre-filter when shrinking a lot by averaging a few taps.
        let taps = (1.0 / s).ceil().clamp(1.0, 4.0) as usize;
        let px: Vec<V3> = (0..w * h)
            .into_par_iter()
            .map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                let mut acc = V3::splat(0.0);
                for ty in 0..taps {
                    for tx in 0..taps {
                        let fx = (x + (tx as f32 + 0.5) / taps as f32) / w as f32;
                        let fy = (y + (ty as f32 + 0.5) / taps as f32) / h as f32;
                        acc = acc + src.sample(u0 + fx * cw, v0 + fy * ch);
                    }
                }
                acc * (1.0 / (taps * taps) as f32)
            })
            .collect();
        return Art { w, h, px, placeholder: false };
    }
    placeholder(card, w, h)
}

/// Empty placeholder: a plain fill in the attribute colour (soft radial falloff so the window
/// still reads as a recessed print). Real illustrations replace it when the files exist.
pub fn placeholder(card: &CardDef, w: usize, h: usize) -> Art {
    let base = srgb8(card.attribute.color());
    let light = base.lerp(V3::splat(1.0), 0.18) * 0.85;
    let dark = base * 0.38;
    let px: Vec<V3> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let u = ((i % w) as f32 + 0.5) / w as f32 - 0.5;
            let v = ((i / w) as f32 + 0.5) / h as f32 - 0.42;
            let r = (u * u * 1.6 + v * v).sqrt();
            light.lerp(dark, smoothstep(0.0, 0.75, r))
        })
        .collect();
    Art { w, h, px, placeholder: true }
}
