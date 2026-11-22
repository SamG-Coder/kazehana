//! The town's cloth: every `kzcloth_*` part of the models put down (the noren in Tsukumodō's
//! doorway) simulated (play::cloth) and drawn as a deformable mesh (akari).
//!
//! Each piece is made from its part's mesh placed in the world (the loader keeps a CPU copy of
//! cloth parts: `akari::Part::cloth`). The top `PIN_BAND` of it is pinned: the sleeve round its
//! rod. It keeps out of the walls round it (the static collision triangles near it, from
//! play::World) and out of the people given each frame (the player's capsule), and it stirs in
//! the house's slow draughts. Cloth far from the camera is left as it is.

use crate::collide::is_cloth;
use crate::plots::PlotsScene;
use akari::glam::{Affine3A, Vec3};
use akari::{Instance, MeshData, MeshId, Renderer};
use anyhow::Result;
use play::cloth::{Air, Cloth, Collider, Settings};

/// The band at the top of a cloth part that stays on its rod (m): its vertices are pinned.
/// (tools/blender/noren_layout.py PIN_BAND)
pub const PIN_BAND: f32 = 0.042;
/// Cloth farther than this from the camera is left still (m).
const AWAKE: f32 = 15.0;
/// A sleeve is gathered a little on its rod (to this fraction of the cloth's width), in small
/// pleats (±PLEAT m front and back, every other column): a sleeve is never quite flat. (The
/// cloth's own folds are in its rest shape: tools/blender/noren_mesh.py.)
const GATHER: f32 = 0.97;
const PLEAT: f32 = 0.002;
const PLEAT_EVERY: f32 = 0.0256;
/// Seconds simulated at load, so the cloth starts hanging in its folds.
const SETTLE: f32 = 3.0;

pub struct ClothPiece {
    /// the part's node name ("kzcloth_noren")
    pub name: String,
    pub sim: Cloth,
    mesh: MeshId,
    indices: Vec<u32>,
    uvs: Vec<[f32; 2]>,
    tangents: bool,
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    tan: Vec<[f32; 4]>,
    /// the middle of its rest shape, and the way its front faces (horizontal, unit)
    pub centre: Vec3,
    pub facing: Vec3,
    /// how tall it hangs at rest (m): its bottom and top
    pub bottom: f32,
    pub top: f32,
    pub air: Air,
    /// moved since its vertices were last handed to the renderer
    dirty: bool,
    /// its particles hanging at rest (settled at load): where `reset` puts them
    settled: Vec<Vec3>,
}

#[derive(Default)]
pub struct Cloths {
    pub pieces: Vec<ClothPiece>,
}

/// A walking person as the cloth meets them: their capsule, moving with them.
pub fn person(fp: &play::FirstPerson) -> Collider {
    let (a, b, r) = fp.capsule();
    Collider::Capsule { a, b, r, vel: fp.vel }
}

impl Cloths {
    /// The cloth parts of everything the plots put down, made ready to simulate and draw.
    /// `world`: the solid shapes (for the walls round each piece).
    pub fn build(r: &mut Renderer, plots: &PlotsScene, world: &play::World) -> Result<Self> {
        let mut pieces = Vec::new();
        for (name, at) in &plots.placed {
            let Some(model) = plots.models.iter().find(|m| &m.name == name) else { continue };
            for part in model.parts.iter().filter(|p| is_cloth(&p.node)) {
                let Some(src) = &part.cloth else { continue };
                let m: Affine3A = *at * part.transform;
                pieces.push(ClothPiece::new(r, &part.node, src, m, world)?);
            }
        }
        if !pieces.is_empty() {
            let n: usize = pieces.iter().map(|p| p.sim.particle_count()).sum();
            println!("cloth: {} pieces, {n} particles", pieces.len());
        }
        Ok(Cloths { pieces })
    }

    /// Advance the cloth near the camera by `dt`, with these people in it. Returns how many
    /// pieces moved.
    pub fn step(&mut self, dt: f32, camera: Vec3, people: &[Collider]) -> usize {
        let mut n = 0;
        for p in &mut self.pieces {
            if (p.centre - camera).length() > AWAKE {
                continue;
            }
            if p.sim.step(dt, &p.air, people) > 0 {
                p.dirty = true;
                n += 1;
            }
        }
        n
    }

    /// Hand the moved pieces' vertices to the renderer (once a frame, before it renders).
    pub fn upload(&mut self, r: &mut Renderer) -> Result<()> {
        for p in self.pieces.iter_mut().filter(|p| p.dirty) {
            p.sim.vertex_positions(&mut p.pos);
            p.sim.vertex_normals(&mut p.nrm);
            if p.tangents {
                p.sim.vertex_tangents(&p.indices, &p.uvs, &p.nrm, &mut p.tan);
            }
            r.update_mesh(p.mesh, &p.pos, &p.nrm, &p.tan)?;
            p.dirty = false;
        }
        Ok(())
    }

    /// Back to hanging at rest, all of it (after a cut).
    pub fn reset(&mut self) {
        for p in &mut self.pieces {
            p.sim.set_positions(&p.settled);
            p.dirty = true;
        }
    }

