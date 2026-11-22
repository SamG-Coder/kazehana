//! Procedural geometry: boxes, cylinders, cones, spheres and ground planes, baked into one
//! `MeshData` per material. UVs are world-space box projections in metres (times a scale), so
//! tiling Megascans surfaces line up across pieces. Used for greybox geometry and by the Blender-
//! free test scenes; real levels come from glTF.

use crate::scene::{MaterialId, MeshData};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};

/// Builds one mesh (one material) out of primitives placed in world space.
pub struct MeshBuilder {
    pub mesh: MeshData,
    /// Texture repeats per metre for the projected UVs.
    pub uv_scale: f32,
}

impl MeshBuilder {
    pub fn new(name: &str, material: MaterialId, uv_scale: f32) -> Self {
        Self { mesh: MeshData { name: name.into(), material: Some(material), ..Default::default() }, uv_scale }
    }

    fn push(&mut self, p: Vec3, n: Vec3, uv: Vec2) -> u32 {
        self.mesh.positions.push(p.to_array());
        self.mesh.normals.push(n.to_array());
        self.mesh.uvs.push(uv.to_array());
        self.mesh.positions.len() as u32 - 1
    }

    /// Box-projected UV for a world point with normal n.
    fn uv(&self, p: Vec3, n: Vec3) -> Vec2 {
        let a = n.abs();
        let uv = if a.y >= a.x && a.y >= a.z {
            Vec2::new(p.x, p.z)
        } else if a.x >= a.z {
            Vec2::new(p.z * -n.x.signum(), -p.y)
        } else {
            Vec2::new(p.x * n.z.signum(), -p.y)
        };
        uv * self.uv_scale
    }

    fn quad(&mut self, c: [Vec3; 4], n: Vec3) {
        let i: Vec<u32> = c.iter().map(|&p| { let uv = self.uv(p, n); self.push(p, n, uv) }).collect();
        self.mesh.indices.extend([i[0], i[1], i[2], i[0], i[2], i[3]]);
    }

    /// A box with an arbitrary transform of the unit cube (-0.5..0.5).
    pub fn cuboid_m(&mut self, m: Mat4) -> &mut Self {
        let faces: [(Vec3, Vec3, Vec3); 6] = [
            (Vec3::X, Vec3::Z, Vec3::Y),
            (Vec3::NEG_X, Vec3::NEG_Z, Vec3::Y),
            (Vec3::Y, Vec3::X, Vec3::Z),
            (Vec3::NEG_Y, Vec3::X, Vec3::NEG_Z),
            (Vec3::Z, Vec3::NEG_X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::X, Vec3::Y),
        ];
        let nm = Mat3::from_mat4(m).inverse().transpose();
        for (n, u, v) in faces {
            let c = n * 0.5;
            let corners = [c - u * 0.5 - v * 0.5, c - u * 0.5 + v * 0.5, c + u * 0.5 + v * 0.5, c + u * 0.5 - v * 0.5].map(|p| m.transform_point3(p));
            self.quad(corners, (nm * n).normalize());
        }
        self
    }

    /// Axis-aligned box from min / max corners.
    pub fn aabb(&mut self, min: Vec3, max: Vec3) -> &mut Self {
        let (lo, hi) = (min.min(max), min.max(max));
        self.cuboid_m(Mat4::from_scale_rotation_translation(hi - lo, Quat::IDENTITY, (lo + hi) * 0.5))
    }

    /// Box from centre and size, turned about Y by `yaw`.
    pub fn cuboid_yaw(&mut self, center: Vec3, size: Vec3, yaw: f32) -> &mut Self {
        self.cuboid_m(Mat4::from_scale_rotation_translation(size, Quat::from_rotation_y(yaw), center))
    }

    /// A flat horizontal rectangle at height y (a single quad, facing up).
    pub fn ground(&mut self, min: Vec2, max: Vec2, y: f32) -> &mut Self {
        let c = [Vec3::new(min.x, y, min.y), Vec3::new(min.x, y, max.y), Vec3::new(max.x, y, max.y), Vec3::new(max.x, y, min.y)];
        self.quad(c, Vec3::Y);
        self
    }

