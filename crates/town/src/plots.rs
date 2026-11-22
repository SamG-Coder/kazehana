//! The outdoors of the house plots: driveway slabs, block walls (ブロック塀), stone walls round
//! the old farmhouses, gate posts and mailboxes. Hedges are plants (plants.rs, species "hedge").
//! The layout (segments with their gaps, posts, mailbox spot) comes from the town plan
//! (tools/blender/town_plan.py `boundaries`).

use crate::plan::House;
use crate::{scan_jobs, surface_material, Town};
use akari::glam::{Affine3A, Quat, Vec2, Vec3};
use akari::{Instance, Material, MaterialId, MeshData, Model, Renderer};
use anyhow::Result;

/// Block wall: 1.2 m of concrete block, 12 cm thick.
const BLOCK_H: f32 = 1.2;
const BLOCK_T: f32 = 0.12;
/// A bank this much higher (or lower) than a plot, along its sides or back, gets a retaining wall.
const BANK: f32 = 0.3;
/// Banks up to this high are held by wooden boards; higher ones by concrete.
const WOOD_MAX: f32 = 1.0;

/// Stone wall round a farmhouse yard: low, broad, mossy.
const STONE_H: f32 = 0.7;
const STONE_T: f32 = 0.45;
/// The block-wall scan is 4 x 2 m; its top 0.38 m are another course, so a wall starts below that.
const SCAN_W: f32 = 4.0;
const SCAN_H: f32 = 2.0;
const SCAN_TOP: f32 = 0.38;

#[derive(Default)]
struct M {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    /// linear colour multiplier per vertex (weathering)
    col: Vec<[f32; 3]>,
    idx: Vec<u32>,
}

