//! Kazehana-chō, built from the town plan (`tools/blender/town_plan.py` writes `assets/town`):
//! the terrain with its scanned ground (land-use mapped), roads with painted lines, bridges, the
//! river, the railway and the paddies' levees. Houses and vegetation are added on top.
//!
//! Plan coordinates are (x east, y north) in metres; the world is (x, height, -y).

pub mod cloth;
pub mod collide;
pub mod plan;
pub mod plants;
pub mod people;
pub mod plots;
pub mod scene;

use akari::glam::{Affine3A, Vec2, Vec3};
use akari::textures::{TexJob, TexKind, TexSource};
use akari::{Instance, Material, MaterialId, MeshData, Renderer};
use anyhow::{bail, Context, Result};
use plan::Plan;
use std::path::{Path, PathBuf};

/// The Fab library cache (Megascans surfaces, exported as glTF with UE textures).
pub const FAB: &str = "D:/EpicVault/VaultCache/FabLibrary";

/// Ground layers in shader order (akari shaders/ground.glsl): Fab folder name prefixes.
pub const GROUND_SCANS: [&str; 13] = [
    "Uncut_Grass",
    "Ploughed_Soil",
    "Soil_Mud",
    "Icelandic_Gravel",
    "Asphalt_Road", // (4: concrete, made from the asphalt grain in ground.glsl)
    "Asphalt_Road",
    "Forest_Floor-cf8af196",
    "Mossy_Riverbed",
    "Wet_Grass_Ground",
    "Mossy_Grass",
    "Sparse_Grassy_Soil",
    "Pebbles_Round",
    "Grass_Dried",
];

/// Heights on the plan's 1 m grid.
#[derive(Clone)]
pub struct HeightField {
    pub nx: usize,
    pub ny: usize,
    pub x0: f32,
    pub y0: f32,
    pub h: Vec<f32>,
}

impl HeightField {
    fn at_ij(&self, i: usize, j: usize) -> f32 {
        self.h[j.min(self.ny - 1) * self.nx + i.min(self.nx - 1)]
    }

    /// Height at a plan point (bilinear).
    pub fn plan(&self, x: f32, y: f32) -> f32 {
        let fx = (x - self.x0).clamp(0.0, (self.nx - 1) as f32 - 1e-3);
        let fy = (y - self.y0).clamp(0.0, (self.ny - 1) as f32 - 1e-3);
        let (i, j) = (fx as usize, fy as usize);
        let (tx, ty) = (fx - i as f32, fy - j as f32);
        let a = self.at_ij(i, j) + (self.at_ij(i + 1, j) - self.at_ij(i, j)) * tx;
        let b = self.at_ij(i, j + 1) + (self.at_ij(i + 1, j + 1) - self.at_ij(i, j + 1)) * tx;
        a + (b - a) * ty
    }

    /// Height under a world point (x, _, z).
    pub fn world(&self, x: f32, z: f32) -> f32 {
        self.plan(x, -z)
    }
}

pub struct Town {
    pub dir: PathBuf,
    pub plan: Plan,
    pub height: HeightField,
}

/// A plan point at height `h` as a world point.
pub fn w3(p: [f32; 2], h: f32) -> Vec3 {
    Vec3::new(p[0], h, -p[1])
}

impl Town {
    pub fn load(dir: &Path) -> Result<Self> {
        let plan: Plan = serde_json::from_slice(&std::fs::read(dir.join("town.json")).with_context(|| format!("{}/town.json (run tools/blender/town_plan.py)", dir.display()))?)?;
        let t = &plan.terrain;
        let raw = std::fs::read(dir.join(&t.file)).with_context(|| format!("{}", t.file))?;
        if raw.len() != t.nx * t.ny * 4 {
            bail!("{}: expected {}x{} heights", t.file, t.nx, t.ny);
        }
        let h = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let height = HeightField { nx: t.nx, ny: t.ny, x0: t.origin[0], y0: t.origin[1], h };
        Ok(Self { dir: dir.into(), plan, height })
    }

    /// Ground height under a plan point.
    pub fn ground(&self, p: [f32; 2]) -> f32 {
        self.height.plan(p[0], p[1])
    }

