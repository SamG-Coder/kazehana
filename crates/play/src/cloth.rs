//! Cloth: hanging fabric (a noren, a curtain, laundry) simulated with extended position based
//! dynamics (XPBD) in small sub-steps (Macklin et al., "Small Steps in Physics Simulation", 2019).
//!
//! A [`Cloth`] is made from a triangle mesh in the world (its render vertices; seams that split a
//! vertex for its UVs are welded back into one particle). Each particle's mass comes from the area
//! of its triangles times the fabric's weight; the particles chosen by the caller are pinned (the
//! top edge on its rod). Constraints:
//! - stretch: every mesh edge keeps its rest length (cotton hardly stretches);
//! - bending: across every inner edge, the two opposite corners keep their rest distance, softly
//!   (`bend_compliance`): what makes heavy cotton fold in broad folds and thin silk in fine ones;
//! - collision: against capsules (people walking through) and fixed triangles (the walls round a
//!   doorway), with friction;
//! - self collision: particles of different pieces of the cloth (the two panels of a noren) keep a
//!   distance apart (within one piece too, optionally).
//!
//! Air: each triangle feels the pressure of the air moving past it (wind minus its own velocity,
//! along its normal), so a breeze makes the cloth sway and the cloth's own motion is damped by the
//! air like real fabric.
//!
//! The simulation runs at a fixed rate (`Settings::rate`) whatever the frame rate: `step` takes
//! the frame's dt and runs as many fixed steps as are due (at most a few: after a hitch it slows
//! down rather than spiral).
//!
//! World space, +Y up, metres, seconds, kilograms.

use crate::collision::closest_on_triangle;
use glam::{Vec2, Vec3};
use std::collections::HashMap;

/// How a fabric behaves.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// weight per area (kg/m²): voile 0.05, shirting 0.12, noren cotton 0.25, canvas 0.4
    pub density: f32,
    /// XPBD compliance of the edges (m/N, scaled by rest length): 0 = inextensible
    pub stretch_compliance: f32,
    /// XPBD compliance of the bending hinges (1/(N m): 1 / the fabric's bending stiffness,
    /// whatever the mesh's resolution): larger is limper
    pub bend_compliance: f32,
    /// damping of the bending (XPBD constraint damping, Macklin et al. 2016, as γ: 0 = none,
    /// ~1 = strong): the fabric's internal friction; quiets the stiff, quick bending modes
    /// without slowing the slow sway
    pub bend_damping: f32,
    /// XPBD compliance of the shear diagonals (woven cloth gives on the bias)
    pub shear_compliance: f32,
    /// velocity lost per second to internal friction (0..1)
    pub damping: f32,
    /// air pressure coefficient: ½ ρ C_d (kg/m³); ~0.6 for woven cotton
    pub drag: f32,
    /// how far a particle keeps from what it collides with (m): half the fabric's thickness plus
    /// a margin so the surface between particles does not cut through
    pub thickness: f32,
    /// how far apart two layers of the cloth keep (m): self collision; 0 = none
    pub self_distance: f32,
    /// self collision also within one piece (a panel folding onto itself), not only between
    /// pieces (the two panels of a noren). Off by default: in tight folds it fights the bending
    /// and keeps the cloth twitching.
    pub self_within_parts: bool,
    /// Coulomb friction against colliders (0..1)
    pub friction: f32,
    /// fixed steps per second, and sub-steps per step
    pub rate: f32,
    pub substeps: u32,
    pub gravity: Vec3,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            density: 0.25,
            stretch_compliance: 0.0,
            // (Bergou's bending stiffness is 1 / this, N m: heavy cotton ~2e-4; values much under
            // ~1e3 are rigid at a few cm of mesh)
            bend_compliance: 5.0e3,
            shear_compliance: 1.0e-4,
            bend_damping: 0.5,
            damping: 0.15,
            drag: 0.6,
            thickness: 0.012,
            self_distance: 0.008,
            self_within_parts: false,
            friction: 0.35,
            rate: 60.0,
            substeps: 12,
            gravity: Vec3::new(0.0, -9.81, 0.0),
        }
    }
}

/// Something the cloth meets.
#[derive(Clone, Copy, Debug)]
pub enum Collider {
    /// the segment a..b swept to radius r (a person: `FirstPerson::capsule`); `vel`: how fast it
    /// moves (m/s), for friction (the cloth is dragged along by what walks into it)
    Capsule { a: Vec3, b: Vec3, r: f32, vel: Vec3 },
    /// a sphere (a head, a hand)
    Sphere { c: Vec3, r: f32, vel: Vec3 },
}

/// How far round a moving body the air moves with it (m).
const WAKE: f32 = 0.6;

/// A distance constraint between two particles.
#[derive(Clone, Copy)]
struct Link {
    i: u32,
    j: u32,
    rest: f32,
    /// 0 stretch (along the threads), 2 shear
    kind: u8,
}

/// A bending hinge: the two triangles on either side of an inner edge. Bending is Bergou et al.'s
/// quadratic model for inextensible surfaces ("A Quadratic Bending Model for Inextensible
/// Surfaces", 2006): the discrete mean curvature vector across the edge, H = Σ k_i x_i (cotangent
/// weights of the rest shape), is held at its rest value. It is linear in the positions, so cheap,
/// and has no effect in the sheet's own plane. (H is not turned with the cloth: right for cloth
/// that hangs, and exact for any cloth that is flat at rest.)
#[derive(Clone, Copy)]
struct Hinge {
    /// edge ends, then the wings
    ids: [u32; 4],
    k: [f32; 4],
    rest: Vec3,
    /// compliance scale for the hinge's size: (A0 + A1) / 3
    scale: f32,
}

