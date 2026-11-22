//! glTF 2.0 loading (.gltf + .bin + PNG textures, or .glb): Fab / Megascans downloads and the
//! levels exported from Blender. Each primitive becomes one renderer mesh with its material; the
//! node hierarchy becomes a list of (mesh, transform) parts.

use crate::renderer::Renderer;
use crate::scene::*;
use crate::textures::{file_key, TexJob, TexKind, TexSource};
use anyhow::{anyhow, bail, Context, Result};
use glam::{Affine3A, Mat4, Vec3};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One mesh of a model, placed by its node.
#[derive(Clone, Debug)]
pub struct Part {
    pub mesh: MeshId,
    pub transform: Affine3A,
    /// The glTF node's name (Fab variants: "SM_xxx_VarA", ...).
    pub node: String,
    pub material: MaterialId,
    /// Bounds in model space.
    pub min: Vec3,
    pub max: Vec3,
    /// The mesh's triangles in its own space (CPU copy, for collision: place with `transform`).
    pub triangles: std::sync::Arc<Vec<[Vec3; 3]>>,
    /// Cloth (`kzcloth_*` nodes): the whole mesh, CPU copy, for the game to simulate and draw
    /// as a deformable mesh (`Renderer::add_deformable_mesh`).
    pub cloth: Option<std::sync::Arc<MeshData>>,
}

/// A loaded model: its parts relative to the model's origin. LOD1+ nodes are skipped.
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub name: String,
    pub parts: Vec<Part>,
    /// Axis-aligned bounds of the whole model (model space).
    pub min: Vec3,
    pub max: Vec3,
    /// The model's materials with their glTF names (the game drives emissive ones by name).
    pub materials: Vec<(MaterialId, String, Material)>,
}

impl Model {
    /// The model placed at `at` (one instance per part).
    pub fn instances(&self, at: Affine3A) -> impl Iterator<Item = Instance> + '_ {
        self.parts.iter().map(move |p| Instance { mesh: p.mesh, transform: at * p.transform })
    }

    /// Only the parts whose node name contains `key` (e.g. one Fab variant: "VarA").
    pub fn variant(&self, key: &str) -> Model {
        let parts: Vec<Part> = self.parts.iter().filter(|p| p.node.contains(key)).cloned().collect();
        let (min, max) = bounds(&parts);
        Model { name: format!("{}:{key}", self.name), parts, min, max, materials: self.materials.clone() }
    }

    /// Each part as a model of its own (Fab packs put several variants side by side at the origin).
    pub fn split(&self) -> Vec<Model> {
        self.parts.iter().map(|p| Model { name: format!("{}:{}", self.name, p.node), parts: vec![p.clone()], min: p.min, max: p.max, materials: self.materials.clone() }).collect()
    }

    /// Names of the variants (distinct node names).
    pub fn node_names(&self) -> Vec<&str> {
        self.parts.iter().map(|p| p.node.as_str()).collect()
    }
}

fn bounds(parts: &[Part]) -> (Vec3, Vec3) {
    parts.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(p.min), b.max(p.max)))
}

/// (base name, LOD index) of a Fab-style node name ("SM_x_LOD2_TIER2_000" -> ("SM_x", 2)).
pub fn lod_of(name: &str) -> (&str, u32) {
    match name.find("_LOD") {
        Some(i) => {
            let digits: String = name[i + 4..].chars().take_while(|c| c.is_ascii_digit()).collect();
            (&name[..i], digits.parse().unwrap_or(0))
        }
        None => (name, 0),
    }
}

/// Keep only the most detailed LOD present for each base name. Some Fab tiers ship LOD0, others
/// only a reduced LOD (e.g. just "_LOD2"): the lowest index present is the one to use.
pub fn best_lods<'a>(names: impl Iterator<Item = &'a str>) -> std::collections::HashMap<String, u32> {
    let mut best: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for n in names {
        let (base, lod) = lod_of(n);
        let e = best.entry(base.to_string()).or_insert(lod);
        *e = (*e).min(lod);
    }
    best
}


fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Options for interpreting a model's materials.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoadOptions {
    /// Treat alpha-masked double-sided materials as thin translucent foliage.
    pub foliage: bool,
}

/// A glTF's materials (and their textures) added to the renderer: each material's id, in the
/// file's order, with its name. For models loaded elsewhere (skinned characters: crate anim).
pub fn load_materials(r: &mut Renderer, path: &Path, opts: LoadOptions) -> Result<Vec<(MaterialId, String, Material)>> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let gltf = gltf::Gltf::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let (_, _, named, _) = materials(r, path, &gltf, base, &name, opts)?;
    Ok(named)
}

