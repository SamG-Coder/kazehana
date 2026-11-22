//! A minimal glTF 2.0 writer for the card and pack props (separate .bin, PNG textures beside it in
//! textures/). Geometry is baked: one mesh, one primitive per material.

use std::path::Path;

#[derive(Clone, Default)]
pub struct Prim {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub idx: Vec<u32>,
}

impl Prim {
    /// Append another primitive's geometry (same material).
    pub fn append(&mut self, o: &Prim) {
        let base = self.pos.len() as u32;
        self.pos.extend_from_slice(&o.pos);
        self.nrm.extend_from_slice(&o.nrm);
        self.uv.extend_from_slice(&o.uv);
        self.idx.extend(o.idx.iter().map(|i| i + base));
    }
    /// Smooth vertex normals from the triangles.
    pub fn smooth_normals(&mut self) {
        let mut acc = vec![[0.0f32; 3]; self.pos.len()];
        for t in self.idx.chunks_exact(3) {
            let (a, b, c) = (self.pos[t[0] as usize], self.pos[t[1] as usize], self.pos[t[2] as usize]);
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            for &i in t {
                for k in 0..3 {
                    acc[i as usize][k] += n[k];
                }
            }
        }
        self.nrm = acc.into_iter().map(norm).collect();
    }
    /// Apply a transform to positions and normals: p' = m * p + t (m orthonormal-ish).
    pub fn transform(&mut self, m: [[f32; 3]; 3], t: [f32; 3]) {
        let mul = |v: [f32; 3]| [m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2], m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2], m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2]];
        for p in self.pos.iter_mut() {
            let q = mul(*p);
            *p = [q[0] + t[0], q[1] + t[1], q[2] + t[2]];
        }
        for n in self.nrm.iter_mut() {
            *n = norm(mul(*n));
        }
    }
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-12 { [v[0] / l, v[1] / l, v[2] / l] } else { [0.0, 1.0, 0.0] }
}

/// Per-vertex tangents (accumulated triangle tangents, Gram-Schmidt, handedness in w).
fn tangents(p: &Prim) -> Vec<[f32; 4]> {
    let n = p.pos.len();
    let mut tan = vec![[0.0f32; 3]; n];
    let mut bit = vec![[0.0f32; 3]; n];
    for t in p.idx.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let (p0, p1, p2) = (p.pos[a], p.pos[b], p.pos[c]);
        let (u0, u1, u2) = (p.uv[a], p.uv[b], p.uv[c]);
        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let (du1, dv1, du2, dv2) = (u1[0] - u0[0], u1[1] - u0[1], u2[0] - u0[0], u2[1] - u0[1]);
        let det = du1 * dv2 - du2 * dv1;
        if det.abs() < 1e-14 {
            continue;
        }
        let r = 1.0 / det;
        for k in 0..3 {
            let tk = (e1[k] * dv2 - e2[k] * dv1) * r;
            let bk = (e2[k] * du1 - e1[k] * du2) * r;
            for i in [a, b, c] {
                tan[i][k] += tk;
                bit[i][k] += bk;
            }
        }
    }
    (0..n)
        .map(|i| {
            let nn = p.nrm[i];
            let t = tan[i];
            let d = nn[0] * t[0] + nn[1] * t[1] + nn[2] * t[2];
            let mut o = [t[0] - nn[0] * d, t[1] - nn[1] * d, t[2] - nn[2] * d];
            let l = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();
            if l < 1e-12 {
                // any vector perpendicular to the normal
                o = if nn[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
                let d = nn[0] * o[0] + nn[1] * o[1] + nn[2] * o[2];
                o = norm([o[0] - nn[0] * d, o[1] - nn[1] * d, o[2] - nn[2] * d]);
            } else {
                o = [o[0] / l, o[1] / l, o[2] / l];
            }
            let c = [nn[1] * o[2] - nn[2] * o[1], nn[2] * o[0] - nn[0] * o[2], nn[0] * o[1] - nn[1] * o[0]];
            let w = if c[0] * bit[i][0] + c[1] * bit[i][1] + c[2] * bit[i][2] < 0.0 { -1.0 } else { 1.0 };
            [o[0], o[1], o[2], w]
        })
        .collect()
}

pub struct Mat {
    pub name: String,
    /// File names inside textures/.
    pub base: String,
    pub normal: Option<String>,
    pub orm: Option<String>,
    pub normal_scale: f32,
    pub double_sided: bool,
}