    /// The cloth to draw (its vertices are in the world already).
    pub fn instances(&self, out: &mut Vec<Instance>) {
        out.extend(self.pieces.iter().map(|p| Instance { mesh: p.mesh, transform: Affine3A::IDENTITY }));
    }
}

impl ClothPiece {
    fn new(r: &mut Renderer, node: &str, src: &MeshData, m: Affine3A, world: &play::World) -> Result<Self> {
        let nm = m.matrix3.inverse().transpose();
        let mut md = src.clone();
        md.name = format!("cloth:{node}");
        md.positions = src.positions.iter().map(|p| m.transform_point3(Vec3::from(*p)).to_array()).collect();
        md.normals = src.normals.iter().map(|n| (nm * Vec3::from(*n)).normalize_or_zero().to_array()).collect();
        md.tangents = src.tangents.iter().map(|t| {
            let v = m.matrix3 * Vec3::new(t[0], t[1], t[2]);
            let v = v.normalize_or_zero();
            [v.x, v.y, v.z, t[3]]
        }).collect();
        let world_pos: Vec<Vec3> = md.positions.iter().map(|p| Vec3::from(*p)).collect();
        let top = world_pos.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let bottom = world_pos.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let mut sim = Cloth::new(&world_pos, &md.indices, |_, v| v.y >= top - PIN_BAND, Settings::default());
        let (lo, hi) = sim.rest_bounds();
        // the way it faces: its normals below the band, flattened
        let f = md.normals.iter().zip(&world_pos).filter(|(_, p)| p.y < top - PIN_BAND).fold(Vec3::ZERO, |a, (n, _)| a + Vec3::from(*n));
        let facing = Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::Z);
        let along = Vec3::Y.cross(facing).normalize_or(Vec3::X);
        // gather each piece's sleeve towards its own middle, in pleats
        let pieces = pieces_along(&sim, along);
        let rest: Vec<Vec3> = sim.rest().to_vec();
        sim.move_pins(|i, p| {
            let (mid, w0) = pieces[i];
            let a = (rest[i] - mid).dot(along);
            let col = ((a - w0) / PLEAT_EVERY).round() as i32;
            p + along * (a * (GATHER - 1.0)) + facing * if col % 2 == 0 { PLEAT } else { -PLEAT }
        });
        let mut solid = Vec::new();
        world.triangles_near(lo - Vec3::splat(0.4), hi + Vec3::splat(0.4), &mut solid);
        sim.set_solid(solid);
        // let it hang a while: its weight and its folds
        let still = Air { mean: Vec3::ZERO, gust: 0.0, gust_rate: 0.0, turbulence: 0.0 };
        let h = 1.0 / sim.settings.rate;
        for _ in 0..(SETTLE / h) as u32 {
            sim.fixed_step(h, &still, &[]);
        }
        let settled = sim.pos.clone();
        sim.vertex_positions(&mut md.positions);
        sim.vertex_normals(&mut md.normals);
        if !md.tangents.is_empty() {
            let mut t = Vec::new();
            sim.vertex_tangents(&md.indices, &md.uvs, &md.normals, &mut t);
            md.tangents = t;
        }
        let mesh = r.add_deformable_mesh(&md)?;
        println!("cloth {node}: {} particles ({} pinned), {} wall triangles near", sim.particle_count(), sim.pinned_count(), sim.solid_count());
        Ok(ClothPiece {
            name: node.to_string(),
            sim,
            mesh,
            tangents: !md.tangents.is_empty(),
            indices: md.indices,
            uvs: md.uvs,
            pos: Vec::new(),
            nrm: Vec::new(),
            tan: Vec::new(),
            centre: (lo + hi) * 0.5,
            facing,
            bottom,
            top,
            air: Air::indoors(),
            dirty: false,
            settled,
        })
    }
}

/// For each particle: the middle of its piece (connected part) along `along`, and that piece's
/// first column's offset from it (pleats are counted from there).
fn pieces_along(sim: &Cloth, along: Vec3) -> Vec<(Vec3, f32)> {
    // pieces: particles grouped by flood fill over closeness at rest would need the topology;
    // the rest box of the particles on either side of each gap along the rod does the same for
    // panels side by side
    let rest = sim.rest();
    let mut a: Vec<(f32, usize)> = rest.iter().enumerate().map(|(i, p)| (p.dot(along), i)).collect();
    a.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = vec![(Vec3::ZERO, 0.0); rest.len()];
    let mut start = 0;
    for k in 1..=a.len() {
        if k == a.len() || a[k].0 - a[k - 1].0 > 0.02 {
            let (lo, hi) = (a[start].0, a[k - 1].0);
            let ids: Vec<usize> = a[start..k].iter().map(|x| x.1).collect();
            let c = ids.iter().fold(Vec3::ZERO, |s, &i| s + rest[i]) / ids.len() as f32;
            let mid = c + along * ((lo + hi) * 0.5 - c.dot(along));
            for &i in &ids {
                out[i] = (mid, lo - (lo + hi) * 0.5);
            }
            start = k;
        }
    }
    out
}
