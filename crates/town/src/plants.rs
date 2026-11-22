//! Real plants: Quixel Megaplant trees and Megascans grass patches, from SmartImposter's species
//! packs (`.mtree` + `.json`, written by its tools/megaplant_to_mtree.py and grass_to_mtree.py).
//!
//! An `.mtree` holds a species' unique meshes (each triangle tagged bark or leaf) and, per variant,
//! the parts that instance those meshes: a tree is hundreds to thousands of instances of a few
//! meshes. Each mesh is split into a bark mesh and a leaf mesh here (Akari has one material per
//! mesh); leaves are two-sided, translucent foliage, alpha-tested for grass cards.
//!
//! `Vegetation::instances` lists the plants near a point as real geometry (the far forest is
//! drawn by impostors).

use crate::plan::Plan;
use crate::HeightField;
use akari::glam::{Affine3A, Quat, Vec3, Vec3A};
use akari::scene::{MAT_ALPHA_MASK, MAT_DOUBLE_SIDED, MAT_FOLIAGE, MAT_RECOLOR};
use akari::textures::{TexJob, TexKind, TexSource};
use akari::{ImpostorVariant, Instance, Material, MeshData, MeshId, PlantPlacement, Renderer};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// SmartImposter's species packs.
pub const SPECIES_DIR: &str = "D:/SmartImposter/assets/species";

pub struct Variant {
    pub name: String,
    pub height: f32,
    pub parts: Vec<(MeshId, Affine3A)>,
    /// how far the plant reaches sideways from its root (m), at scale 1
    pub reach: f32,
}

pub struct Species {
    pub name: String,
    pub grass: bool,
    pub variants: Vec<Variant>,
    /// leaf transmission (for the impostor)
    pub transmission: f32,
    /// a tag for the impostor cache (the leaf tint changes the bake)
    pub tag: String,
}

struct Mtree {
    meshes: Vec<(String, Vec<f32>, Vec<f32>, Vec<f32>, Vec<u32>, Vec<u8>)>,
    trees: Vec<(String, [f32; 6], Vec<(u32, Affine3A)>)>,
}

/// Little-endian 4-byte values from a byte slice (f32 or u32).
fn pod_vec<T: bytemuck::Pod>(b: &[u8]) -> Vec<T> {
    let mut v: Vec<T> = vec![T::zeroed(); b.len() / 4];
    let n = v.len() * 4;
    bytemuck::cast_slice_mut::<T, u8>(&mut v).copy_from_slice(&b[..n]);
    v
}

fn read_mtree(path: &Path) -> Result<Mtree> {
    let d = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut p = 0usize;
    let mut take = |n: usize| -> Result<&[u8]> {
        if p + n > d.len() {
            bail!("{}: truncated", path.display());
        }
        p += n;
        Ok(&d[p - n..p])
    };
    if take(4)? != b"MTRE" {
        bail!("{}: not an .mtree", path.display());
    }
    let u32_ = |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap());
    let ver = u32_(take(4)?);
    let nm = u32_(take(4)?) as usize;
    let mut meshes = Vec::with_capacity(nm);
    for _ in 0..nm {
        let l = u32_(take(4)?) as usize;
        let name = String::from_utf8_lossy(take(l)?).into_owned();
        let v = u32_(take(4)?) as usize;
        let t = u32_(take(4)?) as usize;
        let pos: Vec<f32> = pod_vec(take(v * 12)?);
        let nrm: Vec<f32> = pod_vec(take(v * 12)?);
        let uv: Vec<f32> = if ver >= 2 { pod_vec(take(v * 8)?) } else { vec![0.0; v * 2] };
        let idx: Vec<u32> = pod_vec(take(t * 12)?);
        let mat = take(t.div_ceil(4) * 4)?[..t].to_vec();
        meshes.push((name, pos, nrm, uv, idx, mat));
    }
    let nt = u32_(take(4)?) as usize;
    let mut trees = Vec::with_capacity(nt);
    for _ in 0..nt {
        let l = u32_(take(4)?) as usize;
        let name = String::from_utf8_lossy(take(l)?).into_owned();
        let b: Vec<f32> = pod_vec(take(24)?);
        let n = u32_(take(4)?) as usize;
        let mut parts = Vec::with_capacity(n);
        for _ in 0..n {
            let mesh = u32_(take(4)?);
            let m: Vec<f32> = pod_vec(take(48)?);
            let x = Affine3A::from_cols(Vec3A::new(m[0], m[4], m[8]), Vec3A::new(m[1], m[5], m[9]), Vec3A::new(m[2], m[6], m[10]), Vec3A::new(m[3], m[7], m[11]));
            parts.push((mesh, x));
        }
        trees.push((name, [b[0], b[1], b[2], b[3], b[4], b[5]], parts));
    }
    Ok(Mtree { meshes, trees })
}