/// The cotangent weights of a hinge (edge x0 x1, wings x2 x3), Bergou et al. 2006.
fn hinge_weights(x0: Vec3, x1: Vec3, x2: Vec3, x3: Vec3) -> Option<[f32; 4]> {
    let cot = |a: Vec3, b: Vec3| {
        let c = a.cross(b).length();
        if c < 1e-12 { None } else { Some(a.dot(b) / c) }
    };
    let (e0, e1, e2, e3, e4) = (x1 - x0, x2 - x0, x3 - x0, x2 - x1, x3 - x1);
    let (c01, c02, c03, c04) = (cot(e0, e1)?, cot(e0, e2)?, cot(-e0, e3)?, cot(-e0, e4)?);
    Some([c03 + c04, c01 + c02, -c01 - c03, -c02 - c04])
}

pub struct Cloth {
    pub settings: Settings,
    /// particles: position, position at the start of the sub-step, velocity, 1 / mass (0 = pinned)
    pub pos: Vec<Vec3>,
    prev: Vec<Vec3>,
    vel: Vec<Vec3>,
    inv_mass: Vec<f32>,
    rest: Vec<Vec3>,
    links: Vec<Link>,
    hinges: Vec<Hinge>,
    /// triangles over particles
    tris: Vec<[u32; 3]>,
    /// render vertex -> particle
    vert_particle: Vec<u32>,
    /// each particle's part (connected piece): self collision only between particles that are
    /// far apart in the rest shape
    part: Vec<u32>,
    /// long range attachments: (particle, its nearest pinned particle, rest distance); keeps
    /// heavy cloth from stretching under its own weight (Kim et al. 2012)
    tethers: Vec<(u32, u32, f32)>,
    /// fixed triangles (walls) near the cloth, and this step's (particle, triangle) pairs that
    /// may meet
    solid: Vec<[Vec3; 3]>,
    solid_pairs: Vec<(u32, u32)>,
    /// the clock: time not yet simulated, total time (for gusts)
    pending: f32,
    pub time: f32,
    /// self collision candidates for this step (particle pairs); particles of one part nearer
    /// than `self_near` at rest never collide (the links keep them apart)
    self_pairs: Vec<(u32, u32)>,
    self_near: f32,
    /// dense spatial hash for self collision: cell start per slot, particles sorted by slot
    hash_start: Vec<u32>,
    hash_items: Vec<u32>,
}

/// A small 3D hash noise in -1..1 (smooth): gusts that vary over the cloth and in time.
fn noise(p: Vec3) -> f32 {
    let h = |x: i32, y: i32, z: i32| {
        let mut n = (x as u32).wrapping_mul(0x8da6b343) ^ (y as u32).wrapping_mul(0xd8163841) ^ (z as u32).wrapping_mul(0xcb1ab31f);
        n = (n ^ (n >> 13)).wrapping_mul(0x5bd1e995);
        n ^= n >> 15;
        (n & 0xffff) as f32 / 32767.5 - 1.0
    };
    let f = p.floor();
    let t = p - f;
    let s = t * t * (Vec3::splat(3.0) - 2.0 * t);
    let (x, y, z) = (f.x as i32, f.y as i32, f.z as i32);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(h(x, y, z), h(x + 1, y, z), s.x);
    let x10 = lerp(h(x, y + 1, z), h(x + 1, y + 1, z), s.x);
    let x01 = lerp(h(x, y, z + 1), h(x + 1, y, z + 1), s.x);
    let x11 = lerp(h(x, y + 1, z + 1), h(x + 1, y + 1, z + 1), s.x);
    lerp(lerp(x00, x10, s.y), lerp(x01, x11, s.y), s.z)
}

/// Air moving through a doorway or a street: a mean flow with gusts and turbulence.
#[derive(Clone, Copy, Debug)]
pub struct Air {
    /// the mean flow (m/s)
    pub mean: Vec3,
    /// gust strength (m/s) and how quickly gusts come and go (1/s)
    pub gust: f32,
    pub gust_rate: f32,
    /// small-scale turbulence (m/s) over a few tens of centimetres
    pub turbulence: f32,
}

impl Air {
    /// Still air indoors: the slow draughts of a house (a few cm/s).
    pub fn indoors() -> Self {
        Air { mean: Vec3::ZERO, gust: 0.12, gust_rate: 0.25, turbulence: 0.06 }
    }

    /// The air's velocity at p and time t.
    pub fn at(&self, p: Vec3, t: f32) -> Vec3 {
        let g = t * self.gust_rate;
        // gusts: large, slow, mostly horizontal
        let gust = Vec3::new(noise(Vec3::new(g, 0.3, 1.7)), 0.25 * noise(Vec3::new(4.1, g, 2.3)), noise(Vec3::new(7.9, 3.3, g))) * self.gust;
        // turbulence: smaller eddies drifting with the flow
        let q = p * 3.0 + Vec3::splat(t * 0.8);
        let turb = Vec3::new(noise(q), noise(q + Vec3::new(17.0, 5.0, 3.0)), noise(q + Vec3::new(3.0, 11.0, 23.0))) * self.turbulence;
        self.mean + gust + turb
    }
}

