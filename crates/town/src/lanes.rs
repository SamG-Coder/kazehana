//! The pool's lane ropes, afloat: separate floats strung on a white rope, each riding the water
//! where it is. Every frame each float rises, sinks and tilts with the surface under it (the same
//! ripples the renderer draws: akari shaders/pt.rgen `waterWaves`, on the same clock), bobbing a
//! little on its own as a light float does, and the rope between two floats sags into the water.
//!
//! The pool's glTF carries one float of each colour (`kzfloat_red`, `kzfloat_white`,
//! `kzfloat_blue`: along +X, centred on the origin) and a metre of rope (`kzrope`: +X from 0 to
//! 1); `lanes.json` beside it says where the ropes run (tools/blender/build_school_pool.py).

use akari::glam::{Affine3A, Quat, Vec3};
use akari::{Instance, MeshId, Model};
use serde::Deserialize;

#[derive(Deserialize)]
struct RopeJ {
    /// across the pool (Blender y, m)
    y: f32,
    /// its ends on the end walls (Blender x, m)
    x0: f32,
    x1: f32,
}

#[derive(Deserialize)]
struct LanesJ {
    /// the water's surface (Blender z, m)
    water_z: f32,
    /// float to float (m), a float's length (m)
    pitch: f32,
    float_len: f32,
    /// red floats for this far from each end (m); between, blue and white in groups of `group`
    red_m: f32,
    group: usize,
    ropes: Vec<RopeJ>,
}

struct Float {
    /// where it rests on still water, in the world
    rest: Vec3,
    mesh: usize,
    phase: f32,
}

struct Rope {
    /// the hooks on the end walls (world)
    ends: [Vec3; 2],
    floats: Vec<Float>,
}

pub struct LaneRopes {
    /// red, white, blue
    floats: [(MeshId, Affine3A); 3],
    rope: (MeshId, Affine3A),
    /// the pool's turn in the world, and its frame's +X (along the lanes) in the world
    turn: Quat,
    along: Vec3,
    float_len: f32,
    ropes: Vec<Rope>,
}

/// The ripples' height at world (x, z), time t, and its gradient: as the renderer's water
/// shader has them (akari shaders/pt.rgen `waterWaves`; keep the two in step).
pub fn water_waves(x: f32, z: f32, t: f32) -> (f32, [f32; 2]) {
    const W: [(f32, f32, f32); 10] = [
        (20.0, 2.3, 0.006), (-35.0, 1.4, 0.0045), (75.0, 0.93, 0.0032), (160.0, 0.61, 0.0022), (-110.0, 0.43, 0.0015),
        (5.0, 0.29, 0.0009), (65.0, 0.23, 0.0004), (125.0, 0.18, 0.0003), (-170.0, 0.14, 0.0002), (-55.0, 0.11, 0.00012),
    ];
    let (mut h, mut g) = (0.0, [0.0, 0.0]);
    for (i, (deg, len, amp)) in W.iter().enumerate() {
        let a = deg.to_radians();
        let d = [a.cos(), a.sin()];
        let k = std::f32::consts::TAU / len;
        let ph = k * (d[0] * x + d[1] * z) - (9.81 * k).sqrt() * t + i as f32 * 1.7;
        h += amp * ph.sin();
        g[0] += amp * k * ph.cos() * d[0];
        g[1] += amp * k * ph.cos() * d[1];
    }
    (h, g)
}

