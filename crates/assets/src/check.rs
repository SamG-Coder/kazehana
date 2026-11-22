//! Automated asset checks (docs/ASSET_QUALITY.md, "Automated checks"). CPU only: parses the
//! glTF, walks the LOD0 nodes, measures scale, origin, triangles, attributes, materials, textures
//! and texel density.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Prop,
    Foliage,
    Building,
    Character,
    Surface,
    /// A level's own geometry (level.glb from tools/blender).
    Level,
}

impl Category {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "prop" => Self::Prop,
            "foliage" => Self::Foliage,
            "building" => Self::Building,
            "character" => Self::Character,
            "surface" => Self::Surface,
            "level" => Self::Level,
            _ => bail!("unknown category '{s}' (prop, foliage, building, character, surface, level)"),
        })
    }
    /// Allowed size (m): the largest dimension (height for characters).
    pub fn size_range(self) -> (f32, f32) {
        match self {
            Self::Prop => (0.03, 12.0),
            Self::Foliage => (0.05, 40.0),
            Self::Building => (2.0, 80.0),
            Self::Character => (1.2, 2.0),
            Self::Surface => (0.5, 8.0),
            Self::Level => (10.0, 3000.0),
        }
    }
    pub fn triangle_budget(self) -> usize {
        match self {
            Self::Prop => 60_000,
            Self::Foliage => 80_000,
            Self::Building => 400_000,
            Self::Character => 100_000,
            Self::Surface => 50_000,
            Self::Level => 4_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
    Pass,
    Warn,
    Fail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub level: Level,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PartStats {
    pub node: String,
    pub triangles: usize,
    pub size: [f32; 3],
    pub min_y: f32,
    pub max_y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub id: String,
    pub source: String,
    pub category: Category,
    pub hash: String,
    pub passed: bool,
    pub findings: Vec<Finding>,
    pub parts: Vec<PartStats>,
    pub textures: Vec<(String, u32, u32)>,
    /// Base-colour texels per metre (area weighted over LOD0).
    pub texel_density: f32,
    pub checked_at: u64,
}

impl Report {
    pub fn print(&self) {
        println!("Asset '{}' ({:?}) — {}", self.id, self.category, if self.passed { "PASSED automated checks" } else { "FAILED" });
        println!("  source: {}", self.source);
        println!("  hash:   {}", self.hash);
        for p in &self.parts {
            println!("  part {:<40} {:>7} tris  size {:.2} x {:.2} x {:.2} m  y {:.2}..{:.2}", p.node, p.triangles, p.size[0], p.size[1], p.size[2], p.min_y, p.max_y);
        }
        for (n, w, h) in &self.textures {
            println!("  texture {n} {w}x{h}");
        }
        println!("  texel density: {:.0} px/m", self.texel_density);
        for f in &self.findings {
            if f.level != Level::Pass {
                println!("  [{:?}] {}: {}", f.level, f.check, f.message);
            }
        }
        let n = |l| self.findings.iter().filter(|f| f.level == l).count();
        println!("  {} pass, {} warn, {} fail", n(Level::Pass), n(Level::Warn), n(Level::Fail));
    }
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


/// Every file a glTF asset consists of (the .gltf/.glb itself, external buffers and images).
pub fn asset_files(path: &Path) -> Result<Vec<PathBuf>> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let gltf = gltf::Gltf::from_slice(&bytes)?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut files = BTreeSet::new();
    files.insert(path.to_path_buf());
    for b in gltf.buffers() {
        if let gltf::buffer::Source::Uri(u) = b.source() {
            files.insert(base.join(percent_decode(u)));
        }
    }
    for i in gltf.images() {
        if let gltf::image::Source::Uri { uri, .. } = i.source() {
            files.insert(base.join(percent_decode(uri)));
        }
    }
    Ok(files.into_iter().collect())
}

/// FNV-1a 64 over the content of all the asset's files (in a fixed order).
pub fn hash_files(files: &[PathBuf]) -> Result<String> {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for f in files {
        let b = std::fs::read(f).with_context(|| format!("reading {}", f.display()))?;
        for chunk in [f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().as_bytes(), &b[..]] {
            for &x in chunk {
                h = (h ^ x as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    Ok(format!("{h:016x}"))
}

/// Cheap fingerprint (sizes and modification times) to skip re-hashing unchanged files.
pub fn fingerprint(files: &[PathBuf]) -> String {
    let mut s = String::new();
    for f in files {
        let m = std::fs::metadata(f).ok();
        let len = m.as_ref().map_or(0, |m| m.len());
        let t = m.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        s += &format!("{}:{len}:{t};", f.display());
    }
    s
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let r = dec.read_info().ok()?;
    Some((r.info().width, r.info().height))
}

pub fn check(path: &Path, id: &str, category: Category) -> Result<Report> {
    let mut findings = Vec::new();
    let mut add = |check: &str, level: Level, message: String| findings.push(Finding { check: check.into(), level, message });

    let files = asset_files(path)?;
    let missing: Vec<_> = files.iter().filter(|f| !f.exists()).collect();
    if !missing.is_empty() {
        add("parse", Level::Fail, format!("missing files: {missing:?}"));
    }
    let hash = if missing.is_empty() { hash_files(&files)? } else { "missing".into() };
    let bytes = std::fs::read(path)?;
    let gltf = gltf::Gltf::from_slice(&bytes).map_err(|e| anyhow!("invalid glTF: {e}"))?;
    add("parse", Level::Pass, "glTF parses".into());
    let base = path.parent().unwrap_or(Path::new("."));
    let mut buffers = Vec::new();
    for b in gltf.buffers() {
        buffers.push(match b.source() {
            gltf::buffer::Source::Bin => gltf.blob.clone().unwrap_or_default(),
            gltf::buffer::Source::Uri(u) => std::fs::read(base.join(percent_decode(u))).unwrap_or_default(),
        });
    }

    // Textures.
    let mut textures = Vec::new();
    let mut tex_sizes: Vec<Option<(u32, u32)>> = Vec::new();
    for img in gltf.images() {
        let name = img.name().unwrap_or("image").to_string();
        let (data, mime) = match img.source() {
            gltf::image::Source::Uri { uri, mime_type } => {
                let p = base.join(percent_decode(uri));
                (std::fs::read(&p).unwrap_or_default(), mime_type.map(str::to_string).unwrap_or_else(|| p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()))
            }
            gltf::image::Source::View { view, mime_type } => {
                let b = &buffers[view.buffer().index()];
                (b.get(view.offset()..view.offset() + view.length()).unwrap_or_default().to_vec(), mime_type.to_string())
            }
        };
        if !mime.contains("png") {
            add("textures", Level::Fail, format!("{name}: not PNG ({mime})"));
            tex_sizes.push(None);
            continue;
        }
        match png_size(&data) {
            Some((w, h)) => {
                if w.min(h) < 1024 {
                    add("textures", Level::Fail, format!("{name}: {w}x{h} is below 1024 px"));
                } else if w.min(h) < 2048 {
                    add("textures", Level::Warn, format!("{name}: {w}x{h} is below 2048 px"));
                }
                if !w.is_power_of_two() || !h.is_power_of_two() {
                    add("textures", Level::Warn, format!("{name}: {w}x{h} is not a power of two"));
                }
                textures.push((name, w, h));
                tex_sizes.push(Some((w, h)));
            }
            None => {
                add("textures", Level::Fail, format!("{name}: unreadable PNG"));
                tex_sizes.push(None);
            }
        }
    }

    // Materials.
    let surface = matches!(category, Category::Surface | Category::Level);
    let mut has_alpha = false;
    for m in gltf.materials() {
        let name = m.name().unwrap_or("material");
        if name.contains("Billboard") {
            continue; // far-LOD impostor cards
        }
        let pbr = m.pbr_metallic_roughness();
        // Glass, water and emissive panels (signs, lamps, lit windows) are flat by nature.
        let flat_ok = name == "water" || name.contains("glass") || name.starts_with("kz_glow") || name.starts_with("kz_neon") || name.starts_with("kz_lamp");
        if pbr.base_color_texture().is_none() && !surface && !flat_ok {
            add("base colour", Level::Fail, format!("{name}: no base-colour texture"));
        }
        if m.normal_texture().is_none() && !flat_ok {
            let lvl = if matches!(category, Category::Foliage) { Level::Warn } else { Level::Fail };
            add("normal map", lvl, format!("{name}: no normal map"));
        }
        if pbr.metallic_roughness_texture().is_none() {
            add("orm", Level::Warn, format!("{name}: no metallic-roughness / ORM texture (factors only)"));
        }
        match m.alpha_mode() {
            gltf::material::AlphaMode::Mask => has_alpha = true,
            gltf::material::AlphaMode::Blend => {
                has_alpha = true;
                add("foliage alpha", Level::Warn, format!("{name}: BLEND (rendered as alpha-tested MASK)"));
            }
            _ => {}
        }
    }
    if category == Category::Foliage && !has_alpha {
        add("foliage alpha", Level::Fail, "no alpha-masked material: leaves will render as opaque cards".into());
    }

    // Geometry, per LOD0 part (after node transforms).
    let mut parts = Vec::new();
    let (mut uv_area_tex, mut world_area) = (0.0f64, 0.0f64);
    let (mut degenerate, mut total_tris, mut bad_numbers) = (0usize, 0usize, false);
    let best = best_lods(gltf.nodes().map(|n| n.name().unwrap_or("")));
    fn walk<'a>(n: gltf::Node<'a>, parent: [[f32; 4]; 4], best: &std::collections::HashMap<String, u32>, out: &mut Vec<(gltf::Node<'a>, [[f32; 4]; 4])>) {
        let (base, lod) = lod_of(n.name().unwrap_or(""));
        if best.get(base).is_some_and(|&b| lod > b) {
            return;
        }
        let m = mat_mul(parent, n.transform().matrix());
        out.push((n.clone(), m));
        for c in n.children() {
            walk(c, m, best, out);
        }
    }
    let mut nodes = Vec::new();
    if let Some(s) = gltf.default_scene().or_else(|| gltf.scenes().next()) {
        for n in s.nodes() {
            walk(n, IDENTITY, &best, &mut nodes);
        }
    }
    for (node, m) in nodes {
        let Some(mesh) = node.mesh() else { continue };
        let name = node.name().unwrap_or("node").to_string();
        if name.contains("Billboard") {
            continue;
        }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        let mut tris = 0usize;
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| buffers.get(b.index()).map(|v| &v[..]));
            let attrs: Vec<String> = prim.attributes().map(|(s, _)| format!("{s:?}")).collect();
            let has = |k: &str| attrs.iter().any(|a| a.starts_with(k));
            if !has("Positions") {
                add("attributes", Level::Fail, format!("{name}: no POSITION"));
                continue;
            }
            if !has("TexCoords(0)") {
                add("attributes", Level::Fail, format!("{name}: no TEXCOORD_0"));
            }
            if !has("Normals") {
                add("attributes", Level::Warn, format!("{name}: no NORMAL (generated at load)"));
            }
            if prim.material().normal_texture().is_some() && !has("Tangents") {
                add("attributes", Level::Warn, format!("{name}: normal map but no TANGENT (generated at load)"));
            }
            let pos: Vec<[f32; 3]> = reader.read_positions().map(|p| p.map(|v| transform(m, v)).collect()).unwrap_or_default();
            let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_default();
            let idx: Vec<u32> = reader.read_indices().map(|i| i.into_u32().collect()).unwrap_or_else(|| (0..pos.len() as u32).collect());
            bad_numbers |= pos.iter().flatten().any(|v| !v.is_finite()) || uvs.iter().flatten().any(|v| !v.is_finite());
            for p in &pos {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let base_tex = prim.material().pbr_metallic_roughness().base_color_texture().and_then(|t| tex_sizes.get(t.texture().source().index()).copied().flatten());
            for t in idx.chunks_exact(3) {
                tris += 1;
                let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
                if a >= pos.len() || b >= pos.len() || c >= pos.len() {
                    degenerate += 1;
                    continue;
                }
                let e1 = sub(pos[b], pos[a]);
                let e2 = sub(pos[c], pos[a]);
                let area = 0.5 * len(cross(e1, e2)) as f64;
                if area < 1e-10 {
                    degenerate += 1;
                    continue;
                }
                // distant scenery ("far_*" nodes) is judged by its look-dev, not texel density
                if let (Some((tw, th)), true, false) = (base_tex, uvs.len() == pos.len(), name.starts_with("far_")) {
                    let (u0, u1, u2) = (uvs[a], uvs[b], uvs[c]);
                    let uva = 0.5 * ((u1[0] - u0[0]) * (u2[1] - u0[1]) - (u2[0] - u0[0]) * (u1[1] - u0[1])).abs() as f64;
                    uv_area_tex += uva * tw as f64 * th as f64;
                    world_area += area;
                }
            }
        }
        total_tris += tris;
        if tris == 0 {
            continue;
        }
        parts.push(PartStats { node: name, triangles: tris, size: [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]], min_y: lo[1], max_y: hi[1] });
    }
    if parts.is_empty() {
        add("geometry", Level::Fail, "no LOD0 mesh nodes".into());
    }
    if bad_numbers {
        add("numbers", Level::Fail, "NaN or infinite positions / UVs".into());
    }
    let degen_frac = degenerate as f32 / total_tris.max(1) as f32;
    if degen_frac >= 0.01 {
        add("degenerate", Level::Fail, format!("{:.1}% degenerate triangles", degen_frac * 100.0));
    } else if degenerate > 0 {
        add("degenerate", Level::Warn, format!("{degenerate} degenerate triangles"));
    }

    let (smin, smax) = category.size_range();
    let budget = category.triangle_budget();
    // A level or a building is judged as a whole: its parts are individual walls, kerbs and roofs.
    let whole;
    let judged: &[PartStats] = if matches!(category, Category::Level | Category::Building) {
        let (lo_y, hi_y) = parts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.min_y), b.max(p.max_y)));
        let size = [
            parts.iter().map(|p| p.size[0]).fold(0.0, f32::max),
            hi_y - lo_y,
            parts.iter().map(|p| p.size[2]).fold(0.0, f32::max),
        ];
        whole = [PartStats { node: "level".into(), triangles: total_tris, size, min_y: lo_y, max_y: hi_y }];
        &whole
    } else {
        &parts
    };
    for p in judged {
        let measure = if category == Category::Character { p.size[1] } else { p.size.iter().cloned().fold(0.0, f32::max) };
        if measure < smin || measure > smax {
            add("scale", Level::Fail, format!("{}: {:.2} m is outside {smin}-{smax} m (wrong units or axis?)", p.node, measure));
        } else if measure < smin * 1.1 || measure > smax * 0.9 {
            add("scale", Level::Warn, format!("{}: {:.2} m is near the {smin}-{smax} m limit", p.node, measure));
        }
        if !surface {
            let h = p.size[1].max(0.01);
            if p.min_y > 0.05 || p.min_y < -(0.15 * h).max(0.03) {
                add("origin", Level::Fail, format!("{}: bottom at y = {:.2} m (should sit on y = 0)", p.node, p.min_y));
            }
        }
        if p.triangles > budget {
            add("triangles", Level::Fail, format!("{}: {} triangles, budget {budget}", p.node, p.triangles));
        } else if p.triangles * 4 > budget * 3 {
            add("triangles", Level::Warn, format!("{}: {} triangles (over 75% of {budget})", p.node, p.triangles));
        } else if p.triangles < 12 && !surface {
            add("triangles", Level::Fail, format!("{}: only {} triangles (placeholder?)", p.node, p.triangles));
        }
    }
    let texel_density = if world_area > 0.0 { (uv_area_tex / world_area).sqrt() as f32 } else { 0.0 };
    if world_area > 0.0 {
        if texel_density < 256.0 {
            add("texel density", Level::Fail, format!("{texel_density:.0} px/m (blurry up close; need >= 256)"));
        } else if texel_density < 512.0 {
            add("texel density", Level::Warn, format!("{texel_density:.0} px/m (target 512-2048)"));
        } else if texel_density > 8192.0 {
            add("texel density", Level::Warn, format!("{texel_density:.0} px/m (wasted memory)"));
        }
    }
    let passed = !findings.iter().any(|f| f.level == Level::Fail);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    Ok(Report { id: id.into(), source: path.display().to_string(), category, hash, passed, findings, parts, textures, texel_density, checked_at: now })
}

const IDENTITY: [[f32; 4]; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// Column-major 4x4 multiply (a * b).
fn mat_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut r = [[0.0; 4]; 4];
    for c in 0..4 {
        for row in 0..4 {
            r[c][row] = (0..4).map(|k| a[k][row] * b[c][k]).sum();
        }
    }
    r
}
fn transform(m: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|row| m[0][row] * p[0] + m[1][row] * p[1] + m[2][row] * p[2] + m[3][row])
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
