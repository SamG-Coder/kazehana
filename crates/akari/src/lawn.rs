//! The lawn, ported from SmartImposter (its src/lawn.rs, shaders/lawn.glsl): short grass as a
//! tiling parallax layer instead of geometry.
//!
//! One small square of real blades (`tile`) is baked once, like a plant impostor, into views from
//! a grid of directions (renderer/impostor.rs `bake_lawn`, shaders/bake.rgen): per view and texel,
//! how high the first blade is that a ray from that direction meets, its colour, normal, coverage
//! and how much sky it sees. The views wrap around (the square is baked between copies of
//! itself), so the square tiles the plane. A ray that hits lawn (the terrain's garden lawns, or a
//! mesh whose material is named `*lawn_surface*`: MAT_LAWN) is marched through that height field
//! in the slab of one blade height above the surface (shaders/lawn.glsl, from hit.rchit). Nothing
//! is added to the acceleration structures, and far lawn is a coarser mip of the same atlas.
//!
//! The frames of a view from direction f (towards the viewer) are laid out on the plane of the
//! blade tips: texel (u, v) is the line through the point (u, v) x tile of that plane along f,
//! and stores where the first blade on that line is, as a height above the ground. A lawn is
//! mostly seen at a low angle, so the views are spaced evenly in azimuth and geometrically in
//! slope (cot of the elevation, from SLOPE_MIN to SLOPE_MAX: ~79 to ~2.4 degrees).
//!
//! Repetition: every copy of the square is shifted by its own random amount (lawn.glsl
//! lawnTile), with ragged borders between copies, so no clump lines up into rows.
//!
//! Not ported: SmartImposter's real blades drawn as geometry around the camera. They give tall
//! grass a silhouette close up; Kazehana's lawns and turf are a few centimetres high and seen from
//! standing height, where the layer alone holds up.

use crate::scene::MeshData;
use glam::Vec3;

/// Views: azimuths (evenly round the compass) x slopes.
pub const AZIMUTHS: u32 = 16;
pub const SLOPES: u32 = 12;
/// cot(elevation) of the steepest and the flattest views; the ones between are geometric.
pub const SLOPE_MIN: f32 = 0.2;
pub const SLOPE_MAX: f32 = 24.0;
/// Texels per view, and how many of them on each side repeat the neighbouring copy (so filtered
/// lookups at the tile's edge read the right texels; a texel of the coarsest level).
pub const RES: u32 = 320;
pub const GUTTER: u32 = 20;
/// Atlas levels.
pub const MIPS: u32 = 6;

/// The lawn as baked: its blades' height (m). Each surface stands them at a part of it (the
/// layer squashes or stretches the blades: lawn.glsl).
#[derive(Clone, Copy, Debug)]
pub struct LawnParams {
    /// tallest blade (m)
    pub height: f32,
    /// blades in the tile
    pub blades: usize,
}

impl Default for LawnParams {
    fn default() -> Self {
        Self { height: 0.035, blades: 6500 }
    }
}