/// Unreal's tint convention (0.5 = neutral), as SmartImposter's tri.rchit ueTint.
fn ue_tint(t: [f32; 3]) -> [f32; 3] {
    t.map(|c| 1.0 + ((c * 2.0).clamp(0.0, 2.0) - 1.0) * 0.6)
}

/// Load one species pack into the renderer. `leaf_tint`: extra multiply on the leaves (cherry
/// blossom, autumn), 1 = as scanned.
pub fn load_species(r: &mut Renderer, name: &str, leaf_tint: [f32; 3]) -> Result<Species> {
    let dir = Path::new(SPECIES_DIR);
    let j: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json")))?)?;
    let m = read_mtree(&dir.join(format!("{name}.mtree")))?;
    let grass = j["kind"].as_str() == Some("grass") || j["alpha"].as_bool() == Some(true);
    let tex = |k: &str| j["textures"][k].as_str().map(|s| dir.join(s.replace('\\', "/")));
    let f3 = |k: &str| -> [f32; 3] {
        let a = j[k].as_array();
        a.map_or([0.5; 3], |a| [0, 1, 2].map(|i| a.get(i).and_then(|v| v.as_f64()).unwrap_or(0.5) as f32))
    };
    // textures: foliage colour + alpha, foliage normal (+ translucency), bark colour, bark normal
    let paths: [Option<PathBuf>; 4] = [tex("foliage_ca"), tex("foliage_nt"), tex("bark_c"), tex("bark_nah")];
    let kinds = [TexKind::Color, TexKind::Normal, TexKind::Color, TexKind::Normal];
    let mut jobs = Vec::new();
    let mut slot = [None; 4];
    for (i, p) in paths.iter().enumerate() {
        if let Some(p) = p.as_ref().filter(|p| p.exists()) {
            slot[i] = Some(jobs.len());
            jobs.push(TexJob { key: akari::textures::file_key(p), name: format!("{name} {i}"), kind: kinds[i], source: TexSource::File(p.clone()) });
        }
    }
    let ids = r.add_textures(&jobs)?;
    let t = |i: usize| slot[i].map(|k| ids[k]);
    let lt = ue_tint(f3("leaf_tint"));
    let bt = ue_tint(f3("bark_tint"));
    let tr = f3("translucency_tint");
    let transmission = ((tr[0] + tr[1] + tr[2]) / 3.0 * 0.9).clamp(0.15, 0.5);
    let leaf = r.add_material(Material {
        base_color: if leaf_tint == BLOSSOM { [leaf_tint[0], leaf_tint[1], leaf_tint[2], 1.0] } else { [lt[0] * leaf_tint[0], lt[1] * leaf_tint[1], lt[2] * leaf_tint[2], 1.0] },
        roughness: 0.55,
        base_tex: t(0),
        normal_tex: t(1),
        alpha_cutoff: 0.5,
        flags: MAT_FOLIAGE | MAT_DOUBLE_SIDED | if grass { MAT_ALPHA_MASK } else { 0 } | if leaf_tint == BLOSSOM { MAT_RECOLOR } else { 0 },
        transmission,
        ..Default::default()
    });
    let bark = r.add_material(Material { base_color: [bt[0], bt[1], bt[2], 1.0], roughness: 0.85, base_tex: t(2), normal_tex: t(3), ..Default::default() });

    // split every mesh into its bark and leaf triangles
    let mut datas = Vec::new();
    let mut map: Vec<[Option<usize>; 2]> = Vec::new();
    for (mname, pos, nrm, uv, idx, mat) in &m.meshes {
        let mut out = [None, None];
        for (side, want) in [(0usize, 0u8), (1, 1)] {
            let tris: Vec<usize> = (0..idx.len() / 3).filter(|&k| (mat[k] & 3) == want).collect();
            if tris.is_empty() {
                continue;
            }
            let mut remap = vec![u32::MAX; pos.len() / 3];
            let mut md = MeshData { name: format!("{name} {mname} {}", if side == 0 { "bark" } else { "leaf" }), material: Some(if side == 0 { bark } else { leaf }), ..Default::default() };
            for k in tris {
                for c in 0..3 {
                    let v = idx[k * 3 + c] as usize;
                    if remap[v] == u32::MAX {
                        remap[v] = md.positions.len() as u32;
                        md.positions.push([pos[v * 3], pos[v * 3 + 1], pos[v * 3 + 2]]);
                        md.normals.push([nrm[v * 3], nrm[v * 3 + 1], nrm[v * 3 + 2]]);
                        md.uvs.push([uv[v * 2], uv[v * 2 + 1]]);
                    }
                    md.indices.push(remap[v]);
                }
            }
            md.generate_tangents();
            // Unreal (DirectX) normal maps: green points down; flip the bitangent
            for tg in md.tangents.iter_mut() {
                tg[3] = -tg[3];
            }
            out[side] = Some(datas.len());
            datas.push(md);
        }
        map.push(out);
    }
    // each split mesh's box, for the variants' reach
    let boxes: Vec<(Vec3, Vec3)> = datas
        .iter()
        .map(|d| d.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))))
        .collect();
    let ids = r.add_meshes(&datas)?;
    let variants = m
        .trees
        .into_iter()
        .map(|(vname, b, parts)| {
            let mut reach = 0.0f32;
            for (mi, x) in &parts {
                for &k in map[*mi as usize].iter().flatten() {
                    let (lo, hi) = boxes[k];
                    for c in 0..8 {
                        let q = x.transform_point3(Vec3::new(if c & 1 == 0 { lo.x } else { hi.x }, if c & 2 == 0 { lo.y } else { hi.y }, if c & 4 == 0 { lo.z } else { hi.z }));
                        reach = reach.max(q.x.hypot(q.z));
                    }
                }
            }
            Variant {
                name: vname,
                height: b[4] - b[1],
                parts: parts.iter().flat_map(|(mi, x)| map[*mi as usize].iter().flatten().map(|&k| (ids[k], *x)).collect::<Vec<_>>()).collect(),
                reach,
            }
        })
        .collect();
    let tag = format!("{name}_{:02x}{:02x}{:02x}", (leaf_tint[0] * 50.0) as u8, (leaf_tint[1] * 50.0) as u8, (leaf_tint[2] * 50.0) as u8);
    Ok(Species { name: name.into(), grass, variants, transmission, tag })
}

