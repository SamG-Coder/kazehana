//! Collision: solid shapes placed in the world, and the queries a body needs.
//!
//! A [`Shape`] is a triangle mesh in its own space with a grid over it (built once per model and
//! shared). A body places a shape with a rigid transform (rotation, translation, a uniform scale,
//! possibly mirrored). Static bodies sit in a coarse world grid; moving bodies (doors) are few and
//! are checked one by one. The ground's height (a terrain's heightfield) can be given as a
//! function: it is a floor everywhere.
//!
//! Queries work in each shape's own space (the query is moved into it), so a model's triangles
//! are stored once however often it is placed.

use glam::{Affine3A, Vec3};
use std::collections::HashMap;
use std::sync::Arc;

/// Shape grid cell (m, the shape's own x and z).
const CELL: f32 = 1.0;
/// World grid cell for static bodies (m).
const WCELL: f32 = 16.0;

/// A triangle mesh in its own space, with a grid over x and z.
pub struct Shape {
    tris: Vec<[Vec3; 3]>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    pub min: Vec3,
    pub max: Vec3,
}

fn cell(v: f32, size: f32) -> i32 {
    (v / size).floor() as i32
}

impl Shape {
    /// From triangles (degenerate ones are dropped).
    pub fn new(tris: impl IntoIterator<Item = [Vec3; 3]>) -> Arc<Shape> {
        let tris: Vec<[Vec3; 3]> = tris.into_iter().filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).length_squared() > 1e-12).collect();
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (i, t) in tris.iter().enumerate() {
            let lo = t[0].min(t[1]).min(t[2]);
            let hi = t[0].max(t[1]).max(t[2]);
            min = min.min(lo);
            max = max.max(hi);
            for j in cell(lo.z, CELL)..=cell(hi.z, CELL) {
                for k in cell(lo.x, CELL)..=cell(hi.x, CELL) {
                    grid.entry((k, j)).or_default().push(i as u32);
                }
            }
        }
        if tris.is_empty() {
            (min, max) = (Vec3::ZERO, Vec3::ZERO);
        }
        Arc::new(Shape { tris, grid, min, max })
    }

    /// A box (for things whose triangles would be wasteful to collide with: hedges, foliage).
    pub fn cuboid(min: Vec3, max: Vec3) -> Arc<Shape> {
        let c = |i: u32| Vec3::new(if i & 1 == 0 { min.x } else { max.x }, if i & 2 == 0 { min.y } else { max.y }, if i & 4 == 0 { min.z } else { max.z });
        let quads = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
        Shape::new(quads.iter().flat_map(|q| [[c(q[0]), c(q[1]), c(q[2])], [c(q[0]), c(q[2]), c(q[3])]]))
    }

    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }

    /// The triangles that may touch the box lo..hi (each once).
    fn near(&self, lo: Vec3, hi: Vec3, mut f: impl FnMut(&[Vec3; 3])) {
        if hi.x < self.min.x || lo.x > self.max.x || hi.y < self.min.y || lo.y > self.max.y || hi.z < self.min.z || lo.z > self.max.z {
            return;
        }
        let (x0, x1, z0, z1) = (cell(lo.x, CELL), cell(hi.x, CELL), cell(lo.z, CELL), cell(hi.z, CELL));
        // the candidates of every cell touched, each once (sorted: a triangle in several cells
        // is not tested twice; no quadratic look-ups)
        let mut ids: Vec<u32> = Vec::new();
        for j in z0..=z1 {
            for k in x0..=x1 {
                if let Some(c) = self.grid.get(&(k, j)) {
                    ids.extend_from_slice(c);
                }
            }
        }
        if x0 != x1 || z0 != z1 {
            ids.sort_unstable();
            ids.dedup();
        }
        {
            {
                for i in ids {
                    let t = &self.tris[i as usize];
                    let (tl, th) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
                    if th.x < lo.x || tl.x > hi.x || th.y < lo.y || tl.y > hi.y || th.z < lo.z || tl.z > hi.z {
                        continue;
                    }
                    f(t);
                }
            }
        }
    }
}

/// A placed shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BodyId(pub u32);

struct Body {
    shape: Arc<Shape>,
    to_world: Affine3A,
    to_local: Affine3A,
    /// the transform's uniform scale
    scale: f32,
    min: Vec3,
    max: Vec3,
    moving: bool,
    pub enabled: bool,
}

impl Body {
    fn place(&mut self, at: Affine3A) {
        self.to_world = at;
        self.to_local = at.inverse();
        self.scale = at.matrix3.x_axis.length().max(1e-6);
        let (lo, hi) = (self.shape.min, self.shape.max);
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for k in 0..8 {
            let p = at.transform_point3(Vec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z }));
            min = min.min(p);
            max = max.max(p);
        }
        (self.min, self.max) = (min, max);
    }
}