impl LawnParams {
    /// Edge of the square that is baked and repeated (m): four blade heights, ~1.3 mm a texel.
    pub fn tile_size(&self) -> f32 {
        (4.0 * self.height).clamp(0.42, 1.6)
    }
    /// How many copies of the tile the bake needs on each side of it: the flattest view's rays
    /// run SLOPE_MAX blade heights sideways before they reach the ground.
    pub fn copies(&self) -> i32 {
        (self.height * SLOPE_MAX / self.tile_size() + 0.05).ceil() as i32
    }
}

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// One square (`tile_size`) of grass blades, bases at y = 0: curved, tapering ribbons in clumps,
/// leaning every way, mown to about one length. Each blade's colour is in its vertex colours
/// (linear: darker and bluer at the root, lighter and yellower at the tip, every blade its own
/// shade, a few straw); the material multiplies them.
pub fn tile(p: &LawnParams) -> MeshData {
    let t_size = p.tile_size();
    let height = p.height;
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let grown = (height / 0.045).sqrt().min(2.6);
    let n = p.blades;
    // (a clump: where it stands and its shade of green, which its blades share)
    let clumps: Vec<[f32; 3]> = (0..n / 28).map(|_| [rng.f() * t_size, rng.f() * t_size, rng.f()]).collect();
    // taller and shorter patches a hand across (wrapping round the tile)
    let patches = |x: f32, z: f32| {
        let (u, v) = (x / t_size * std::f32::consts::TAU, z / t_size * std::f32::consts::TAU);
        0.5 + 0.25 * ((2.0 * u + 1.3).sin() * (3.0 * v + 0.4).cos() + (3.0 * u - 2.0 * v + 2.1).sin())
    };
    let mut m = MeshData { name: "lawn tile".into(), ..Default::default() };
    const SEG: usize = 4;
    for _ in 0..n {
        // most blades stand in clumps a couple of centimetres across, the rest anywhere
        let (bx, bz, shade) = if rng.f() < 0.7 {
            let c = clumps[(rng.f() * clumps.len() as f32) as usize % clumps.len()];
            let (r, a) = (0.018 * grown * rng.f().sqrt(), rng.f() * std::f32::consts::TAU);
            ((c[0] + r * a.cos()).rem_euclid(t_size), (c[1] + r * a.sin()).rem_euclid(t_size), c[2] + 0.3 * (rng.f() - 0.5))
        } else {
            (rng.f() * t_size, rng.f() * t_size, rng.f())
        };
        let (r_len, r_bend) = (rng.f(), rng.f() * rng.f());
        let length = height * (0.5 + 0.5 * r_len.powf(0.6)) * (0.78 + 0.22 * patches(bx, bz));
        let bend = (0.12 + 0.8 * r_bend).min(0.95); // how far the tip hangs over, x its length
        let az = rng.f() * std::f32::consts::TAU;
        let lean = Vec3::new(az.cos(), 0.0, az.sin());
        let twist = (rng.f() - 0.5) * 1.2;
        let width = (0.0024 + 0.0012 * rng.f()) * grown;
        // the blade's own colour (SmartImposter tri.rchit's lawn blade, here in vertex colours)
        let (h1, h2) = (shade.clamp(0.0, 1.0), rng.f());
        let tintb = Vec3::new(1.10, 1.0, 0.72).lerp(Vec3::new(0.90, 1.0, 1.10), h1) * (0.78 + 0.42 * h2);
        let straw = h2 > 0.94;
        let first = m.positions.len() as u32;
        for s in 0..=SEG {
            let t = s as f32 / SEG as f32;
            let centre = Vec3::new(bx, 0.0, bz) + lean * (length * bend * t * t) + Vec3::Y * (length * t * (1.0 - 0.3 * bend * t));
            // along the blade, and across it (turning a little towards the tip)
            let along = (lean * (2.0 * bend * t) + Vec3::Y * (1.0 - 0.6 * bend * t)).normalize();
            let a = az + std::f32::consts::FRAC_PI_2 + twist * t;
            let across = Vec3::new(a.cos(), 0.0, a.sin());
            let across = (across - along * across.dot(along)).normalize();
            let half = 0.5 * width * (1.0 - 0.8 * t.powf(1.6));
            let mut face = across.cross(along).normalize();
            if face.y < 0.0 {
                face = -face;
            }
            let k = (t / 0.85).min(1.0);
            let k = k * k * (3.0 - 2.0 * k);
            let mut col = Vec3::new(0.026, 0.050, 0.012).lerp(Vec3::new(0.070, 0.118, 0.028), k) * tintb;
            if straw {
                col = col.lerp(Vec3::new(0.16, 0.13, 0.06) * (0.7 + 0.4 * h1), ((t - 0.2) / 0.5).clamp(0.0, 1.0));
            }
            for side in [-1.0f32, 1.0] {
                let pos = centre + across * (half * side);
                // normals lean up (a blade is lit from above whichever way it faces), and the
                // two edges fold apart a little (the blade's V section)
                let nrm = (face * 0.6 + Vec3::Y * 0.4 + across * (0.3 * side)).normalize();
                m.positions.push(pos.to_array());
                m.normals.push(nrm.to_array());
                m.uvs.push([0.5 + 0.5 * side, t]);
                // (the midrib a little darker than the edges)
                m.colors.push((col * 0.93).to_array());
            }
        }
        for s in 0..SEG as u32 {
            let k = first + 2 * s;
            m.indices.extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tile_is_blades_under_the_lawn_height() {
        let p = LawnParams::default();
        let m = tile(&p);
        assert!(m.indices.len() / 3 > 40_000);
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for q in &m.positions {
            lo = lo.min(Vec3::from(*q));
            hi = hi.max(Vec3::from(*q));
        }
        assert!(lo.y >= -1e-4 && hi.y <= p.height, "{} .. {}", lo.y, hi.y);
        // blades lean over the tile's edge (the bake puts copies around it), but not far
        assert!(lo.x > -p.height && hi.x < p.tile_size() + p.height);
        // the flattest view's rays end inside the copies the bake puts around the tile
        assert!(p.copies() as f32 * p.tile_size() >= p.height * SLOPE_MAX);
        assert!(AZIMUTHS.is_multiple_of(4) && SLOPES.is_multiple_of(4) && RES > 2 * GUTTER);
    }
}