    /// The surface you walk on at a plan point: the ground, or a road deck above it (bridges).
    pub fn surface(&self, p: [f32; 2]) -> f32 {
        let g = self.ground(p);
        let q = Vec2::from(p);
        let mut best = g;
        for road in &self.plan.roads {
            let Some(h) = &road.h else { continue };
            for i in 1..road.pts.len() {
                let (a, b) = (Vec2::from(road.pts[i - 1]), Vec2::from(road.pts[i]));
                let ab = b - a;
                let t = ((q - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                if (a + ab * t - q).length() <= road.width / 2.0 {
                    let rh = h[i - 1] + (h[i] - h[i - 1]) * t + 0.04;
                    if rh > best && rh - g < 6.0 {
                        best = rh;
                    }
                }
            }
        }
        best
    }
}

/// What `build` put in the renderer.
pub struct TownScene {
    pub instances: Vec<Instance>,
    pub triangles: usize,
}

/// A Megascans surface's B / N / ORM textures (found under a Fab folder starting with `prefix`).
pub fn scan_textures(prefix: &str) -> Result<[PathBuf; 3]> {
    let dir = std::fs::read_dir(FAB)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(prefix)))
        .with_context(|| format!("Fab surface '{prefix}' not downloaded"))?;
    let mut files = Vec::new();
    collect(&dir, &mut files, 0);
    let pick = |s: &str| files.iter().find(|p| p.to_string_lossy().ends_with(s)).cloned().with_context(|| format!("{prefix}: no *{s}"));
    Ok([pick("_B.png")?, pick("_N.png")?, pick("_ORM.png")?])
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 6 {
        return;
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                collect(&p, out, depth + 1);
            } else if p.extension().is_some_and(|x| x == "png") {
                out.push(p);
            }
        }
    }
}

pub(crate) fn scan_jobs(prefix: &str) -> Result<Vec<TexJob>> {
    let [b, n, orm] = scan_textures(prefix)?;
    let job = |p: PathBuf, kind, tag: &str| TexJob { key: akari::textures::file_key(&p), name: format!("{prefix} {tag}"), kind, source: TexSource::File(p) };
    Ok(vec![job(b, TexKind::Color, "B"), job(n, TexKind::Normal, "N"), job(orm, TexKind::Data, "ORM")])
}

/// A scanned surface as an ordinary textured material (UV-mapped meshes: roads, levees).
pub fn surface_material(r: &mut Renderer, prefix: &str, tint: f32) -> Result<MaterialId> {
    surface_material_flags(r, prefix, tint, 0)
}

/// `surface_material` with material flags (e.g. MAT_ROAD for the road shader).
pub fn surface_material_flags(r: &mut Renderer, prefix: &str, tint: f32, flags: u32) -> Result<MaterialId> {
    let t = r.add_textures(&scan_jobs(prefix)?)?;
    Ok(r.add_material(Material {
        base_color: [tint, tint, tint, 1.0],
        roughness: 1.0,
        metallic: 1.0,
        base_tex: Some(t[0]),
        normal_tex: Some(t[1]),
        mr_tex: Some(t[2]),
        occlusion_tex: Some(t[2]),
        flags,
        ..Default::default()
    }))
}

/// The road material (akari shaders/road.glsl): the asphalt scan, and the scanned decals packed
/// by tools/pack_decals.py (assets/decals) attached after it, cracks / patches / potholes.
fn road_material(r: &mut Renderer) -> Result<MaterialId> {
    #[derive(serde::Deserialize)]
    struct Decal {
        id: String,
        kind: String,
        size: f32,
    }
    let dir = Path::new("assets/decals");
    let decals: Vec<Decal> = match std::fs::read(dir.join("decals.json")) {
        Ok(b) => serde_json::from_slice(&b)?,
        Err(_) => {
            eprintln!("road decals not packed (python tools/pack_decals.py): plain asphalt");
            Vec::new()
        }
    };
    let asphalt = scan_jobs("Asphalt_Road")?;
    let t = r.add_textures(&asphalt)?;
    let mut m = Material {
        roughness: 1.0,
        metallic: 1.0,
        base_tex: Some(t[0]),
        normal_tex: Some(t[1]),
        mr_tex: Some(t[2]),
        occlusion_tex: Some(t[2]),
        flags: akari::scene::MAT_ROAD,
        ..Default::default()
    };
    if !decals.is_empty() {
        let mut jobs = Vec::new();
        let mut counts = [0.0f32; 3];
        let mut sizes = [0.5f32, 2.0, 0.5];
        for (k, kind) in ["crack", "patch", "pothole"].iter().enumerate() {
            for d in decals.iter().filter(|d| d.kind == *kind) {
                counts[k] += 1.0;
                sizes[k] = if counts[k] == 1.0 { d.size } else { sizes[k].max(d.size) };
                for (tag, tk) in [("C", TexKind::Color), ("N", TexKind::Normal), ("D", TexKind::Data)] {
                    let p = dir.join(format!("{}_{tag}.png", d.id));
                    jobs.push(TexJob { key: akari::textures::file_key(&p), name: format!("{} {tag}", d.id), kind: tk, source: TexSource::File(p) });
                }
            }
        }
        let dt = r.add_textures(&jobs)?;
        m.emissive_tex = Some(dt[0]);
        m.emissive = counts;
        m.base_color = [sizes[0], sizes[1], sizes[2], 1.0];
    }
    Ok(r.add_material(m))
}