impl M {
    fn quad(&mut self, p: [Vec3; 4], n: Vec3, uv: [[f32; 2]; 4]) {
        let b = self.pos.len() as u32;
        for k in 0..4 {
            self.pos.push(p[k].to_array());
            self.nrm.push(n.to_array());
            self.uv.push(uv[k]);
            self.col.push([1.0; 3]);
        }
        self.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    /// A wall along a -> b (plan), standing on `ground`: both faces, top and ends, UVs in metres
    /// scaled by (su, sv) with v measured down from the top plus `v0`.
    #[allow(clippy::too_many_arguments)]
    fn wall(&mut self, a: Vec2, b: Vec2, ground: f32, h: f32, t: f32, su: f32, sv: f32, v0: f32) {
        let len = (b - a).length();
        if len < 0.05 {
            return;
        }
        let d = (b - a) / len;
        let l = Vec2::new(-d.y, d.x) * (t * 0.5);
        let w3 = |p: Vec2, y: f32| Vec3::new(p.x, y, -p.y);
        let (y0, y1) = (ground - 0.15, ground + h); // a little into the ground
        let v = |y: f32| v0 + (ground + h - y) / sv;
        let face_n = |s: f32| Vec3::new(l.x * s, 0.0, -l.y * s).normalize();
        for s in [1.0f32, -1.0] {
            let (p, q) = if s > 0.0 { (a + l, b + l) } else { (b - l, a - l) };
            let (u0, u1) = if s > 0.0 { (0.0, len / su) } else { (len / su, 0.0) };
            self.quad([w3(p, y0), w3(q, y0), w3(q, y1), w3(p, y1)], face_n(s), [[u0, v(y0)], [u1, v(y0)], [u1, v(y1)], [u0, v(y1)]]);
        }
        // top (the cap course), ends
        let tv = v0 + 0.02;
        self.quad([w3(a - l, y1), w3(a + l, y1), w3(b + l, y1), w3(b - l, y1)], Vec3::Y, [[0.0, tv], [t / sv, tv], [t / sv, tv + len / su], [0.0, tv + len / su]]);
        for (p, s) in [(a, -1.0f32), (b, 1.0)] {
            let n = Vec3::new(d.x * s, 0.0, -d.y * s);
            let (c0, c1) = if s > 0.0 { (p + l, p - l) } else { (p - l, p + l) };
            self.quad([w3(c0, y0), w3(c1, y0), w3(c1, y1), w3(c0, y1)], n, [[0.0, v(y0)], [t / su, v(y0)], [t / su, v(y1)], [0.0, v(y1)]]);
        }
    }

    /// A box of world-projected UVs (posts, slabs): centre, half extents, yaw.
    fn block(&mut self, c: Vec3, half: Vec3, yaw: f32, tile: f32) {
        let r = Quat::from_rotation_y(yaw);
        let ax = [r * Vec3::X * half.x, Vec3::Y * half.y, r * Vec3::Z * half.z];
        let p = |sx: f32, sy: f32, sz: f32| c + ax[0] * sx + ax[1] * sy + ax[2] * sz;
        let faces = [
            ([p(-1., 1., -1.), p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.)], Vec3::Y),
            ([p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)], ax[2].normalize()),
            ([p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.)], -ax[2].normalize()),
            ([p(1., -1., 1.), p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.)], ax[0].normalize()),
            ([p(-1., -1., -1.), p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.)], -ax[0].normalize()),
        ];
        for (q, n) in faces {
            let (t, b) = n.any_orthonormal_pair();
            let uv = q.map(|v| [v.dot(t) / tile, v.dot(b) / tile]);
            self.quad(q, n, uv);
        }
    }

    /// A wall along a polyline (plan points `pts`, already where the wall's centre line runs),
    /// `t` thick, with its own bottom and top at every point: both faces, the top, both ends.
    /// UVs in metres / `tile` (u along, v up).
    fn run_wall(&mut self, pts: &[Vec2], bot: &[f32], top: &[f32], t: f32, tile: f32) {
        if pts.len() < 2 {
            return;
        }
        let w3 = |p: Vec2, y: f32| Vec3::new(p.x, y, -p.y);
        let mut along = vec![0.0f32];
        for k in 1..pts.len() {
            along.push(along[k - 1] + (pts[k] - pts[k - 1]).length());
        }
        let side = |k: usize| {
            let d = (pts[(k + 1).min(pts.len() - 1)] - pts[k.saturating_sub(1)]).normalize_or(Vec2::X);
            Vec2::new(-d.y, d.x) * (t * 0.5)
        };
        for s in [1.0f32, -1.0] {
            for k in 0..pts.len() - 1 {
                let (l0, l1) = (side(k) * s, side(k + 1) * s);
                let (p0, p1) = (pts[k] + l0, pts[k + 1] + l1);
                let n = Vec3::new(l0.x, 0.0, -l0.y).normalize();
                let (u0, u1) = (along[k] / tile, along[k + 1] / tile);
                let q = [w3(p0, bot[k]), w3(p1, bot[k + 1]), w3(p1, top[k + 1]), w3(p0, top[k])];
                let uv = [[u0, bot[k] / tile], [u1, bot[k + 1] / tile], [u1, top[k + 1] / tile], [u0, top[k] / tile]];
                if s > 0.0 {
                    self.quad(q, n, uv);
                } else {
                    self.quad([q[1], q[0], q[3], q[2]], n, [uv[1], uv[0], uv[3], uv[2]]);
                }
            }
        }
        for k in 0..pts.len() - 1 {
            let (l0, l1) = (side(k), side(k + 1));
            let q = [w3(pts[k] - l0, top[k]), w3(pts[k] + l0, top[k]), w3(pts[k + 1] + l1, top[k + 1]), w3(pts[k + 1] - l1, top[k + 1])];
            let n = (q[1] - q[0]).cross(q[3] - q[0]).normalize_or(Vec3::Y);
            let (q, n) = if n.y >= 0.0 { (q, n) } else { ([q[0], q[3], q[2], q[1]], -n) };
            self.quad(q, n, q.map(|v| [v.x / tile, v.z / tile]));
        }
        for (k, s) in [(0usize, -1.0f32), (pts.len() - 1, 1.0)] {
            let l = side(k);
            let d = (pts[(k + 1).min(pts.len() - 1)] - pts[k.saturating_sub(1)]).normalize_or(Vec2::X) * s;
            let n = Vec3::new(d.x, 0.0, -d.y);
            let (a, b) = if s > 0.0 { (pts[k] + l, pts[k] - l) } else { (pts[k] - l, pts[k] + l) };
            self.quad([w3(a, bot[k]), w3(b, bot[k]), w3(b, top[k]), w3(a, top[k])], n, [[0.0, 0.0], [t / tile, 0.0], [t / tile, 1.0], [0.0, 1.0]]);
        }
    }

    /// Weather the vertices added since `from`: dirt and rain splash darkening the foot of a wall
    /// (fading out over `band` m above `foot`), and a tint for the whole piece (each wall was built
    /// and has aged on its own).
    fn weather(&mut self, from: usize, foot: f32, band: f32, tint: [f32; 3]) {
        for k in from..self.pos.len() {
            let y = self.pos[k][1];
            let t = ((y - foot) / band).clamp(0.0, 1.0);
            let t = t * t * (3.0 - 2.0 * t);
            let grime = [0.62, 0.6, 0.55];
            self.col[k] = [0, 1, 2].map(|i| tint[i] * (grime[i] + (1.0 - grime[i]) * t));
        }
    }

    /// A driveway slab whose top follows the ground (3 cm proud of it), `depth` thick: a grid of
    /// about 0.5 m cells over the quad c (corners in order), world-projected UVs.
    fn driveway(&mut self, c: [Vec2; 4], ground: impl Fn(Vec2) -> f32, depth: f32, tile: f32, seed: u32) {
        let from = self.pos.len();
        let nu = (((c[1] - c[0]).length() / 0.25).ceil() as usize).max(1);
        let nv = (((c[3] - c[0]).length() / 0.25).ceil() as usize).max(1);
        let at = |i: usize, j: usize| {
            let (u, v) = (i as f32 / nu as f32, j as f32 / nv as f32);
            let a = c[0].lerp(c[1], u);
            let b = c[3].lerp(c[2], u);
            let q = a.lerp(b, v);
            Vec3::new(q.x, ground(q) + 0.03, -q.y)
        };
        let up_sign = {
            let n = (at(1, 0) - at(0, 0)).cross(at(0, 1) - at(0, 0));
            n.y >= 0.0
        };
        for j in 0..nv {
            for i in 0..nu {
                let q = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                let q = if up_sign { q } else { [q[0], q[3], q[2], q[1]] };
                let n = (q[1] - q[0]).cross(q[3] - q[0]).normalize_or(Vec3::Y);
                self.quad(q, n, q.map(|v| [v.x / tile, v.z / tile]));
            }
        }
        // the four sides, down `depth`
        let ring: Vec<(usize, usize)> = (0..nu).map(|i| (i, 0)).chain((0..nv).map(|j| (nu, j))).chain((1..=nu).rev().map(|i| (i, nv))).chain((1..=nv).rev().map(|j| (0, j))).collect();
        let centre = (at(0, 0) + at(nu, nv)) * 0.5;
        for w in 0..ring.len() {
            let (p, q) = (ring[w], ring[(w + 1) % ring.len()]);
            let (a, b) = (at(p.0, p.1), at(q.0, q.1));
            let mut quad = [a - Vec3::Y * depth, b - Vec3::Y * depth, b, a];
            let mut n = (quad[1] - quad[0]).cross(quad[3] - quad[0]).normalize_or(Vec3::X);
            if n.dot((a + b) * 0.5 - centre) < 0.0 {
                quad = [b - Vec3::Y * depth, a - Vec3::Y * depth, a, b];
                n = -n;
            }
            let along = (b - a).length();
            self.quad(quad, n, [[0.0, 0.0], [along / tile, 0.0], [along / tile, depth / tile], [0.0, depth / tile]]);
        }
        // weathering: each driveway poured at its own time (a tint), two darker tyre tracks
        // running in from the road, an oil stain or two where the car stands, dirt along the edges
        let h = |k: u32| ((seed ^ k.wrapping_mul(0x9E37_79B9)).wrapping_mul(0x85EB_CA6B) >> 8) as f32 / 16_777_216.0;
        let tint = 0.78 + 0.3 * h(1);
        let warm = [1.0 + 0.04 * h(2), 1.0, 1.0 - 0.05 * h(2)];
        let (e0, e1, e3) = (c[0], c[1] - c[0], c[3] - c[0]);
        let (w, d) = (e1.length(), e3.length());
        let (eu, ev) = (e1 / w.max(1e-3), e3 / d.max(1e-3));
        let stains: Vec<(f32, f32, f32)> = (0..(1 + (h(3) * 3.0) as u32)).map(|k| (0.25 + 0.5 * h(10 + k), 0.45 + 0.4 * h(20 + k), 0.25 + 0.35 * h(30 + k))).collect();
        for k in from..self.pos.len() {
            let p = Vec2::new(self.pos[k][0], -self.pos[k][2]) - e0;
            let (u, v) = (p.dot(eu) / w.max(1e-3), p.dot(ev) / d.max(1e-3));
            let mut f = tint;
            // tyre tracks: two bands ~1.5 m apart, darker and fading in from the road
            for t in [0.5 - 0.75 / w.max(1.0), 0.5 + 0.75 / w.max(1.0)] {
                let x = (u - t) * w / 0.22;
                f *= 1.0 - 0.22 * (-x * x).exp() * (1.0 - 0.5 * v);
            }
            for &(su, sv, sr) in &stains {
                let r = ((u - su) * w).hypot((v - sv) * d) / sr;
                f *= 1.0 - 0.35 * (-r * r).exp();
            }
            // dirt and moss creeping in at the edges
            let edge = (u * w).min((1.0 - u) * w).min(v * d).min((1.0 - v) * d).max(0.0);
            let dirt = (1.0 - edge / 0.35).clamp(0.0, 1.0);
            let col = [0.62 * warm[0], 0.58, 0.48 * warm[2]];
            self.col[k] = [0, 1, 2].map(|i| f * warm[i] * (1.0 - dirt * (1.0 - col[i])));
        }
    }

    fn finish(self, name: &str, mat: MaterialId) -> Option<MeshData> {
        if self.idx.is_empty() {
            return None;
        }
        let mut m = MeshData { name: name.into(), positions: self.pos, normals: self.nrm, uvs: self.uv, colors: self.col, indices: self.idx, material: Some(mat), ..Default::default() };
        m.generate_tangents();
        Some(m)
    }
}

