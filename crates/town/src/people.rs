//! Animated people in the renderer: a skinned glTF character (crate anim) whose meshes are the
//! renderer's deformable meshes, skinned on the CPU each frame from its animation.
//!
//! A character's file is a quality-gated asset (category character): one skin, its skinned
//! primitives with their materials, and its clips (idle, walk, ...).

use akari::glam::Affine3A;
use akari::gltf_load::{load_materials, LoadOptions};
use akari::{Instance, MeshData, MeshId, Renderer};
use anim::{Clip, Pose, Skeleton, Skinned, SkinnedMesh, Springs};
use anyhow::Result;
use std::path::Path;

/// A character model in the renderer: its skeleton, its clips, each skinned primitive as a
/// deformable mesh with its bind-pose vertices.
pub struct CharacterModel {
    pub skeleton: Skeleton,
    pub clips: Vec<Clip>,
    parts: Vec<(MeshId, SkinnedMesh)>,
}

impl CharacterModel {
    pub fn load(r: &mut Renderer, path: &Path) -> Result<Self> {
        let loaded = anim::load(path)?;
        let mats = load_materials(r, path, LoadOptions::default())?;
        let mut parts = Vec::new();
        for p in loaded.primitives {
            let mut md = MeshData {
                name: p.name.clone(),
                positions: p.mesh.positions.clone(),
                normals: p.mesh.normals.clone(),
                uvs: p.uvs.clone(),
                tangents: p.mesh.tangents.clone(),
                indices: p.indices.clone(),
                material: p.material.and_then(|i| mats.get(i)).map(|m| m.0),
                ..Default::default()
            };
            if md.tangents.len() != md.positions.len() {
                md.generate_tangents();
            }
            let mut mesh = p.mesh;
            mesh.tangents = md.tangents.clone();
            let id = r.add_deformable_mesh(&md)?;
            parts.push((id, mesh));
        }
        Ok(CharacterModel { skeleton: loaded.skeleton, clips: loaded.clips, parts })
    }

    pub fn clip(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|c| c.name.eq_ignore_ascii_case(name) || c.name.to_lowercase().contains(&name.to_lowercase()))
    }
}

/// One character in the world: where it stands, which clip it plays (blending from the one before).
pub struct Person {
    pub at: Affine3A,
    pub clip: Option<usize>,
    pub time: f32,
    from: Option<(usize, f32)>,
    blend: f32,
    scratch: Skinned,
    /// its spring bones (skirt, apron, hair), made on its first frame
    springs: Option<Springs>,
}

impl Person {
    pub fn new(at: Affine3A, clip: Option<usize>) -> Self {
        Person { at, clip, time: 0.0, from: None, blend: 1.0, scratch: Skinned::default(), springs: None }
    }

    /// Change to another clip, cross-fading over `fade` seconds.
    pub fn play(&mut self, clip: Option<usize>, fade: f32) {
        if clip == self.clip {
            return;
        }
        self.from = self.clip.map(|c| (c, self.time));
        self.clip = clip;
        self.time = 0.0;
        self.blend = if fade > 0.0 { 0.0 } else { 1.0 };
    }

    /// Move the animation on and skin the meshes (into the renderer); add its instances.
    pub fn frame(&mut self, r: &mut Renderer, m: &CharacterModel, dt: f32, out: &mut Vec<Instance>) -> Result<()> {
        const FADE: f32 = 0.25;
        self.time += dt;
        self.blend = (self.blend + dt / FADE).min(1.0);
        let mut pose: Pose = match self.clip.and_then(|c| m.clips.get(c)) {
            Some(c) => c.sample(&m.skeleton, self.time, true),
            None => m.skeleton.rest_pose(),
        };
        if self.blend < 1.0 {
            if let Some((c, t)) = self.from {
                if let Some(clip) = m.clips.get(c) {
                    let mut old = clip.sample(&m.skeleton, t + self.time, true);
                    old.blend(&pose, self.blend);
                    pose = old;
                }
            }
        }
        let springs = self.springs.get_or_insert_with(|| Springs::new(&m.skeleton));
        springs.apply(&m.skeleton, &mut pose, akari::glam::Mat4::from(self.at), dt);
        let mats = m.skeleton.skin_matrices(&pose);
        for (id, mesh) in &m.parts {
            mesh.skin(&mats, &mut self.scratch);
            r.update_mesh(*id, &self.scratch.positions, &self.scratch.normals, &self.scratch.tangents)?;
            out.push(Instance { mesh: *id, transform: self.at });
        }
        Ok(())
    }
}