// ---------------------------------------------------------------------------- mesh helpers

#[derive(Default)]
struct Mesh {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    /// per-vertex data for special shaders (roads: half width, kind, seed)
    col: Vec<[f32; 3]>,
    idx: Vec<u32>,
}

impl Mesh {
    fn vert(&mut self, p: Vec3, n: Vec3, uv: [f32; 2]) -> u32 {
        self.pos.push(p.to_array());
        self.nrm.push(n.to_array());
        self.uv.push(uv);
        self.pos.len() as u32 - 1
    }

    fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.idx.extend_from_slice(&[a, b, c, a, c, d]);
    }

    /// A bar along a polyline: offsets `a`..`b` across, from `y0` to `y1` above `h`: its top and
    /// both sides (rails, crossing panels). UVs in metres.
    #[allow(clippy::too_many_arguments)]
    fn bar(&mut self, pts: &[Vec2], h: &[f32], a: f32, b: f32, y0: f32, y1: f32, keep: impl Fn(usize) -> bool) {
        let tg = tangents(pts);
        let mut along = 0.0;
        for i in 1..pts.len() {
            let len = (pts[i] - pts[i - 1]).length();
            if keep(i - 1) && keep(i) {
                let (l0, l1) = (left(tg[i - 1]), left(tg[i]));
                let w = |p: Vec2, y: f32| Vec3::new(p.x, y, -p.y);
                let (u0, u1) = (along, along + len);
                let (p0a, p0b, p1a, p1b) = (pts[i - 1] + l0 * a, pts[i - 1] + l0 * b, pts[i] + l1 * a, pts[i] + l1 * b);
                let (t0, t1, b0, b1) = (h[i - 1] + y1, h[i] + y1, h[i - 1] + y0, h[i] + y0);
                // top
                let q = [self.vert(w(p0a, t0), Vec3::Y, [a, u0]), self.vert(w(p1a, t1), Vec3::Y, [a, u1]), self.vert(w(p1b, t1), Vec3::Y, [b, u1]), self.vert(w(p0b, t0), Vec3::Y, [b, u0])];
                self.quad(q[0], q[1], q[2], q[3]);
                // the sides (a: facing -left, b: facing +left)
                let na = -Vec3::new(l0.x, 0.0, -l0.y);
                let s = [self.vert(w(p0a, b0), na, [u0, b0]), self.vert(w(p1a, b1), na, [u1, b1]), self.vert(w(p1a, t1), na, [u1, t1]), self.vert(w(p0a, t0), na, [u0, t0])];
                self.quad(s[0], s[1], s[2], s[3]);
                let s = [self.vert(w(p0b, b0), -na, [u0, b0]), self.vert(w(p1b, b1), -na, [u1, b1]), self.vert(w(p1b, t1), -na, [u1, t1]), self.vert(w(p0b, t0), -na, [u0, t0])];
                self.quad(s[1], s[0], s[3], s[2]);
            }
            along += len;
        }
    }

    /// An axis box with world-projected UVs (parapets, girders, sleepers).
    fn oriented_box(&mut self, center: Vec3, x: Vec3, y: Vec3, z: Vec3) {
        let c = |sx: f32, sy: f32, sz: f32| center + x * sx + y * sy + z * sz;
        let faces = [
            ([c(-1., -1., 1.), c(1., -1., 1.), c(1., 1., 1.), c(-1., 1., 1.)], z),
            ([c(1., -1., -1.), c(-1., -1., -1.), c(-1., 1., -1.), c(1., 1., -1.)], -z),
            ([c(1., -1., 1.), c(1., -1., -1.), c(1., 1., -1.), c(1., 1., 1.)], x),
            ([c(-1., -1., -1.), c(-1., -1., 1.), c(-1., 1., 1.), c(-1., 1., -1.)], -x),
            ([c(-1., 1., 1.), c(1., 1., 1.), c(1., 1., -1.), c(-1., 1., -1.)], y),
            ([c(-1., -1., -1.), c(1., -1., -1.), c(1., -1., 1.), c(-1., -1., 1.)], -y),
        ];
        for (q, n) in faces {
            let n = n.normalize_or_zero();
            let (t, b) = n.any_orthonormal_pair();
            let ids: Vec<u32> = q.iter().map(|p| self.vert(*p, n, [p.dot(t) * 0.5, p.dot(b) * 0.5])).collect();
            self.quad(ids[0], ids[1], ids[2], ids[3]);
        }
    }

    fn finish(self, name: &str, material: MaterialId, tangents: bool) -> Option<MeshData> {
        if self.idx.is_empty() {
            return None;
        }
        let mut m = MeshData { name: name.into(), positions: self.pos, normals: self.nrm, uvs: self.uv, colors: self.col, indices: self.idx, material: Some(material), ..Default::default() };
        if tangents {
            m.generate_tangents();
        } else {
            m.uvs.clear();
        }
        Some(m)
    }
}