/// Cherry and plum blossom: pale pink (leaves recoloured, MAT_RECOLOR).
const BLOSSOM: [f32; 3] = [0.92, 0.74, 0.80];

/// The town's species (tools/blender/town_plan.py) as Megaplants: the nearest match available,
/// with a leaf tint for the ones that are in flower in April.
fn stand_in(species: &str) -> (&'static str, [f32; 3]) {
    match species {
        "sugi" | "hinoki" => ("Tree_Norway_Spruce", [0.85, 0.9, 0.8]),
        "konara" | "kunugi" | "kuri" | "keyaki" => ("Tree_English_Oak", [1.0, 1.0, 1.0]),
        "shii" | "kashi" | "kusu" | "tsubaki" => ("Tree_European_Beech", [0.75, 0.85, 0.7]),
        "momiji" => ("Tree_Norway_Maple", [1.0, 1.0, 1.0]),
        // cherries in blossom in early April: pale pink crowns
        "yamazakura" | "someiyoshino" => ("Tree_Norway_Maple", BLOSSOM),
        "bamboo" => ("Tree_Giant_Bamboo", [1.0, 1.0, 1.0]),
        "ume" => ("Tree_Rowan", BLOSSOM),
        // clipped garden hedges along plot fronts
        "hedge" => ("Shrub_European_Spindle", [0.85, 0.95, 0.8]),
        _ => ("Tree_Common_Hazel", [1.0, 1.0, 1.0]), // garden trees: kaki, matsu, maki
    }
}

pub struct Vegetation {
    pub species: Vec<Species>,
    /// (x, y plan, height above ground m, species index, variant index, yaw, scale, sideways scale)
    trees: Vec<(f32, f32, f32, usize, usize, f32, f32, f32)>,
    /// grass species indices: lawn / meadow mix, tall grass, reeds
    grass: Vec<usize>,
    /// each grass species' variants as impostors (the grass beyond the real patches)
    grass_imp: Vec<Vec<ImpostorVariant>>,
    /// the river's centre line and half width of the water (reeds grow only along its edges)
    river: Vec<[f32; 2]>,
    water_half: f32,
    landuse: Vec<u8>,
    nx: usize,
    ny: usize,
    x0: f32,
    y1: f32,
}