#[allow(clippy::type_complexity)]
fn materials(r: &mut Renderer, path: &Path, gltf: &gltf::Gltf, base: &Path, name: &str, opts: LoadOptions) -> Result<(Vec<Vec<u8>>, Vec<MaterialId>, Vec<(MaterialId, String, Material)>, usize)> {
    // Buffers.
    let mut buffers: Vec<Vec<u8>> = Vec::new();
    for b in gltf.buffers() {
        let data = match b.source() {
            gltf::buffer::Source::Bin => gltf.blob.clone().ok_or_else(|| anyhow!("{}: GLB has no binary chunk", path.display()))?,
            gltf::buffer::Source::Uri(uri) => {
                if uri.starts_with("data:") {
                    bail!("{}: embedded data URIs are not supported (export as .glb or .gltf + .bin)", path.display());
                }
                let p = base.join(percent_decode(uri));
                std::fs::read(&p).with_context(|| format!("reading {}", p.display()))?
            }
        };
        buffers.push(data);
    }

    // Which images are used as what (decides the GPU format).
    let mut kinds: HashMap<usize, TexKind> = HashMap::new();
    for m in gltf.materials() {
        let pbr = m.pbr_metallic_roughness();
        let mut mark = |t: Option<gltf::texture::Texture>, k: TexKind| {
            if let Some(t) = t {
                kinds.entry(t.source().index()).or_insert(k);
            }
        };
        mark(pbr.base_color_texture().map(|i| i.texture()), TexKind::Color);
        mark(m.emissive_texture().map(|i| i.texture()), TexKind::Color);
        mark(m.normal_texture().map(|i| i.texture()), TexKind::Normal);
        mark(pbr.metallic_roughness_texture().map(|i| i.texture()), TexKind::Data);
        mark(m.occlusion_texture().map(|i| i.texture()), TexKind::Data);
    }
    let images: Vec<gltf::Image> = gltf.images().collect();
    let mut jobs = Vec::new();
    let mut job_of_image: HashMap<usize, usize> = HashMap::new();
    let mut sorted: Vec<_> = kinds.iter().map(|(i, k)| (*i, *k)).collect();
    sorted.sort_by_key(|x| x.0);
    for (img_idx, kind) in sorted {
        let img = &images[img_idx];
        let label = img.name().map(str::to_string).unwrap_or_else(|| format!("{name} image {img_idx}"));
        let (source, key, mime) = match img.source() {
            gltf::image::Source::Uri { uri, mime_type } => {
                let p: PathBuf = base.join(percent_decode(uri));
                let key = file_key(&p);
                (TexSource::File(p.clone()), key, mime_type.map(str::to_string).unwrap_or_else(|| p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()))
            }
            gltf::image::Source::View { view, mime_type } => {
                let b = &buffers[view.buffer().index()][view.offset()..view.offset() + view.length()];
                let key = format!("{}#{}#{}", file_key(path), img_idx, b.len());
                (TexSource::Bytes(b.to_vec()), key, mime_type.to_string())
            }
        };
        if !(mime.contains("png")) {
            eprintln!("warning: {label}: only PNG textures are supported (got {mime}); using the material factor");
            continue;
        }
        job_of_image.insert(img_idx, jobs.len());
        jobs.push(TexJob { key, name: label, kind, source });
    }
    let tex_ids = r.add_textures(&jobs)?;
    let tex = |t: Option<gltf::texture::Texture>| -> Option<TextureId> { t.and_then(|t| job_of_image.get(&t.source().index()).map(|&j| tex_ids[j])) };

    // Materials.
    let mut mat_ids = Vec::new();
    let mut named = Vec::new();
    for m in gltf.materials() {
        let pbr = m.pbr_metallic_roughness();
        let mut mat = Material {
            base_color: pbr.base_color_factor(),
            emissive: m.emissive_factor(),
            roughness: pbr.roughness_factor(),
            metallic: pbr.metallic_factor(),
            alpha_cutoff: m.alpha_cutoff().unwrap_or(0.5),
            normal_scale: m.normal_texture().map_or(1.0, |n| n.scale()),
            occlusion_strength: m.occlusion_texture().map_or(1.0, |o| o.strength()),
            base_tex: tex(pbr.base_color_texture().map(|i| i.texture())),
            normal_tex: tex(m.normal_texture().map(|i| i.texture())),
            mr_tex: tex(pbr.metallic_roughness_texture().map(|i| i.texture())),
            occlusion_tex: tex(m.occlusion_texture().map(|i| i.texture())),
            emissive_tex: tex(m.emissive_texture().map(|i| i.texture())),
            flags: 0,
            transmission: 0.0,
        };
        if m.alpha_mode() != gltf::material::AlphaMode::Opaque {
            mat.flags |= MAT_ALPHA_MASK;
        }
        if m.double_sided() {
            mat.flags |= MAT_DOUBLE_SIDED;
        }
        if m.name().is_some_and(|n| n.contains("clear_glass")) {
            mat.flags |= crate::scene::MAT_GLASS;
        }
        if opts.foliage && mat.flags & MAT_ALPHA_MASK != 0 {
            mat.flags |= MAT_FOLIAGE;
            mat.transmission = 0.35;
        }
        let id = r.add_material(mat);
        named.push((id, m.name().unwrap_or("").to_string(), mat));
        mat_ids.push(id);
    }
    Ok((buffers, mat_ids, named, jobs.len()))
}

