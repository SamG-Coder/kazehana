//! Signed distance functions in pixel space (negative inside) and coverage helpers.

pub type P = (f32, f32);

/// Anti-aliased coverage of an SDF value measured in pixels.
#[inline]
pub fn cov(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}
/// Coverage with a softer edge of `w` pixels.
#[inline]
pub fn cov_soft(d: f32, w: f32) -> f32 {
    (0.5 - d / w.max(1e-3)).clamp(0.0, 1.0)
}

/// Rounded rectangle centred at `c` with half size `h` and corner radius `r`.
pub fn rbox(p: P, c: P, h: P, r: f32) -> f32 {
    let qx = (p.0 - c.0).abs() - h.0 + r;
    let qy = (p.1 - c.1).abs() - h.1 + r;
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// Rounded rectangle from corners.
pub fn rrect(p: P, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    rbox(p, ((x0 + x1) * 0.5, (y0 + y1) * 0.5), ((x1 - x0) * 0.5, (y1 - y0) * 0.5), r)
}

/// Rectangle with chamfered (cut) corners of size `ch`.
pub fn chamfer(p: P, x0: f32, y0: f32, x1: f32, y1: f32, ch: f32) -> f32 {
    let c = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    let h = ((x1 - x0) * 0.5, (y1 - y0) * 0.5);
    let qx = (p.0 - c.0).abs();
    let qy = (p.1 - c.1).abs();
    let b = rbox(p, c, h, 0.0);
    let diag = (qx + qy - (h.0 + h.1 - ch)) * std::f32::consts::FRAC_1_SQRT_2;
    b.max(diag)
}

pub fn circle(p: P, c: P, r: f32) -> f32 {
    ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt() - r
}

/// Distance to segment ab and the parameter t along it.
pub fn seg_t(p: P, a: P, b: P) -> (f32, f32) {
    let (pax, pay) = (p.0 - a.0, p.1 - a.1);
    let (bax, bay) = (b.0 - a.0, b.1 - a.1);
    let l = (bax * bax + bay * bay).max(1e-9);
    let t = ((pax * bax + pay * bay) / l).clamp(0.0, 1.0);
    (((pax - bax * t).powi(2) + (pay - bay * t).powi(2)).sqrt(), t)
}
pub fn seg(p: P, a: P, b: P) -> f32 {
    seg_t(p, a, b).0
}

/// Signed distance to a closed polygon (any winding, may be concave).
pub fn poly(p: P, v: &[P]) -> f32 {
    let n = v.len();
    let mut d = (p.0 - v[0].0).powi(2) + (p.1 - v[0].1).powi(2);
    let mut s = 1.0f32;
    let mut j = n - 1;
    for i in 0..n {
        let e = (v[j].0 - v[i].0, v[j].1 - v[i].1);
        let w = (p.0 - v[i].0, p.1 - v[i].1);
        let t = ((w.0 * e.0 + w.1 * e.1) / (e.0 * e.0 + e.1 * e.1).max(1e-12)).clamp(0.0, 1.0);
        let b = (w.0 - e.0 * t, w.1 - e.1 * t);
        d = d.min(b.0 * b.0 + b.1 * b.1);
        let c1 = p.1 >= v[i].1;
        let c2 = p.1 < v[j].1;
        let c3 = e.0 * w.1 > e.1 * w.0;
        if (c1 && c2 && c3) || (!c1 && !c2 && !c3) {
            s = -s;
        }
        j = i;
    }
    s * d.sqrt()
}

/// Distance to an open polyline, plus the normalised arc position (0..1) of the nearest point.
pub fn polyline(p: P, v: &[P]) -> (f32, f32) {
    let mut total = 0.0;
    let mut lens = Vec::with_capacity(v.len());
    for w in v.windows(2) {
        let l = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        lens.push(l);
        total += l;
    }
    let (mut best, mut best_s, mut acc) = (f32::MAX, 0.0, 0.0);
    for (i, w) in v.windows(2).enumerate() {
        let (d, t) = seg_t(p, w[0], w[1]);
        if d < best {
            best = d;
            best_s = (acc + t * lens[i]) / total.max(1e-9);
        }
        acc += lens[i];
    }
    (best, best_s)
}

/// Regular star polygon points.
pub fn star_pts(c: P, n: usize, r_out: f32, r_in: f32, rot: f32) -> Vec<P> {
    (0..n * 2)
        .map(|i| {
            let a = rot + i as f32 * std::f32::consts::PI / n as f32;
            let r = if i % 2 == 0 { r_out } else { r_in };
            (c.0 + a.sin() * r, c.1 - a.cos() * r)
        })
        .collect()
}

pub fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

/// Height profile for a bevelled shape: 0 at the edge rising to 1 at `w` pixels inside.
pub fn bevel(d: f32, w: f32) -> f32 {
    let t = (-d / w).clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Rotate point p around c by angle a.
pub fn rot(p: P, c: P, a: f32) -> P {
    let (s, co) = a.sin_cos();
    let (x, y) = (p.0 - c.0, p.1 - c.1);
    (c.0 + x * co + y * s, c.1 - x * s + y * co)
}