/// The land-use codes grass and weeds grow on: grass, levees, gardens (meadow mix), the river's
/// edges (reeds), dirt verges and gravel (sparse weeds). Never concrete (driveways, buildings,
/// yards), asphalt, paddies or forest floor.
pub fn grows_on(code: u8) -> bool {
    matches!(code, 0 | 3 | 7 | 8 | 9 | 10)
}

fn hash(a: u32) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFFFF) as f32 / 16_777_216.0
}

impl Vegetation {
    pub fn load(r: &mut Renderer, plan: &Plan, hf: &HeightField, landuse: Vec<u8>) -> Result<Self> {
        let t0 = std::time::Instant::now();
        let mut species: Vec<Species> = Vec::new();
        let mut by_key: Vec<((&str, [u32; 3]), usize)> = Vec::new();
        let mut index = |r: &mut Renderer, species: &mut Vec<Species>, key: (&'static str, [f32; 3])| -> Result<usize> {
            let k = (key.0, key.1.map(|v| (v * 100.0) as u32));
            if let Some((_, i)) = by_key.iter().find(|(kk, _)| *kk == k) {
                return Ok(*i);
            }
            species.push(load_species(r, key.0, key.1)?);
            by_key.push((k, species.len() - 1));
            Ok(species.len() - 1)
        };
        let mut trees = Vec::with_capacity(plan.trees.len());
        for (k, (x, y, sp, h)) in plan.trees.iter().enumerate() {
            let si = index(r, &mut species, stand_in(sp))?;
            let vs = &species[si].variants;
            // the variant closest in height, then scaled to the planned height (within reason)
            let (vi, scale) = if sp == "hedge" {
                // a clipped hedge: the broadest variant (widest for its height), scaled to hedge height
                let (vi, v) = vs.iter().enumerate().max_by(|a, b| (a.1.reach / a.1.height).total_cmp(&(b.1.reach / b.1.height))).unwrap();
                (vi, h / v.height.max(0.5))
            } else {
                let (vi, v) = vs.iter().enumerate().min_by(|a, b| (a.1.height - h).abs().total_cmp(&(b.1.height - h).abs())).unwrap();
                (vi, (h / v.height.max(0.5)).clamp(0.7, 1.35))
            };
            // the stand-in bamboo's culms are thin for moso: thicker, the same height
            let wide = if sp == "bamboo" { 1.9 } else { 1.0 };
            trees.push((*x, *y, hf.plan(*x, *y), si, vi, hash(k as u32) * std::f32::consts::TAU, scale, wide));
        }
        // every tree variant in use gets an impostor; every tree a placement
        let mut baked: std::collections::HashMap<(usize, usize), ImpostorVariant> = Default::default();
        let mut placements = Vec::with_capacity(trees.len());
        let tb = std::time::Instant::now();
        for (k, &(x, y, g, si, vi, yaw, s, w)) in trees.iter().enumerate() {
            let iv = match baked.get(&(si, vi)) {
                Some(v) => *v,
                None => {
                    let sp = &species[si];
                    let v = &sp.variants[vi];
                    let iv = r.bake_impostor(&format!("{}_{}", sp.tag, v.name), &v.parts, sp.grass, sp.transmission)?;
                    baked.insert((si, vi), iv);
                    iv
                }
            };
            placements.push(PlantPlacement {
                transform: Affine3A::from_scale_rotation_translation(Vec3::new(s * w, s, s * w), Quat::from_rotation_y(yaw), Vec3::new(x, g, -y)),
                variant: iv,
                seed: k as u32 & 0xFFFF,
            });
        }
        r.set_plants(&placements)?;
        println!("impostors: {} variants ready in {:.1}s", baked.len(), tb.elapsed().as_secs_f32());
        let grass = vec![
            index(r, &mut species, ("Wild_Grass", [1.0, 1.0, 1.0]))?,
            index(r, &mut species, ("Grass_Clumps", [1.0, 1.0, 1.0]))?,
            index(r, &mut species, ("Leafy_Dandelions", [1.0, 1.0, 1.0]))?,
            index(r, &mut species, ("Tall_Grass", [1.0, 1.0, 1.0]))?,
            index(r, &mut species, ("Giant_Reed", [1.0, 1.0, 1.0]))?,
        ];
        let mut grass_imp = Vec::new();
        for &gs in &grass {
            let sp = &species[gs];
            let mut v = Vec::new();
            for var in &sp.variants {
                v.push(r.bake_impostor(&format!("{}_{}", sp.tag, var.name), &var.parts, true, sp.transmission)?);
            }
            grass_imp.push(v);
        }
        let t = &plan.terrain;
        println!("vegetation: {} species, {} trees, loaded in {:.1}s", species.len(), trees.len(), t0.elapsed().as_secs_f32());
        Ok(Self { species, trees, grass, grass_imp, river: plan.river.pts.clone(), water_half: plan.river.water_half, landuse, nx: t.nx, ny: t.ny, x0: t.origin[0], y1: t.origin[1] + (t.ny - 1) as f32 })
    }