/// A capsule touching a surface: the nearest point on the surface, the direction out of it
/// (world, unit) and how far the capsule is into it.
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub point: Vec3,
    pub normal: Vec3,
    pub depth: f32,
    pub body: Option<BodyId>,
}

/// A ray's first hit.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub point: Vec3,
    /// facing the ray
    pub normal: Vec3,
    /// None: the ground
    pub body: Option<BodyId>,
}

pub type Ground = Box<dyn Fn(f32, f32) -> f32 + Send + Sync>;

#[derive(Default)]
pub struct World {
    bodies: Vec<Body>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    moving: Vec<u32>,
    ground: Option<Ground>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    /// The ground's height at world (x, z): a floor everywhere (a terrain).
    pub fn set_ground(&mut self, ground: impl Fn(f32, f32) -> f32 + Send + Sync + 'static) {
        self.ground = Some(Box::new(ground));
    }

    pub fn ground(&self, x: f32, z: f32) -> Option<f32> {
        self.ground.as_ref().map(|g| g(x, z))
    }

    fn push(&mut self, shape: Arc<Shape>, at: Affine3A, moving: bool) -> BodyId {
        let mut b = Body { shape, to_world: at, to_local: at, scale: 1.0, min: Vec3::ZERO, max: Vec3::ZERO, moving, enabled: true };
        b.place(at);
        let id = self.bodies.len() as u32;
        if moving {
            self.moving.push(id);
        } else {
            for j in cell(b.min.z, WCELL)..=cell(b.max.z, WCELL) {
                for k in cell(b.min.x, WCELL)..=cell(b.max.x, WCELL) {
                    self.grid.entry((k, j)).or_default().push(id);
                }
            }
        }
        self.bodies.push(b);
        BodyId(id)
    }

    /// A shape that stays where it is.
    pub fn add(&mut self, shape: Arc<Shape>, at: Affine3A) -> BodyId {
        self.push(shape, at, false)
    }

    /// A shape that moves (a door): place it again with [`World::set_transform`].
    pub fn add_moving(&mut self, shape: Arc<Shape>, at: Affine3A) -> BodyId {
        self.push(shape, at, true)
    }

    pub fn set_transform(&mut self, id: BodyId, at: Affine3A) {
        let b = &mut self.bodies[id.0 as usize];
        debug_assert!(b.moving, "only moving bodies move");
        b.place(at);
    }

    pub fn transform(&self, id: BodyId) -> Affine3A {
        self.bodies[id.0 as usize].to_world
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// The bodies whose bounds meet lo..hi.
    fn bodies_near(&self, lo: Vec3, hi: Vec3, mut f: impl FnMut(u32, &Body)) {
        let overlap = |b: &Body| b.enabled && !(b.max.x < lo.x || b.min.x > hi.x || b.max.y < lo.y || b.min.y > hi.y || b.max.z < lo.z || b.min.z > hi.z);
        let mut ids: Vec<u32> = Vec::new();
        for j in cell(lo.z, WCELL)..=cell(hi.z, WCELL) {
            for k in cell(lo.x, WCELL)..=cell(hi.x, WCELL) {
                ids.extend_from_slice(self.grid.get(&(k, j)).map_or(&[][..], |v| v.as_slice()));
            }
        }
        ids.sort_unstable();
        ids.dedup();
        for i in ids {
            let b = &self.bodies[i as usize];
            if overlap(b) {
                f(i, b);
            }
        }
        for &i in &self.moving {
            let b = &self.bodies[i as usize];
            if overlap(b) {
                f(i, b);
            }
        }
    }

    /// The static triangles (world space) that may meet the box lo..hi: for things that keep
    /// out of the walls on their own (cloth). Not the ground, not moving bodies (doors).
    pub fn triangles_near(&self, lo: Vec3, hi: Vec3, out: &mut Vec<[Vec3; 3]>) {
        self.bodies_near(lo, hi, |_, body| {
            if body.moving {
                return;
            }
            // the box in the shape's own space
            let (mut llo, mut lhi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for k in 0..8 {
                let p = body.to_local.transform_point3(Vec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z }));
                llo = llo.min(p);
                lhi = lhi.max(p);
            }
            body.shape.near(llo, lhi, |t| out.push(t.map(|v| body.to_world.transform_point3(v))));
        });
    }

    /// Everything a capsule (the segment a..b, radius r) overlaps. Not the ground.
    pub fn capsule_contacts(&self, a: Vec3, b: Vec3, r: f32, out: &mut Vec<Contact>) {
        let (lo, hi) = (a.min(b) - Vec3::splat(r), a.max(b) + Vec3::splat(r));
        self.bodies_near(lo, hi, |id, body| capsule_body(body, Some(BodyId(id)), a, b, r, out));
    }