pub struct PlotsScene {
    /// what to draw (door leaves are left out: their doors draw them, where they are)
    pub instances: Vec<Instance>,
    /// every model placed: (its name, where); for collision and doors (crate::collide)
    pub placed: Vec<(String, Affine3A)>,
    /// Tsukumodō's frame (its building's model space to the world): where the game hangs the
    /// things it draws itself (the calendar)
    pub home: Option<Affine3A>,
    /// your desk in 1-A (the school_desk prop's frame to the world): where the game puts your
    /// paper and your seat
    pub your_desk: Option<Affine3A>,
    /// places for people in the buildings, from their layouts: (room, spot, frame in the world);
    /// spots are "seat:N" (a desk's frame: its seat is behind it) and named ones ("front")
    pub spots: Vec<(String, String, Affine3A)>,
    /// the models placed
    pub models: Vec<Model>,
    /// the walls, posts and slabs built here, as triangles in the world (collision)
    pub solid: Vec<[Vec3; 3]>,
}

/// Put a model down: its parts as instances (not its door leaves), and a note of where.
fn put(instances: &mut Vec<Instance>, placed: &mut Vec<(String, Affine3A)>, model: &Model, at: Affine3A) {
    instances.extend(crate::collide::static_instances(model, at));
    placed.push((model.name.clone(), at));
}