impl LaneRopes {
    /// The ropes of the pool `model` placed at `at`, from `lanes` (its lanes.json). None when the
    /// model carries no floats.
    pub fn new(model: &Model, at: Affine3A, lanes: &[u8]) -> anyhow::Result<Option<Self>> {
        let part = |name: &str| model.parts.iter().find(|p| p.node.starts_with(name)).map(|p| (p.mesh, p.transform));
        let (Some(red), Some(white), Some(blue), Some(rope)) = (part("kzfloat_red"), part("kzfloat_white"), part("kzfloat_blue"), part("kzrope")) else {
            return Ok(None);
        };
        let j: LanesJ = serde_json::from_slice(lanes)?;
        let (_, turn, _) = at.to_scale_rotation_translation();
        // (Blender x, y, z -> the model's glTF x, z, -y)
        let world = |x: f32, y: f32, z: f32| at.transform_point3(Vec3::new(x, z, -y));
        let mut ropes = Vec::new();
        for (ri, r) in j.ropes.iter().enumerate() {
            let n = ((r.x1 - r.x0) / j.pitch).floor() as usize;
            let start = r.x0 + (r.x1 - r.x0 - n as f32 * j.pitch) / 2.0;
            let floats = (0..n)
                .map(|i| {
                    let x = start + (i as f32 + 0.5) * j.pitch;
                    let from_end = (x - r.x0).min(r.x1 - x);
                    let mesh = if from_end < j.red_m { 0 } else if (i / j.group.max(1)) % 2 == 0 { 2 } else { 1 };
                    // (each float's own bob: a phase from its place on the rope)
                    let phase = ((ri * 977 + i * 131) % 1000) as f32 * 0.00628;
                    Float { rest: world(x, r.y, j.water_z), mesh, phase }
                })
                .collect();
            ropes.push(Rope { ends: [world(r.x0, r.y, j.water_z + 0.005), world(r.x1, r.y, j.water_z + 0.005)], floats });
        }
        Ok(Some(Self { floats: [red, white, blue], rope, turn, along: at.transform_vector3(Vec3::X).normalize(), float_len: j.float_len, ropes }))
    }

    /// This moment's floats and rope (`t`: the renderer's animation clock, s) into `out`.
    pub fn instances(&self, t: f32, out: &mut Vec<Instance>) {
        let rope_seg = |a: Vec3, b: Vec3, out: &mut Vec<Instance>| {
            let d = b - a;
            let len = d.length();
            if len < 1e-4 {
                return;
            }
            let rot = Quat::from_rotation_arc(Vec3::X, d / len);
            out.push(Instance { mesh: self.rope.0, transform: Affine3A::from_scale_rotation_translation(Vec3::new(len, 1.0, 1.0), rot, a) * self.rope.1 });
        };
        for r in &self.ropes {
            let mut prev = r.ends[0];
            for f in &r.floats {
                let (h, g) = water_waves(f.rest.x, f.rest.z, t);
                // a float rides the surface, a little proud of it, bobbing on its own a few mm;
                // it leans with the water's slope (shown a few times over: the ripples are small)
                let bob = 0.004 * (1.6 * t + f.phase).sin() + 0.002 * (2.7 * t + 2.0 * f.phase).sin();
                let c = f.rest + Vec3::Y * (h * 1.4 + bob + 0.01);
                let n = Vec3::new(-g[0] * 4.0, 1.0, -g[1] * 4.0).normalize();
                let lean = Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_axis_angle(self.along, 0.05 * (1.3 * t + 3.0 * f.phase).sin());
                let (mesh, local) = self.floats[f.mesh];
                out.push(Instance { mesh, transform: Affine3A::from_rotation_translation(lean * self.turn, c) * local });
                // the rope from the last float (or the wall) to this one, sagging into the water
                let axis = lean * self.along;
                let (a, b) = (prev, c - axis * (self.float_len * 0.5));
                let mid = (a + b) * 0.5;
                let (mh, _) = water_waves(mid.x, mid.z, t);
                let sag = Vec3::new(mid.x, f.rest.y + mh - 0.008, mid.z);
                rope_seg(a, sag, out);
                rope_seg(sag, b, out);
                prev = c + axis * (self.float_len * 0.5);
            }
            rope_seg(prev, r.ends[1], out);
        }
    }
}

/// Is this node one of the floats or the rope (drawn by LaneRopes, not where the model lies)?
pub fn is_lane_part(node: &str) -> bool {
    node.starts_with("kzfloat_") || node.starts_with("kzrope")
}
