//! Scene description types: materials, meshes, instances and lights as the game hands them to
//! the renderer. GPU mirrors live in renderer.rs (rt.glsl declares the same layouts).

use bytemuck::{Pod, Zeroable};
use glam::{Affine3A, Vec3};

pub const MAT_ALPHA_MASK: u32 = 1;
pub const MAT_DOUBLE_SIDED: u32 = 2;
/// Thin translucent leaves: diffuse light passes through (`Material::transmission`).
pub const MAT_FOLIAGE: u32 = 4;
/// Emission seen by the camera only: lamp bulbs that also have an analytic light.
pub const MAT_UNLIT_EMISSIVE: u32 = 8;
/// Trading-card foil: `emissive_tex` holds the card's foil mask (tools/cardgen foil.png, loaded as
/// `TexKind::Data`) instead of emission; the hit shader turns it into view-dependent holo foil
/// (shaders/foil.glsl).
pub const MAT_CARD_FOIL: u32 = 16;
/// Land-use mapped terrain (shaders/ground.glsl); see `Material::ground`.
pub const MAT_GROUND: u32 = 32;
/// Asphalt road ribbon (shaders/road.glsl): uv = (metres across, metres along), vertex colour =
/// (half width, kind, seed).
pub const MAT_ROAD: u32 = 64;
/// The base texture gives light and shade only: colour = its luminance x base colour (blossom on
/// leaf textures).
pub const MAT_RECOLOR: u32 = 128;
/// Thin clear glass (shop windows): rays pass straight through or reflect (Fresnel); shadow rays
/// pass through. Set by the glTF loader for materials named `*clear_glass*`.
pub const MAT_GLASS: u32 = 256;
pub const NO_TEX: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeshId(pub u32);

/// A PBR metallic-roughness material (glTF 2.0 semantics). Colours are linear.
#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub base_color: [f32; 4],
    pub emissive: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub alpha_cutoff: f32,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
    pub base_tex: Option<TextureId>,
    pub normal_tex: Option<TextureId>,
    /// glTF metallic-roughness texture: G = roughness, B = metallic.
    pub mr_tex: Option<TextureId>,
    /// R = ambient occlusion (often the same texture as `mr_tex`: an ORM map).
    pub occlusion_tex: Option<TextureId>,
    pub emissive_tex: Option<TextureId>,
    pub flags: u32,
    /// Foliage: fraction of diffuse light transmitted through the leaf.
    pub transmission: f32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [0.8, 0.8, 0.8, 1.0],
            emissive: [0.0; 3],
            roughness: 0.8,
            metallic: 0.0,
            alpha_cutoff: 0.5,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            base_tex: None,
            normal_tex: None,
            mr_tex: None,
            occlusion_tex: None,
            emissive_tex: None,
            flags: 0,
            transmission: 0.0,
        }
    }
}

impl Material {
    /// A plain coloured surface (sRGB colour).
    pub fn color(srgb: [f32; 3], roughness: f32) -> Self {
        let lin = |c: f32| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        Self { base_color: [lin(srgb[0]), lin(srgb[1]), lin(srgb[2]), 1.0], roughness, ..Default::default() }
    }

    /// Land-use mapped terrain (shaders/ground.glsl). `landuse`: one code per metre (R8, row 0 at
    /// `z0`), its first texel at world (x0, z0) and its size; `first_layer`: the first of 3
    /// consecutive textures (albedo, normal, ORM) per ground layer.
    pub fn ground(landuse: TextureId, x0: f32, z0: f32, cols: u32, rows: u32, first_layer: TextureId) -> Self {
        Self {
            base_color: [x0, z0, cols as f32, rows as f32],
            base_tex: Some(landuse),
            normal_tex: Some(first_layer),
            roughness: 1.0,
            flags: MAT_GROUND,
            ..Default::default()
        }
    }

    pub(crate) fn gpu(&self) -> MaterialGpu {
        let t = |x: Option<TextureId>| x.map_or(NO_TEX, |t| t.0);
        MaterialGpu {
            base_color: self.base_color,
            emissive: self.emissive,
            roughness: self.roughness,
            metallic: self.metallic,
            alpha_cutoff: self.alpha_cutoff,
            normal_scale: self.normal_scale,
            occlusion: self.occlusion_strength,
            tex_base: t(self.base_tex),
            tex_normal: t(self.normal_tex),
            tex_mr: t(self.mr_tex),
            tex_occlusion: t(self.occlusion_tex),
            tex_emissive: t(self.emissive_tex),
            flags: self.flags,
            transmission: self.transmission,
            uv_scale: 1.0,
        }
    }
}