/// Build the outdoors of the plots whose gate is within `radius` of `centre` (plan coordinates;
/// None = every plot). `mailbox`: the approved mailbox model (asset registry), if loaded.
/// `hedge`: the approved boxwood hedge asset (asset registry), if loaded: its dense shapes clip
/// into hedges.
/// `houses`: approved house assets by plan type ("modern_gable", ...): placed on every plot of
/// that type (plots of other types stay empty for now).
pub fn build(r: &mut Renderer, town: &Town, centre: [f32; 2], radius: Option<f32>, mailbox: Option<Model>, hedge: Option<Model>, houses: Vec<(String, Model)>) -> Result<PlotsScene> {
    // the boxwood's clipped box shape (VarF) only: it reads as a trimmed hedge
    let hedge: Vec<Model> = hedge.as_ref().map(|m| ["VarF"].iter().map(|k| m.variant(k)).filter(|v| !v.parts.is_empty()).collect()).unwrap_or_default();
    let block = surface_material(r, "Japanese_Concrete_Block_Wall", 1.0)?;
    let stone = surface_material(r, "Mossy_Stone_Wall", 1.0)?;
    // poured concrete (driveways, posts): an even light grey with the asphalt scan's fine grain in
    // the normal map (the concrete scans have panel joints)
    let grain = r.add_textures(&scan_jobs("Asphalt_Road")?)?;
    let concrete = surface_material(r, "Concrete_Floor-f4b85cb0", 1.0)?;
    // gate posts: older, darker concrete than the driveways
    let post_mat = r.add_material(Material { normal_tex: Some(grain[1]), normal_scale: 0.6, ..Material::color([0.44, 0.43, 0.41], 0.9) });
    let pole_mat = r.add_material(Material { base_color: [0.05, 0.05, 0.05, 1.0], roughness: 0.45, metallic: 0.7, ..Default::default() });
    let (mut walls, mut stones, mut slabs, mut posts, mut poles) = (M::default(), M::default(), M::default(), M::default(), M::default());
    // retaining walls (yōheki): poured concrete with formwork joints, or boards between posts
    let retain_mat = surface_material(r, "Precast_Concrete_Wall", 0.72)?;
    let board_mat = surface_material(r, "Wood_Planks", 0.75)?;
    let (mut retains, mut boards) = (M::default(), M::default());
    // is a plan point inside some house plot?
    let in_plot = |q: Vec2| {
        town.plan.houses.iter().any(|o| {
            let c: Vec<Vec2> = o.plot.iter().map(|p| Vec2::from(*p)).collect();
            (0..4).all(|k| (c[(k + 1) % 4] - c[k]).perp_dot(q - c[k]) >= 0.0) || (0..4).all(|k| (c[(k + 1) % 4] - c[k]).perp_dot(q - c[k]) <= 0.0)
        })
    };
    let mut instances = Vec::new();
    let mut placed: Vec<(String, Affine3A)> = Vec::new();
    let mut home: Option<Affine3A> = None;
    let mut your_desk: Option<Affine3A> = None;
    let mut spots: Vec<(String, String, Affine3A)> = Vec::new();
    let near = |h: &House| radius.is_none_or(|rr| (h.gate[0] - centre[0]).hypot(h.gate[1] - centre[1]) < rr);
    let mut n = 0;
    for h in town.plan.houses.iter().filter(|h| near(h)) {
        n += 1;
        let g = h.ground;
        // the plot's frame: corners 0, 1 on the road side
        let pl: Vec<Vec2> = h.plot.iter().map(|q| Vec2::from(*q)).collect();
        let centre = pl.iter().fold(Vec2::ZERO, |s, p| s + *p) / 4.0;
        // the ground just outside a point of the plot's edge (1.2 m out)
        let outside = |p: Vec2| -> (f32, Vec2) {
            let out = (p - centre).normalize_or(Vec2::X);
            let q = p + out * 1.2;
            (town.ground(q.to_array()), q)
        };
        // retaining walls along the sides and back (not the road side): continuous runs whose top
        // follows the bank behind them (sampled every 0.5 m, smoothed); a cut plot's walls hold the
        // hill, a filled plot's hold its fill
        for k in 1..4 {
            let (a, b) = (pl[k], pl[(k + 1) % 4]);
            let len = (b - a).length();
            let inw = (centre - (a + b) * 0.5).normalize_or(Vec2::Y);
            let ns = ((len / 0.5).ceil() as usize).max(2);
            let raw: Vec<(f32, Vec2)> = (0..=ns).map(|t| outside(a.lerp(b, t as f32 / ns as f32))).collect();
            // the bank: smoothed over +-1 m
            let bank: Vec<f32> = (0..=ns).map(|t| {
                let (t0, t1) = (t.saturating_sub(2), (t + 2).min(ns));
                raw[t0..=t1].iter().map(|r| r.0).sum::<f32>() / (t1 - t0 + 1) as f32
            }).collect();
            // which points need a wall: the hill above (cut), or open ground below (fill)
            let need: Vec<bool> = (0..=ns).map(|t| bank[t] > g + BANK || (bank[t] < g - BANK && !in_plot(raw[t].1))).collect();
            let mut t = 0;
            while t <= ns {
                if !need[t] {
                    t += 1;
                    continue;
                }
                let mut e = t;
                while e < ns && need[e + 1] {
                    e += 1;
                }
                // the run, half a metre longer at each free end (covers the ground's step there)
                let (r0, r1) = (t.saturating_sub(1), (e + 1).min(ns));
                let pts: Vec<Vec2> = (r0..=r1).map(|q| a.lerp(b, q as f32 / ns as f32) + inw * 0.375).collect();
                let top: Vec<f32> = (r0..=r1).map(|q| bank[q].max(g) + 0.05).collect();
                let bot: Vec<f32> = (r0..=r1).map(|q| bank[q].min(g) - 0.3).collect();
                let high = top.iter().zip(&bot).map(|(tp, _)| tp - g).fold(0.0f32, f32::max);
                let low = bot.iter().cloned().fold(f32::MAX, f32::min);
                if high <= WOOD_MAX && low >= g - 0.35 {
                    boards.run_wall(&pts, &bot, &top, 0.75, 1.2);
                    // posts every 1.8 m, on the plot side
                    let run = (pts[pts.len() - 1] - pts[0]).length();
                    let np = (run / 1.8).ceil().max(1.0) as usize;
                    for m in 0..=np {
                        let f = m as f32 / np as f32;
                        let qi = ((pts.len() - 1) as f32 * f).round() as usize;
                        let c = pts[0].lerp(pts[pts.len() - 1], f) + inw * 0.42;
                        boards.block(Vec3::new(c.x, (bot[qi] + top[qi] + 0.08) * 0.5, -c.y), Vec3::new(0.06, (top[qi] + 0.08 - bot[qi]) * 0.5, 0.06), 0.0, 1.0);
                    }
                } else {
                    retains.run_wall(&pts, &bot, &top, 0.75, 2.0);
                }
                t = e + 1;
            }
        }
        for sg in &h.boundary.segments {
            let (a, b) = (Vec2::from(sg.a), Vec2::from(sg.b));
            match sg.kind.as_str() {
                "block" | "stone" => {
                    let seed = (h.id as u32).wrapping_mul(2_654_435_761) ^ a.x.to_bits();
                    let f = |k: u32| ((seed ^ k).wrapping_mul(0x9E37_79B9) >> 8) as f32 / 16_777_216.0;
                    // each wall aged on its own: a little lighter or darker, warmer or cooler
                    let tint = [0.86 + 0.16 * f(1), 0.86 + 0.14 * f(1) - 0.02 * f(2), 0.84 + 0.12 * f(1) - 0.04 * f(2)];
                    let len = (b - a).length();
                    let mid_out = outside((a + b) * 0.5).0;
                    if mid_out > g + BANK {
                        continue; // the retaining wall is the boundary here
                    }
                    let lo = if mid_out < g - BANK {
                        g // on top of the fill's retaining wall
                    } else {
                        (0..=((len / 0.5) as usize + 1)).map(|k| town.ground(a.lerp(b, (k as f32 * 0.5 / len.max(1e-3)).min(1.0)).to_array())).fold(g, f32::min)
                    };
                    if sg.kind == "block" {
                        let from = walls.pos.len();
                        walls.wall(a, b, lo, BLOCK_H + (g - lo), BLOCK_T, SCAN_W, SCAN_H, SCAN_TOP / SCAN_H);
                        walls.weather(from, lo - 0.15, 0.55, tint);
                    } else {
                        let from = stones.pos.len();
                        stones.wall(a, b, lo, STONE_H + (g - lo), STONE_T, 2.0, 2.0, 0.0);
                        stones.weather(from, lo - 0.15, 0.3, tint);
                    }
                }
                "hedge" => {
                    // dense boxwood bushes, clipped to a 1 m hedge
                    if !hedge.is_empty() {
                        let len = (b - a).length();
                        let d = (b - a) / len.max(1e-3);
                        // one continuous clipped hedge: every bush the same height and thickness,
                        // turned along the hedge (or end for end), overlapping its neighbours by
                        // half, the top level along the whole run
                        let m = &hedge[0];
                        let size = m.max - m.min;
                        // scaled evenly (the bush keeps its own proportions): 0.9 m tall, spaced at
                        // 60% of its width so neighbours overlap
                        let sc = 0.9 / size.y.max(0.1);
                        let n = (len / (size.x * sc * 0.6)).ceil().max(1.0) as usize;
                        let step = len / n as f32;
                        if outside((a + b) * 0.5).0 > g + BANK {
                            continue;
                        }
                        let on_wall = false;
                        let top = (0..=n).map(|k| town.ground((a + d * (k as f32 * step)).to_array())).fold(f32::MIN, f32::max) + 0.9;
                        let base_yaw = (-d.y).atan2(d.x); // local +x along the hedge
                        for k in 0..n {
                            let p = a + d * ((k as f32 + 0.5) * step);
                            let ground = if on_wall { g } else { town.ground(p.to_array()) - 0.05 }; // sunk a little into the soil
                            let sy = (top - ground) / size.y.max(0.1);
                            let (along, across) = (sc, sc);
                            let yaw = base_yaw + if (h.id as usize + k) % 2 == 0 { 0.0 } else { std::f32::consts::PI };
                            let t = Affine3A::from_scale_rotation_translation(Vec3::new(along, sy, across), Quat::from_rotation_y(yaw), Vec3::new(p.x, ground - m.min.y * sy, -p.y));
                            put(&mut instances, &mut placed, m, t);
                        }
                    }
                }
                _ => {}
            }
        }
        // gate posts: concrete, a little taller than the wall, with a cap
        for p in &h.boundary.posts {
            let from = posts.pos.len();
            let lo = town.ground(*p).min(g);
            let top = g + 1.3;
            posts.block(Vec3::new(p[0], (lo - 0.15 + top) * 0.5, -p[1]), Vec3::new(0.12, (top - lo + 0.15) * 0.5, 0.12), -h.yaw, 2.0);
            posts.block(Vec3::new(p[0], top + 0.03, -p[1]), Vec3::new(0.15, 0.03, 0.15), -h.yaw, 2.0);
            posts.weather(from, lo - 0.15, 0.5, [1.0; 3]);
        }
        // the driveway: a concrete slab standing a few cm proud of the yard
        if let Some(pk) = &h.parking {
            // corners 0, 1 are on the road side: there the slab comes down to the road, behind it
            // lies 2 cm proud of the yard
            // its top follows the ground exactly (the road grading ramps the plot's front edge)
            let c: Vec<Vec2> = pk.iter().map(|q| Vec2::from(*q)).collect();
            let inward = (c[3] - c[0]).normalize_or(Vec2::Y);
            // the plot starts 0.5 m back from the road: the slab runs on to the asphalt's edge
            let (r0, r1) = (c[0] - inward * 0.75, c[1] - inward * 0.75);
            let road = Vec2::from(h.gate) - inward * 0.75;
            let surface = |q: Vec2| -> f32 {
                // the front half metre meets the road surface; behind, the ground
                let t = ((q - road).dot(inward) / 0.9).clamp(0.0, 1.0);
                let s = town.surface(q.to_array());
                let g0 = town.ground(q.to_array());
                s + (g0 - s) * t
            };
            slabs.driveway([r0, r1, c[2], c[3]], surface, 0.3, 2.0, h.id as u32); // (the scan is 2 x 2 m)
        }
        // the house: asset frame +X along its long side, -Z (Blender +Y) its entrance side, the
        // garden side opposite; turned so the garden faces the plot's garden side
        // the layout whose front door faces the road: the garden side ("_s") when the road runs
        // along the garden, the east gable end ("_e") when it runs along a side, else the north
        let want = match h.garden.as_str() {
            _ if h.name.as_deref() == Some("Tsukumodō") => "tsukumodo".to_string(), // the player's home
            "front" => format!("{}_s", h.ty),
            "left" | "right" => format!("{}_e", h.ty),
            _ => h.ty.clone(),
        };
        let mut choices: Vec<&Model> = houses.iter().filter(|(t, _)| *t == want).map(|(_, m)| m).collect();
        if choices.is_empty() {
            choices = houses.iter().filter(|(t, _)| *t == h.ty).map(|(_, m)| m).collect();
        }
        if !choices.is_empty() {
            // each plot its own colour scheme
            let pick = (h.id as u32).wrapping_mul(2_654_435_761) >> 7;
            let model = choices[pick as usize % choices.len()];
            let q: Vec<Vec2> = h.house.iter().map(|p| Vec2::from(*p)).collect();
            let c = q.iter().fold(Vec2::ZERO, |s, p| s + *p) / q.len() as f32;
            let e = (q[1] - q[0]).normalize_or(Vec2::X); // along the road
            let n = Vec2::new(-e.y, e.x); // away from the road
            let garden = match h.garden.as_str() {
                "front" => -n,
                "back" => n,
                "right" => e,
                _ => -e,
            };
            let theta = garden.x.atan2(-garden.y);
            // the door is at the house's +X end: mirror the house when the driveway is at the other
            let x_dir = Vec2::new(theta.cos(), theta.sin());
            let road = -n; // towards the road
            let mirror = if h.garden == "left" || h.garden == "right" {
                // the door is on the +X gable end: that end must face the road
                x_dir.dot(road) < 0.0
            } else {
                h.parking.as_ref().is_some_and(|pk| {
                    let pc = pk.iter().fold(Vec2::ZERO, |s, p| s + Vec2::from(*p)) / pk.len() as f32;
                    (pc - c).dot(x_dir) < 0.0
                })
            };
            let t = Affine3A::from_scale_rotation_translation(Vec3::new(if mirror { -1.0 } else { 1.0 }, 1.0, 1.0), Quat::IDENTITY, Vec3::ZERO);
            let t = Affine3A::from_rotation_translation(Quat::from_rotation_y(theta), Vec3::new(c.x, h.ground, -c.y)) * t;
            put(&mut instances, &mut placed, model, t);
            // the building's own fittings (a shop's showcases, shelves, tables), from its layout
            if want == "tsukumodo" {
                home = Some(t);
                #[derive(serde::Deserialize)]
                struct Fit {
                    id: String,
                    at: [f32; 3],
                    yaw: f32,
                }
                match std::fs::read("assets/buildings/shop_tsukumodo/layout.json").map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice::<Vec<Fit>>(&b)?)) {
                    Ok(fits) => {
                        for f in fits {
                            let key = format!("prop:{}", f.id);
                            if let Some((_, m)) = houses.iter().find(|(k, _)| *k == key) {
                                // (Blender x, y, z -> glTF x, z, -y; a turn about Blender +Z is one about +Y)
                                let local = Affine3A::from_rotation_translation(Quat::from_rotation_y(f.yaw.to_radians()), Vec3::new(f.at[0], f.at[2], -f.at[1]));
                                put(&mut instances, &mut placed, m, t * local);
                            }
                        }
                    }
                    Err(e) => eprintln!("shop layout: {e:#}"),
                }
            }
        }
        if let (Some(mb), Some(model)) = (&h.boundary.mailbox, &mailbox) {
            // on a slim black pole by the gate, at about chest height, facing the road
            const POLE: f32 = 0.85;
            let (at, base_h) = (Vec2::from(mb.at), town.ground(mb.at));
            let base = Vec3::new(at.x, base_h, -at.y);
            poles.block(base + Vec3::Y * (POLE * 0.5 - 0.1), Vec3::new(0.025, POLE * 0.5 + 0.1, 0.025), mb.yaw, 1.0);
            let lift = POLE - model.min.y;
            let t = Affine3A::from_rotation_translation(Quat::from_rotation_y(mb.yaw), base + Vec3::Y * lift);
            put(&mut instances, &mut placed, model, t);
        }
    }
    // the sites' buildings: the junior high's main building on its footprint, its classrooms
    // (the model's +Y) towards the sports ground, on the site's graded ground
    // (the main building, the gym and the pool each on its footprint; the grounds' fittings in
    // the site's frame, from its north-west corner: tools/blender/build_school_grounds.py)
    for site in town.plan.sites.iter().filter(|s| s.kind == "school" && s.name.contains("Junior High")) {
        let Some(main) = site.buildings.iter().find(|b| b.name == "school building") else { continue };
        let mq: Vec<Vec2> = main.poly.iter().map(|p| Vec2::from(*p)).collect();
        let east = (mq[1] - mq[0]).normalize_or(Vec2::X);
        let south = (mq[3] - mq[0]).normalize_or(Vec2::Y); // towards the sports ground
        // (a model's +Y is the world's -Z: turned to the plan's `south`)
        let theta = (-south.x).atan2(south.y);
        let ground = site.ground.unwrap_or_else(|| town.ground([mq[0].x, mq[0].y]));
        let at = |c: Vec2| Affine3A::from_rotation_translation(Quat::from_rotation_y(theta), Vec3::new(c.x, ground, -c.y));
        for (key, footprint) in [("site:school_main", "school building"), ("site:school_gym", "gym"), ("site:school_pool", "pool")] {
            let (Some((_, model)), Some(b)) = (houses.iter().find(|(k, _)| k == key), site.buildings.iter().find(|b| b.name == footprint)) else { continue };
            let q: Vec<Vec2> = b.poly.iter().map(|p| Vec2::from(*p)).collect();
            let c = q.iter().fold(Vec2::ZERO, |s, p| s + *p) / q.len() as f32;
            put(&mut instances, &mut placed, model, at(c));
            println!("plots: the junior high's {footprint} at ({:.0}, {:.0}), ground {ground:.1}", c.x, c.y);
            // the main building's classroom furniture, from its layout (tools/school_layout.py)
            if key == "site:school_main" {
                #[derive(serde::Deserialize)]
                struct Fit {
                    id: String,
                    at: [f32; 3],
                    yaw: f32,
                    #[serde(default)]
                    you: bool,
                    #[serde(default)]
                    room: Option<String>,
                    #[serde(default)]
                    seat: Option<u8>,
                }
                match std::fs::read("assets/buildings/school_main/layout.json").map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice::<Vec<Fit>>(&b)?)) {
                    Ok(fits) => {
                        let mut n = 0;
                        for f in fits {
                            let local = Affine3A::from_rotation_translation(Quat::from_rotation_y(f.yaw.to_radians()), Vec3::new(f.at[0], f.at[2], -f.at[1]));
                            // the room's places for people: each desk's seat, the spots
                            if let Some(room) = &f.room {
                                if let Some(name) = f.id.strip_prefix("spot:") {
                                    spots.push((room.clone(), name.to_string(), at(c) * local));
                                } else if let (Some(seat), "school_desk") = (f.seat, f.id.as_str()) {
                                    spots.push((room.clone(), format!("seat:{seat}"), at(c) * local));
                                }
                            }
                            if let Some((_, m)) = houses.iter().find(|(k, _)| *k == format!("prop:{}", f.id)) {
                                // (Blender x, y, z -> glTF x, z, -y; a turn about Blender +Z is one about +Y)
                                put(&mut instances, &mut placed, m, at(c) * local);
                                n += 1;
                                if f.you {
                                    your_desk = Some(at(c) * local);
                                    let w = (at(c) * local).transform_point3(Vec3::ZERO);
                                    // (the class faces the model's +X: the board)
                                    let f = at(c).transform_vector3(Vec3::X);
                                    println!("plots: your desk in 1-A at ({:.2}, {:.2}, {:.2}), facing yaw {:.1}° (world)", w.x, w.y, w.z, f.x.atan2(-f.z).to_degrees());
                                }
                            }
                        }
                        println!("plots: the junior high's classroom furniture: {n} pieces");
                    }
                    Err(e) => eprintln!("school layout: {e:#}"),
                }
            }
        }
        if let Some((_, model)) = houses.iter().find(|(k, _)| k == "site:school_grounds") {
            let corner = mq[0] - east * 8.0 - south * 10.0;
            put(&mut instances, &mut placed, model, at(corner));
        }
    }
    let mut meshes: Vec<MeshData> = [walls.finish("block walls", block), stones.finish("stone walls", stone), slabs.finish("driveways", concrete), posts.finish("gate posts", post_mat), poles.finish("mailbox poles", pole_mat), retains.finish("retaining walls", retain_mat), boards.finish("board retaining walls", board_mat)].into_iter().flatten().collect();
    let mut solid = Vec::new();
    for m in meshes.iter_mut().filter(|m| m.name != "mailbox poles") {
        let p = |i: u32| Vec3::from(m.positions[i as usize]);
        solid.extend(m.indices.chunks_exact(3).map(|t| [p(t[0]), p(t[1]), p(t[2])]));
    }
    if !meshes.is_empty() {
        let ids = r.add_meshes(&meshes)?;
        instances.extend(ids.into_iter().map(|mesh| Instance { mesh, transform: Affine3A::IDENTITY }));
    }
    println!("plots: outdoors of {n} plots");
    Ok(PlotsScene { instances, placed, home, your_desk, spots, models: mailbox.into_iter().chain(hedge).chain(houses.into_iter().map(|(_, m)| m)).collect(), solid })
}