    fn land(&self, x: f32, y: f32) -> u8 {
        // (row 0 is the north edge: the row covering plan y is y1 + 1 - y, floored)
        let i = (x - self.x0).floor();
        let j = (self.y1 + 1.0 - y).floor();
        if i < 0.0 || j < 0.0 || i as usize >= self.nx || j as usize >= self.ny {
            return 255;
        }
        self.landuse[j as usize * self.nx + i as usize]
    }

    /// Which trees are drawn as their real meshes near a plan point (the rest are impostors):
    /// pass to `Renderer::set_plant_state`.
    pub fn mesh_state(&self, at: [f32; 2], tree_r: f32) -> Vec<bool> {
        let r2 = tree_r * tree_r;
        self.trees.iter().map(|t| (t.0 - at[0]).powi(2) + (t.1 - at[1]).powi(2) < r2).collect()
    }

    /// Real geometry for the plants near a plan point: trees within `tree_r`, grass patches
    /// within `grass_r` (on meadow, levee and garden ground).
    pub fn instances(&self, hf: &HeightField, at: [f32; 2], tree_r: f32, grass_r: f32) -> Vec<Instance> {
        let mut out = Vec::new();
        let place = |out: &mut Vec<Instance>, v: &Variant, x: f32, y: f32, g: f32, yaw: f32, s: f32, w: f32| {
            let root = Affine3A::from_scale_rotation_translation(Vec3::new(s * w, s, s * w), Quat::from_rotation_y(yaw), Vec3::new(x, g, -y));
            out.extend(v.parts.iter().map(|(mesh, xf)| Instance { mesh: *mesh, transform: root * *xf }));
        };
        let r2 = tree_r * tree_r;
        for &(x, y, g, si, vi, yaw, s, w) in &self.trees {
            let (dx, dy) = (x - at[0], y - at[1]);
            if dx * dx + dy * dy < r2 {
                place(&mut out, &self.species[si].variants[vi], x, y, g, yaw, s, w);
            }
        }
        for p in self.grass_patches(at, grass_r, 0.0) {
            if p.mesh {
                place(&mut out, &self.species[self.grass[p.species]].variants[p.variant], p.x, p.y, hf.plan(p.x, p.y), p.yaw, p.scale, 1.0);
            }
        }
        out
    }

    /// The grass beyond the real patches, as impostors (out to `far`, thinning out over its last
    /// 20%): pass to `Renderer::set_plant_group(1, ..)` and refresh as the camera moves.
    pub fn grass_impostors(&self, hf: &HeightField, at: [f32; 2], mesh_r: f32, far: f32) -> Vec<PlantPlacement> {
        self.grass_patches(at, mesh_r, far)
            .into_iter()
            .filter(|p| !p.mesh)
            .map(|p| PlantPlacement {
                transform: Affine3A::from_scale_rotation_translation(Vec3::splat(p.scale), Quat::from_rotation_y(p.yaw), Vec3::new(p.x, hf.plan(p.x, p.y), -p.y)),
                variant: self.grass_imp[p.species][p.variant],
                seed: p.seed & 0xFFFF,
            })
            .collect()
    }

