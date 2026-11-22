//! The old blockout levels: Home, the Path to school, School grounds. NOT DRAWN: the game walks
//! in the real town (town::scene). They remain only as the fixtures the game's tests use for its
//! interaction zones (shop counter, classroom, futon), until those zones are placed in the town;
//! then this module goes.
#![allow(dead_code)]

use akari::glam::{Mat4, Quat, Vec3};
use crate::paint::{MeshBuilder, MeshData};
use sim::people::Spot;
use sim::Location;
use tcg::Rng;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min: min.min(max), max: min.max(max) }
    }
    pub fn contains_xz(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.z >= self.min.z && p.z <= self.max.z
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub pos: Vec3,
    /// Emitter radius (soft shadows).
    pub radius: f32,
    /// Linear colour x intensity, renderer units.
    pub intensity: Vec3,
    /// Ignored beyond this distance (m).
    pub range: f32,
    /// Only on at night (street lamps); otherwise always on (indoor lights).
    pub night_only: bool,
    /// Spot lights: direction and cosine of the outer cone.
    pub spot: Option<(Vec3, f32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Exit {
    pub area: Aabb,
    pub to: Location,
    pub spawn: Vec3,
    pub yaw: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interact {
    ShopCounter,
    Bed,
    Classroom,
}

pub struct Level {
    pub location: Location,
    pub name: &'static str,
    pub mesh: MeshData,
    /// Window panes, signs, lamp heads: drawn with emissive strength.
    pub glow: MeshData,
    /// Signs and fixtures that glow day and night.
    pub neon: MeshData,
    pub colliders: Vec<Aabb>,
    pub lights: Vec<Light>,
    pub exits: Vec<Exit>,
    pub zones: Vec<(Aabb, Interact)>,
    /// Where students stand for each schedule spot in this level (with facing yaw).
    pub anchors: Vec<(Spot, Vec3, f32)>,
    /// Fog density outside; 0 for none.
    pub fog: f32,
}

impl Level {
    pub fn spawn_default(&self) -> (Vec3, f32) {
        match self.location {
            Location::Home => (Vec3::new(4.5, 0.0, 7.0), 0.0),
            Location::Path => (Vec3::new(0.0, 0.0, -2.0), 0.0),
            Location::School => (Vec3::new(0.0, 0.0, 38.0), 0.0),
        }
    }
}

struct Lb {
    mb: MeshBuilder,
    glow: MeshBuilder,
    neon: MeshBuilder,
    colliders: Vec<Aabb>,
    lights: Vec<Light>,
    rng: Rng,
}

// Palette (sRGB).
const ASPHALT: [f32; 3] = [0.24, 0.24, 0.26];
const WHITE_LINE: [f32; 3] = [0.9, 0.9, 0.88];
const PLASTER: [f32; 3] = [0.86, 0.83, 0.76];
const DARK_WOOD: [f32; 3] = [0.30, 0.20, 0.13];
const WOOD: [f32; 3] = [0.55, 0.38, 0.22];
const ROOF_TILE: [f32; 3] = [0.22, 0.25, 0.30];
const GRASS: [f32; 3] = [0.36, 0.52, 0.24];
const VERMILION: [f32; 3] = [0.85, 0.22, 0.12];
const CONCRETE: [f32; 3] = [0.66, 0.66, 0.64];
const WINDOW_WARM: [f32; 3] = [1.0, 0.82, 0.55];

impl Lb {
    fn new(seed: &str) -> Self {
        Self {
            mb: MeshBuilder::default(),
            glow: MeshBuilder::default(),
            neon: MeshBuilder::default(),
            colliders: Vec::new(),
            lights: Vec::new(),
            rng: Rng::derive(7, seed),
        }
    }

    /// Visible box with a collider.
    fn solid(&mut self, min: Vec3, max: Vec3, color: [f32; 3]) {
        let b = Aabb::new(min, max);
        self.mb.cuboid((b.min + b.max) * 0.5, b.max - b.min, color);
        self.colliders.push(b);
    }

    /// Visible box, no collision.
    fn deco(&mut self, min: Vec3, max: Vec3, color: [f32; 3]) {
        let b = Aabb::new(min, max);
        self.mb.cuboid((b.min + b.max) * 0.5, b.max - b.min, color);
    }

    fn glow_box(&mut self, min: Vec3, max: Vec3, color: [f32; 3]) {
        let b = Aabb::new(min, max);
        self.glow.cuboid((b.min + b.max) * 0.5, b.max - b.min, color);
    }

    fn neon_box(&mut self, min: Vec3, max: Vec3, color: [f32; 3]) {
        let b = Aabb::new(min, max);
        self.neon.cuboid((b.min + b.max) * 0.5, b.max - b.min, color);
    }

    fn light(&mut self, pos: Vec3, radius: f32, color: Vec3, night_only: bool) {
        // (greybox units: colour x intensity in old raster units; reach = radius)
        self.lights.push(Light { pos, radius: 0.08, intensity: color * 0.012, range: radius * 1.6, night_only, spot: None });
    }

    fn ground(&mut self, half: f32, color: [f32; 3]) {
        self.mb.cuboid(Vec3::new(0.0, -0.05, 0.0), Vec3::new(half * 2.0, 0.1, half * 2.0), color);
    }

    /// A two-storey wooden Japanese house with a tiled hip roof, facing +X or -X (`face` = ±1).
    fn house(&mut self, cx: f32, cz: f32, w: f32, d: f32, face: f32) {
        let floors = if self.rng.chance(0.6) { 2.0 } else { 1.0 };
        let h = 2.8 * floors;
        let wall = if self.rng.chance(0.5) { PLASTER } else { [0.62, 0.50, 0.38] };
        self.solid(Vec3::new(cx - w / 2.0, 0.0, cz - d / 2.0), Vec3::new(cx + w / 2.0, h, cz + d / 2.0), wall);
        // Dark timber band between floors and a base.
        self.deco(Vec3::new(cx - w / 2.0 - 0.03, 0.0, cz - d / 2.0 - 0.03), Vec3::new(cx + w / 2.0 + 0.03, 0.4, cz + d / 2.0 + 0.03), DARK_WOOD);
        if floors > 1.0 {
            self.deco(Vec3::new(cx - w / 2.0 - 0.05, 2.7, cz - d / 2.0 - 0.05), Vec3::new(cx + w / 2.0 + 0.05, 2.9, cz + d / 2.0 + 0.05), DARK_WOOD);
        }
        // Hip roof from two slabs and a ridge.
        let over = 0.6;
        let slope = 0.45f32;
        let half = w / 2.0 + over;
        let len = half / slope.cos();
        for s in [-1.0f32, 1.0] {
            let m = Mat4::from_scale_rotation_translation(
                Vec3::new(len, 0.15, d + 2.0 * over),
                Quat::from_rotation_z(-s * slope),
                Vec3::new(cx + s * half / 2.0, h + half * slope.tan() / 2.0, cz),
            );
            self.mb.cuboid_m(m, ROOF_TILE);
        }
        // Windows on the street side (glow at night) and the door.
        let fx = cx + face * (w / 2.0 + 0.02);
        for f in 0..floors as i32 {
            let y = 0.9 + f as f32 * 2.8;
            for k in [-0.28f32, 0.28] {
                let z = cz + k * d;
                self.glow_box(Vec3::new(fx - 0.03, y, z - 0.6), Vec3::new(fx + 0.03, y + 1.1, z + 0.6), WINDOW_WARM);
                self.deco(Vec3::new(fx - 0.05, y - 0.08, z - 0.68), Vec3::new(fx + 0.05, y, z + 0.68), DARK_WOOD);
            }
        }
        self.deco(Vec3::new(fx - 0.05, 0.0, cz - 0.5), Vec3::new(fx + 0.05, 2.1, cz + 0.5), DARK_WOOD);
    }

    fn tree(&mut self, x: f32, z: f32, scale: f32) {
        let trunk = [0.36, 0.26, 0.18];
        self.mb.cylinder(Vec3::new(x, 0.0, z), 0.18 * scale, 2.0 * scale, trunk);
        self.colliders.push(Aabb::new(Vec3::new(x - 0.25, 0.0, z - 0.25), Vec3::new(x + 0.25, 3.0, z + 0.25)));
        let g = 0.8 + self.rng.f32() * 0.3;
        let leaf = [0.18 * g, 0.36 * g, 0.16 * g];
        self.mb.sphere(Vec3::new(x, 2.8 * scale, z), Vec3::splat(1.5 * scale), leaf);
        self.mb.sphere(Vec3::new(x + 0.6 * scale, 2.3 * scale, z + 0.3 * scale), Vec3::splat(1.0 * scale), leaf);
    }

    fn cedar(&mut self, x: f32, z: f32, scale: f32) {
        self.mb.cylinder(Vec3::new(x, 0.0, z), 0.2 * scale, 3.0 * scale, [0.33, 0.22, 0.15]);
        self.colliders.push(Aabb::new(Vec3::new(x - 0.3, 0.0, z - 0.3), Vec3::new(x + 0.3, 3.0, z + 0.3)));
        self.mb.cone(Vec3::new(x, 1.5 * scale, z), 1.6 * scale, 6.5 * scale, [0.12, 0.26, 0.14]);
    }

    fn sakura(&mut self, x: f32, z: f32) {
        self.mb.cylinder(Vec3::new(x, 0.0, z), 0.2, 2.2, [0.30, 0.22, 0.20]);
        self.colliders.push(Aabb::new(Vec3::new(x - 0.25, 0.0, z - 0.25), Vec3::new(x + 0.25, 3.0, z + 0.25)));
        let pink = [0.98, 0.76, 0.84];
        for (dx, dy, dz, r) in [(0.0, 3.2, 0.0, 1.8), (1.0, 2.8, 0.5, 1.2), (-0.9, 2.9, -0.4, 1.3), (0.2, 3.9, -0.6, 1.1)] {
            self.mb.sphere(Vec3::new(x + dx, dy, z + dz), Vec3::new(r, r * 0.8, r), pink);
        }
    }

    fn utility_pole(&mut self, x: f32, z: f32) {
        self.mb.cylinder(Vec3::new(x, 0.0, z), 0.14, 9.0, CONCRETE);
        self.colliders.push(Aabb::new(Vec3::new(x - 0.2, 0.0, z - 0.2), Vec3::new(x + 0.2, 9.0, z + 0.2)));
        self.deco(Vec3::new(x - 0.8, 8.2, z - 0.06), Vec3::new(x + 0.8, 8.32, z + 0.06), [0.3, 0.3, 0.3]);
        self.mb.cylinder(Vec3::new(x + 0.3, 7.0, z), 0.22, 0.6, [0.45, 0.45, 0.47]); // transformer
    }

    /// Wire between two pole tops (a thin sagging line approximated by 4 segments).
    fn wire(&mut self, a: Vec3, b: Vec3) {
        let pts: Vec<Vec3> = (0..=4).map(|i| {
            let t = i as f32 / 4.0;
            a.lerp(b, t) - Vec3::Y * (4.0 * t * (1.0 - t)) * 0.5
        }).collect();
        for w in pts.windows(2) {
            let d = w[1] - w[0];
            let m = Mat4::from_scale_rotation_translation(
                Vec3::new(0.025, d.length(), 0.025),
                Quat::from_rotation_arc(Vec3::Y, d.normalize()),
                w[0],
            );
            self.mb.cylinder_m(m, [0.08, 0.08, 0.08]);
        }
    }

    fn street_lamp(&mut self, x: f32, z: f32, arm: f32) {
        self.mb.cylinder(Vec3::new(x, 0.0, z), 0.07, 5.0, [0.5, 0.5, 0.52]);
        self.deco(Vec3::new(x.min(x + arm), 4.9, z - 0.05), Vec3::new(x.max(x + arm), 5.0, z + 0.05), [0.5, 0.5, 0.52]);
        let hx = x + arm;
        self.glow_box(Vec3::new(hx - 0.25, 4.75, z - 0.12), Vec3::new(hx + 0.25, 4.88, z + 0.12), [0.9, 0.95, 1.0]);
        self.light(Vec3::new(hx, 4.6, z), 14.0, Vec3::new(3.0, 3.2, 3.5), true);
    }

    fn torii(&mut self, x: f32, z: f32, yaw: f32, s: f32) {
        let (c, sn) = (yaw.cos(), yaw.sin());
        let place = |lx: f32, lz: f32| Vec3::new(x + lx * c - lz * sn, 0.0, z + lx * sn + lz * c);
        for side in [-1.0f32, 1.0] {
            let p = place(side * 1.4 * s, 0.0);
            self.mb.cylinder(p, 0.16 * s, 3.4 * s, VERMILION);
            self.colliders.push(Aabb::new(p - Vec3::new(0.2, 0.0, 0.2), p + Vec3::new(0.2, 3.0, 0.2)));
        }
        let mid = place(0.0, 0.0);
        self.mb.cuboid_yaw(mid + Vec3::Y * 2.7 * s, Vec3::new(3.4 * s, 0.2 * s, 0.25 * s), -yaw, VERMILION);
        self.mb.cuboid_yaw(mid + Vec3::Y * 3.4 * s, Vec3::new(4.2 * s, 0.25 * s, 0.35 * s), -yaw, [0.12, 0.1, 0.1]);
        self.mb.cuboid_yaw(mid + Vec3::Y * 3.2 * s, Vec3::new(3.9 * s, 0.2 * s, 0.3 * s), -yaw, VERMILION);
    }

    fn vending_machine(&mut self, x: f32, z: f32, face_z: f32, color: [f32; 3]) {
        self.solid(Vec3::new(x - 0.5, 0.0, z - 0.4), Vec3::new(x + 0.5, 1.85, z + 0.4), color);
        let fz = z + face_z * 0.41;
        self.neon_box(Vec3::new(x - 0.4, 0.9, fz - 0.01), Vec3::new(x + 0.4, 1.7, fz + 0.01), [0.95, 0.97, 1.0]);
        self.light(Vec3::new(x, 1.3, z + face_z * 1.0), 4.0, Vec3::new(1.0, 1.05, 1.1), true);
    }

    fn finish(self, location: Location, name: &'static str, exits: Vec<Exit>, zones: Vec<(Aabb, Interact)>, anchors: Vec<(Spot, Vec3, f32)>, fog: f32) -> Level {
        Level {
            location,
            name,
            mesh: self.mb.finish(),
            glow: self.glow.finish(),
            neon: self.neon.finish(),
            colliders: self.colliders,
            lights: self.lights,
            exits,
            zones,
            anchors,
            fog,
        }
    }
}

const PI: f32 = std::f32::consts::PI;

// ---------------------------------------------------------------------------------------------

/// Home: the Tsukumodō game & card shop (ground floor), living room and bedroom behind it, and the
/// street out front. The shop front faces north.
pub fn build_home() -> Level {
    let mut b = Lb::new("home");
    b.ground(80.0, GRASS);
    let (h, t) = (3.2, 0.2);
    let floor_wood = [0.62, 0.48, 0.32];
    b.deco(Vec3::new(-6.0, 0.0, -5.0), Vec3::new(6.0, 0.02, 10.0), floor_wood);
    b.deco(Vec3::new(-6.2, h, -5.2), Vec3::new(6.2, h + 0.2, 10.2), PLASTER); // ceiling
    // Outer walls. North (shop front): door x∈[-1,1], big windows either side.
    let wall = [0.9, 0.88, 0.82];
    for (x0, x1) in [(-6.2, -5.0), (-2.0, -1.0), (1.0, 2.0), (5.0, 6.2)] {
        b.solid(Vec3::new(x0, 0.0, -5.2), Vec3::new(x1, h, -5.0), wall);
    }
    for (x0, x1) in [(-5.0, -2.0), (2.0, 5.0)] {
        b.solid(Vec3::new(x0, 0.0, -5.2), Vec3::new(x1, 0.9, -5.0), wall);
        b.deco(Vec3::new(x0, 2.4, -5.2), Vec3::new(x1, h, -5.0), wall);
    }
    b.deco(Vec3::new(-1.0, 2.3, -5.2), Vec3::new(1.0, h, -5.0), wall);
    b.solid(Vec3::new(-6.2, 0.0, -5.2), Vec3::new(-6.0, h, 10.2), wall);
    b.solid(Vec3::new(6.0, 0.0, -5.2), Vec3::new(6.2, h, 10.2), wall);
    b.solid(Vec3::new(-6.2, 0.0, 10.0), Vec3::new(6.2, h, 10.2), wall);
    // Back wall of the shop with a doorway to the house (x∈[3.4,4.6]).
    b.solid(Vec3::new(-6.0, 0.0, 5.0), Vec3::new(3.4, h, 5.0 + t), wall);
    b.solid(Vec3::new(4.6, 0.0, 5.0), Vec3::new(6.0, h, 5.0 + t), wall);
    b.deco(Vec3::new(3.4, 2.2, 5.0), Vec3::new(4.6, h, 5.0 + t), wall);
    // Noren curtain over the back doorway.
    b.deco(Vec3::new(3.4, 1.5, 5.05), Vec3::new(4.6, 2.2, 5.12), [0.15, 0.25, 0.5]);
    // Shop sign above the front (always lit).
    b.solid(Vec3::new(-4.0, h + 0.2, -5.6), Vec3::new(4.0, h + 1.3, -5.2), DARK_WOOD);
    b.neon_box(Vec3::new(-3.6, h + 0.35, -5.65), Vec3::new(3.6, h + 1.15, -5.6), [1.0, 0.75, 0.3]);
    b.light(Vec3::new(0.0, h + 0.7, -6.5), 8.0, Vec3::new(2.0, 1.4, 0.6), false);

    // Counter with register; the pack wall behind it.
    b.solid(Vec3::new(-4.5, 0.0, 2.0), Vec3::new(0.5, 1.0, 2.8), WOOD);
    b.deco(Vec3::new(-4.55, 1.0, 1.95), Vec3::new(0.55, 1.06, 2.85), [0.85, 0.85, 0.8]);
    b.deco(Vec3::new(-1.0, 1.06, 2.2), Vec3::new(-0.4, 1.35, 2.6), [0.2, 0.2, 0.22]);
    b.solid(Vec3::new(-5.0, 0.0, 4.5), Vec3::new(0.0, 2.6, 5.0), DARK_WOOD);
    let pack_colors = [[0.85, 0.25, 0.15], [0.2, 0.45, 0.85], [0.2, 0.7, 0.5], [0.9, 0.75, 0.2], [0.5, 0.3, 0.75]];
    for row in 0..5 {
        for col in 0..12 {
            let x = -4.8 + col as f32 * 0.4;
            let y = 0.4 + row as f32 * 0.45;
            let c = pack_colors[(col / 3 + row) % pack_colors.len()];
            b.deco(Vec3::new(x, y, 4.38), Vec3::new(x + 0.3, y + 0.38, 4.5), c);
        }
    }
    // Singles glass case along the west wall.
    b.solid(Vec3::new(-6.0, 0.0, -4.0), Vec3::new(-5.2, 1.0, 1.0), DARK_WOOD);
    b.glow_box(Vec3::new(-5.95, 1.0, -3.95), Vec3::new(-5.25, 1.02, 0.95), [0.9, 0.95, 1.0]);
    for i in 0..10 {
        let z = -3.7 + i as f32 * 0.46;
        let c = pack_colors[i % pack_colors.len()];
        b.deco(Vec3::new(-5.8, 1.02, z), Vec3::new(-5.45, 1.04, z + 0.28), c);
    }
    // Game shelves along the east wall.
    for z in [-4.5f32, -2.5] {
        b.solid(Vec3::new(5.4, 0.0, z), Vec3::new(6.0, 2.4, z + 1.8), WOOD);
        for s in 0..4 {
            let y = 0.3 + s as f32 * 0.55;
            for k in 0..4 {
                let c = pack_colors[(k + s) % pack_colors.len()];
                b.deco(Vec3::new(5.35, y, z + 0.1 + k as f32 * 0.42), Vec3::new(5.45, y + 0.42, z + 0.45 + k as f32 * 0.42), c);
            }
        }
    }
    // Two play tables with stools and playmats.
    for z in [-2.6f32, 0.9] {
        b.solid(Vec3::new(1.6, 0.0, z - 0.6), Vec3::new(4.2, 0.75, z + 0.6), [0.75, 0.72, 0.68]);
        b.deco(Vec3::new(1.8, 0.75, z - 0.5), Vec3::new(2.8, 0.76, z + 0.5), [0.15, 0.2, 0.45]);
        b.deco(Vec3::new(3.0, 0.75, z - 0.5), Vec3::new(4.0, 0.76, z + 0.5), [0.45, 0.15, 0.2]);
        for x in [1.1f32, 4.7] {
            b.mb.cylinder(Vec3::new(x, 0.0, z), 0.22, 0.48, [0.3, 0.3, 0.32]);
        }
    }
    // Posters.
    for (x, c) in [(-3.5f32, [0.8, 0.2, 0.2]), (-1.6, [0.2, 0.3, 0.8]), (1.2, [0.95, 0.75, 0.2])] {
        b.deco(Vec3::new(x, 1.4, 4.99), Vec3::new(x + 0.6, 2.25, 5.0), c);
    }
    // Ceiling fluorescent tubes.
    for (x, z) in [(-3.0f32, -2.0f32), (3.0, -2.0), (-3.0, 2.0), (3.0, 2.0)] {
        b.neon_box(Vec3::new(x - 0.8, h - 0.08, z - 0.08), Vec3::new(x + 0.8, h - 0.02, z + 0.08), [0.95, 0.98, 1.0]);
        b.light(Vec3::new(x, h - 0.3, z), 7.0, Vec3::new(2.2, 2.3, 2.4), false);
    }

    // Living room / kitchen (x<2) and your bedroom (x>2) behind the shop.
    b.solid(Vec3::new(2.0, 0.0, 5.2), Vec3::new(2.15, h, 8.4), wall);
    b.deco(Vec3::new(2.0, 2.2, 8.4), Vec3::new(2.15, h, 10.0), wall);
    b.deco(Vec3::new(-6.0, 0.02, 5.2), Vec3::new(2.0, 0.04, 10.0), [0.78, 0.74, 0.55]); // tatami
    b.solid(Vec3::new(-2.5, 0.0, 6.8), Vec3::new(-0.5, 0.4, 8.2), DARK_WOOD); // low table
    b.solid(Vec3::new(-6.0, 0.0, 5.2), Vec3::new(-5.3, 0.9, 9.0), [0.85, 0.85, 0.85]); // kitchen counter
    b.solid(Vec3::new(-6.0, 0.0, 9.3), Vec3::new(-5.2, 1.8, 10.0), [0.9, 0.9, 0.92]); // fridge
    b.mb.cylinder(Vec3::new(-1.5, 2.6, 7.5), 0.35, 0.3, [1.0, 0.95, 0.85]);
    b.light(Vec3::new(-1.5, 2.4, 7.5), 6.0, Vec3::new(2.0, 1.6, 1.1), false);
    // Bedroom: futon, desk, a binder of cards, a little window.
    b.deco(Vec3::new(2.15, 0.02, 5.2), Vec3::new(6.0, 0.04, 10.0), [0.8, 0.76, 0.58]);
    b.deco(Vec3::new(3.4, 0.04, 8.0), Vec3::new(5.6, 0.22, 9.7), [0.92, 0.92, 0.95]);
    b.deco(Vec3::new(3.4, 0.22, 8.0), Vec3::new(5.6, 0.3, 9.0), [0.3, 0.5, 0.8]);
    b.deco(Vec3::new(3.5, 0.22, 9.2), Vec3::new(4.2, 0.35, 9.6), [0.95, 0.95, 0.95]);
    b.solid(Vec3::new(5.2, 0.0, 5.4), Vec3::new(6.0, 0.75, 6.8), WOOD);
    b.deco(Vec3::new(5.4, 0.75, 5.8), Vec3::new(5.8, 0.82, 6.3), [0.6, 0.15, 0.2]);
    b.mb.cylinder(Vec3::new(4.0, 2.6, 7.5), 0.3, 0.3, [1.0, 0.95, 0.85]);
    b.light(Vec3::new(4.0, 2.4, 7.5), 5.0, Vec3::new(1.8, 1.5, 1.1), false);

    // The street out front, houses opposite, and the road north to school through a gap.
    b.deco(Vec3::new(-40.0, 0.0, -7.2), Vec3::new(40.0, 0.04, -5.2), CONCRETE);
    b.deco(Vec3::new(-40.0, 0.0, -13.0), Vec3::new(40.0, 0.03, -7.2), ASPHALT);
    b.deco(Vec3::new(-3.0, 0.0, -40.0), Vec3::new(3.0, 0.03, -13.0), ASPHALT);
    for x in [-16.0f32, 16.0] {
        b.house(x, 2.0, 7.0, 9.0, -1.0);
    }
    for x in [-24.0f32, -12.0, 12.0, 24.0] {
        b.house(x, -19.0, 8.0, 7.0, 1.0);
    }
    b.vending_machine(-3.5, -5.8, -1.0, [0.85, 0.2, 0.2]);
    b.vending_machine(-4.6, -5.8, -1.0, [0.2, 0.4, 0.85]);
    b.street_lamp(5.0, -7.4, -1.2);
    b.utility_pole(-8.0, -7.6);
    b.utility_pole(10.0, -7.6);
    b.wire(Vec3::new(-8.0, 8.2, -7.6), Vec3::new(10.0, 8.2, -7.6));
    b.cedar(-30.0, 8.0, 1.2);
    b.tree(28.0, 7.0, 1.1);

    let exits = vec![Exit {
        area: Aabb::new(Vec3::new(-3.0, 0.0, -45.0), Vec3::new(3.0, 3.0, -24.0)),
        to: Location::Path,
        spawn: Vec3::new(0.0, 0.0, -2.0),
        yaw: 0.0,
    }];
    let zones = vec![
        (Aabb::new(Vec3::new(-4.5, 0.0, 0.8), Vec3::new(0.5, 2.0, 2.0)), Interact::ShopCounter),
        (Aabb::new(Vec3::new(2.2, 0.0, 7.3), Vec3::new(6.0, 2.0, 10.0)), Interact::Bed),
    ];
    let anchors = vec![(Spot::Shop, Vec3::new(2.9, 0.0, -0.8), PI / 2.0)];
    b.finish(Location::Home, "Tsukumodō", exits, zones, anchors, 0.004)
}

/// The walk to school: houses, a konbini, rice paddies, an Inari shrine, a river bridge and a
/// level crossing. Home end at z≈0, school end at z≈-200.
pub fn build_path() -> Level {
    let mut b = Lb::new("path");
    b.ground(600.0, GRASS);
    // Road with white edge lines and gutters.
    b.deco(Vec3::new(-3.0, 0.0, -205.0), Vec3::new(3.0, 0.03, 10.0), ASPHALT);
    for x in [-2.75f32, 2.65] {
        b.deco(Vec3::new(x, 0.03, -205.0), Vec3::new(x + 0.1, 0.035, 10.0), WHITE_LINE);
    }
    for x in [-3.4f32, 3.0] {
        b.deco(Vec3::new(x, -0.25, -205.0), Vec3::new(x + 0.4, 0.02, 10.0), [0.5, 0.5, 0.48]);
    }
    // Tsukumodō's front at the home end, so the walk feels continuous.
    b.solid(Vec3::new(-6.0, 0.0, 6.0), Vec3::new(6.0, 3.4, 9.0), [0.9, 0.88, 0.82]);
    b.neon_box(Vec3::new(-3.6, 3.5, 5.9), Vec3::new(3.6, 4.3, 6.0), [1.0, 0.75, 0.3]);
    // Houses along the first stretch.
    let mut z = -4.0;
    while z > -70.0 {
        let w = 6.0 + b.rng.f32() * 3.0;
        let d = 7.0 + b.rng.f32() * 3.0;
        if !(-64.0..=-44.0).contains(&z) {
            b.house(-6.5 - w / 2.0, z - d / 2.0, w, d, 1.0);
        }
        b.house(6.5 + w / 2.0, z - d / 2.0 - 2.0, w, d, -1.0);
        // Low block walls and hedges in front.
        b.solid(Vec3::new(4.0, 0.0, z - d - 1.0), Vec3::new(4.3, 1.1, z), [0.6, 0.6, 0.58]);
        z -= d + 2.0;
    }
    // Konbini with glowing windows and sign.
    b.deco(Vec3::new(-24.0, 0.0, -64.0), Vec3::new(-3.5, 0.04, -44.0), [0.4, 0.4, 0.42]);
    b.solid(Vec3::new(-24.0, 0.0, -62.0), Vec3::new(-12.0, 3.8, -46.0), [0.95, 0.95, 0.95]);
    b.neon_box(Vec3::new(-11.98, 0.3, -60.0), Vec3::new(-11.9, 2.6, -48.0), [1.0, 1.0, 0.95]);
    b.neon_box(Vec3::new(-11.95, 2.9, -62.0), Vec3::new(-11.85, 3.5, -46.0), [0.2, 0.6, 0.35]);
    b.light(Vec3::new(-9.0, 2.5, -54.0), 12.0, Vec3::new(3.0, 3.0, 3.0), true);
    b.vending_machine(-11.0, -63.0, 1.0, [0.2, 0.55, 0.3]);
    // Rice paddies: flooded fields with rows of young rice.
    for (x0, x1) in [(-90.0f32, -6.0f32), (6.0, 90.0)] {
        let mut z = -74.0;
        while z > -148.0 {
            let z1 = z - 22.0;
            b.deco(Vec3::new(x0, -0.2, z1), Vec3::new(x1, 0.0, z), [0.32, 0.4, 0.38]);
            // Raised levees.
            b.deco(Vec3::new(x0, 0.0, z - 0.5), Vec3::new(x1, 0.25, z), [0.42, 0.36, 0.26]);
            let mut rz = z - 1.5;
            while rz > z1 + 1.0 {
                b.deco(Vec3::new(x0 + 1.0, 0.0, rz - 0.12), Vec3::new(x1 - 1.0, 0.28, rz + 0.12), [0.42, 0.62, 0.26]);
                rz -= 0.9;
            }
            z = z1 - 0.5;
        }
    }
    // Utility poles with wires all the way.
    let mut prev: Option<Vec3> = None;
    let mut z = -2.0;
    while z > -200.0 {
        b.utility_pole(3.8, z);
        let top = Vec3::new(3.8, 8.2, z);
        if let Some(p) = prev {
            b.wire(p, top);
        }
        prev = Some(top);
        z -= 32.0;
    }
    let mut z = -12.0;
    while z > -200.0 {
        b.street_lamp(-3.6, z, 1.2);
        z -= 28.0;
    }
    // Inari shrine to the west: a tunnel of torii, the hall, fox statues, cedars.
    b.deco(Vec3::new(-40.0, 0.0, -112.0), Vec3::new(-3.0, 0.05, -108.0), [0.6, 0.58, 0.52]);
    let mut x = -6.0;
    while x > -32.0 {
        b.torii(x, -110.0, PI / 2.0, 0.9);
        x -= 2.6;
    }
    b.solid(Vec3::new(-46.0, 0.0, -116.0), Vec3::new(-38.0, 0.8, -104.0), [0.55, 0.55, 0.52]);
    b.solid(Vec3::new(-45.0, 0.8, -114.5), Vec3::new(-39.0, 4.0, -105.5), [0.6, 0.18, 0.1]);
    let roof = Mat4::from_scale_rotation_translation(Vec3::new(8.5, 0.3, 11.5), Quat::IDENTITY, Vec3::new(-42.0, 4.3, -110.0));
    b.mb.cuboid_m(roof, [0.18, 0.18, 0.2]);
    for z in [-113.0f32, -107.0] {
        b.mb.cuboid(Vec3::new(-36.5, 0.5, z), Vec3::new(0.8, 1.0, 0.8), [0.55, 0.55, 0.52]);
        b.mb.sphere(Vec3::new(-36.5, 1.4, z), Vec3::new(0.35, 0.5, 0.3), [0.9, 0.9, 0.88]);
    }
    b.light(Vec3::new(-37.0, 2.0, -110.0), 8.0, Vec3::new(2.5, 1.2, 0.4), true);
    for (x, z) in [(-48.0f32, -100.0f32), (-50.0, -118.0), (-33.0, -100.0), (-34.0, -121.0), (-44.0, -96.0), (-52.0, -109.0)] {
        b.cedar(x, z, 1.4);
    }
    // River and bridge.
    b.deco(Vec3::new(-300.0, -1.6, -166.0), Vec3::new(300.0, -1.4, -152.0), [0.2, 0.35, 0.45]);
    b.deco(Vec3::new(-300.0, -1.6, -152.0), Vec3::new(300.0, 0.0, -151.0), [0.45, 0.42, 0.36]);
    b.deco(Vec3::new(-300.0, -1.6, -167.0), Vec3::new(300.0, 0.0, -166.0), [0.45, 0.42, 0.36]);
    b.deco(Vec3::new(-3.5, -0.3, -167.0), Vec3::new(3.5, 0.03, -151.0), CONCRETE);
    for x in [-3.5f32, 3.3] {
        b.solid(Vec3::new(x, 0.0, -167.0), Vec3::new(x + 0.2, 1.1, -151.0), [0.75, 0.2, 0.15]);
    }
    // Keep the player off the riverbanks.
    b.colliders.push(Aabb::new(Vec3::new(-300.0, 0.0, -166.0), Vec3::new(-3.5, 2.0, -152.0)));
    b.colliders.push(Aabb::new(Vec3::new(3.5, 0.0, -166.0), Vec3::new(300.0, 2.0, -152.0)));
    // Level crossing.
    b.deco(Vec3::new(-300.0, 0.0, -180.0), Vec3::new(300.0, 0.12, -176.0), [0.4, 0.36, 0.3]);
    for z in [-178.8f32, -177.2] {
        b.deco(Vec3::new(-300.0, 0.12, z - 0.05), Vec3::new(300.0, 0.22, z + 0.05), [0.55, 0.55, 0.58]);
    }
    for (x, z) in [(-3.8f32, -175.0f32), (3.8, -181.0)] {
        b.mb.cylinder(Vec3::new(x, 0.0, z), 0.1, 3.0, [0.95, 0.95, 0.95]);
        for i in 0..4 {
            b.deco(Vec3::new(x - 0.11, 0.4 + i as f32 * 0.6, z - 0.11), Vec3::new(x + 0.11, 0.7 + i as f32 * 0.6, z + 0.11), [0.1, 0.1, 0.1]);
        }
        b.neon_box(Vec3::new(x - 0.3, 2.6, z - 0.15), Vec3::new(x + 0.3, 2.85, z + 0.15), [0.9, 0.15, 0.1]);
    }
    // School approach: sakura and a couple of houses.
    for z in [-186.0f32, -194.0] {
        b.sakura(-6.0, z);
        b.sakura(6.0, z);
    }
    b.house(-14.0, -190.0, 7.0, 8.0, 1.0);
    // School wall at the end of the road.
    b.solid(Vec3::new(-60.0, 0.0, -206.0), Vec3::new(-4.0, 2.0, -205.0), CONCRETE);
    b.solid(Vec3::new(4.0, 0.0, -206.0), Vec3::new(60.0, 2.0, -205.0), CONCRETE);
    // Distant mountains and forest on all sides.
    let mut rng = Rng::derive(3, "mountains");
    for i in 0..40 {
        let a = i as f32 / 40.0 * std::f32::consts::TAU;
        let r = 520.0 + rng.f32() * 220.0;
        let h = 90.0 + rng.f32() * 140.0;
        b.mb.cone(Vec3::new(a.cos() * r, -5.0, a.sin() * r - 100.0), h * 0.9, h, [0.16, 0.26, 0.2]);
    }
    for _ in 0..90 {
        let x = (rng.f32() - 0.5) * 2.0 * 200.0;
        let z = -100.0 + (rng.f32() - 0.5) * 2.0 * 220.0;
        let near_road = x.abs() < 95.0 && z > -150.0 && z < -70.0;
        if x.abs() > 20.0 && !near_road && !(z < -150.0 && z > -168.0) {
            b.cedar(x, z, 1.2 + rng.f32() * 0.8);
        }
    }
    // Invisible walls so you stay on the walkable strip.
    b.colliders.push(Aabb::new(Vec3::new(-100.0, 0.0, 12.0), Vec3::new(100.0, 3.0, 14.0)));

    let exits = vec![
        Exit { area: Aabb::new(Vec3::new(-6.0, 0.0, 2.0), Vec3::new(6.0, 3.0, 6.0)), to: Location::Home, spawn: Vec3::new(0.0, 0.0, -18.0), yaw: PI },
        Exit { area: Aabb::new(Vec3::new(-4.0, 0.0, -210.0), Vec3::new(4.0, 3.0, -203.0)), to: Location::School, spawn: Vec3::new(0.0, 0.0, 38.0), yaw: 0.0 },
    ];
    let anchors = vec![(Spot::Path, Vec3::new(1.2, 0.0, -100.0), 0.0)];
    b.finish(Location::Path, "Path to School", exits, Vec::new(), anchors, 0.0035)
}

/// Kazehana Junior High: gate and sakura avenue, main building with classroom 1-A, gym, card club
/// prefab, sports ground and courtyard. The gate is at z≈40 (south), the building at z≈-12..0.
pub fn build_school() -> Level {
    let mut b = Lb::new("school");
    b.ground(400.0, GRASS);
    let plaster = [0.9, 0.9, 0.86];
    // Perimeter wall with the gate gap at x∈[-4,4].
    b.solid(Vec3::new(-64.0, 0.0, 41.0), Vec3::new(-4.0, 1.8, 42.0), CONCRETE);
    b.solid(Vec3::new(4.0, 0.0, 41.0), Vec3::new(64.0, 1.8, 42.0), CONCRETE);
    b.solid(Vec3::new(-64.0, 0.0, -32.0), Vec3::new(-63.0, 1.8, 42.0), CONCRETE);
    b.solid(Vec3::new(63.0, 0.0, -32.0), Vec3::new(64.0, 1.8, 42.0), CONCRETE);
    b.solid(Vec3::new(-64.0, 0.0, -33.0), Vec3::new(64.0, 1.8, -32.0), CONCRETE);
    for x in [-4.6f32, 4.0] {
        b.solid(Vec3::new(x, 0.0, 40.5), Vec3::new(x + 0.6, 2.4, 42.5), [0.5, 0.5, 0.5]);
    }
    b.deco(Vec3::new(4.05, 1.4, 42.52), Vec3::new(4.55, 2.2, 42.55), [0.85, 0.82, 0.7]); // name plate
    // Avenue from the gate to the entrance, lined with sakura.
    b.deco(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(3.0, 0.03, 42.0), [0.72, 0.7, 0.66]);
    let mut z = 36.0;
    while z > 6.0 {
        b.sakura(-6.0, z);
        b.sakura(6.0, z);
        z -= 7.5;
    }
    // Main building (3 storeys) with classroom 1-A carved out of the east ground floor.
    let (bh, fh) = (12.0, 3.6);
    b.solid(Vec3::new(-30.0, 0.0, -12.0), Vec3::new(13.8, bh, 0.0), plaster);
    b.solid(Vec3::new(26.2, 0.0, -12.0), Vec3::new(30.0, bh, 0.0), plaster);
    b.solid(Vec3::new(13.8, fh, -12.0), Vec3::new(26.2, bh, 0.0), plaster);
    b.solid(Vec3::new(13.8, 0.0, -12.0), Vec3::new(26.2, fh, -10.0), plaster);
    // Classroom south wall: door at x∈[14.5,16], windows above 0.9m.
    b.solid(Vec3::new(13.8, 0.0, -0.2), Vec3::new(14.5, fh, 0.0), plaster);
    b.solid(Vec3::new(16.0, 0.0, -0.2), Vec3::new(26.2, 0.9, 0.0), plaster);
    b.deco(Vec3::new(16.0, 2.6, -0.2), Vec3::new(26.2, fh, 0.0), plaster);
    for x in [18.5f32, 21.0, 23.5] {
        b.deco(Vec3::new(x, 0.9, -0.2), Vec3::new(x + 0.15, 2.6, 0.0), [0.6, 0.6, 0.62]);
    }
    b.deco(Vec3::new(14.5, 2.2, -0.2), Vec3::new(16.0, fh, 0.0), plaster);
    // Classroom interior: floor, blackboard, teacher's desk, 5×6 desks.
    b.deco(Vec3::new(13.8, 0.0, -10.0), Vec3::new(26.2, 0.03, 0.0), [0.6, 0.45, 0.3]);
    b.deco(Vec3::new(25.9, 0.9, -8.5), Vec3::new(26.15, 2.3, -1.5), [0.12, 0.3, 0.2]);
    b.solid(Vec3::new(24.0, 0.0, -6.0), Vec3::new(25.0, 0.8, -4.0), WOOD);
    let mut desks = Vec::new();
    for r in 0..5 {
        for c in 0..6 {
            let x = 15.5 + r as f32 * 1.6;
            let z = -8.6 + c as f32 * 1.35;
            b.deco(Vec3::new(x - 0.3, 0.7, z - 0.3), Vec3::new(x + 0.3, 0.75, z + 0.3), [0.75, 0.6, 0.42]);
            b.deco(Vec3::new(x - 0.25, 0.0, z - 0.25), Vec3::new(x + 0.25, 0.7, z + 0.25), [0.45, 0.45, 0.48]);
            desks.push(Vec3::new(x - 0.5, 0.0, z));
        }
    }
    for (x, z) in [(17.0f32, -5.0f32), (22.0, -5.0)] {
        b.neon_box(Vec3::new(x - 1.0, fh - 0.08, z - 0.08), Vec3::new(x + 1.0, fh - 0.02, z + 0.08), [0.95, 0.98, 1.0]);
        b.light(Vec3::new(x, fh - 0.3, z), 8.0, Vec3::new(2.4, 2.5, 2.6), false);
    }
    // Facade windows on every floor (glow at night when the building is lit).
    for f in 0..3 {
        let y = 1.0 + f as f32 * fh;
        let mut x = -28.5;
        while x < 28.0 {
            let in_classroom = f == 0 && x > 13.5 && x < 26.2;
            if !in_classroom {
                b.glow_box(Vec3::new(x, y, 0.0), Vec3::new(x + 2.2, y + 1.6, 0.05), [0.75, 0.85, 0.95]);
            }
            x += 3.0;
        }
        b.deco(Vec3::new(-30.2, y - 0.25, 0.0), Vec3::new(30.2, y - 0.1, 0.6), [0.8, 0.8, 0.78]);
    }
    // Entrance canopy, clock, roof fence.
    b.deco(Vec3::new(-4.0, 3.0, 0.0), Vec3::new(4.0, 3.2, 3.0), CONCRETE);
    b.mb.cylinder_m(Mat4::from_scale_rotation_translation(Vec3::new(1.6, 0.15, 1.6), Quat::from_rotation_x(PI / 2.0), Vec3::new(0.0, 10.5, 0.15)), [0.95, 0.95, 0.95]);
    for x in [-30.0f32, 30.0] {
        b.deco(Vec3::new(x - 0.05, bh, -12.0), Vec3::new(x + 0.05, bh + 1.4, 0.0), [0.4, 0.45, 0.4]);
    }
    b.deco(Vec3::new(-30.0, bh, -0.05), Vec3::new(30.0, bh + 1.4, 0.05), [0.4, 0.45, 0.4]);
    b.deco(Vec3::new(-30.0, bh, -12.05), Vec3::new(30.0, bh + 1.4, -11.95), [0.4, 0.45, 0.4]);
    // Gym.
    b.solid(Vec3::new(34.0, 0.0, -24.0), Vec3::new(56.0, 9.0, -2.0), [0.82, 0.8, 0.74]);
    b.deco(Vec3::new(33.5, 9.0, -24.5), Vec3::new(56.5, 9.6, -1.5), [0.3, 0.42, 0.55]);
    for x in [36.0f32, 41.0, 46.0, 51.0] {
        b.glow_box(Vec3::new(x, 5.5, -1.98), Vec3::new(x + 3.0, 7.5, -1.94), [0.8, 0.85, 0.9]);
    }
    // Card Club prefab.
    b.solid(Vec3::new(34.0, 0.0, 8.0), Vec3::new(46.0, 3.0, 15.0), [0.78, 0.82, 0.86]);
    b.glow_box(Vec3::new(35.0, 1.0, 7.98), Vec3::new(45.0, 2.2, 8.02), WINDOW_WARM);
    b.light(Vec3::new(40.0, 2.0, 6.5), 7.0, Vec3::new(2.0, 1.7, 1.2), true);
    // Sports ground with a track.
    b.deco(Vec3::new(-60.0, 0.0, -28.0), Vec3::new(-34.0, 0.03, 36.0), [0.72, 0.6, 0.45]);
    for (z0, z1) in [(-24.0f32, -23.6f32), (31.6, 32.0)] {
        b.deco(Vec3::new(-56.0, 0.03, z0), Vec3::new(-38.0, 0.04, z1), WHITE_LINE);
    }
    for x in [-56.0f32, -38.4] {
        b.deco(Vec3::new(x, 0.03, -24.0), Vec3::new(x + 0.4, 0.04, 32.0), WHITE_LINE);
    }
    for z in [-20.0f32, 28.0] {
        // Soccer goals.
        for x in [-49.5f32, -44.5] {
            b.mb.cylinder(Vec3::new(x, 0.0, z), 0.06, 2.4, [0.95, 0.95, 0.95]);
        }
        b.deco(Vec3::new(-49.5, 2.35, z - 0.06), Vec3::new(-44.5, 2.45, z + 0.06), [0.95, 0.95, 0.95]);
    }
    // Courtyard: a big tree and benches.
    b.tree(12.0, 16.0, 1.6);
    for z in [12.0f32, 20.0] {
        b.solid(Vec3::new(9.0, 0.0, z), Vec3::new(11.0, 0.45, z + 0.5), WOOD);
    }
    for (x, z) in [(-20.0f32, 20.0f32), (-25.0, 10.0), (20.0, 30.0), (50.0, 30.0), (-15.0, 35.0)] {
        b.tree(x, z, 1.2);
    }
    // Lamps along the avenue for evenings.
    for z in [10.0f32, 25.0] {
        b.street_lamp(3.6, z, -1.0);
    }
    // Mountains.
    let mut rng = Rng::derive(4, "mountains");
    for i in 0..36 {
        let a = i as f32 / 36.0 * std::f32::consts::TAU;
        let r = 480.0 + rng.f32() * 220.0;
        let h = 90.0 + rng.f32() * 130.0;
        b.mb.cone(Vec3::new(a.cos() * r, -5.0, a.sin() * r), h * 0.9, h, [0.16, 0.26, 0.2]);
    }

    let exits = vec![Exit {
        area: Aabb::new(Vec3::new(-4.0, 0.0, 42.6), Vec3::new(4.0, 3.0, 50.0)),
        to: Location::Path,
        spawn: Vec3::new(0.0, 0.0, -198.0),
        yaw: PI,
    }];
    let zones = vec![(Aabb::new(Vec3::new(13.8, 0.0, -10.0), Vec3::new(26.2, 3.0, -0.2)), Interact::Classroom)];
    let mut anchors: Vec<(Spot, Vec3, f32)> = desks.into_iter().map(|p| (Spot::Classroom, p, PI / 2.0)).collect();
    anchors.extend([
        (Spot::Corridor, Vec3::new(0.0, 0.0, 2.0), PI),
        (Spot::Library, Vec3::new(-20.0, 0.0, 2.5), PI),
        (Spot::Courtyard, Vec3::new(10.0, 0.0, 14.5), PI / 2.0),
        (Spot::Roof, Vec3::new(-10.0, bh, -4.0), PI),
        (Spot::Gym, Vec3::new(45.0, 0.0, 1.0), PI),
        (Spot::CardClub, Vec3::new(40.0, 0.0, 6.0), 0.0),
        (Spot::SportsGround, Vec3::new(-47.0, 0.0, 4.0), 0.0),
    ]);
    b.finish(Location::School, "Kazehana Junior High", exits, zones, anchors, 0.003)
}