impl Cloth {
    /// A cloth from a mesh in the world: `positions` (render vertices), triangle `indices`, and
    /// which render vertices are pinned. Vertices at the same place are welded into one particle.
    pub fn new(positions: &[Vec3], indices: &[u32], pinned: impl Fn(usize, Vec3) -> bool, settings: Settings) -> Self {
        // weld (seams split vertices for their UVs)
        let mut vert_particle = Vec::with_capacity(positions.len());
        let mut pos: Vec<Vec3> = Vec::new();
        let mut at: HashMap<[i32; 3], u32> = HashMap::new();
        let mut pin: Vec<bool> = Vec::new();
        for (k, &p) in positions.iter().enumerate() {
            let key = (p * 1.0e4).round().as_ivec3().to_array();
            let id = *at.entry(key).or_insert_with(|| {
                pos.push(p);
                pin.push(false);
                pos.len() as u32 - 1
            });
            pin[id as usize] |= pinned(k, p);
            vert_particle.push(id);
        }
        let n = pos.len();
        let tris: Vec<[u32; 3]> = indices.chunks_exact(3).map(|t| [vert_particle[t[0] as usize], vert_particle[t[1] as usize], vert_particle[t[2] as usize]]).filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]).collect();
        // lumped masses: a third of each triangle's area to each corner
        let mut mass = vec![0.0f32; n];
        for t in &tris {
            let a = 0.5 * (pos[t[1] as usize] - pos[t[0] as usize]).cross(pos[t[2] as usize] - pos[t[0] as usize]).length();
            for &i in t {
                mass[i as usize] += a * settings.density / 3.0;
            }
        }
        let inv_mass: Vec<f32> = (0..n).map(|i| if pin[i] || mass[i] <= 0.0 { 0.0 } else { 1.0 / mass[i] }).collect();
        // edges, and the opposite corners across each inner edge
        let mut edges: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
        for t in &tris {
            for k in 0..3 {
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                edges.entry((a.min(b), a.max(b))).or_default().push(c);
            }
        }
        let mut links = Vec::new();
        let mut keys: Vec<_> = edges.keys().copied().collect();
        keys.sort_unstable();
        let dist = |a: u32, b: u32| (pos[a as usize] - pos[b as usize]).length();
        // shear edges: the diagonal of a quad split in two (the longest edge of each of its
        // triangles, clearly longer than the next): woven cloth gives on the bias, so these are
        // soft. (Stiff diagonals would also over-constrain the sheet in its plane, and the solver
        // then shakes.)
        let mut votes: HashMap<(u32, u32), (u32, u32)> = HashMap::new(); // (triangles, as longest)
        for t in &tris {
            let mut e: Vec<((u32, u32), f32)> = (0..3).map(|k| {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                ((a.min(b), a.max(b)), dist(a, b))
            }).collect();
            e.sort_by(|x, y| y.1.total_cmp(&x.1));
            for (k, (key, _)) in e.iter().enumerate() {
                let v = votes.entry(*key).or_default();
                v.0 += 1;
                if k == 0 && e[0].1 > e[1].1 * 1.2 {
                    v.1 += 1;
                }
            }
        }
        for (a, b) in &keys {
            let v = votes[&(*a, *b)];
            links.push(Link { i: *a, j: *b, rest: dist(*a, *b), kind: if v.1 == v.0 { 2 } else { 0 } });
        }
        let mut hinges = Vec::new();
        let area = |a: u32, b: u32, c: u32| 0.5 * (pos[b as usize] - pos[a as usize]).cross(pos[c as usize] - pos[a as usize]).length();
        for (a, b) in &keys {
            let opp = &edges[&(*a, *b)];
            if opp.len() != 2 || opp[0] == opp[1] {
                continue;
            }
            let ids = [*a, *b, opp[0], opp[1]];
            let x = ids.map(|i| pos[i as usize]);
            let Some(k) = hinge_weights(x[0], x[1], x[2], x[3]) else { continue };
            let rest = x[0] * k[0] + x[1] * k[1] + x[2] * k[2] + x[3] * k[3];
            hinges.push(Hinge { ids, k, rest, scale: (area(*a, *b, opp[0]) + area(*a, *b, opp[1])) / 3.0 });
        }
        hinges.retain(|h| h.ids.iter().any(|&i| inv_mass[i as usize] > 0.0));
        let links_mean = links.iter().map(|l| l.rest).sum::<f32>() / links.len().max(1) as f32;
        links.retain(|l| inv_mass[l.i as usize] + inv_mass[l.j as usize] > 0.0);
        // parts: connected pieces (union find over the edges)
        let mut parent: Vec<u32> = (0..n as u32).collect();
        fn find(p: &mut [u32], mut i: u32) -> u32 {
            while p[i as usize] != i {
                p[i as usize] = p[p[i as usize] as usize];
                i = p[i as usize];
            }
            i
        }
        for (a, b) in &keys {
            let (ra, rb) = (find(&mut parent, *a), find(&mut parent, *b));
            parent[ra as usize] = rb;
        }
        let part: Vec<u32> = (0..n as u32).map(|i| find(&mut parent, i)).collect();
        // tethers: each free particle to the nearest pin of its part (straight-line rest distance:
        // right for flat hanging panels, a little slack for anything else)
        let pins: Vec<u32> = (0..n as u32).filter(|&i| inv_mass[i as usize] == 0.0).collect();
        let mut tethers = Vec::new();
        for i in 0..n {
            if inv_mass[i] == 0.0 {
                continue;
            }
            let best = pins.iter().filter(|&&q| part[q as usize] == part[i]).map(|&q| (q, (pos[q as usize] - pos[i]).length())).min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((q, d)) = best {
                tethers.push((i as u32, q, d * 1.01));
            }
        }
        Cloth {
            tethers,
            hinges,
            settings,
            prev: pos.clone(),
            vel: vec![Vec3::ZERO; n],
            rest: pos.clone(),
            pos,
            inv_mass,
            links,
            tris,
            vert_particle,
            part,
            solid: Vec::new(),
            solid_pairs: Vec::new(),
            pending: 0.0,
            time: 0.0,
            self_pairs: Vec::new(),
            self_near: 1.6 * links_mean,
            hash_start: vec![0; 2 * n + 1],
            hash_items: vec![0; n],
        }
    }

    pub fn particle_count(&self) -> usize {
        self.pos.len()
    }

    /// How many wall triangles it keeps out of.
    pub fn solid_count(&self) -> usize {
        self.solid.len()
    }

    pub fn pinned_count(&self) -> usize {
        self.inv_mass.iter().filter(|&&w| w == 0.0).count()
    }

    /// The box round the cloth now.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        self.pos.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)))
    }

    /// The box round the cloth at rest.
    pub fn rest_bounds(&self) -> (Vec3, Vec3) {
        self.rest.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)))
    }

    /// Fixed triangles the cloth must stay out of (the walls and jambs round it). Each particle
    /// keeps to the side of a triangle its rest position is on.
    pub fn set_solid(&mut self, tris: Vec<[Vec3; 3]>) {
        self.solid = tris;
    }

    /// Move the pinned particles (gather a sleeve on its rod, carry a curtain along its rail).
    /// The rest shape stays: what hangs from them finds its own folds.
    pub fn move_pins(&mut self, f: impl Fn(usize, Vec3) -> Vec3) {
        for i in 0..self.pos.len() {
            if self.inv_mass[i] == 0.0 {
                self.pos[i] = f(i, self.pos[i]);
                self.prev[i] = self.pos[i];
            }
        }
    }

    /// Is particle i pinned?
    pub fn is_pinned(&self, i: usize) -> bool {
        self.inv_mass[i] == 0.0
    }

    /// The particles' rest positions.
    pub fn rest(&self) -> &[Vec3] {
        &self.rest
    }

    /// Put every particle somewhere, still (a pose saved from `pos`).
    pub fn set_positions(&mut self, pos: &[Vec3]) {
        self.pos.copy_from_slice(pos);
        self.prev.copy_from_slice(pos);
        self.vel.iter_mut().for_each(|v| *v = Vec3::ZERO);
        self.pending = 0.0;
    }

    /// Back to the rest shape, still.
    pub fn reset(&mut self) {
        self.pos.copy_from_slice(&self.rest);
        self.prev.copy_from_slice(&self.rest);
        self.vel.iter_mut().for_each(|v| *v = Vec3::ZERO);
        self.pending = 0.0;
    }

    /// Advance by `dt` (the frame's time): as many fixed steps as are due, at most 4.
    pub fn step(&mut self, dt: f32, air: &Air, colliders: &[Collider]) -> u32 {
        let h = 1.0 / self.settings.rate;
        self.pending = (self.pending + dt.max(0.0)).min(h * 4.0);
        let mut n = 0;
        while self.pending >= h * 0.999 {
            self.pending -= h;
            self.fixed_step(h, air, colliders);
            n += 1;
        }
        n
    }

    /// One fixed step of length h.
    pub fn fixed_step(&mut self, h: f32, air: &Air, colliders: &[Collider]) {
        let s = self.settings;
        let sub = s.substeps.max(1);
        let dt = h / sub as f32;
        self.time += h;
        // external accelerations for the step: gravity, and the air on each triangle
        let mut acc = vec![s.gravity; self.pos.len()];
        let mut force = vec![Vec3::ZERO; self.pos.len()];
        for t in &self.tris {
            let [a, b, c] = t.map(|i| i as usize);
            let (pa, pb, pc) = (self.pos[a], self.pos[b], self.pos[c]);
            let cr = (pb - pa).cross(pc - pa);
            let area = 0.5 * cr.length();
            if area < 1e-9 {
                continue;
            }
            let nrm = cr / (2.0 * area);
            let centre = (pa + pb + pc) / 3.0;
            let v = (self.vel[a] + self.vel[b] + self.vel[c]) / 3.0;
            let mut flow = air.at(centre, self.time);
            // a body moving through the air drags some of it along (its wake): a noren stirs
            // when someone walks by close
            for col in colliders {
                let (q, r, cv) = match *col {
                    Collider::Capsule { a, b, r, vel } => {
                        let ab = b - a;
                        let t = ((centre - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                        (a + ab * t, r, vel)
                    }
                    Collider::Sphere { c, r, vel } => (c, r, vel),
                };
                let d = ((centre - q).length() - r).max(0.0);
                if d < WAKE {
                    let f = 1.0 - d / WAKE;
                    flow += cv * (0.5 * f * f);
                }
            }
            let rel = flow - v;
            let vn = rel.dot(nrm);
            // pressure along the normal; a little skin friction along the surface
            let f = nrm * (s.drag * area * vn * vn.abs()) + (rel - nrm * vn) * (0.02 * s.drag * area * rel.length());
            for i in [a, b, c] {
                force[i] += f / 3.0;
            }
        }
        for i in 0..self.pos.len() {
            acc[i] += force[i] * self.inv_mass[i];
        }
        self.find_self_pairs(h);
        self.find_solid_pairs(h);
        let damp = (1.0 - s.damping).max(0.0).powf(dt);
        let alpha = [s.stretch_compliance / (dt * dt), s.bend_compliance / (dt * dt), s.shear_compliance / (dt * dt)];
        let wall = s.thickness;
        let gamma = s.bend_damping;
        for _ in 0..sub {
            // predict
            for i in 0..self.pos.len() {
                self.prev[i] = self.pos[i];
                if self.inv_mass[i] == 0.0 {
                    continue;
                }
                self.vel[i] = (self.vel[i] + acc[i] * dt) * damp;
                self.pos[i] += self.vel[i] * dt;
            }
            // links (one XPBD iteration: the small steps do the converging)
            for l in &self.links {
                let (i, j) = (l.i as usize, l.j as usize);
                let (wi, wj) = (self.inv_mass[i], self.inv_mass[j]);
                let d = self.pos[i] - self.pos[j];
                let len = d.length();
                if len < 1e-9 {
                    continue;
                }
                let c = len - l.rest;
                let a = alpha[l.kind as usize] / l.rest.max(1e-3);
                let dl = -c / (wi + wj + a);
                let g = d / len;
                self.pos[i] += g * (dl * wi);
                self.pos[j] -= g * (dl * wj);
            }
            // bending: each hinge's curvature back to its rest value
            for hg in &self.hinges {
                let ids = hg.ids.map(|i| i as usize);
                let w = ids.map(|i| self.inv_mass[i]);
                let hv = self.pos[ids[0]] * hg.k[0] + self.pos[ids[1]] * hg.k[1] + self.pos[ids[2]] * hg.k[2] + self.pos[ids[3]] * hg.k[3] - hg.rest;
                // how the curvature changed this sub-step (for the damping)
                let dh = (self.pos[ids[0]] - self.prev[ids[0]]) * hg.k[0] + (self.pos[ids[1]] - self.prev[ids[1]]) * hg.k[1] + (self.pos[ids[2]] - self.prev[ids[2]]) * hg.k[2] + (self.pos[ids[3]] - self.prev[ids[3]]) * hg.k[3];
                let wk = w[0] * hg.k[0] * hg.k[0] + w[1] * hg.k[1] * hg.k[1] + w[2] * hg.k[2] * hg.k[2] + w[3] * hg.k[3] * hg.k[3];
                let denom = (1.0 + gamma) * wk + alpha[1] * hg.scale;
                if denom < 1e-12 {
                    continue;
                }
                let dl = (hv + dh * gamma) * (-1.0 / denom);
                for k in 0..4 {
                    self.pos[ids[k]] += dl * (w[k] * hg.k[k]);
                }
            }
            // tethers: never farther from the rod than at rest
            for &(i, q, d) in &self.tethers {
                let (i, q) = (i as usize, q as usize);
                let v = self.pos[i] - self.pos[q];
                let len = v.length();
                if len > d {
                    self.pos[i] = self.pos[q] + v * (d / len);
                }
            }
            // self collision: particles of different panels (or far apart on one) keep apart
            let r2 = s.self_distance;
            for &(i, j) in &self.self_pairs {
                let (i, j) = (i as usize, j as usize);
                let (wi, wj) = (self.inv_mass[i], self.inv_mass[j]);
                if wi + wj == 0.0 {
                    continue;
                }
                let d = self.pos[i] - self.pos[j];
                let len = d.length();
                if len >= r2 || len < 1e-9 {
                    continue;
                }
                let g = d / len * ((r2 - len) / (wi + wj));
                self.pos[i] += g * wi;
                self.pos[j] -= g * wj;
                // and they move together (friction between the layers: no energy from the push)
                let (di, dj) = (self.pos[i] - self.prev[i], self.pos[j] - self.prev[j]);
                let avg = (di * wj + dj * wi) / (wi + wj);
                self.pos[i] += (avg - di) * (s.friction * (wi / (wi + wj)) * 2.0).min(1.0);
                self.pos[j] += (avg - dj) * (s.friction * (wj / (wi + wj)) * 2.0).min(1.0);
            }
            // colliders: out of them, with friction relative to their motion
            for i in 0..self.pos.len() {
                if self.inv_mass[i] == 0.0 {
                    continue;
                }
                let p = self.pos[i];
                for c in colliders {
                    let (q, r, v) = match *c {
                        Collider::Capsule { a, b, r, vel } => {
                            let ab = b - a;
                            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                            (a + ab * t, r, vel)
                        }
                        Collider::Sphere { c, r, vel } => (c, r, vel),
                    };
                    let d = self.pos[i] - q;
                    let len = d.length();
                    let rr = r + wall;
                    if len >= rr {
                        continue;
                    }
                    let n = if len > 1e-6 { d / len } else { Vec3::Y };
                    let pen = rr - len;
                    self.pos[i] += n * pen;
                    // friction: the step's motion along the surface, relative to the collider's
                    let moved = (self.pos[i] - self.prev[i]) - v * dt;
                    let tang = moved - n * moved.dot(n);
                    let tl = tang.length();
                    if tl > 1e-9 {
                        self.pos[i] -= tang * (s.friction * pen / tl).min(1.0);
                    }
                }
            }
            // walls: stay on the rest side, a thickness off
            for &(i, t) in &self.solid_pairs {
                let (i, t) = (i as usize, &self.solid[t as usize]);
                let p = self.pos[i];
                let q = closest_on_triangle(p, t[0], t[1], t[2]);
                if (p - q).length_squared() >= wall * wall {
                    continue;
                }
                let nrm = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
                let side = if (self.rest[i] - t[0]).dot(nrm) >= 0.0 { nrm } else { -nrm };
                let depth = wall - (p - q).dot(side);
                if depth > 0.0 {
                    self.pos[i] += side * depth.min(wall * 4.0);
                    let moved = self.pos[i] - self.prev[i];
                    let tang = moved - side * moved.dot(side);
                    self.pos[i] -= tang * s.friction.min(1.0);
                }
            }
            // velocities from the positions
            for i in 0..self.pos.len() {
                if self.inv_mass[i] != 0.0 {
                    self.vel[i] = (self.pos[i] - self.prev[i]) / dt;
                }
            }
        }
    }

    /// Particle pairs close enough to meet this step (spatial hash), from different places of the
    /// rest shape (different panels, or farther apart than they could ever come by folding
    /// along the surface... approximated as twice the collision distance).
    fn find_self_pairs(&mut self, h: f32) {
        self.self_pairs.clear();
        let n = self.pos.len();
        let r = self.settings.self_distance;
        if r <= 0.0 {
            return;
        }
        // pairs that may meet within the step: as close as the distance two particles can close
        // in it (bounded: the pair list stays short when something whips the cloth about)
        let travel = self.vel.iter().map(|v| v.length_squared()).fold(0.0f32, f32::max).sqrt() * h;
        let reach = r * 1.5 + (travel * 2.0).min(0.04);
        // only between pieces (the usual case): nothing to do while their boxes are apart
        if !self.settings.self_within_parts {
            let mut boxes: Vec<(u32, Vec3, Vec3)> = Vec::new();
            for (i, p) in self.pos.iter().enumerate() {
                match boxes.iter_mut().find(|b| b.0 == self.part[i]) {
                    Some(b) => {
                        b.1 = b.1.min(*p);
                        b.2 = b.2.max(*p);
                    }
                    None => boxes.push((self.part[i], *p, *p)),
                }
            }
            let near = |a: &(u32, Vec3, Vec3), b: &(u32, Vec3, Vec3)| (a.1 - Vec3::splat(reach)).cmple(b.2).all() && (b.1 - Vec3::splat(reach)).cmple(a.2).all();
            if !(0..boxes.len()).any(|i| (i + 1..boxes.len()).any(|j| near(&boxes[i], &boxes[j]))) {
                return;
            }
        }
        let cell = reach.max(0.01);
        let slots = self.hash_start.len() - 1;
        let cell_of = |p: Vec3| ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32, (p.z / cell).floor() as i32);
        let slot = |c: (i32, i32, i32)| ((c.0.wrapping_mul(92837111) ^ c.1.wrapping_mul(689287499) ^ c.2.wrapping_mul(283923481)).unsigned_abs() as usize) % slots;
        // counting sort of the particles by slot
        self.hash_start.iter_mut().for_each(|c| *c = 0);
        for p in &self.pos {
            self.hash_start[slot(cell_of(*p))] += 1;
        }
        let mut acc = 0;
        for c in self.hash_start.iter_mut() {
            acc += *c;
            *c = acc;
        }
        for (i, p) in self.pos.iter().enumerate() {
            let s = slot(cell_of(*p));
            self.hash_start[s] -= 1;
            self.hash_items[self.hash_start[s] as usize] = i as u32;
        }
        let reach2 = reach * reach;
        let far2 = self.self_near * self.self_near;
        for i in 0..n {
            let (x, y, z) = cell_of(self.pos[i]);
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let s = slot((x + dx, y + dy, z + dz));
                        for k in self.hash_start[s]..self.hash_start[s + 1] {
                            let j = self.hash_items[k as usize] as usize;
                            if j <= i || (self.pos[i] - self.pos[j]).length_squared() > reach2 {
                                continue;
                            }
                            // neighbours on the surface are kept apart by the links
                            if self.part[i] == self.part[j] && (!self.settings.self_within_parts || (self.rest[i] - self.rest[j]).length_squared() < far2) {
                                continue;
                            }
                            self.self_pairs.push((i as u32, j as u32));
                        }
                    }
                }
            }
        }
        self.self_pairs.sort_unstable();
        self.self_pairs.dedup();
    }

    /// (particle, wall triangle) pairs near enough to meet this step: the triangles near the
    /// cloth's box, then the particles near each triangle's box.
    fn find_solid_pairs(&mut self, h: f32) {
        self.solid_pairs.clear();
        if self.solid.is_empty() {
            return;
        }
        let travel = self.vel.iter().map(|v| v.length_squared()).fold(0.0f32, f32::max).sqrt() * h;
        let reach = Vec3::splat(self.settings.thickness + travel.min(0.1) * 1.5);
        let (lo, hi) = self.bounds();
        for (k, t) in self.solid.iter().enumerate() {
            let (tl, th) = (t[0].min(t[1]).min(t[2]) - reach, t[0].max(t[1]).max(t[2]) + reach);
            if (th.cmplt(lo)).any() || (tl.cmpgt(hi)).any() {
                continue;
            }
            for (i, p) in self.pos.iter().enumerate() {
                if self.inv_mass[i] > 0.0 && p.cmpge(tl).all() && p.cmple(th).all() {
                    self.solid_pairs.push((i as u32, k as u32));
                }
            }
        }
    }

    /// The render vertices' positions now.
    pub fn vertex_positions(&self, out: &mut Vec<[f32; 3]>) {
        out.clear();
        out.extend(self.vert_particle.iter().map(|&p| self.pos[p as usize].to_array()));
    }

    /// Smooth normals of the render vertices (area weighted over the particles' triangles),
    /// oriented like the render mesh's triangles.
    pub fn vertex_normals(&self, out: &mut Vec<[f32; 3]>) {
        let mut acc = vec![Vec3::ZERO; self.pos.len()];
        for t in &self.tris {
            let [a, b, c] = t.map(|i| i as usize);
            let n = (self.pos[b] - self.pos[a]).cross(self.pos[c] - self.pos[a]);
            acc[a] += n;
            acc[b] += n;
            acc[c] += n;
        }
        out.clear();
        out.extend(self.vert_particle.iter().map(|&p| acc[p as usize].normalize_or(Vec3::Z).to_array()));
    }

    /// Tangents of the render vertices for normal mapping (glTF convention: xyz, w handedness),
    /// from their UVs and the given normals.
    pub fn vertex_tangents(&self, indices: &[u32], uvs: &[[f32; 2]], normals: &[[f32; 3]], out: &mut Vec<[f32; 4]>) {
        let n = self.vert_particle.len();
        let mut tan = vec![Vec3::ZERO; n];
        let mut bit = vec![Vec3::ZERO; n];
        let p = |v: u32| self.pos[self.vert_particle[v as usize] as usize];
        for t in indices.chunks_exact(3) {
            let (e1, e2) = (p(t[1]) - p(t[0]), p(t[2]) - p(t[0]));
            let (u0, u1, u2) = (Vec2::from(uvs[t[0] as usize]), Vec2::from(uvs[t[1] as usize]), Vec2::from(uvs[t[2] as usize]));
            let (d1, d2) = (u1 - u0, u2 - u0);
            let det = d1.x * d2.y - d2.x * d1.y;
            if det.abs() < 1e-14 {
                continue;
            }
            let r = 1.0 / det;
            let tt = (e1 * d2.y - e2 * d1.y) * r;
            let bt = (e2 * d1.x - e1 * d2.x) * r;
            for &v in t {
                tan[v as usize] += tt;
                bit[v as usize] += bt;
            }
        }
        out.clear();
        out.extend((0..n).map(|i| {
            let nn = Vec3::from(normals[i]);
            let t = (tan[i] - nn * nn.dot(tan[i])).normalize_or(nn.any_orthonormal_vector());
            let w = if nn.cross(t).dot(bit[i]) < 0.0 { -1.0 } else { 1.0 };
            [t.x, t.y, t.z, w]
        }));
    }

    /// How far the cloth is from its rest shape now (the largest particle offset, m).
    pub fn max_offset(&self) -> f32 {
        self.pos.iter().zip(&self.rest).map(|(p, r)| (*p - *r).length()).fold(0.0, f32::max)
    }

    /// The largest particle speed (m/s).
    pub fn max_speed(&self) -> f32 {
        self.vel.iter().map(|v| v.length()).fold(0.0, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A panel w x h hanging in the x-y plane at z = 0, `nx` x `ny` quads, its top row pinned.
    fn panel(x0: f32, w: f32, h: f32, nx: usize, ny: usize) -> (Vec<Vec3>, Vec<u32>) {
        let mut p = Vec::new();
        let mut idx = Vec::new();
        for j in 0..=ny {
            for i in 0..=nx {
                p.push(Vec3::new(x0 + w * i as f32 / nx as f32, 2.0 - h * j as f32 / ny as f32, 0.0));
            }
        }
        let row = nx as u32 + 1;
        for j in 0..ny as u32 {
            for i in 0..nx as u32 {
                let a = j * row + i;
                idx.extend([a, a + row, a + 1, a + 1, a + row, a + row + 1]);
            }
        }
        (p, idx)
    }

    fn noren() -> Cloth {
        let (mut p, mut idx) = panel(-0.45, 0.43, 0.8, 16, 30);
        let (p2, idx2) = panel(0.02, 0.43, 0.8, 16, 30);
        let off = p.len() as u32;
        p.extend(p2);
        idx.extend(idx2.iter().map(|i| i + off));
        Cloth::new(&p, &idx, |_, v| v.y > 1.999, Settings::default())
    }

    #[test]
    fn hangs_still_and_does_not_stretch() {
        let mut c = noren();
        assert_eq!(c.particle_count(), 2 * 17 * 31);
        assert_eq!(c.pinned_count(), 2 * 17);
        for _ in 0..240 {
            c.step(1.0 / 60.0, &Air { mean: Vec3::ZERO, gust: 0.0, gust_rate: 0.0, turbulence: 0.0 }, &[]);
        }
        // hanging straight down, hardly longer than it is
        let (lo, _) = c.bounds();
        assert!(lo.y > 2.0 - 0.8 - 0.02, "stretched to {}", lo.y);
        assert!(c.max_offset() < 0.02, "moved {}", c.max_offset());
        assert!(c.max_speed() < 0.05, "still moving at {}", c.max_speed());
    }

    #[test]
    fn sways_in_a_draught() {
        let mut c = noren();
        let air = Air::indoors();
        let mut most = 0.0f32;
        for _ in 0..600 {
            c.step(1.0 / 60.0, &air, &[]);
            most = most.max(c.max_offset());
        }
        // a little: centimetres, not a flag
        assert!(most > 0.003 && most < 0.15, "swayed {most}");
    }

    #[test]
    fn parts_round_a_person_and_falls_back() {
        let mut c = noren();
        let still = Air { mean: Vec3::ZERO, gust: 0.0, gust_rate: 0.0, turbulence: 0.0 };
        for _ in 0..60 {
            c.step(1.0 / 60.0, &still, &[]);
        }
        // a child walking through the slit at 1.4 m/s along -z
        let speed = 1.4;
        // (the cloth hangs from 2.0 m to 1.2 m; the child is 1.55 m tall)
        let mut z = 0.8f32;
        let mut deepest = 0.0f32;
        for _ in 0..90 {
            z -= speed / 60.0;
            let feet = Vec3::new(0.0, 0.05, z);
            let cap = Collider::Capsule { a: feet + Vec3::Y * 0.58, b: feet + Vec3::Y * 1.29, r: 0.26, vel: Vec3::new(0.0, 0.0, -speed) };
            c.step(1.0 / 60.0, &still, &[cap]);
            // nothing inside the body
            for p in &c.pos {
                let q = Vec3::new(0.0, p.y.clamp(feet.y + 0.58, feet.y + 1.29), z);
                assert!((*p - q).length() > 0.26 - 0.005 || p.y > feet.y + 1.6, "particle inside the body");
            }
            deepest = deepest.min(c.bounds().0.z);
        }
        assert!(deepest < -0.15, "the cloth was not pushed along: {deepest}");
        for _ in 0..600 {
            c.step(1.0 / 60.0, &still, &[]);
        }
        assert!(c.max_offset() < 0.05, "did not fall back: {}", c.max_offset());
        let (i, v) = c.vel.iter().enumerate().max_by(|a, b| a.1.length().total_cmp(&b.1.length())).unwrap();
        assert!(c.max_speed() < 0.1, "still moving: {} at {i}: rest {:?} off {:?} vel {v:?}", c.max_speed(), c.rest[i], c.pos[i] - c.rest[i]);
    }

    #[test]
    fn keeps_off_a_wall() {
        let mut c = noren();
        // a wall 6 cm behind the cloth; a hand pressing it back to 3 cm short of the wall
        let w = -0.06;
        c.set_solid(vec![[Vec3::new(-1.0, 0.0, w), Vec3::new(1.0, 0.0, w), Vec3::new(1.0, 3.0, w)], [Vec3::new(-1.0, 0.0, w), Vec3::new(1.0, 3.0, w), Vec3::new(-1.0, 3.0, w)]]);
        let still = Air { mean: Vec3::ZERO, gust: 0.0, gust_rate: 0.0, turbulence: 0.0 };
        for k in 0..60 {
            let z = (0.5 - k as f32 * 0.01).max(0.17);
            let cap = Collider::Sphere { c: Vec3::new(-0.25, 1.6, z), r: 0.2, vel: Vec3::new(0.0, 0.0, -0.6) };
            c.step(1.0 / 60.0, &still, &[cap]);
        }
        let lo = c.bounds().0.z;
        assert!(lo > w, "through the wall: {lo}");
    }

    #[test]
    fn fast_enough() {
        let time = |name: &str, mut c: Cloth, air: Air| {
            let t = std::time::Instant::now();
            for _ in 0..60 {
                c.step(1.0 / 60.0, &air, &[Collider::Capsule { a: Vec3::new(0.0, 1.4, 0.1), b: Vec3::new(0.0, 2.2, 0.1), r: 0.26, vel: Vec3::ZERO }]);
            }
            let ms = t.elapsed().as_secs_f64() * 1e3 / 60.0;
            println!("{name}: {} particles, {} links: {ms:.3} ms per step", c.particle_count(), c.links.len());
            ms
        };
        let still = Air { mean: Vec3::ZERO, gust: 0.0, gust_rate: 0.0, turbulence: 0.0 };
        time("still, no self", { let mut c = noren(); c.settings.self_distance = 0.0; c }, still);
        time("still", noren(), still);
        let ms = time("indoors", noren(), Air::indoors());
        // (optimised builds only: `cargo test --release`)
        assert!(cfg!(debug_assertions) || ms < 1.5, "too slow: {ms} ms");
    }
}