    /// The grass patches round a point: one layout for both kinds, so the hand-over from real
    /// patches (within `mesh_r`, thinning out over its outer 40%) to impostors (out to `far`) has
    /// no gaps or doubles: each patch is one or the other by the same random number.
    fn grass_patches(&self, at: [f32; 2], mesh_r: f32, far: f32) -> Vec<GrassPatch> {
        let mut out = Vec::new();
        let step = 1.2f32;
        let reach = mesh_r.max(far);
        let n = (reach / step) as i32 + 1;
        let (cx, cy) = ((at[0] / step).floor() as i32, (at[1] / step).floor() as i32);
        for j in -n..=n {
            for i in -n..=n {
                let (gi, gj) = (cx + i, cy + j);
                let seed = (gi as u32).wrapping_mul(73_856_093) ^ (gj as u32).wrapping_mul(19_349_663);
                let x = (gi as f32 + hash(seed)) * step;
                let y = (gj as f32 + hash(seed ^ 0x51)) * step;
                let dist = (x - at[0]).hypot(y - at[1]);
                if dist > reach {
                    continue;
                }
                let h = hash(seed ^ 0x5D);
                let p_mesh = ((mesh_r - dist) / (0.4 * mesh_r)).clamp(0.0, 1.0);
                let mesh = h < p_mesh;
                if !mesh && (far <= 0.0 || h > ((far - dist) / (0.2 * far)).clamp(0.0, 1.0)) {
                    continue;
                }
                let code = self.land(x, y);
                let pick = hash(seed ^ 0xA7);
                let gs = match code {
                    0 | 9 => if pick < 0.55 { 0 } else if pick < 0.85 { 1 } else { 2 },
                    8 => if pick < 0.5 { 0 } else if pick < 0.8 { 3 } else { 1 },
                    7 => {
                        // reeds: only in a band along each bank, in clumps with gaps between,
                        // some tall grass among them
                        let d = self.river.iter().map(|q| (q[0] - x).hypot(q[1] - y)).fold(f32::MAX, f32::min);
                        let from_edge = self.water_half - d;
                        let clump = hash(((x / 7.0).floor() as i32 as u32).wrapping_mul(2_654_435_761) ^ ((y / 7.0).floor() as i32 as u32).wrapping_mul(40_503));
                        if !(0.0..2.5).contains(&from_edge) || clump > 0.45 || pick > 0.75 - from_edge * 0.2 {
                            continue;
                        }
                        if hash(seed ^ 0x2F) < 0.25 { 3 } else { 4 }
                    }
                    // weeds on the dirt verges and a few in the gravel: small clumps, sparse
                    10 | 3 => {
                        if hash(seed ^ 0x3C) > if code == 10 { 0.2 } else { 0.05 } {
                            continue;
                        }
                        if pick < 0.6 { 1 } else { 2 }
                    }
                    _ => continue,
                };
                if code == 0 && hash(seed ^ 0x33) < 0.15 {
                    continue; // gaps
                }
                let sp = &self.species[self.grass[gs]];
                let vi = (hash(seed ^ 0x77) * sp.variants.len() as f32) as usize % sp.variants.len();
                let v = &sp.variants[vi];
                let scale = match code {
                    7 => 0.55 + 0.75 * hash(seed ^ 0x99),
                    10 | 3 => 0.4 + 0.35 * hash(seed ^ 0x99),
                    _ => 0.8 + 0.5 * hash(seed ^ 0x99),
                };
                // the whole patch, out to its real reach and a margin, must be on grass ground: no
                // blades over the asphalt (the road ribbon runs 0.25 m past its land-use code),
                // the gravel or the paddies. Reeds may lean over the riverbed.
                let rr = v.reach * scale + match code {
                    7 => 0.0,
                    10 | 3 => 0.05, // (weeds: in the narrow verges, up to the edge)
                    _ => 0.5,
                };
                let grassy = |c: u8| c == code || (matches!(code, 0 | 8 | 9) && matches!(c, 0 | 8 | 9));
                // (the patch's outer edge may lean over gravel, dirt and ballast, as weeds do;
                // never over asphalt or the paddies)
                let edge_ok = |c: u8| grassy(c) || (code != 7 && matches!(c, 3 | 10 | 13));
                let fits = (0..8).all(|k| {
                    let a = k as f32 * std::f32::consts::FRAC_PI_4;
                    grassy(self.land(x + a.cos() * rr * 0.5, y + a.sin() * rr * 0.5)) && edge_ok(self.land(x + a.cos() * rr, y + a.sin() * rr))
                });
                if !fits {
                    continue;
                }
                out.push(GrassPatch { x, y, species: gs, variant: vi, yaw: hash(seed ^ 0x13) * std::f32::consts::TAU, scale, mesh, seed });
            }
        }
        out
    }
}

struct GrassPatch {
    x: f32,
    y: f32,
    /// index into `Vegetation::grass`
    species: usize,
    variant: usize,
    yaw: f32,
    scale: f32,
    /// a real patch (else an impostor)
    mesh: bool,
    seed: u32,
}