/// Write `<dir>/<id>.gltf` + `<id>.bin`; textures must already be in `<dir>/textures/`.
pub fn write(dir: &Path, id: &str, prims: &[(usize, Prim)], mats: &[Mat]) {
    std::fs::create_dir_all(dir).unwrap();
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accs = Vec::new();
    let mut prim_json = Vec::new();
    let mut push = |bin: &mut Vec<u8>, bytes: &[u8], target: u32| -> usize {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let off = bin.len();
        bin.extend_from_slice(bytes);
        views.push(format!("{{\"buffer\":0,\"byteOffset\":{off},\"byteLength\":{},\"target\":{target}}}", bytes.len()));
        views.len() - 1
    };
    for (mat, p) in prims {
        let tan = tangents(p);
        let f = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        let pos: Vec<f32> = p.pos.iter().flatten().copied().collect();
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for q in &p.pos {
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let vp = push(&mut bin, &f(&pos), 34962);
        let vn = push(&mut bin, &f(&p.nrm.iter().flatten().copied().collect::<Vec<_>>()), 34962);
        let vt = push(&mut bin, &f(&tan.iter().flatten().copied().collect::<Vec<_>>()), 34962);
        let vu = push(&mut bin, &f(&p.uv.iter().flatten().copied().collect::<Vec<_>>()), 34962);
        let vi = push(&mut bin, &p.idx.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>(), 34963);
        let n = p.pos.len();
        let a0 = accs.len();
        accs.push(format!("{{\"bufferView\":{vp},\"componentType\":5126,\"count\":{n},\"type\":\"VEC3\",\"min\":[{},{},{}],\"max\":[{},{},{}]}}", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]));
        accs.push(format!("{{\"bufferView\":{vn},\"componentType\":5126,\"count\":{n},\"type\":\"VEC3\"}}"));
        accs.push(format!("{{\"bufferView\":{vt},\"componentType\":5126,\"count\":{n},\"type\":\"VEC4\"}}"));
        accs.push(format!("{{\"bufferView\":{vu},\"componentType\":5126,\"count\":{n},\"type\":\"VEC2\"}}"));
        accs.push(format!("{{\"bufferView\":{vi},\"componentType\":5125,\"count\":{},\"type\":\"SCALAR\"}}", p.idx.len()));
        prim_json.push(format!(
            "{{\"attributes\":{{\"POSITION\":{},\"NORMAL\":{},\"TANGENT\":{},\"TEXCOORD_0\":{}}},\"indices\":{},\"material\":{mat}}}",
            a0,
            a0 + 1,
            a0 + 2,
            a0 + 3,
            a0 + 4
        ));
    }
    // textures (one image per distinct file)
    let mut images: Vec<String> = Vec::new();
    let mut tex = |name: &str| -> usize {
        if let Some(i) = images.iter().position(|n| n == name) {
            i
        } else {
            images.push(name.to_string());
            images.len() - 1
        }
    };
    let mut mat_json = Vec::new();
    for m in mats {
        let b = tex(&m.base);
        let mut s = format!("{{\"name\":\"{}\",\"doubleSided\":{},\"pbrMetallicRoughness\":{{\"baseColorTexture\":{{\"index\":{b}}}", m.name, m.double_sided);
        if let Some(o) = &m.orm {
            let i = tex(o);
            s += &format!(",\"metallicRoughnessTexture\":{{\"index\":{i}}},\"metallicFactor\":1.0,\"roughnessFactor\":1.0}}");
            s += &format!(",\"occlusionTexture\":{{\"index\":{i}}}");
        } else {
            s += ",\"metallicFactor\":0.0,\"roughnessFactor\":0.05}";
        }
        if let Some(nm) = &m.normal {
            let i = tex(nm);
            s += &format!(",\"normalTexture\":{{\"index\":{i},\"scale\":{}}}", m.normal_scale);
        }
        s += "}";
        mat_json.push(s);
    }
    let img_json: Vec<String> = images.iter().map(|n| format!("{{\"uri\":\"textures/{n}\"}}")).collect();
    let tex_json: Vec<String> = (0..images.len()).map(|i| format!("{{\"sampler\":0,\"source\":{i}}}")).collect();
    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"tools/cardgen props\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"name\":\"{id}\",\"mesh\":0}}],\"meshes\":[{{\"name\":\"{id}\",\"primitives\":[{}]}}],\"materials\":[{}],\"textures\":[{}],\"images\":[{}],\"samplers\":[{{\"magFilter\":9729,\"minFilter\":9987,\"wrapS\":33071,\"wrapT\":33071}}],\"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"uri\":\"{id}.bin\",\"byteLength\":{}}}]}}",
        prim_json.join(","),
        mat_json.join(","),
        tex_json.join(","),
        img_json.join(","),
        accs.join(","),
        views.join(","),
        bin.len()
    );
    std::fs::write(dir.join(format!("{id}.bin")), &bin).unwrap();
    std::fs::write(dir.join(format!("{id}.gltf")), json).unwrap();
}