/// Unit tangents along a polyline (averaged at the joints).
fn tangents(pts: &[Vec2]) -> Vec<Vec2> {
    (0..pts.len())
        .map(|i| {
            let a = pts[i.saturating_sub(1)];
            let b = pts[(i + 1).min(pts.len() - 1)];
            (b - a).normalize_or(Vec2::X)
        })
        .collect()
}

fn left(t: Vec2) -> Vec2 {
    Vec2::new(-t.y, t.x)
}

/// How far an asphalt road's crown stands above its graded ground (m), and its kind for the road
/// shader. Where roads meet, the more important one is on top (its paint runs through).
pub fn road_lift(kind: &str) -> (f32, f32) {
    match kind {
        "pref" => (0.050, 1.0),
        "school" => (0.048, 3.0),
        "old" => (0.046, 2.0),
        "street" => (0.044, 0.0),
        "lane" => (0.042, 0.0),
        _ => (0.040, 0.0),
    }
}

/// An asphalt road's edges stand this fraction of its crown's lift above the ground (camber).
pub const EDGE_LIFT: f32 = 0.55;

/// The asphalt's height above its graded centre-line height, `a` m across from the centre of
/// a road `hw` half wide: the crown at the middle falling to the edges.
pub fn road_profile(kind: &str, a: f32, hw: f32) -> f32 {
    let lift = road_lift(kind).0;
    lift + (lift * EDGE_LIFT - lift) * (a.abs() / hw).min(1.0)
}

/// A ribbon along a polyline: offsets `a`..`b` across (metres, left positive), at heights `h`
/// (+ `lift`). UVs: u across, v along, both in metres / `tile`.
fn ribbon(m: &mut Mesh, pts: &[Vec2], h: &[f32], a: f32, b: f32, lift: f32, tile: f32, keep: impl Fn(usize) -> bool) {
    ribbon_info(m, pts, h, a, b, lift, tile, None, keep)
}

/// `ribbon`, with `info` written to every vertex's colour stream.
#[allow(clippy::too_many_arguments)]
fn ribbon_info(m: &mut Mesh, pts: &[Vec2], h: &[f32], a: f32, b: f32, lift: f32, tile: f32, info: Option<[f32; 3]>, keep: impl Fn(usize) -> bool) {
    ribbon_slope(m, pts, h, a, b, lift, lift, tile, info, keep)
}

/// A ribbon whose two edges are at different heights (`lift_a` at offset `a`, `lift_b` at `b`):
/// the road's edges sloping down into the ground, so the road doesn't float on it.
#[allow(clippy::too_many_arguments)]
fn ribbon_slope(m: &mut Mesh, pts: &[Vec2], h: &[f32], a: f32, b: f32, lift_a: f32, lift_b: f32, tile: f32, info: Option<[f32; 3]>, keep: impl Fn(usize) -> bool) {
    let tg = tangents(pts);
    let mut along = 0.0;
    let mut prev: Option<(u32, u32)> = None;
    for i in 0..pts.len() {
        if i > 0 {
            along += (pts[i] - pts[i - 1]).length();
        }
        if !keep(i) {
            prev = None;
            continue;
        }
        let l = left(tg[i]);
        let pa = pts[i] + l * a;
        let pb = pts[i] + l * b;
        let va = m.vert(Vec3::new(pa.x, h[i] + lift_a, -pa.y), Vec3::Y, [a / tile, along / tile]);
        let vb = m.vert(Vec3::new(pb.x, h[i] + lift_b, -pb.y), Vec3::Y, [b / tile, along / tile]);
        if let Some(c) = info {
            m.col.push(c);
            m.col.push(c);
        }
        if let Some((pa_, pb_)) = prev {
            // counter-clockwise seen from above (world: +y up, plan y = -z)
            m.quad(pa_, va, vb, pb_);
        }
        prev = Some((va, vb));
    }
}

// ---------------------------------------------------------------------------- build