    /// Does the capsule overlap this one body?
    pub fn capsule_overlaps(&self, id: BodyId, a: Vec3, b: Vec3, r: f32) -> bool {
        let mut out = Vec::new();
        capsule_body(&self.bodies[id.0 as usize], Some(id), a, b, r, &mut out);
        out.iter().any(|c| c.depth > 1e-3)
    }

    /// The first thing along a ray from `o` in direction `d` (unit), within `max`; the ground
    /// counts.
    pub fn raycast(&self, o: Vec3, d: Vec3, max: f32) -> Option<Hit> {
        let e = o + d * max;
        let (lo, hi) = (o.min(e) - Vec3::splat(0.01), o.max(e) + Vec3::splat(0.01));
        let mut best: Option<Hit> = None;
        self.bodies_near(lo, hi, |id, body| {
            let (ol, el) = (body.to_local.transform_point3(o), body.to_local.transform_point3(e));
            let dl = el - ol;
            body.shape.near(ol.min(el) - Vec3::splat(0.01), ol.max(el) + Vec3::splat(0.01), |t| {
                if let Some(s) = ray_triangle(ol, dl, t) {
                    let tw = s * max;
                    if best.is_none_or(|h| tw < h.t) {
                        let n = body.to_world.matrix3 * (t[1] - t[0]).cross(t[2] - t[0]);
                        let n = n.normalize_or_zero();
                        best = Some(Hit { t: tw, point: o + d * tw, normal: if n.dot(d) > 0.0 { -n } else { n }, body: Some(BodyId(id)) });
                    }
                }
            });
        });
        if let Some(g) = &self.ground {
            if let Some(t) = ray_ground(g, o, d, best.map_or(max, |h| h.t)) {
                let p = o + d * t;
                let n = ground_normal(g, p.x, p.z);
                best = Some(Hit { t, point: p, normal: n, body: None });
            }
        }
        best
    }
}

fn capsule_body(body: &Body, id: Option<BodyId>, a: Vec3, b: Vec3, r: f32, out: &mut Vec<Contact>) {
    let (al, bl) = (body.to_local.transform_point3(a), body.to_local.transform_point3(b));
    let rl = r / body.scale;
    body.shape.near(al.min(bl) - Vec3::splat(rl), al.max(bl) + Vec3::splat(rl), |t| {
        let (on_seg, on_tri) = segment_triangle(al, bl, t);
        let d = on_seg - on_tri;
        let dist = d.length();
        if dist >= rl {
            return;
        }
        let nl = if dist > 1e-6 {
            d / dist
        } else {
            // (the segment passes through the triangle: out along its face, the near side)
            let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
            if n.dot((al + bl) * 0.5 - on_tri) < 0.0 { -n } else { n }
        };
        let normal = (body.to_world.matrix3 * nl).normalize_or_zero();
        out.push(Contact { point: body.to_world.transform_point3(on_tri), normal, depth: (rl - dist) * body.scale, body: id });
    });
}

/// Ray (origin o, direction d, segment length 1 along d) and triangle, two-sided: the parameter
/// along d of the hit, in 0..1.
fn ray_triangle(o: Vec3, d: Vec3, t: &[Vec3; 3]) -> Option<f32> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let w = e2.dot(q) * inv;
    (0.0..=1.0).contains(&w).then_some(w)
}

/// A ray against the ground: marching, then bisecting the crossing.
fn ray_ground(g: &Ground, o: Vec3, d: Vec3, max: f32) -> Option<f32> {
    let above = |t: f32| {
        let p = o + d * t;
        p.y - g(p.x, p.z)
    };
    if above(0.0) < 0.0 {
        return None; // (starts underground: not a hit)
    }
    let step = if d.y.abs() > 0.99 { max } else { 0.25 };
    let mut t0 = 0.0;
    while t0 < max {
        let t1 = (t0 + step).min(max);
        if above(t1) <= 0.0 {
            let (mut a, mut b) = (t0, t1);
            for _ in 0..24 {
                let m = 0.5 * (a + b);
                if above(m) > 0.0 {
                    a = m;
                } else {
                    b = m;
                }
            }
            return Some(b);
        }
        t0 = t1;
    }
    None
}

fn ground_normal(g: &Ground, x: f32, z: f32) -> Vec3 {
    let e = 0.25;
    Vec3::new(g(x - e, z) - g(x + e, z), 2.0 * e, g(x, z - e) - g(x, z + e)).normalize()
}

// --------------------------------------------------------------------------- closest points
// (Ericson, Real-Time Collision Detection, 5.1.5 and 5.1.9)