/// rt.glsl `Material` (scalar layout, 80 bytes).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct MaterialGpu {
    pub base_color: [f32; 4],
    pub emissive: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub alpha_cutoff: f32,
    pub normal_scale: f32,
    pub occlusion: f32,
    pub tex_base: u32,
    pub tex_normal: u32,
    pub tex_mr: u32,
    pub tex_occlusion: u32,
    pub tex_emissive: u32,
    pub flags: u32,
    pub transmission: f32,
    pub uv_scale: f32,
}

/// Triangle mesh data for one material. Optional streams may be empty.
#[derive(Clone, Default, Debug)]
pub struct MeshData {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// xyz tangent, w handedness (glTF convention).
    pub tangents: Vec<[f32; 4]>,
    /// Linear vertex colours (multiply the base colour).
    pub colors: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub material: Option<MaterialId>,
}

impl MeshData {
    /// Fill in tangents from positions, normals and UVs (per-vertex accumulation of triangle
    /// tangents, Gram-Schmidt against the normal, handedness from the bitangent).
    pub fn generate_tangents(&mut self) {
        let n = self.positions.len();
        if self.uvs.len() != n || self.normals.len() != n || n == 0 {
            return;
        }
        let mut tan = vec![Vec3::ZERO; n];
        let mut bit = vec![Vec3::ZERO; n];
        for tri in self.indices.chunks_exact(3) {
            let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let (p0, p1, p2) = (Vec3::from(self.positions[a]), Vec3::from(self.positions[b]), Vec3::from(self.positions[c]));
            let (u0, u1, u2) = (self.uvs[a], self.uvs[b], self.uvs[c]);
            let (e1, e2) = (p1 - p0, p2 - p0);
            let (du1, dv1, du2, dv2) = (u1[0] - u0[0], u1[1] - u0[1], u2[0] - u0[0], u2[1] - u0[1]);
            let det = du1 * dv2 - du2 * dv1;
            if det.abs() < 1e-12 {
                continue;
            }
            let r = 1.0 / det;
            let t = (e1 * dv2 - e2 * dv1) * r;
            let bt = (e2 * du1 - e1 * du2) * r;
            for i in [a, b, c] {
                tan[i] += t;
                bit[i] += bt;
            }
        }
        self.tangents = (0..n)
            .map(|i| {
                let nn = Vec3::from(self.normals[i]);
                let t = (tan[i] - nn * nn.dot(tan[i])).normalize_or(nn.any_orthonormal_vector());
                let w = if nn.cross(t).dot(bit[i]) < 0.0 { -1.0 } else { 1.0 };
                [t.x, t.y, t.z, w]
            })
            .collect();
    }

    /// Flat normals for meshes that have none (one normal per corner after unwelding is not
    /// needed for blockout geometry: averaged per vertex).
    pub fn generate_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.positions.len()];
        for tri in self.indices.chunks_exact(3) {
            let p: [Vec3; 3] = std::array::from_fn(|k| Vec3::from(self.positions[tri[k] as usize]));
            let nrm = (p[1] - p[0]).cross(p[2] - p[0]);
            for &i in tri {
                acc[i as usize] += nrm;
            }
        }
        self.normals = acc.into_iter().map(|v| v.normalize_or(Vec3::Y).to_array()).collect();
    }
}

/// One placement of a mesh in the world.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub mesh: MeshId,
    pub transform: Affine3A,
}

/// A point or spot light (street lamp, fluorescent tube, window glow, vending machine).
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub position: Vec3,
    /// Emitter radius (m): soft shadow size.
    pub radius: f32,
    /// Linear colour x intensity (radiant intensity in the renderer's units: the sun at noon
    /// delivers ~1 to a surface facing it, so a 100 W bulb is roughly 0.02 at 1 m).
    pub intensity: Vec3,
    /// Beyond this distance the light is ignored (smooth window).
    pub range: f32,
    /// Spot lights: direction and cosine of the outer cone angle. None = point light.
    pub spot: Option<(Vec3, f32)>,
}

/// rt.glsl `Light`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct LightGpu {
    pub pos_radius: [f32; 4],
    pub color: [f32; 4],
    pub spot: [f32; 4],
}

impl Light {
    pub(crate) fn gpu(&self) -> LightGpu {
        let (dir, cos) = self.spot.unwrap_or((Vec3::NEG_Y, -2.0));
        LightGpu {
            pos_radius: [self.position.x, self.position.y, self.position.z, self.radius],
            color: [self.intensity.x, self.intensity.y, self.intensity.z, self.range],
            spot: [dir.x, dir.y, dir.z, cos],
        }
    }
}
