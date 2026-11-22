//! Vector motifs: attribute sigils, kind icons, rarity crests, traditional pattern engravings
//! and scroll ornaments. All return signed distances in pixels (negative = ink).

use crate::math::*;
use crate::sdf::*;
use std::f32::consts::{PI, TAU};
use tcg::{Attribute, Kind};

/// Closed Catmull-Rom smoothing of a control polygon.
pub fn smooth_closed(pts: &[P], sub: usize) -> Vec<P> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n * sub);
    for i in 0..n {
        let p0 = pts[(i + n - 1) % n];
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        let p3 = pts[(i + 2) % n];
        for s in 0..sub {
            let t = s as f32 / sub as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out
}

fn scale_pts(pts: &[P], c: P, r: f32) -> Vec<P> {
    pts.iter().map(|p| (c.0 + p.0 * r, c.1 + p.1 * r)).collect()
}

/// A prepared vector shape (pre-built point lists so per-pixel evaluation is cheap).
pub struct Shape {
    pub parts: Vec<Part>,
}

pub enum Part {
    /// Filled polygon (union).
    Fill(Vec<P>),
    /// Polygon cut out of everything before it.
    Cut(Vec<P>),
    /// Tapered stroke: points, start width, end width.
    Stroke(Vec<P>, f32, f32),
    /// Stroke that is cut out.
    CutStroke(Vec<P>, f32, f32),
    Circle(P, f32),
    CutCircle(P, f32),
    /// Cut a wavy band |y - (y0 + a sin(f x))| < w/2 (in pixels).
    CutWave { y0: f32, a: f32, f: f32, w: f32, x0: f32, x1: f32 },
}

impl Shape {
    pub fn dist(&self, p: P) -> f32 {
        let mut d = f32::MAX;
        for part in &self.parts {
            match part {
                Part::Fill(v) => d = d.min(poly(p, v)),
                Part::Cut(v) => d = d.max(-poly(p, v)),
                Part::Stroke(v, w0, w1) => {
                    let (dd, s) = polyline(p, v);
                    d = d.min(dd - mix(*w0, *w1, s) * 0.5);
                }
                Part::CutStroke(v, w0, w1) => {
                    let (dd, s) = polyline(p, v);
                    d = d.max(-(dd - mix(*w0, *w1, s) * 0.5));
                }
                Part::Circle(c, r) => d = d.min(circle(p, *c, *r)),
                Part::CutCircle(c, r) => d = d.max(-circle(p, *c, *r)),
                Part::CutWave { y0, a, f, w, x0, x1 } => {
                    if p.0 > *x0 && p.0 < *x1 {
                        let yy = y0 + a * (f * p.0).sin();
                        d = d.max(-((p.1 - yy).abs() - w * 0.5));
                    }
                }
            }
        }
        d
    }
}

/// Attribute sigil centred at c with radius r.
pub fn sigil(a: Attribute, c: P, r: f32) -> Shape {
    let sc = |v: &[P]| scale_pts(&smooth_closed(v, 8), c, r);
    let raw = |v: &[P]| scale_pts(v, c, r);
    let parts = match a {
        Attribute::Flame => vec![
            Part::Fill(sc(&[
                (0.10, -1.0), (0.30, -0.66), (0.58, -0.30), (0.70, 0.12), (0.58, 0.52), (0.30, 0.80), (0.0, 0.88),
                (-0.32, 0.80), (-0.60, 0.52), (-0.70, 0.14), (-0.58, -0.22), (-0.40, -0.44), (-0.30, -0.14),
                (-0.16, -0.48), (-0.04, -0.76),
            ])),
            Part::Cut(sc(&[(0.02, -0.28), (0.20, 0.04), (0.30, 0.38), (0.12, 0.62), (-0.14, 0.62), (-0.30, 0.38), (-0.18, 0.08)])),
        ],
        Attribute::Tide => vec![
            Part::Fill(sc(&[
                (0.0, -1.0), (0.30, -0.58), (0.58, -0.16), (0.70, 0.22), (0.58, 0.58), (0.30, 0.84), (0.0, 0.90),
                (-0.30, 0.84), (-0.58, 0.58), (-0.70, 0.22), (-0.58, -0.16), (-0.30, -0.58),
            ])),
            Part::CutWave { y0: c.1 + 0.30 * r, a: 0.09 * r, f: 7.0 / r, w: 0.13 * r, x0: c.0 - r, x1: c.0 + r },
            Part::CutStroke(
                (0..12).map(|i| {
                    let t = -2.3 + i as f32 * 0.07;
                    (c.0 + t.cos() * 0.42 * r, c.1 + 0.18 * r + t.sin() * 0.42 * r)
                }).collect(),
                0.10 * r,
                0.04 * r,
            ),
        ],
        Attribute::Gale => {
            let spiral: Vec<P> = (0..90)
                .map(|i| {
                    let t = i as f32 / 89.0;
                    let th = 0.4 + t * 2.7 * PI;
                    let rr = 0.86 * (-0.30 * (th - 0.4)).exp();
                    (c.0 + th.cos() * rr * r, c.1 + th.sin() * rr * r - 0.08 * r)
                })
                .collect();
            let streak = |y: f32, x0: f32, x1: f32| -> Vec<P> {
                (0..20).map(|i| {
                    let t = i as f32 / 19.0;
                    (c.0 + mix(x0, x1, t) * r, c.1 + (y + 0.06 * (t * PI).sin()) * r)
                }).collect()
            };
            vec![
                Part::Stroke(spiral, 0.24 * r, 0.06 * r),
                Part::Stroke(streak(0.80, -0.85, 0.35), 0.03 * r, 0.12 * r),
                Part::Stroke(streak(0.62, 0.10, 0.92), 0.10 * r, 0.03 * r),
            ]
        }
        Attribute::Terra => vec![
            Part::Fill(raw(&[(0.0, -1.0), (0.34, -0.62), (0.34, 0.52), (0.0, 0.92), (-0.34, 0.52), (-0.34, -0.62)])),
            Part::CutStroke(raw(&[(-0.34, -0.62), (0.0, -0.30), (0.34, -0.62)]), 0.07 * r, 0.07 * r),
            Part::CutStroke(raw(&[(0.0, -0.30), (0.0, 0.95)]), 0.07 * r, 0.07 * r),
            Part::Fill(raw(&[(-0.60, -0.20), (-0.42, -0.02), (-0.42, 0.56), (-0.60, 0.76), (-0.78, 0.56), (-0.78, -0.02)])),
            Part::Fill(raw(&[(0.60, 0.0), (0.76, 0.16), (0.76, 0.60), (0.60, 0.78), (0.44, 0.60), (0.44, 0.16)])),
        ],
        Attribute::Radiant => {
            let mut parts = vec![Part::Circle(c, 0.36 * r)];
            for i in 0..12 {
                let a = i as f32 * TAU / 12.0;
                let long = i % 3 == 0;
                let (r0, r1, w) = if long { (0.48, 1.0, 0.11) } else { (0.48, 0.76, 0.08) };
                let (s, co) = a.sin_cos();
                let tip = (c.0 + s * r1 * r, c.1 - co * r1 * r);
                let b0 = (c.0 + s * r0 * r + co * w * r, c.1 - co * r0 * r + s * w * r);
                let b1 = (c.0 + s * r0 * r - co * w * r, c.1 - co * r0 * r - s * w * r);
                parts.push(Part::Fill(vec![b0, tip, b1]));
            }
            parts.push(Part::CutCircle(c, 0.20 * r));
            parts.push(Part::Circle(c, 0.12 * r));
            parts
        }
        Attribute::Umbral => vec![
            Part::Circle((c.0 - 0.08 * r, c.1), 0.86 * r),
            Part::CutCircle((c.0 + 0.30 * r, c.1 - 0.22 * r), 0.70 * r),
            Part::Fill(star_pts((c.0 + 0.42 * r, c.1 + 0.02 * r), 4, 0.30 * r, 0.07 * r, 0.0)),
            Part::Fill(star_pts((c.0 + 0.12 * r, c.1 - 0.52 * r), 4, 0.14 * r, 0.04 * r, 0.0)),
        ],
    };
    Shape { parts }
}

/// Card-kind icon.
pub fn kind_icon(k: Kind, c: P, r: f32) -> Shape {
    let parts = match k {
        Kind::Monster => (0..3)
            .map(|i| {
                let o = (i as f32 - 1.0) * 0.42;
                let pts: Vec<P> = (0..24)
                    .map(|j| {
                        let t = j as f32 / 23.0;
                        let x = o + mix(-0.38, 0.38, t) + 0.16 * (t * PI).sin();
                        let y = mix(-0.88, 0.88, t) - 0.08 * i as f32 * (t * PI).sin();
                        (c.0 + x * r, c.1 + y * r)
                    })
                    .collect();
                Part::Stroke(pts, 0.05 * r, 0.24 * r)
            })
            .flat_map(|s| {
                // tapered both ends: split into two halves
                if let Part::Stroke(pts, _, _) = s {
                    let mid = pts.len() / 2;
                    vec![
                        Part::Stroke(pts[..=mid].to_vec(), 0.03 * r, 0.24 * r),
                        Part::Stroke(pts[mid..].to_vec(), 0.24 * r, 0.03 * r),
                    ]
                } else {
                    vec![s]
                }
            })
            .collect(),
        Kind::Spell => vec![
            Part::Fill(star_pts((c.0 - 0.08 * r, c.1 + 0.08 * r), 4, 0.92 * r, 0.22 * r, 0.0)),
            Part::Fill(star_pts((c.0 + 0.62 * r, c.1 - 0.60 * r), 4, 0.36 * r, 0.09 * r, 0.0)),
            Part::Circle((c.0 + 0.66 * r, c.1 + 0.56 * r), 0.11 * r),
        ],
        Kind::Relic => vec![
            Part::Fill(scale_pts(&[(-0.86, -0.28), (-0.50, -0.76), (0.50, -0.76), (0.86, -0.28), (0.0, 0.92)], c, r)),
            Part::CutStroke(scale_pts(&[(-0.86, -0.28), (0.86, -0.28)], c, r), 0.07 * r, 0.07 * r),
            Part::CutStroke(scale_pts(&[(-0.30, -0.28), (0.0, 0.92), (0.30, -0.28)], c, r), 0.06 * r, 0.06 * r),
            Part::CutStroke(scale_pts(&[(-0.50, -0.76), (-0.30, -0.28), (0.0, -0.76), (0.30, -0.28), (0.50, -0.76)], c, r), 0.06 * r, 0.06 * r),
        ],
    };
    Shape { parts }
}

/// Rarity crest outline. 0 Common circle, 1 Uncommon diamond, 2 Rare star, 3 Mythic 8-point
/// star, 4 Secret (gold) 8-point star with a cut ring.
pub fn crest(level: u8, c: P, r: f32) -> Shape {
    let parts = match level {
        0 => vec![Part::Circle(c, r)],
        1 => vec![Part::Fill(scale_pts(&[(0.0, -1.1), (0.92, 0.0), (0.0, 1.1), (-0.92, 0.0)], c, r))],
        2 => vec![Part::Fill(star_pts(c, 5, 1.1 * r, 0.48 * r, 0.0))],
        3 => vec![Part::Fill(star_pts(c, 8, 1.1 * r, 0.62 * r, 0.0))],
        _ => vec![
            Part::Fill(star_pts(c, 8, 1.15 * r, 0.66 * r, PI / 8.0)),
            Part::CutStroke(
                (0..=64).map(|i| {
                    let a = i as f32 / 64.0 * TAU;
                    (c.0 + a.cos() * 0.55 * r, c.1 + a.sin() * 0.55 * r)
                }).collect(),
                0.08 * r,
                0.08 * r,
            ),
        ],
    };
    Shape { parts }
}

/// A tapered spiral scroll (filigree curl) starting at `o`, heading along angle `a0`, curling
/// clockwise (dir = 1) or anticlockwise (dir = -1).
pub fn scroll(o: P, size: f32, a0: f32, dir: f32, w: f32) -> Part {
    let mut pts = Vec::with_capacity(48);
    // Walk along a curve whose curvature increases (a clothoid-like curl).
    let (mut x, mut y, mut a) = (o.0, o.1, a0);
    let n = 48;
    let step = size / n as f32 * 2.2;
    for i in 0..n {
        pts.push((x, y));
        let t = i as f32 / n as f32;
        a += dir * (0.02 + t * t * 0.32);
        x += a.cos() * step * (1.0 - 0.6 * t);
        y += a.sin() * step * (1.0 - 0.6 * t);
    }
    Part::Stroke(pts, w, w * 0.25)
}

/// Traditional pattern engraving for an attribute, evaluated at pixel p with cell size s.
/// Returns (signed distance of the ink in pixels, cell id 0..1).
pub fn pattern(a: Attribute, p: P, s: f32, line_w: f32) -> (f32, f32) {
    let (x, y) = p;
    match a {
        // Uroko: alternating filled triangles (fish/dragon scales).
        Attribute::Flame => {
            let th = s * 0.8660254;
            let v = y / th;
            let u = x / s;
            let aa = u - v * 0.5;
            let (ia, ib) = (aa.floor(), v.floor());
            let (fa, fb) = (aa - ia, v - ib);
            let up = fa + fb < 1.0;
            let d = if up { -(fa.min(fb).min(1.0 - fa - fb)) * th } else { (1.0 - fa).min(1.0 - fb).min(fa + fb - 1.0) * th };
            let id = hash2f(ia as i32 * 2 + if up { 0 } else { 1 }, ib as i32, 11);
            // inset so neighbouring triangles separate by a thin line
            (d + line_w * 0.5, id)
        }
        // Seigaiha: overlapping concentric wave arcs.
        Attribute::Tide => {
            let r = s * 0.5;
            let vs = r * 0.5;
            let row = (y / vs).floor() as i32;
            let mut best: Option<(f32, i32, i32)> = None;
            for j in (row - 1..=row + 3).rev() {
                let cy = j as f32 * vs;
                let off = if j.rem_euclid(2) == 1 { r } else { 0.0 };
                let i = ((x - off) / (2.0 * r)).round() as i32;
                for ii in [i - 1, i, i + 1] {
                    let cx = ii as f32 * 2.0 * r + off;
                    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                    if d < r && cy >= y - 0.001 {
                        if best.map_or(true, |b| j > b.1 || (j == b.1 && d < b.0)) {
                            best = Some((d, j, ii));
                        }
                    }
                }
                if best.is_some() {
                    break;
                }
            }
            match best {
                Some((d, j, i)) => {
                    let rings = 4.0;
                    let q = d / r * rings;
                    let ld = (q - q.round()).abs() * r / rings;
                    (ld.min((r - d).abs()) - line_w * 0.5, hash2f(i, j, 23))
                }
                None => (9.0, 0.0),
            }
        }
        // Tatewaku: rising wavy lines (steam / wind).
        Attribute::Gale => {
            let i0 = (x / s).round() as i32;
            let mut best = f32::MAX;
            let mut id = 0;
            for i in [i0 - 1, i0, i0 + 1] {
                let sign = if i.rem_euclid(2) == 0 { 1.0 } else { -1.0 };
                let amp = s * 0.28;
                let k = TAU / (s * 2.2);
                let xi = i as f32 * s + sign * amp * (k * y).sin();
                let slope = sign * amp * k * (k * y).cos();
                let d = (x - xi).abs() / (1.0 + slope * slope).sqrt();
                if d < best {
                    best = d;
                    id = i;
                }
            }
            let band = ((y / (s * 2.2) + if id.rem_euclid(2) == 0 { 0.0 } else { 0.5 }).floor()) as i32;
            (best - line_w * 0.5, hash2f(id, band, 37))
        }
        // Kikkō: tortoise-shell hexagons.
        Attribute::Terra => {
            let (ux, uy) = (x / s, y / s);
            let (rx, ry) = (1.0f32, 1.7320508f32);
            let m = |v: f32, d: f32| v - d * (v / d).floor();
            let a = (m(ux, rx) - rx * 0.5, m(uy, ry) - ry * 0.5);
            let b = (m(ux - rx * 0.5, rx) - rx * 0.5, m(uy - ry * 0.5, ry) - ry * 0.5);
            let g = if a.0 * a.0 + a.1 * a.1 < b.0 * b.0 + b.1 * b.1 { a } else { b };
            let (gx, gy) = (g.0.abs(), g.1.abs());
            let hd = (gx * 0.5 + gy * 0.8660254).max(gx);
            let edge = (0.5 - hd) * s;
            let cid = hash2f(((ux - g.0) * 2.0).round() as i32, ((uy - g.1) * 2.0).round() as i32, 41);
            // double line: outer hex edge plus an inner hex
            let inner = (edge - s * 0.12).abs();
            (edge.min(inner) - line_w * 0.5, cid)
        }
        // Asanoha: hemp-leaf stars on a triangular lattice.
        Attribute::Radiant => {
            let th = s * 0.8660254;
            let v = y / th;
            let aa = x / s - v * 0.5;
            let (ia, ib) = (aa.floor(), v.floor());
            let (fa, fb) = (aa - ia, v - ib);
            let up = fa + fb < 1.0;
            let lat = |i: f32, j: f32| ((i + j * 0.5) * s, j * th);
            let tri = if up {
                [lat(ia, ib), lat(ia + 1.0, ib), lat(ia, ib + 1.0)]
            } else {
                [lat(ia + 1.0, ib), lat(ia, ib + 1.0), lat(ia + 1.0, ib + 1.0)]
            };
            let cx = (tri[0].0 + tri[1].0 + tri[2].0) / 3.0;
            let cy = (tri[0].1 + tri[1].1 + tri[2].1) / 3.0;
            let e = if up { fa.min(fb).min(1.0 - fa - fb) } else { (1.0 - fa).min(1.0 - fb).min(fa + fb - 1.0) } * th;
            let mut d = e;
            for t in tri {
                d = d.min(seg(p, (cx, cy), t));
            }
            (d - line_w * 0.5, hash2f(ia as i32 * 2 + up as i32, ib as i32, 53))
        }
        // Shippō: interlocking circles.
        Attribute::Umbral => {
            let r = s * 0.5;
            let mut best = f32::MAX;
            let mut near = (0, 0);
            let mut nd = f32::MAX;
            for (ox, oy) in [(0.0, 0.0), (r, r)] {
                let i = ((x - ox) / (2.0 * r)).round();
                let j = ((y - oy) / (2.0 * r)).round();
                for dj in -1..=1 {
                    for di in -1..=1 {
                        let cx = (i + di as f32) * 2.0 * r + ox;
                        let cy = (j + dj as f32) * 2.0 * r + oy;
                        let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                        best = best.min((d - r).abs());
                        if d < nd {
                            nd = d;
                            near = (((cx / r).round()) as i32, ((cy / r).round()) as i32);
                        }
                    }
                }
            }
            (best - line_w * 0.5, hash2f(near.0, near.1, 67))
        }
    }
}

/// Guilloche rosette line distance at p around centre c: a family of `n` phase-shifted waves
/// r(θ) = R + amp·sin(lobes·θ + phase).
pub fn guilloche(p: P, c: P, radius: f32, amp: f32, lobes: f32, n: usize) -> f32 {
    let dx = p.0 - c.0;
    let dy = p.1 - c.1;
    let r = (dx * dx + dy * dy).sqrt();
    let th = dy.atan2(dx);
    let mut best = f32::MAX;
    for k in 0..n {
        let ph = k as f32 / n as f32 * TAU;
        let rr = radius + amp * (lobes * th + ph).sin();
        best = best.min((r - rr).abs());
    }
    best
}
