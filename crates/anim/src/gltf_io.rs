//! A glTF file's skinned meshes, its skeleton and its animations.

use crate::{Clip, Joint, Skeleton, SkinnedMesh, Track, Trs, Values, INFLUENCES};
use anyhow::{anyhow, Context, Result};
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;
use std::path::Path;

/// One skinned primitive: its bind-pose vertices and weights, its UVs and triangles, and which
/// glTF material it uses (an index into the file's materials).
pub struct SkinnedPrimitive {
    pub name: String,
    pub mesh: SkinnedMesh,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
}

pub struct Loaded {
    pub skeleton: Skeleton,
    pub primitives: Vec<SkinnedPrimitive>,
    pub clips: Vec<Clip>,
}

fn trs(n: &gltf::Node) -> Trs {
    let (t, r, s) = n.transform().decomposed();
    Trs { t: Vec3::from(t), r: Quat::from_array(r), s: Vec3::from(s) }
}

/// Load the first skin of a glTF (.gltf + .bin, or .glb), every primitive skinned to it, and
/// every animation (only the tracks that move its joints).
pub fn load(path: &Path) -> Result<Loaded> {
    let (doc, buffers, _) = gltf::import(path).with_context(|| format!("reading {}", path.display()))?;
    let skin = doc.skins().next().ok_or_else(|| anyhow!("{}: no skin", path.display()))?;
    // the skin's joints in parent-before-child order
    let gltf_joints: Vec<gltf::Node> = skin.joints().collect();
    let mut parent_of: HashMap<usize, usize> = HashMap::new();
    for n in doc.nodes() {
        for c in n.children() {
            parent_of.insert(c.index(), n.index());
        }
    }
    let in_skin: HashMap<usize, usize> = gltf_joints.iter().enumerate().map(|(i, n)| (n.index(), i)).collect();
    let mut order: Vec<usize> = Vec::new(); // skin joint indices, sorted parents first
    let depth = |mut n: usize| {
        let mut d = 0;
        while let Some(&p) = parent_of.get(&n) {
            n = p;
            d += 1;
        }
        d
    };
    let mut by_depth: Vec<(usize, usize)> = gltf_joints.iter().enumerate().map(|(i, n)| (depth(n.index()), i)).collect();
    by_depth.sort();
    order.extend(by_depth.iter().map(|x| x.1));
    let new_of_skin: HashMap<usize, usize> = order.iter().enumerate().map(|(new, &old)| (old, new)).collect();

    let reader = skin.reader(|b| Some(&buffers[b.index()].0[..]));
    let ibms: Vec<Mat4> = reader.read_inverse_bind_matrices().map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect()).unwrap_or_else(|| vec![Mat4::IDENTITY; gltf_joints.len()]);

    // the transform of the nodes above the top joint(s)
    let top = gltf_joints[order[0]].index();
    let mut root = Mat4::IDENTITY;
    let mut n = parent_of.get(&top).copied();
    let nodes: Vec<gltf::Node> = doc.nodes().collect();
    while let Some(i) = n {
        root = trs(&nodes[i]).matrix() * root;
        n = parent_of.get(&i).copied();
    }

    let joints: Vec<Joint> = order
        .iter()
        .map(|&old| {
            let node = &gltf_joints[old];
            let parent = parent_of.get(&node.index()).and_then(|p| in_skin.get(p)).map(|p| new_of_skin[p]);
            Joint { name: node.name().unwrap_or("").to_string(), parent, rest: trs(node) }
        })
        .collect();
    let inverse_bind: Vec<Mat4> = order.iter().map(|&old| ibms[old]).collect();
    let skeleton = Skeleton { joints, inverse_bind, root };

    // skinned primitives (every mesh on a node that uses this skin)
    let mut primitives = Vec::new();
    for node in doc.nodes().filter(|n| n.skin().is_some_and(|s| s.index() == skin.index())) {
        let Some(mesh) = node.mesh() else { continue };
        for (pi, prim) in mesh.primitives().enumerate() {
            let r = prim.reader(|b| Some(&buffers[b.index()].0[..]));
            let Some(pos) = r.read_positions() else { continue };
            let positions: Vec<[f32; 3]> = pos.collect();
            let n = positions.len();
            let normals: Vec<[f32; 3]> = r.read_normals().map(|x| x.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; n]);
            let tangents: Vec<[f32; 4]> = r.read_tangents().map(|x| x.collect()).unwrap_or_default();
            // the weights in sets of four (JOINTS_0/WEIGHTS_0, JOINTS_1/WEIGHTS_1), up to INFLUENCES
            let mut joints = vec![[0u16; INFLUENCES]; n];
            let mut weights = vec![[0f32; INFLUENCES]; n];
            for set in 0..(INFLUENCES / 4) as u32 {
                let (Some(js), Some(ws)) = (r.read_joints(set), r.read_weights(set)) else {
                    if set == 0 {
                        return Err(anyhow!("{}: a skinned primitive has no JOINTS_0/WEIGHTS_0", path.display()));
                    }
                    break;
                };
                let base = set as usize * 4;
                for (i, (j, w)) in js.into_u16().zip(ws.into_f32()).enumerate().take(n) {
                    for k in 0..4 {
                        joints[i][base + k] = new_of_skin.get(&(j[k] as usize)).copied().unwrap_or(0) as u16;
                        weights[i][base + k] = w[k];
                    }
                }
            }
            for w in &mut weights {
                let s: f32 = w.iter().sum();
                if s > 0.0 {
                    w.iter_mut().for_each(|x| *x /= s);
                } else {
                    w[0] = 1.0;
                }
            }
            primitives.push(SkinnedPrimitive {
                name: format!("{}#{pi}", mesh.name().unwrap_or("mesh")),
                mesh: SkinnedMesh { positions, normals, tangents, joints, weights },
                uvs: r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.0; 2]; n]),
                indices: r.read_indices().map(|i| i.into_u32().collect()).unwrap_or_else(|| (0..n as u32).collect()),
                material: prim.material().index(),
            });
        }
    }

    // animations: the tracks on the skin's joints
    let mut clips = Vec::new();
    for a in doc.animations() {
        let mut tracks = Vec::new();
        let mut duration = 0.0f32;
        for ch in a.channels() {
            let Some(&old) = in_skin.get(&ch.target().node().index()) else { continue };
            let r = ch.reader(|b| Some(&buffers[b.index()].0[..]));
            let Some(times) = r.read_inputs() else { continue };
            let times: Vec<f32> = times.collect();
            duration = duration.max(times.last().copied().unwrap_or(0.0));
            use gltf::animation::util::ReadOutputs;
            let (what, values): (Values, Vec<[f32; 4]>) = match r.read_outputs() {
                Some(ReadOutputs::Translations(v)) => (Values::Translation, v.map(|x| [x[0], x[1], x[2], 0.0]).collect()),
                Some(ReadOutputs::Scales(v)) => (Values::Scale, v.map(|x| [x[0], x[1], x[2], 0.0]).collect()),
                Some(ReadOutputs::Rotations(v)) => (Values::Rotation, v.into_f32().collect()),
                _ => continue,
            };
            // (cubic spline keys carry in/out tangents: keep the values only)
            let values = if values.len() == times.len() * 3 { values.chunks(3).map(|c| c[1]).collect() } else { values };
            tracks.push(Track { joint: new_of_skin[&old], what, times, values });
        }
        clips.push(Clip { name: a.name().unwrap_or("").to_string(), duration, tracks });
    }
    Ok(Loaded { skeleton, primitives, clips })
}
