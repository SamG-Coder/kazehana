//! Colour-per-primitive geometry for the old blockout levels (crate::world: not drawn, test
//! fixtures only).
#![allow(dead_code)]

use akari::glam::{Mat4, Quat, Vec3};
use akari::{MaterialId, MeshBuilder as Builder, MeshData as Mesh};
use std::collections::BTreeMap;

fn key(c: [f32; 3]) -> [u32; 3] {
    c.map(|v| (v.clamp(0.0, 4.0) * 1000.0) as u32)
}

/// Collects primitives by colour.
#[derive(Default)]
pub struct MeshBuilder {
    groups: BTreeMap<[u32; 3], ([f32; 3], Builder)>,
}

/// The result: one mesh per colour (material assigned at upload).
#[derive(Default)]
pub struct MeshData {
    pub groups: Vec<([f32; 3], Mesh)>,
}

impl MeshBuilder {
    fn b(&mut self, c: [f32; 3]) -> &mut Builder {
        &mut self.groups.entry(key(c)).or_insert_with(|| (c, Builder::new("paint", MaterialId(0), 1.0))).1
    }
    pub fn cuboid(&mut self, center: Vec3, size: Vec3, c: [f32; 3]) -> &mut Self {
        self.b(c).cuboid_m(Mat4::from_scale_rotation_translation(size, Quat::IDENTITY, center));
        self
    }
    pub fn cuboid_yaw(&mut self, center: Vec3, size: Vec3, yaw: f32, c: [f32; 3]) -> &mut Self {
        self.b(c).cuboid_yaw(center, size, yaw);
        self
    }
    pub fn cuboid_m(&mut self, m: Mat4, c: [f32; 3]) -> &mut Self {
        self.b(c).cuboid_m(m);
        self
    }
    /// Vertical cylinder standing on `base`.
    pub fn cylinder(&mut self, base: Vec3, radius: f32, height: f32, c: [f32; 3]) -> &mut Self {
        self.b(c).cylinder(base, radius, height, 12);
        self
    }
    /// Unit cylinder (radius 0.5, y 0..1) under a transform.
    pub fn cylinder_m(&mut self, m: Mat4, c: [f32; 3]) -> &mut Self {
        self.b(c).frustum_m(m, 0.5, 0.5, 12);
        self
    }
    pub fn cone(&mut self, base: Vec3, radius: f32, height: f32, c: [f32; 3]) -> &mut Self {
        self.b(c).frustum_m(Mat4::from_scale_rotation_translation(Vec3::new(1.0, height, 1.0), Quat::IDENTITY, base), radius, 0.0, 10);
        self
    }
    /// Ellipsoid with radii `r`.
    pub fn sphere(&mut self, center: Vec3, r: Vec3, c: [f32; 3]) -> &mut Self {
        self.b(c).sphere(center, r, 8, 12);
        self
    }
    pub fn finish(self) -> MeshData {
        MeshData { groups: self.groups.into_values().filter(|(_, b)| !b.is_empty()).map(|(c, b)| (c, b.finish())).collect() }
    }
}
