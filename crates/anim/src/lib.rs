//! Skeletal animation, engine-free:
//!
//! - [`Skeleton`]: joints in parent order, their rest pose and inverse bind matrices (a glTF skin).
//! - [`Clip`]: keyframed joint tracks (glTF animations), sampled into a [`Pose`].
//! - [`Pose`]: each joint's local transform; blended, then turned into skinning matrices.
//! - [`SkinnedMesh`]: a mesh's bind-pose vertices with up to eight joint weights each, skinned on
//!   the CPU into positions, normals and tangents (the renderer's deformable meshes take those).
//! - [`load`]: a glTF file's skins, skinned primitives and animations.
//! - [`Springs`]: spring bones (skirts, aprons, hair) swinging on top of the animation.
//!
//! Frames are glTF's (and the renderer's): +Y up, metres.

mod gltf_io;
pub mod spring;

pub use gltf_io::{load, Loaded, SkinnedPrimitive};
pub use spring::{SpringParams, Springs};

use glam::{Mat4, Quat, Vec3};

/// A joint's local transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trs {
    pub t: Vec3,
    pub r: Quat,
    pub s: Vec3,
}

impl Default for Trs {
    fn default() -> Self {
        Trs { t: Vec3::ZERO, r: Quat::IDENTITY, s: Vec3::ONE }
    }
}

impl Trs {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.s, self.r, self.t)
    }

    /// Between two transforms (`k` 0..1): translations and scales linearly, rotations by slerp.
    pub fn lerp(&self, o: &Trs, k: f32) -> Trs {
        Trs { t: self.t.lerp(o.t, k), r: self.r.slerp(o.r, k), s: self.s.lerp(o.s, k) }
    }
}

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    /// index of the parent joint; parents always come before their children
    pub parent: Option<usize>,
    pub rest: Trs,
}

#[derive(Clone, Debug)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
    /// model space -> each joint's bind space
    pub inverse_bind: Vec<Mat4>,
    /// the skeleton's root placed in the model (the transform of the nodes above it)
    pub root: Mat4,
}

impl Skeleton {
    pub fn find(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j.name == name)
    }

    pub fn rest_pose(&self) -> Pose {
        Pose { local: self.joints.iter().map(|j| j.rest).collect() }
    }

    /// Each joint's model-space transform for a pose.
    pub fn globals(&self, pose: &Pose) -> Vec<Mat4> {
        let mut g: Vec<Mat4> = Vec::with_capacity(self.joints.len());
        for (i, j) in self.joints.iter().enumerate() {
            let l = pose.local[i].matrix();
            let m = match j.parent {
                Some(p) => g[p] * l,
                None => self.root * l,
            };
            g.push(m);
        }
        g
    }

    /// The skinning matrices for a pose (bind space -> posed model space).
    pub fn skin_matrices(&self, pose: &Pose) -> Vec<Mat4> {
        self.globals(pose).iter().zip(&self.inverse_bind).map(|(g, ib)| *g * *ib).collect()
    }
}

/// Every joint's local transform.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub local: Vec<Trs>,
}