pub fn load(r: &mut Renderer, path: &Path, opts: LoadOptions) -> Result<Model> {
    let t0 = std::time::Instant::now();
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let gltf = gltf::Gltf::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();

    let (buffers, mat_ids, named, njobs) = materials(r, path, &gltf, base, &name, opts)?;
    let default_mat = r.add_material(Material::default());

    // Meshes: one renderer mesh per primitive.
    let mut datas = Vec::new();
    let mut prim_slots: Vec<Vec<usize>> = Vec::new(); // gltf mesh -> indices into datas
    let mut local_bounds: Vec<(Vec3, Vec3)> = Vec::new();
    for mesh in gltf.meshes() {
        let mut slots = Vec::new();
        for (pi, prim) in mesh.primitives().enumerate() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = prim.reader(|b| Some(&buffers[b.index()][..]));
            let Some(pos) = reader.read_positions() else { continue };
            let positions: Vec<[f32; 3]> = pos.collect();
            let (lo, hi) = positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(Vec3::from(*p)), b.max(Vec3::from(*p))));
            let mut d = MeshData {
                name: format!("{name}/{}#{pi}", mesh.name().unwrap_or("mesh")),
                normals: reader.read_normals().map(|n| n.collect()).unwrap_or_default(),
                uvs: reader.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_default(),
                tangents: reader.read_tangents().map(|t| t.collect()).unwrap_or_default(),
                colors: reader.read_colors(0).map(|c| c.into_rgb_f32().collect()).unwrap_or_default(),
                indices: match reader.read_indices() {
                    Some(i) => i.into_u32().collect(),
                    None => (0..positions.len() as u32).collect(),
                },
                material: Some(prim.material().index().map_or(default_mat, |i| mat_ids[i])),
                positions,
            };
            if d.normals.len() != d.positions.len() {
                d.generate_normals();
            }
            let has_normal_map = prim.material().normal_texture().is_some();
            if has_normal_map && d.tangents.len() != d.positions.len() {
                d.generate_tangents();
            }
            if d.colors.len() != d.positions.len() {
                d.colors.clear();
            }
            slots.push(datas.len());
            local_bounds.push((lo, hi));
            datas.push(d);
        }
        prim_slots.push(slots);
    }
    let mesh_ids = r.add_meshes(&datas)?;

    // Node hierarchy.
    struct Ctx<'a> {
        prim_slots: &'a [Vec<usize>],
        mesh_ids: &'a [MeshId],
        datas: &'a [MeshData],
        local_bounds: &'a [(Vec3, Vec3)],
        tris: Vec<std::sync::Arc<Vec<[Vec3; 3]>>>,
        best: std::collections::HashMap<String, u32>,
    }
    fn place(cx: &Ctx, slot: usize, m: Affine3A, node: &str) -> Part {
        let (lo, hi) = cx.local_bounds[slot];
        let corners = (0..8).map(|k| m.transform_point3(Vec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z })));
        let (min, max) = corners.fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), c| (a.min(c), b.max(c)));
        let cloth = node.starts_with("kzcloth_").then(|| std::sync::Arc::new(cx.datas[slot].clone()));
        Part { mesh: cx.mesh_ids[slot], transform: m, node: node.to_string(), material: cx.datas[slot].material.unwrap(), min, max, triangles: cx.tris[slot].clone(), cloth }
    }
    fn walk(cx: &Ctx, node: gltf::Node, parent: Mat4, parts: &mut Vec<Part>) {
        let name = node.name().unwrap_or("");
        let (base, lod) = lod_of(name);
        if cx.best.get(base).is_some_and(|&b| lod > b) {
            return;
        }
        let m = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        if let Some(mesh) = node.mesh() {
            for &slot in &cx.prim_slots[mesh.index()] {
                parts.push(place(cx, slot, Affine3A::from_mat4(m), name));
            }
        }
        for c in node.children() {
            walk(cx, c, m, parts);
        }
    }
    let tris = datas
        .iter()
        .map(|d| {
            let p = |i: u32| Vec3::from(d.positions[i as usize]);
            std::sync::Arc::new(d.indices.chunks_exact(3).map(|t| [p(t[0]), p(t[1]), p(t[2])]).collect::<Vec<_>>())
        })
        .collect();
    let cx = Ctx { prim_slots: &prim_slots, mesh_ids: &mesh_ids, datas: &datas, local_bounds: &local_bounds, tris, best: best_lods(gltf.nodes().map(|n| n.name().unwrap_or(""))) };
    let mut parts = Vec::new();
    match gltf.default_scene().or_else(|| gltf.scenes().next()) {
        Some(s) => {
            for n in s.nodes() {
                walk(&cx, n, Mat4::IDENTITY, &mut parts);
            }
        }
        None => {
            for slots in &prim_slots {
                for &s in slots {
                    parts.push(place(&cx, s, Affine3A::IDENTITY, ""));
                }
            }
        }
    }
    let (lo, hi) = bounds(&parts);
    let tris: usize = datas.iter().map(|d| d.indices.len() / 3).sum();
    println!("Loaded {name}: {} parts, {} triangles, {} textures in {:.2}s", parts.len(), tris, njobs, t0.elapsed().as_secs_f32());
    Ok(Model { name, parts, min: lo, max: hi, materials: named })
}