    /// Frustum (cylinder / cone) along the transformed unit Y axis: radius r0 at y=0, r1 at y=1.
    pub fn frustum_m(&mut self, m: Mat4, r0: f32, r1: f32, segments: u32) -> &mut Self {
        let nm = Mat3::from_mat4(m).inverse().transpose();
        let slope = (r0 - r1).atan2(1.0);
        for i in 0..segments {
            let a0 = i as f32 / segments as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let (d0, d1) = (Vec3::new(a0.cos(), 0.0, a0.sin()), Vec3::new(a1.cos(), 0.0, a1.sin()));
            let side_n = |d: Vec3| (nm * (d * slope.cos() + Vec3::Y * slope.sin())).normalize();
            let pts = [d0 * r0, d0 * r1 + Vec3::Y, d1 * r1 + Vec3::Y, d1 * r0].map(|p| m.transform_point3(p));
            let ns = [side_n(d0), side_n(d0), side_n(d1), side_n(d1)];
            // cylindrical UVs: around x height
            let circ = std::f32::consts::TAU * r0.max(r1) * Mat3::from_mat4(m).x_axis.length();
            let len = Mat3::from_mat4(m).y_axis.length();
            let us = [a0, a0, a1, a1].map(|a| a / std::f32::consts::TAU * circ);
            let vs = [0.0, len, len, 0.0];
            let idx: Vec<u32> = (0..4).map(|k| self.push(pts[k], ns[k], Vec2::new(us[k], -vs[k]) * self.uv_scale)).collect();
            self.mesh.indices.extend([idx[0], idx[1], idx[2], idx[0], idx[2], idx[3]]);
            for (y, r, n) in [(0.0f32, r0, Vec3::NEG_Y), (1.0, r1, Vec3::Y)] {
                if r <= 0.0 {
                    continue;
                }
                let c = Vec3::Y * y;
                let wn = (nm * n).normalize();
                let tri = [c, c + d0 * r, c + d1 * r].map(|p| m.transform_point3(p));
                let k: Vec<u32> = tri.iter().map(|&p| { let uv = self.uv(p, wn); self.push(p, wn, uv) }).collect();
                if n.y > 0.0 { self.mesh.indices.extend([k[0], k[2], k[1]]) } else { self.mesh.indices.extend([k[0], k[1], k[2]]) }
            }
        }
        self
    }

    /// Vertical cylinder standing on `base`.
    pub fn cylinder(&mut self, base: Vec3, radius: f32, height: f32, segments: u32) -> &mut Self {
        self.frustum_m(Mat4::from_scale_rotation_translation(Vec3::new(1.0, height, 1.0), Quat::IDENTITY, base), radius, radius, segments)
    }

    /// UV sphere (ellipsoid with radii `r`).
    pub fn sphere(&mut self, center: Vec3, r: Vec3, rings: u32, segments: u32) -> &mut Self {
        let base = self.mesh.positions.len() as u32;
        for i in 0..=rings {
            let phi = i as f32 / rings as f32 * std::f32::consts::PI;
            for s in 0..=segments {
                let th = s as f32 / segments as f32 * std::f32::consts::TAU;
                let d = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
                let p = center + d * r;
                let n = (d / r).normalize();
                let uv = Vec2::new(th * r.x.max(r.z), phi * r.y) * self.uv_scale;
                self.push(p, n, uv);
            }
        }
        let w = segments + 1;
        for i in 0..rings {
            for s in 0..segments {
                let (a, b) = (base + i * w + s, base + (i + 1) * w + s);
                self.mesh.indices.extend([a, a + 1, b + 1, a, b + 1, b]);
            }
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.mesh.indices.is_empty()
    }

    /// The finished mesh, with tangents for normal mapping.
    pub fn finish(mut self) -> MeshData {
        self.mesh.generate_tangents();
        self.mesh
    }
}
