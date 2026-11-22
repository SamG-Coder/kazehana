//! Small vector maths, colour helpers, hashing and noise.

use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3(pub f32, pub f32, pub f32);

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        V3(self.0 + o.0, self.1 + o.1, self.2 + o.2)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        V3(self.0 - o.0, self.1 - o.1, self.2 - o.2)
    }
}
impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, s: f32) -> V3 {
        V3(self.0 * s, self.1 * s, self.2 * s)
    }
}
impl Mul<V3> for V3 {
    type Output = V3;
    fn mul(self, o: V3) -> V3 {
        V3(self.0 * o.0, self.1 * o.1, self.2 * o.2)
    }
}

impl V3 {
    pub const fn splat(v: f32) -> V3 {
        V3(v, v, v)
    }
    pub fn dot(self, o: V3) -> f32 {
        self.0 * o.0 + self.1 * o.1 + self.2 * o.2
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> V3 {
        let l = self.len().max(1e-9);
        self * (1.0 / l)
    }
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
    pub fn luma(self) -> f32 {
        0.2126 * self.0 + 0.7152 * self.1 + 0.0722 * self.2
    }
    pub fn max_c(self) -> f32 {
        self.0.max(self.1).max(self.2)
    }
    pub fn clamp01(self) -> V3 {
        V3(self.0.clamp(0.0, 1.0), self.1.clamp(0.0, 1.0), self.2.clamp(0.0, 1.0))
    }
    /// Change saturation around luma (s = 0 grey, 1 unchanged).
    pub fn sat(self, s: f32) -> V3 {
        let l = V3::splat(self.luma());
        l.lerp(self, s)
    }
}

/// sRGB byte colour to linear.
pub fn srgb8(c: [u8; 3]) -> V3 {
    V3(to_lin(c[0] as f32 / 255.0), to_lin(c[1] as f32 / 255.0), to_lin(c[2] as f32 / 255.0))
}
pub fn hex(h: u32) -> V3 {
    srgb8([(h >> 16) as u8, (h >> 8) as u8, h as u8])
}
pub fn to_lin(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
pub fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
pub fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
pub fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// Hue (0..1) to a soft spectral colour, linear.
pub fn spectral(h: f32) -> V3 {
    let h = fract(h) * 6.0;
    let r = (h - 3.0).abs() - 1.0;
    let g = 2.0 - (h - 2.0).abs();
    let b = 2.0 - (h - 4.0).abs();
    V3(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

// ---------------------------------------------------------------- hashing / noise

pub fn hash_u(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^= x >> 16;
    x
}
pub fn hash2i(x: i32, y: i32, seed: u32) -> u32 {
    hash_u((x as u32).wrapping_mul(0x9E3779B1) ^ hash_u((y as u32).wrapping_add(seed.wrapping_mul(0x85EBCA77))))
}
pub fn hash2f(x: i32, y: i32, seed: u32) -> f32 {
    (hash2i(x, y, seed) >> 8) as f32 / 16_777_216.0
}

/// Smooth value noise, 0..1.
pub fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let fx = x - xi;
    let fy = y - yi;
    let (xi, yi) = (xi as i32, yi as i32);
    let u = fx * fx * (3.0 - 2.0 * fx);
    let v = fy * fy * (3.0 - 2.0 * fy);
    let a = hash2f(xi, yi, seed);
    let b = hash2f(xi + 1, yi, seed);
    let c = hash2f(xi, yi + 1, seed);
    let d = hash2f(xi + 1, yi + 1, seed);
    mix(mix(a, b, u), mix(c, d, u), v)
}

/// Fractal value noise, roughly 0..1.
pub fn fbm(x: f32, y: f32, oct: u32, seed: u32) -> f32 {
    let (mut s, mut a, mut f, mut n) = (0.0, 0.5, 1.0, 0.0);
    for i in 0..oct {
        s += a * vnoise(x * f + i as f32 * 17.3, y * f - i as f32 * 9.1, seed.wrapping_add(i * 101));
        n += a;
        a *= 0.5;
        f *= 2.03;
    }
    s / n
}

/// Voronoi: (distance to nearest, distance to second nearest, nearest cell hash 0..1).
pub fn voronoi(x: f32, y: f32, seed: u32) -> (f32, f32, f32) {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let (mut d1, mut d2, mut id) = (9.0f32, 9.0f32, 0.0f32);
    for j in -1..=1 {
        for i in -1..=1 {
            let cx = xi + i;
            let cy = yi + j;
            let px = cx as f32 + 0.15 + 0.7 * hash2f(cx, cy, seed);
            let py = cy as f32 + 0.15 + 0.7 * hash2f(cx, cy, seed ^ 0x5bd1e995);
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = hash2f(cx, cy, seed ^ 0x1234567);
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    (d1, d2, id)
}