/// The point of triangle a b c nearest to p.
pub fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// The nearest points of segments p1 q1 and p2 q2.
pub fn closest_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (s, t);
    if a <= 1e-12 && e <= 1e-12 {
        return (p1, p2);
    }
    if a <= 1e-12 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= 1e-12 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > 1e-12 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (p1 + d1 * s, p2 + d2 * t)
}

/// The nearest points of segment p q and a triangle: (on the segment, on the triangle).
pub fn segment_triangle(p: Vec3, q: Vec3, t: &[Vec3; 3]) -> (Vec3, Vec3) {
    if let Some(s) = ray_triangle(p, q - p, t) {
        let x = p + (q - p) * s;
        return (x, x);
    }
    let mut best = (p, closest_on_triangle(p, t[0], t[1], t[2]));
    let mut bd = best.0.distance_squared(best.1);
    let mut try_ = |c: (Vec3, Vec3)| {
        let d = c.0.distance_squared(c.1);
        if d < bd {
            bd = d;
            best = c;
        }
    };
    try_((q, closest_on_triangle(q, t[0], t[1], t[2])));
    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
        try_(closest_segments(p, q, a, b));
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dense model (31k triangles in one small cell, as a booster box) queried as a walking
    /// body does each frame: fast, whatever the cell layout.
    #[test]
    fn dense_shapes_are_quick_to_query() {
        let mut tris = Vec::new();
        for k in 0..31_000 {
            let f = k as f32 * 1e-5;
            tris.push([Vec3::new(0.95 + f, 0.0, 0.95), Vec3::new(1.05, 0.1, 0.95 + f), Vec3::new(1.0, 0.2, 1.05)]);
        }
        let shape = Shape::new(tris);
        let mut w = World::new();
        for k in 0..40 {
            w.add(shape.clone(), Affine3A::from_translation(Vec3::new(k as f32 * 0.14, 0.0, 0.0)));
        }
        let t = std::time::Instant::now();
        let mut c = Vec::new();
        for k in 0..200 {
            c.clear();
            // (straddling cell boundaries: the slow case before)
            w.capsule_contacts(Vec3::new(2.0 + k as f32 * 0.01, 0.6, 1.2), Vec3::new(2.0 + k as f32 * 0.01, 1.3, 1.2), 0.26, &mut c);
        }
        let ms = t.elapsed().as_secs_f64() * 1e3 / 200.0;
        assert!(ms < 2.0, "a capsule query takes {ms:.2} ms");
    }

    #[test]
    fn segment_and_triangle() {
        let t = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 2.0)];
        // above the face
        let (s, p) = segment_triangle(Vec3::new(0.5, 1.0, 0.5), Vec3::new(0.5, 3.0, 0.5), &t);
        assert!((s - Vec3::new(0.5, 1.0, 0.5)).length() < 1e-5 && (p - Vec3::new(0.5, 0.0, 0.5)).length() < 1e-5);
        // through it
        let (s, p) = segment_triangle(Vec3::new(0.5, -1.0, 0.5), Vec3::new(0.5, 1.0, 0.5), &t);
        assert!(s.distance(p) < 1e-5);
        // beside an edge, parallel to it
        let (s, p) = segment_triangle(Vec3::new(-1.0, 0.0, -0.5), Vec3::new(3.0, 0.0, -0.5), &t);
        assert!((s.distance(p) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn rays_and_capsules_through_placed_shapes() {
        let mut w = World::new();
        let wall = Shape::cuboid(Vec3::new(-0.1, 0.0, -5.0), Vec3::new(0.1, 3.0, 5.0));
        // turned a quarter round and moved: the wall runs along x at z = -4
        let at = Affine3A::from_rotation_translation(glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), Vec3::new(0.0, 0.0, -4.0));
        w.add(wall, at);
        w.set_ground(|_, _| 0.0);
        let h = w.raycast(Vec3::new(1.0, 1.0, 0.0), Vec3::NEG_Z, 10.0).unwrap();
        assert!((h.t - 3.9).abs() < 1e-4 && h.normal.z > 0.99 && h.body.is_some());
        let g = w.raycast(Vec3::new(1.0, 1.0, 0.0), Vec3::NEG_Y, 5.0).unwrap();
        assert!((g.t - 1.0).abs() < 1e-4 && g.body.is_none());
        let mut c = Vec::new();
        w.capsule_contacts(Vec3::new(1.0, 0.5, -3.75), Vec3::new(1.0, 1.5, -3.75), 0.3, &mut c);
        let deepest = c.iter().fold(0.0f32, |m, c| m.max(c.depth));
        assert!((deepest - 0.15).abs() < 1e-4 && c.iter().all(|c| c.normal.z > 0.99));
    }
}