pub fn build(r: &mut Renderer, town: &Town) -> Result<TownScene> {
    let t0 = std::time::Instant::now();
    let plan = &town.plan;
    let mut meshes: Vec<MeshData> = Vec::new();

    // ---- ground material: the land-use map and the scanned layers
    let mut jobs = vec![TexJob {
        key: format!("landuse {}", town.dir.join(&plan.terrain.landuse).display()),
        name: "town land use".into(),
        kind: TexKind::Raw8,
        source: TexSource::File(town.dir.join(&plan.terrain.landuse)),
    }];
    for s in GROUND_SCANS {
        jobs.extend(scan_jobs(s)?);
    }
    let tex = r.add_textures(&jobs)?;
    let ter = &plan.terrain;
    let x0 = ter.origin[0];
    let y1 = ter.origin[1] + (ter.ny - 1) as f32;
    let ground = r.add_material(Material::ground(tex[0], x0, -(y1 + 1.0), ter.nx as u32, ter.ny as u32, tex[1]));

    // ---- terrain: 128 m chunks; 1 m cells where anything but forest is, 2 m on the hill foot,
    // 4 m on the hills; skirts hide the cracks between resolutions
    let hf = &town.height;
    let lu = load_landuse(&town.dir.join(&ter.landuse))?;
    const C: usize = 128;
    for cj in (0..hf.ny - 1).step_by(C) {
        for ci in (0..hf.nx - 1).step_by(C) {
            let (i1, j1) = ((ci + C).min(hf.nx - 1), (cj + C).min(hf.ny - 1));
            let mut fine = false;
            let mut low = false;
            for j in (cj..=j1).step_by(2) {
                for i in (ci..=i1).step_by(2) {
                    let code = lu[(hf.ny - 1 - j) * hf.nx + i];
                    if code != 6 {
                        fine = true;
                    }
                    if hf.at_ij(i, j) - (22.0 - 0.004 * (hf.x0 + i as f32)) < 20.0 {
                        low = true;
                    }
                }
            }
            let step = if fine { 1 } else if low { 2 } else { 4 };
            let mut m = Mesh::default();
            let nxs = (i1 - ci) / step + 1;
            let nys = (j1 - cj) / step + 1;
            let gi = |k: usize| (ci + k * step).min(i1);
            let gj = |k: usize| (cj + k * step).min(j1);
            let normal = |i: usize, j: usize| {
                let hx = (hf.at_ij(i + 1, j) - hf.at_ij(i.saturating_sub(1), j)) / 2.0;
                let hy = (hf.at_ij(i, j + 1) - hf.at_ij(i, j.saturating_sub(1))) / 2.0;
                Vec3::new(-hx, 1.0, hy).normalize()
            };
            for b in 0..nys {
                for a in 0..nxs {
                    let (i, j) = (gi(a), gj(b));
                    let p = Vec3::new(hf.x0 + i as f32, hf.at_ij(i, j), -(hf.y0 + j as f32));
                    m.vert(p, normal(i, j), [0.0, 0.0]);
                }
            }
            for b in 0..nys - 1 {
                for a in 0..nxs - 1 {
                    let v = |a: usize, b: usize| (b * nxs + a) as u32;
                    m.quad(v(a, b), v(a + 1, b), v(a + 1, b + 1), v(a, b + 1));
                }
            }
            // skirts
            let edge: Vec<(usize, usize)> = (0..nxs).map(|a| (a, 0)).chain((0..nys).map(|b| (nxs - 1, b))).chain((0..nxs).rev().map(|a| (a, nys - 1))).chain((0..nys).rev().map(|b| (0, b))).collect();
            for w in edge.windows(2) {
                let (p, q) = (w[0], w[1]);
                if p == q {
                    continue;
                }
                let up_p = (p.1 * nxs + p.0) as u32;
                let up_q = (q.1 * nxs + q.0) as u32;
                let pp = Vec3::from(m.pos[up_p as usize]) - Vec3::Y * 1.5;
                let pq = Vec3::from(m.pos[up_q as usize]) - Vec3::Y * 1.5;
                let n = Vec3::from(m.nrm[up_p as usize]);
                let dp = m.vert(pp, n, [0.0, 0.0]);
                let dq = m.vert(pq, n, [0.0, 0.0]);
                m.quad(up_p, dp, dq, up_q);
            }
            if let Some(md) = m.finish(&format!("terrain {ci},{cj}"), ground, false) {
                meshes.push(md);
            }
        }
    }

    // ---- roads, painted lines, bridges
    let asphalt = road_material(r)?;
    let gravel = surface_material(r, "Icelandic_Gravel", 2.6)?;
    let concrete = surface_material(r, "Precast_Concrete_Wall", 1.3)?;
    let paint = r.add_material(Material::color([0.86, 0.86, 0.82], 0.55));
    let water = r.add_material(Material { base_color: [0.025, 0.04, 0.035, 1.0], roughness: 0.03, metallic: 0.0, ..Default::default() });
    let steel = r.add_material(Material { base_color: [0.45, 0.42, 0.40, 1.0], roughness: 0.35, metallic: 1.0, ..Default::default() });
    // creosoted timber sleepers: the planks scan, darkened
    let sleeper = surface_material(r, "Wooden_Planks", 0.45)?;
    let river: Vec<Vec2> = plan.river.pts.iter().map(|p| Vec2::from(*p)).collect();
    let near_river = |p: Vec2| river.iter().map(|q| (*q - p).length_squared()).fold(f32::MAX, f32::min).sqrt();
    let (mut asph, mut grav, mut lines, mut bridge) = (Mesh::default(), Mesh::default(), Mesh::default(), Mesh::default());
    for (ri, road) in plan.roads.iter().enumerate() {
        let Some(h) = &road.h else { continue };
        let pts: Vec<Vec2> = road.pts.iter().map(|p| Vec2::from(*p)).collect();
        let hw = road.width / 2.0;
        // where roads meet, the more important one is on top (its paint runs through)
        let (lift, kind) = road_lift(&road.kind);
        if matches!(road.kind.as_str(), "farm" | "levee") {
            ribbon(&mut grav, &pts, h, -hw, hw, 0.04, 1.5, |_| true);
            ribbon_slope(&mut grav, &pts, h, hw, hw + 0.2, 0.04, -0.06, 1.5, None, |_| true);
            ribbon_slope(&mut grav, &pts, h, -hw - 0.2, -hw, -0.06, 0.04, 1.5, None, |_| true);
        } else {
            let (dp, dh) = densify(&pts, h, 4.0);
            let info = Some([hw, kind, ri as f32 * 1.618 % 97.0]);
            // cambered: the crown `lift` above the graded ground, the edges almost flush with the
            // shoulder (still in the same order where roads meet), then a worn lip into the gravel
            let el = lift * EDGE_LIFT; // (the 1 m ground grid bends a little off the road on ramps)
            ribbon_slope(&mut asph, &dp, &dh, -hw, 0.0, el, lift, 1.0, info, |_| true);
            ribbon_slope(&mut asph, &dp, &dh, 0.0, hw, lift, el, 1.0, info, |_| true);
            ribbon_slope(&mut asph, &dp, &dh, hw, hw + 0.14, el, -0.03, 1.0, info, |_| true);
            ribbon_slope(&mut asph, &dp, &dh, -hw - 0.14, -hw, -0.03, el, 1.0, info, |_| true);
        }
        if false {
            // white edge lines (outside lines on both sides), a dashed centre line on the
            // prefectural road
            ribbon(&mut lines, &pts, h, hw - 0.4, hw - 0.25, 0.05, 1.0, |_| true);
            ribbon(&mut lines, &pts, h, -hw + 0.25, -hw + 0.4, 0.05, 1.0, |_| true);
            if road.kind == "pref" {
                // 5 m dashes (the road's vertices are 20 m apart: densify first)
                let (dp, dh) = densify(&pts, h, 2.5);
                let mut along = 0.0;
                let keep: Vec<bool> = (0..dp.len())
                    .map(|i| {
                        if i > 0 {
                            along += (dp[i] - dp[i - 1]).length();
                        }
                        (along / 5.0) as i32 % 2 == 0
                    })
                    .collect();
                ribbon(&mut lines, &dp, &dh, -0.075, 0.075, 0.05, 1.0, |i| keep[i]);
            }
        }
        // bridges: where the road crosses the river, a deck with concrete parapets
        let tg = tangents(&pts);
        for i in 0..pts.len() {
            if near_river(pts[i]) < 13.0 && i + 1 < pts.len() {
                let a = pts[i];
                let b = pts[i + 1];
                let mid = (a + b) * 0.5;
                let along = (b - a).length();
                let l = left(tg[i]);
                let y = (h[i] + h[i + 1]) * 0.5;
                let x_axis = Vec3::new(tg[i].x, 0.0, -tg[i].y) * (along * 0.5 + 0.02);
                let z_axis = Vec3::new(l.x, 0.0, -l.y);
                // the deck slab under the road surface
                bridge.oriented_box(Vec3::new(mid.x, y - 0.35, -mid.y), x_axis, Vec3::Y * 0.36, z_axis * (hw + 0.45));
                for s in [-1.0f32, 1.0] {
                    let c = mid + l * s * (hw + 0.25);
                    bridge.oriented_box(Vec3::new(c.x, y + 0.45, -c.y), x_axis, Vec3::Y * 0.5, z_axis * 0.18);
                }
            }
        }
    }
    meshes.extend(asph.finish("roads", asphalt, true));
    meshes.extend(grav.finish("farm roads", gravel, true));
    meshes.extend(lines.finish("road lines", paint, false));
    meshes.extend(bridge.finish("bridges", concrete, true));

    // ---- the river surface
    let mut wm = Mesh::default();
    let wl: Vec<f32> = plan.river.water.clone();
    ribbon(&mut wm, &river, &wl, -(plan.river.water_half + 0.6), plan.river.water_half + 0.6, 0.0, 8.0, |_| true);
    meshes.extend(wm.finish("river", water, false));

    // ---- the railway: two rails on timber sleepers, on the ballast
    // the track's level: the ballast top the terrain export recorded (older plans: the ground)
    let rail: Vec<Vec2> = plan.rail.iter().map(|p| Vec2::from(*p)).collect();
    let rail_h: Vec<f32> = if plan.rail_h.len() == rail.len() { plan.rail_h.clone() } else { rail.iter().map(|p| hf.plan(p.x, p.y)).collect() };
    let (rp, rh) = densify(&rail, &rail_h, 0.6);
    // level crossings: where a road (not a field track) runs over the track
    let crossing: Vec<bool> = rp
        .iter()
        .map(|p| {
            plan.roads.iter().filter(|r| !matches!(r.kind.as_str(), "farm" | "levee")).any(|r| {
                r.pts.windows(2).any(|w| {
                    let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
                    let t = ((*p - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0);
                    (a + (b - a) * t - *p).length() < r.width / 2.0 + 0.9
                })
            })
        })
        .collect();
    let mut rails = Mesh::default();
    let mut sl = Mesh::default();
    let mut panels = Mesh::default();
    let tg = tangents(&rp);
    // rails: 0.07 m heads, 0.16 m tall, on the sleepers; the ballast is packed up to the sleepers'
    // tops (they show 0.08 m above it)
    for g in [-0.5335f32, 0.5335] {
        rails.bar(&rp, &rh, g - 0.035, g + 0.035, 0.08, 0.24, |i| near_river(rp[i]) > 0.0);
    }
    for (i, p) in rp.iter().enumerate() {
        if crossing[i] {
            continue; // under the crossing's panels
        }
        let x = Vec3::new(tg[i].x, 0.0, -tg[i].y) * 0.12;
        let z = Vec3::new(-tg[i].y, 0.0, -tg[i].x) * 1.0;
        sl.oriented_box(Vec3::new(p.x, rh[i] - 0.03, -p.y), x, Vec3::Y * 0.11, z); // bedded into the ballast
    }
    // the crossing: concrete panels between and beside the rails, flush with the rail heads,
    // a flangeway either side of each rail for the wheels
    for (a, b) in [(-1.5f32, -0.6), (-0.47, 0.47), (0.6, 1.5)] {
        panels.bar(&rp, &rh, a, b, -0.05, 0.242, |i| crossing[i]);
    }
    meshes.extend(rails.finish("rails", steel, false));
    meshes.extend(sl.finish("sleepers", sleeper, true));
    let panel_mat = surface_material(r, "Concrete_Floor-f4b85cb0", 0.8)?;
    meshes.extend(panels.finish("crossing panels", panel_mat, true));

    // ---- paddy levees (aze): a low rounded earth ridge round every paddy, grassed over, its foot
    // sunk into the paddy (and into the ground on the other side)
    let levee = surface_material(r, "Uncut_Grass", 0.85)?;
    let mut az = Mesh::default();
    // the cross-section: (offset across m, height above the paddy m), crown 0.25 high, 0.9 wide
    let prof: Vec<(f32, f32)> = (0..=8)
        .map(|k| {
            let u = k as f32 / 8.0 * 2.0 - 1.0; // -1 .. 1
            (u * 0.45, 0.25 * (std::f32::consts::FRAC_PI_2 * u).cos().max(0.0).powf(0.8) - 0.06)
        })
        .collect();
    let lu = LandUse::load(town)?;
    for (q, lvl) in plan.paddies.iter().zip(&plan.paddy_levels) {
        for (a, b) in levee_spans(&lu, q) {
            let t = (b - a).normalize_or(Vec2::X);
            let l = left(t);
            let s = (b - a).length();
            let mut prev: Option<(u32, u32)> = None;
            for w in 0..prof.len() {
                let (o, y) = prof[w];
                // the profile's slope here, for the normal
                let (o0, y0) = prof[w.saturating_sub(1)];
                let (o1, y1) = prof[(w + 1).min(prof.len() - 1)];
                let slope = (y1 - y0) / (o1 - o0);
                let nn = (Vec3::new(l.x, 0.0, -l.y) * -slope + Vec3::Y).normalize();
                let (pa, pb) = (a + l * o, b + l * o);
                let va = az.vert(Vec3::new(pa.x, lvl + y, -pa.y), nn, [o, 0.0]);
                let vb = az.vert(Vec3::new(pb.x, lvl + y, -pb.y), nn, [o, s]);
                if let Some((qa, qb)) = prev {
                    az.quad(qa, va, vb, qb);
                }
                prev = Some((va, vb));
            }
        }
    }
    meshes.extend(az.finish("paddy levees", levee, true));

    let ids = r.add_meshes(&meshes)?;
    let instances = ids.into_iter().map(|mesh| Instance { mesh, transform: Affine3A::IDENTITY }).collect::<Vec<_>>();
    let triangles = meshes.iter().map(|m| m.indices.len() / 3).sum::<usize>();
    println!("town: {} meshes, {:.1}M triangles, built in {:.1}s", instances.len(), triangles as f64 / 1e6, t0.elapsed().as_secs_f32());
    Ok(TownScene { instances, triangles })
}

/// The land-use map (tools/blender/town_terrain.py): one code a metre, row 0 the north edge.
pub struct LandUse {
    pub codes: Vec<u8>,
    pub nx: usize,
    pub ny: usize,
    pub x0: f32,
    pub y1: f32,
}

impl LandUse {
    pub fn load(town: &Town) -> Result<Self> {
        let t = &town.plan.terrain;
        Ok(LandUse { codes: load_landuse(&town.dir.join(&t.landuse))?, nx: t.nx, ny: t.ny, x0: t.origin[0], y1: t.origin[1] + (t.ny - 1) as f32 })
    }

    /// The code at a plan point (255 off the map). (The row covering plan y is y1 + 1 - y.)
    pub fn at(&self, p: Vec2) -> u8 {
        let (i, j) = ((p.x - self.x0).floor(), (self.y1 + 1.0 - p.y).floor());
        if i < 0.0 || j < 0.0 || i as usize >= self.nx || j as usize >= self.ny {
            return 255;
        }
        self.codes[j as usize * self.nx + i as usize]
    }
}

/// Where along a paddy's edges its levee (aze) runs: only over the fields themselves (paddy,
/// vegetable plots, grass between them), never across a road, a plot, a site or the river; a
/// little past the corners so the ridges meet. The pieces, as (start, end) plan points.
pub fn levee_spans(lu: &LandUse, paddy: &[[f32; 2]]) -> Vec<(Vec2, Vec2)> {
    let field = |c: u8| matches!(c, 0 | 1 | 2);
    let mut out = Vec::new();
    let n = paddy.len();
    for k in 0..n {
        let (a, b) = (Vec2::from(paddy[k]), Vec2::from(paddy[(k + 1) % n]));
        let t = (b - a).normalize_or(Vec2::X);
        let (a, b) = (a - t * 0.4, b + t * 0.4);
        let len = (b - a).length();
        let steps = ((len / 0.5).ceil() as usize).max(1);
        let mut start: Option<usize> = None;
        for i in 0..=steps {
            // (a step is kept when the ground under it and a ridge's half width either side is
            // field)
            let ok = i < steps && {
                let m = a.lerp(b, (i as f32 + 0.5) / steps as f32);
                let l = Vec2::new(-t.y, t.x) * 0.45;
                field(lu.at(m)) && field(lu.at(m + l)) && field(lu.at(m - l))
            };
            match (ok, start) {
                (true, None) => start = Some(i),
                (false, Some(s0)) => {
                    let (p0, p1) = (a.lerp(b, s0 as f32 / steps as f32), a.lerp(b, i as f32 / steps as f32));
                    if (p1 - p0).length() > 0.9 {
                        out.push((p0, p1));
                    }
                    start = None;
                }
                _ => {}
            }
        }
    }
    out
}

/// Resample a polyline (and its heights) to vertices at most `step` apart.
fn densify(pts: &[Vec2], h: &[f32], step: f32) -> (Vec<Vec2>, Vec<f32>) {
    let mut p = vec![pts[0]];
    let mut hh = vec![h[0]];
    for i in 1..pts.len() {
        let (a, b) = (pts[i - 1], pts[i]);
        let n = ((b - a).length() / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = k as f32 / n as f32;
            p.push(a.lerp(b, t));
            hh.push(h[i - 1] + (h[i] - h[i - 1]) * t);
        }
    }
    (p, hh)
}

/// The land-use codes (row 0 north), as tools/blender/town_terrain.py wrote them.
pub fn load_landuse(path: &Path) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    let (_, _, rgba) = akari::textures::decode_png(&bytes)?;
    Ok(rgba.chunks(4).map(|c| c[0]).collect())
}