impl Pose {
    /// Blend towards another pose by `k` (0: this one, 1: the other).
    pub fn blend(&mut self, o: &Pose, k: f32) {
        if k <= 0.0 {
            return;
        }
        for (a, b) in self.local.iter_mut().zip(&o.local) {
            *a = a.lerp(b, k.min(1.0));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Values {
    Translation,
    Rotation,
    Scale,
}

/// One joint's keyframes of one property (linear interpolation; rotations slerped).
#[derive(Clone, Debug)]
pub struct Track {
    pub joint: usize,
    pub what: Values,
    pub times: Vec<f32>,
    /// xyz (translation, scale) or xyzw (rotation)
    pub values: Vec<[f32; 4]>,
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<Track>,
}

impl Clip {
    /// The pose at time `t` (wrapped when `looping`), over the skeleton's rest pose (joints the
    /// clip does not move keep their rest transform).
    pub fn sample(&self, skel: &Skeleton, t: f32, looping: bool) -> Pose {
        let mut pose = skel.rest_pose();
        self.sample_into(&mut pose, t, looping);
        pose
    }

    pub fn sample_into(&self, pose: &mut Pose, t: f32, looping: bool) {
        let t = if looping && self.duration > 0.0 { t.rem_euclid(self.duration) } else { t.clamp(0.0, self.duration) };
        for tr in &self.tracks {
            let n = tr.times.len();
            if n == 0 || tr.joint >= pose.local.len() {
                continue;
            }
            let i = tr.times.partition_point(|&x| x <= t).clamp(1, n) - 1;
            let j = (i + 1).min(n - 1);
            let k = if j == i { 0.0 } else { ((t - tr.times[i]) / (tr.times[j] - tr.times[i]).max(1e-6)).clamp(0.0, 1.0) };
            let (a, b) = (tr.values[i], tr.values[j]);
            let l = &mut pose.local[tr.joint];
            match tr.what {
                Values::Translation => l.t = Vec3::new(a[0], a[1], a[2]).lerp(Vec3::new(b[0], b[1], b[2]), k),
                Values::Scale => l.s = Vec3::new(a[0], a[1], a[2]).lerp(Vec3::new(b[0], b[1], b[2]), k),
                Values::Rotation => l.r = Quat::from_array(a).normalize().slerp(Quat::from_array(b).normalize(), k),
            }
        }
    }
}

/// How many joints a vertex can be weighted to (MetaHuman bodies use up to eight: their twist
/// and corrective joints share the skin with the main ones).
pub const INFLUENCES: usize = 8;

/// A mesh's bind-pose vertices, each weighted to up to [`INFLUENCES`] joints.
#[derive(Clone, Debug, Default)]
pub struct SkinnedMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tangents: Vec<[f32; 4]>,
    pub joints: Vec<[u16; INFLUENCES]>,
    pub weights: Vec<[f32; INFLUENCES]>,
}

/// A skinned frame's vertices.
#[derive(Clone, Debug, Default)]
pub struct Skinned {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tangents: Vec<[f32; 4]>,
}

impl SkinnedMesh {
    /// Linear blend skinning with the pose's skinning matrices, into `out` (reused each frame).
    pub fn skin(&self, mats: &[Mat4], out: &mut Skinned) {
        let n = self.positions.len();
        out.positions.resize(n, [0.0; 3]);
        out.normals.resize(n, [0.0; 3]);
        out.tangents.resize(if self.tangents.len() == n { n } else { 0 }, [0.0; 4]);
        for i in 0..n {
            let (j, w) = (self.joints[i], self.weights[i]);
            let mut m = Mat4::ZERO;
            for k in 0..INFLUENCES {
                if w[k] != 0.0 {
                    m += mats[j[k] as usize] * w[k];
                }
            }
            let p = Vec3::from(self.positions[i]);
            out.positions[i] = m.transform_point3(p).to_array();
            out.normals[i] = m.transform_vector3(Vec3::from(self.normals[i])).normalize_or_zero().to_array();
            if !out.tangents.is_empty() {
                let t = self.tangents[i];
                let v = m.transform_vector3(Vec3::new(t[0], t[1], t[2])).normalize_or_zero();
                out.tangents[i] = [v.x, v.y, v.z, t[3]];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two joints: a root at the origin and an elbow 1 m up; a vertex 2 m up weighted to the
    /// elbow; a clip turning the elbow a quarter round about Z.
    fn arm() -> (Skeleton, SkinnedMesh, Clip) {
        let skel = Skeleton {
            joints: vec![
                Joint { name: "root".into(), parent: None, rest: Trs::default() },
                Joint { name: "elbow".into(), parent: Some(0), rest: Trs { t: Vec3::Y, ..Default::default() } },
            ],
            inverse_bind: vec![Mat4::IDENTITY, Mat4::from_translation(-Vec3::Y)],
            root: Mat4::IDENTITY,
        };
        let mesh = SkinnedMesh { positions: vec![[0.0, 2.0, 0.0]], normals: vec![[1.0, 0.0, 0.0]], tangents: vec![], joints: vec![[1, 0, 0, 0, 0, 0, 0, 0]], weights: vec![[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]] };
        let q = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let clip = Clip { name: "bend".into(), duration: 1.0, tracks: vec![Track { joint: 1, what: Values::Rotation, times: vec![0.0, 1.0], values: vec![Quat::IDENTITY.to_array(), q.to_array()] }] };
        (skel, mesh, clip)
    }

    #[test]
    fn rest_pose_leaves_the_mesh_as_it_is() {
        let (skel, mesh, _) = arm();
        let mut out = Skinned::default();
        mesh.skin(&skel.skin_matrices(&skel.rest_pose()), &mut out);
        assert!((Vec3::from(out.positions[0]) - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn a_clip_bends_the_arm() {
        let (skel, mesh, clip) = arm();
        let mut out = Skinned::default();
        mesh.skin(&skel.skin_matrices(&clip.sample(&skel, 1.0, false)), &mut out);
        // the tip turned a quarter round the elbow: from (0, 2) to (-1, 1)
        assert!((Vec3::from(out.positions[0]) - Vec3::new(-1.0, 1.0, 0.0)).length() < 1e-4, "{:?}", out.positions[0]);
        assert!((Vec3::from(out.normals[0]) - Vec3::Y).length() < 1e-4);
        // half way, and looping wraps
        let half = clip.sample(&skel, 0.5, false);
        let looped = clip.sample(&skel, 1.5, true);
        assert!(half.local[1].r.angle_between(looped.local[1].r) < 1e-4);
    }

    #[test]
    fn poses_blend() {
        let (skel, _, clip) = arm();
        let mut a = skel.rest_pose();
        a.blend(&clip.sample(&skel, 1.0, false), 0.5);
        let angle = a.local[1].r.angle_between(Quat::IDENTITY);
        assert!((angle - std::f32::consts::FRAC_PI_4).abs() < 1e-4);
    }
}
