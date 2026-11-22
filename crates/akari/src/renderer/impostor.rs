//! Plant impostors, ported from SmartImposter: a far plant is one AABB of a cell BLAS, and the
//! intersection shader (shaders/impostor.rint) ray-marches a baked atlas of its views.
//!
//! * `bake_impostor`: renders a plant variant (a list of mesh instances) from 12 x 12
//!   hemi-octahedral directions with the frame's own hit shaders (shaders/bake.rgen), builds
//!   coverage-weighted mips, BC-compresses them on the CPU and caches the result
//!   (assets/cache/impostors), then uploads albedo (BC7), normal (BC5) and depth + coverage (BC5).
//! * `set_plants`: every plant's record and box, grouped into 128 m cells (one BLAS each).
//! * `set_plant_state`: per plant, whether its real mesh is drawn this frame (the impostor then
//!   steps aside, so near plants are never doubled).

use super::*;
use crate::scene::MeshId;
use glam::{Affine3A, Vec3};
use std::path::PathBuf;

const GRID: u32 = 12;
const CACHE_DIR: &str = "assets/cache/impostors";
const CACHE_VERSION: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
pub(super) struct ImpInfoGpu {
    center_radius: [f32; 4],
    bmin: [f32; 4],
    bmax: [f32; 4],
    frame: [f32; 4],
    grid: u32,
    res: u32,
    mips: u32,
    tex: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PlantGpu {
    r0: [f32; 4],
    r1: [f32; 4],
    r2: [f32; 4],
    data: u32,
}

/// A baked plant variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImpostorVariant(pub u32);

/// A plant drawn as an impostor when far: where, which variant, a seed for colour variation.
#[derive(Clone, Copy, Debug)]
pub struct PlantPlacement {
    pub transform: Affine3A,
    pub variant: ImpostorVariant,
    pub seed: u32,
}

/// A group of plants placed together (the town's trees; the grass ring round the camera, which
/// is replaced as the camera moves). Each has its own cell BLASes, records and state.
#[derive(Default)]
struct Group {
    cells: Vec<(Accel, Buffer, u32)>,
    recs: Vec<PlantGpu>,
    /// record index (within the group) of each plant, in placement order
    order: Vec<u32>,
    state: Vec<u32>,
    /// first record of the group in the shared buffer
    base: u32,
}

pub(super) struct Impostors {
    pub(super) info_buf: Buffer,
    pub(super) plant_buf: Buffer,
    pub(super) state_buf: Buffer,
    infos: Vec<ImpInfoGpu>,
    groups: Vec<Group>,
    bake: Option<RtPipeline>,
    pub(super) visible: bool,
}

impl Impostors {
    pub(super) fn new(gpu: &Gpu) -> Result<Self> {
        let dummy = |name| gpu.upload_buffer(&[0u8; 256], vk::BufferUsageFlags::STORAGE_BUFFER, name);
        Ok(Self {
            info_buf: dummy("impostor info")?,
            plant_buf: dummy("plants")?,
            state_buf: gpu.create_buffer(256, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::CpuToGpu, "plant state")?,
            infos: Vec::new(),
            groups: Vec::new(),
            bake: None,
            visible: true,
        })
    }

    pub(super) fn push_instances(&self, insts: &mut Vec<vk::AccelerationStructureInstanceKHR>) {
        if !self.visible {
            return;
        }
        let flags = vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE;
        for g in &self.groups {
            for (blas, _, first) in &g.cells {
                insts.push(accel::make_instance(&Affine3A::IDENTITY, g.base + *first, 0xFF, 1, flags, blas.addr));
            }
        }
    }

    fn clear_group(gpu: &Gpu, g: &mut Group) {
        for (mut a, mut b, _) in g.cells.drain(..) {
            accel::destroy_accel(gpu, &mut a);
            gpu.destroy_buffer(&mut b);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        for g in self.groups.iter_mut() {
            Self::clear_group(gpu, g);
        }
        for b in [&mut self.info_buf, &mut self.plant_buf, &mut self.state_buf] {
            gpu.destroy_buffer(b);
        }
        if let Some(mut p) = self.bake.take() {
            unsafe { gpu.device.destroy_pipeline(p.pipe, None) };
            gpu.destroy_buffer(&mut p.sbt);
        }
    }

    /// Write every group's state into the GPU buffer.
    fn write_state(&mut self) {
        let total: usize = self.groups.iter().map(|g| g.state.len()).sum();
        let dst: &mut [u32] = bytemuck::cast_slice_mut(&mut self.state_buf.mapped()[..total.max(1) * 4]);
        for g in &self.groups {
            dst[g.base as usize..g.base as usize + g.state.len()].copy_from_slice(&g.state);
        }
    }
}

fn f16_to_f32(h: u16) -> f32 {
    let s = ((h >> 15) & 1) as u32;
    let e = ((h >> 10) & 0x1F) as u32;
    let m = (h & 0x3FF) as u32;
    let bits = if e == 0 {
        if m == 0 { s << 31 } else {
            // subnormal
            let mut e2 = 127 - 15 + 1;
            let mut m2 = m;
            while m2 & 0x400 == 0 {
                m2 <<= 1;
                e2 -= 1;
            }
            (s << 31) | ((e2 as u32) << 23) | ((m2 & 0x3FF) << 13)
        }
    } else if e == 31 {
        (s << 31) | 0x7F80_0000 | (m << 13)
    } else {
        (s << 31) | ((e + 127 - 15) << 23) | (m << 13)
    };
    f32::from_bits(bits)
}

/// BC-encode an image on all cores (block rows are independent).
fn encode_parallel(w: u32, h: u32, rgba: &[u8], bc5: bool) -> Vec<u8> {
    let rows = h / 4;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) as u32;
    let per = rows.div_ceil(threads).max(1);
    let mut parts: Vec<Vec<u8>> = Vec::new();
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..rows)
            .step_by(per as usize)
            .map(|r0| {
                let r1 = (r0 + per).min(rows);
                let slice = &rgba[(r0 * 4 * w * 4) as usize..(r1 * 4 * w * 4) as usize];
                s.spawn(move || if bc5 { crate::bc7::encode_channels(w, (r1 - r0) * 4, slice, &[0, 1]) } else { crate::bc7::encode_image(w, (r1 - r0) * 4, slice) })
            })
            .collect();
        parts = hs.into_iter().map(|h| h.join().unwrap()).collect();
    });
    parts.concat()
}

/// Atlas levels, coverage-weighted: empty texels do not drag colour, normal or depth towards 0.
/// Each level: (albedo rgba, normal rg in rgba, depth + coverage in rgba).
fn mip_chain(w: u32, h: u32, alb: Vec<u8>, nrm: Vec<u8>, dep: Vec<u8>, mips: u32) -> Vec<(u32, u32, Vec<u8>, Vec<u8>, Vec<u8>)> {
    let mut out = vec![(w, h, alb, nrm, dep)];
    for _ in 1..mips {
        let (pw, ph, pa, pn, pd) = out.last().unwrap();
        let (w2, h2) = (pw / 2, ph / 2);
        let mut a2 = vec![0u8; (w2 * h2 * 4) as usize];
        let mut n2 = vec![0u8; (w2 * h2 * 4) as usize];
        let mut d2 = vec![0u8; (w2 * h2 * 4) as usize];
        for y in 0..h2 {
            for x in 0..w2 {
                let mut cw = 0.0f32;
                let mut acc = [0.0f32; 4];
                let mut nacc = Vec3::ZERO;
                let mut dacc = 0.0f32;
                let mut cov = 0.0f32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = (((y * 2 + dy) * pw + x * 2 + dx) * 4) as usize;
                    let c = pd[i + 1] as f32 / 255.0;
                    cov += c * 0.25;
                    if c <= 0.0 {
                        continue;
                    }
                    cw += c;
                    for k in 0..4 {
                        acc[k] += pa[i + k] as f32 * c;
                    }
                    let e = glam::Vec2::new(pn[i] as f32 / 127.5 - 1.0, pn[i + 1] as f32 / 127.5 - 1.0);
                    nacc += oct_decode(e) * c;
                    dacc += pd[i] as f32 * c;
                }
                let o = ((y * w2 + x) * 4) as usize;
                if cw > 0.0 {
                    for k in 0..4 {
                        a2[o + k] = (acc[k] / cw).round() as u8;
                    }
                    let e = oct_encode(nacc.normalize_or(Vec3::Y));
                    n2[o] = ((e.x * 0.5 + 0.5) * 255.0).round() as u8;
                    n2[o + 1] = ((e.y * 0.5 + 0.5) * 255.0).round() as u8;
                    d2[o] = (dacc / cw).round() as u8;
                } else {
                    n2[o] = 128;
                    n2[o + 1] = 128;
                }
                d2[o + 1] = (cov * 255.0).round() as u8;
                n2[o + 3] = 255;
                d2[o + 3] = 255;
            }
        }
        out.push((w2, h2, a2, n2, d2));
    }
    out
}

fn oct_encode(n: Vec3) -> glam::Vec2 {
    let n = n / (n.x.abs() + n.y.abs() + n.z.abs());
    let mut p = glam::Vec2::new(n.x, n.z);
    if n.y < 0.0 {
        let s = |v: f32| if v >= 0.0 { 1.0 } else { -1.0 };
        p = glam::Vec2::new((1.0 - p.y.abs()) * s(p.x), (1.0 - p.x.abs()) * s(p.y));
    }
    p
}

fn oct_decode(e: glam::Vec2) -> Vec3 {
    let mut v = Vec3::new(e.x, 1.0 - e.x.abs() - e.y.abs(), e.y);
    if v.y < 0.0 {
        let s = |x: f32| if x >= 0.0 { 1.0 } else { -1.0 };
        let (px, pz) = ((1.0 - v.z.abs()) * s(v.x), (1.0 - v.x.abs()) * s(v.z));
        v.x = px;
        v.z = pz;
    }
    v.normalize_or(Vec3::Y)
}

struct Baked {
    info: ImpInfoGpu,
    w: u32,
    h: u32,
    /// per texture (albedo, normal, depth): its levels
    levels: [Vec<Vec<u8>>; 3],
}

fn cache_path(name: &str, key: u64) -> PathBuf {
    let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    std::path::Path::new(CACHE_DIR).join(format!("{clean}-{key:016x}.imp"))
}

fn read_cache(p: &std::path::Path) -> Option<Baked> {
    let b = std::fs::read(p).ok()?;
    if b.len() < 8 + 80 + 12 || &b[..8] != b"AKIMPST2" {
        return None;
    }
    let info: ImpInfoGpu = *bytemuck::from_bytes(&b[8..88]);
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let (w, h, mips) = (u(88), u(92), u(96));
    let mut o = 100;
    let mut levels: [Vec<Vec<u8>>; 3] = Default::default();
    for t in 0..3 {
        for m in 0..mips {
            let n = (((w >> m) / 4) * ((h >> m) / 4) * 16) as usize;
            levels[t].push(b.get(o..o + n)?.to_vec());
            o += n;
        }
    }
    Some(Baked { info, w, h, levels })
}

fn write_cache(p: &std::path::Path, k: &Baked) {
    let mut out = Vec::new();
    out.extend_from_slice(b"AKIMPST2");
    out.extend_from_slice(bytemuck::bytes_of(&k.info));
    for v in [k.w, k.h, k.levels[0].len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for t in &k.levels {
        for l in t {
            out.extend_from_slice(l);
        }
    }
    let _ = std::fs::create_dir_all(CACHE_DIR);
    let tmp = p.with_extension("part");
    if std::fs::write(&tmp, out).is_ok() {
        let _ = std::fs::rename(&tmp, p);
    }
}

impl Renderer {
    /// Bake (or load from the cache) the impostor of a plant variant: `parts` are its mesh
    /// instances in plant space (Y up, origin at the root). `foliage_only`: grass (no bark);
    /// `transmission`: the leaves' diffuse transmission. `name` keys the cache together with the
    /// geometry (counts and bounds) and the bake shaders.
    pub fn bake_impostor(&mut self, name: &str, parts: &[(MeshId, Affine3A)], foliage_only: bool, transmission: f32) -> Result<ImpostorVariant> {
        // bounds of the plant
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (m, x) in parts {
            let (a, b) = self.meshes[m.0 as usize].bounds;
            for k in 0..8 {
                let c = Vec3::new(if k & 1 == 0 { a.x } else { b.x }, if k & 2 == 0 { a.y } else { b.y }, if k & 4 == 0 { a.z } else { b.z });
                let p = x.transform_point3(c);
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        let c = (lo + hi) * 0.5;
        let r = ((hi - lo) * 0.5).length();
        let across = ((hi.x - lo.x).hypot(hi.z - lo.z) * 0.5).min(r);
        let texel = (2.0 * r / 224.0).clamp(0.03, 0.3);
        let res_a = ((2.0 * across / texel / 32.0).ceil() as u32 * 32).clamp(32, 256);
        let res_u = ((2.0 * r / texel / 32.0).ceil() as u32 * 32).clamp(32, 256);
        let mips = 4u32;
        let mut info = ImpInfoGpu {
            center_radius: [c.x, c.y, c.z, r],
            bmin: [lo.x, lo.y, lo.z, if foliage_only { 1.0 } else { 0.0 }],
            bmax: [hi.x, hi.y, hi.z, 2.0 * across / res_a as f32],
            frame: [across, r, transmission, 0.0],
            grid: GRID,
            res: res_a | (res_u << 16),
            mips,
            tex: 0,
        };
        use std::hash::{Hash, Hasher};
        let mut hsh = std::collections::hash_map::DefaultHasher::new();
        (name, parts.len(), CACHE_VERSION, foliage_only, (transmission * 1000.0) as i32).hash(&mut hsh);
        for v in [lo, hi] {
            for k in 0..3 {
                ((v[k] * 1000.0) as i32).hash(&mut hsh);
            }
        }
        spv!("bake.rgen").len().hash(&mut hsh);
        spv!("hit.rchit").hash(&mut hsh);
        let path = cache_path(name, hsh.finish());

        let baked = match read_cache(&path) {
            Some(b) => b,
            None => {
                let t0 = Instant::now();
                let b = self.bake_now(&info, parts, res_a, res_u, mips, (GRID, GRID))?;
                write_cache(&path, &b);
                println!("impostor {name}: baked {}x{} in {:.1}s", b.w, b.h, t0.elapsed().as_secs_f32());
                b
            }
        };
        // upload: albedo BC7, normal BC5, depth BC5 (consecutive textures)
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        let first = self.textures.len() as u32;
        for (t, fmt) in [(0usize, vk::Format::BC7_UNORM_BLOCK), (1, vk::Format::BC5_UNORM_BLOCK), (2, vk::Format::BC5_UNORM_BLOCK)] {
            let img = crate::textures::upload_levels(&self.gpu, baked.w, baked.h, &baked.levels[t], fmt, name)?;
            self.textures.push(img);
        }
        info = baked.info;
        info.tex = first;
        self.imp.infos.push(info);
        self.upload_imp_infos()?;
        Ok(ImpostorVariant(self.imp.infos.len() as u32 - 1))
    }

    fn upload_imp_infos(&mut self) -> Result<()> {
        let mut old = std::mem::replace(&mut self.imp.info_buf, self.gpu.upload_buffer(bytemuck::cast_slice(&self.imp.infos), vk::BufferUsageFlags::STORAGE_BUFFER, "impostor info")?);
        self.gpu.destroy_buffer(&mut old);
        self.write_rt_set();
        Ok(())
    }

    /// Bake `parts` into views of res_a x res_u texels, `views` (across, down) of them in the atlas.
    fn bake_now(&mut self, info: &ImpInfoGpu, parts: &[(MeshId, Affine3A)], res_a: u32, res_u: u32, mips: u32, views: (u32, u32)) -> Result<Baked> {
        let (w, h) = (views.0 * res_a, views.1 * res_u);
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        self.sync_scene_buffers()?;
        // the variant's info in slot `n` (bake.rgen reads imps[g.opts.w])
        let slot = self.imp.infos.len() as u32;
        self.imp.infos.push(*info);
        self.upload_imp_infos()?;
        if self.imp.bake.is_none() {
            self.imp.bake = Some(unsafe { Self::create_rt_pipeline(&self.gpu, self.rt_layout, spv!("bake.rgen"))? });
        }
        // a TLAS with just this plant
        let mut tlas = Tlas::new(&self.gpu, parts.len() as u32)?;
        let insts: Vec<_> = parts
            .iter()
            .map(|(m, x)| {
                let gm = &self.meshes[m.0 as usize];
                let flags = vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE
                    | if gm.alpha { vk::GeometryInstanceFlagsKHR::FORCE_NO_OPAQUE } else { vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE };
                accel::make_instance(x, m.0, 0xFF, 0, flags, gm.blas.addr)
            })
            .collect();
        tlas.write(&insts);
        let usage = vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC;
        let mut alb = self.gpu.create_image(w, h, 1, vk::Format::R8G8B8A8_UNORM, usage, "bake albedo")?;
        let mut nd = self.gpu.create_image(w, h, 1, vk::Format::R16G16B16A16_SFLOAT, usage, "bake normal depth")?;
        let mut rb_a = self.gpu.create_buffer((w * h * 4) as u64, vk::BufferUsageFlags::TRANSFER_DST, MemoryLocation::GpuToCpu, "bake readback")?;
        let mut rb_n = self.gpu.create_buffer((w * h * 8) as u64, vk::BufferUsageFlags::TRANSFER_DST, MemoryLocation::GpuToCpu, "bake readback")?;
        // globals: the slot to bake
        let g = Globals { opts: [1, 0, 1, slot], ..Default::default() }; // z = 1: baking (rt.glsl leafVariation)
        self.ubo.mapped()[..std::mem::size_of::<Globals>()].copy_from_slice(bytemuck::bytes_of(&g));
        // bind the bake TLAS and targets in place of the frame's (restored below)
        unsafe {
            let mut as_info = vk::WriteDescriptorSetAccelerationStructureKHR::default().acceleration_structures(std::slice::from_ref(&tlas.accel.raw));
            let ia = [vk::DescriptorImageInfo::default().image_view(alb.view).image_layout(vk::ImageLayout::GENERAL)];
            let ib = [vk::DescriptorImageInfo::default().image_view(nd.view).image_layout(vk::ImageLayout::GENERAL)];
            let w_ = |b: u32, ty| vk::WriteDescriptorSet::default().dst_set(self.rt_set).dst_binding(b).descriptor_type(ty);
            let writes = [
                w_(0, vk::DescriptorType::ACCELERATION_STRUCTURE_KHR).descriptor_count(1).push_next(&mut as_info),
                w_(8, vk::DescriptorType::STORAGE_IMAGE).image_info(&ia),
                w_(9, vk::DescriptorType::STORAGE_IMAGE).image_info(&ib),
            ];
            self.gpu.device.update_descriptor_sets(&writes, &[]);
        }
        let bake = self.imp.bake.as_ref().unwrap();
        let (pipe, rgen, miss, hit) = (bake.pipe, bake.rgen, bake.miss, bake.hit);
        let (layout, set) = (self.rt_layout, self.rt_set);
        let gpu = &self.gpu;
        gpu.one_time(|cb| unsafe {
            let d = &gpu.device;
            tlas.cmd_build(gpu, cb, insts.len() as u32);
            let all = vk::PipelineStageFlags2::ALL_COMMANDS;
            gpu.memory_barrier(cb, all, vk::AccessFlags2::MEMORY_WRITE, all, vk::AccessFlags2::MEMORY_READ);
            for img in [alb.raw, nd.raw] {
                gpu.image_barrier(cb, img, vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL, vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE, all, vk::AccessFlags2::SHADER_WRITE);
            }
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::RAY_TRACING_KHR, pipe);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::RAY_TRACING_KHR, layout, 0, &[set], &[]);
            gpu.rt_fn.cmd_trace_rays(cb, &rgen, &miss, &hit, &vk::StridedDeviceAddressRegionKHR::default(), res_a, res_u, views.0 * views.1);
            for img in [alb.raw, nd.raw] {
                gpu.image_barrier(cb, img, vk::ImageLayout::GENERAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, all, vk::AccessFlags2::SHADER_WRITE, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_READ);
            }
            let region = vk::BufferImageCopy {
                image_subresource: vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 },
                image_extent: vk::Extent3D { width: w, height: h, depth: 1 },
                ..Default::default()
            };
            d.cmd_copy_image_to_buffer(cb, alb.raw, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, rb_a.raw, &[region]);
            d.cmd_copy_image_to_buffer(cb, nd.raw, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, rb_n.raw, &[region]);
        })?;
        let a: Vec<u8> = rb_a.mapped()[..(w * h * 4) as usize].to_vec();
        let ndraw: Vec<u16> = bytemuck::cast_slice::<u8, u16>(&rb_n.mapped()[..(w * h * 8) as usize]).to_vec();
        for b in [&mut rb_a, &mut rb_n] {
            self.gpu.destroy_buffer(b);
        }
        self.gpu.destroy_image(&mut alb);
        self.gpu.destroy_image(&mut nd);
        tlas.destroy(&self.gpu);
        self.imp.infos.pop();
        self.upload_imp_infos()?; // (also restores the frame's descriptors)

        // CPU: normal and depth + coverage as 8-bit, mips, BC
        let n = (w * h) as usize;
        let mut nrm = vec![0u8; n * 4];
        let mut dep = vec![0u8; n * 4];
        for i in 0..n {
            let q = |k: usize| f16_to_f32(ndraw[i * 4 + k]).clamp(0.0, 1.0);
            nrm[i * 4] = (q(0) * 255.0).round() as u8;
            nrm[i * 4 + 1] = (q(1) * 255.0).round() as u8;
            nrm[i * 4 + 3] = 255;
            dep[i * 4] = (q(2) * 255.0).round() as u8;
            dep[i * 4 + 1] = (q(3) * 255.0).round() as u8;
            dep[i * 4 + 3] = 255;
        }
        let chain = mip_chain(w, h, a, nrm, dep, mips);
        let mut levels: [Vec<Vec<u8>>; 3] = Default::default();
        for (lw, lh, la, ln, ld) in &chain {
            levels[0].push(encode_parallel(*lw, *lh, la, false));
            levels[1].push(encode_parallel(*lw, *lh, ln, true));
            levels[2].push(encode_parallel(*lw, *lh, ld, true));
        }
        Ok(Baked { info: *info, w, h, levels })
    }

    /// Bake (or load from the cache) the lawn (crate::lawn): its square of blades, between copies
    /// of itself, seen from every view of the lawn's grid (bake.rgen: an info with grid 0 is the
    /// lawn), and make it the lawn every lawn surface shows. Its info: centre_radius = blade
    /// height, tile edge, steepest and flattest slope; bmin = azimuths, slopes, texels a view,
    /// gutter texels; bmax.x = how far the layer is drawn (m).
    pub fn bake_lawn(&mut self, p: &crate::lawn::LawnParams) -> Result<()> {
        use crate::lawn::*;
        let mat = self.add_material(crate::scene::Material { base_color: [1.0, 1.0, 1.0, 1.0], roughness: 0.55, flags: crate::scene::MAT_DOUBLE_SIDED, ..Default::default() });
        let mut mesh = tile(p);
        mesh.material = Some(mat);
        let id = self.add_meshes(&[mesh])?[0];
        let t = p.tile_size();
        let c = p.copies();
        let mut parts = Vec::new();
        for i in -c..=c {
            for j in -c..=c {
                parts.push((id, Affine3A::from_translation(Vec3::new(i as f32 * t, 0.0, j as f32 * t))));
            }
        }
        let mut info = ImpInfoGpu {
            center_radius: [p.height, t, SLOPE_MIN, SLOPE_MAX],
            bmin: [AZIMUTHS as f32, SLOPES as f32, RES as f32, GUTTER as f32],
            bmax: [220.0, 0.0, 0.0, 0.0],
            frame: [0.0; 4],
            grid: 0,
            res: RES | (RES << 16),
            mips: MIPS,
            tex: 0,
        };
        use std::hash::{Hash, Hasher};
        let mut hsh = std::collections::hash_map::DefaultHasher::new();
        ("lawn", (p.height * 1e4) as i32, p.blades, RES, GUTTER, AZIMUTHS, SLOPES, MIPS, CACHE_VERSION).hash(&mut hsh);
        spv!("bake.rgen").hash(&mut hsh);
        spv!("hit.rchit").hash(&mut hsh);
        let path = cache_path("lawn", hsh.finish());
        let baked = match read_cache(&path) {
            Some(b) => b,
            None => {
                let t0 = Instant::now();
                let b = self.bake_now(&info, &parts, RES, RES, MIPS, (AZIMUTHS, SLOPES))?;
                write_cache(&path, &b);
                println!("lawn: baked {}x{} in {:.1}s", b.w, b.h, t0.elapsed().as_secs_f32());
                b
            }
        };
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        let first = self.textures.len() as u32;
        for (k, fmt) in [(0usize, vk::Format::BC7_UNORM_BLOCK), (1, vk::Format::BC5_UNORM_BLOCK), (2, vk::Format::BC5_UNORM_BLOCK)] {
            let img = crate::textures::upload_levels(&self.gpu, baked.w, baked.h, &baked.levels[k], fmt, "lawn")?;
            self.textures.push(img);
        }
        info = baked.info;
        info.tex = first;
        self.imp.infos.push(info);
        self.upload_imp_infos()?;
        self.lawn = Some(self.imp.infos.len() as u32 - 1);
        Ok(())
    }

    /// The town's plants drawn as impostors when far (group 0). Replaces any earlier set.
    pub fn set_plants(&mut self, plants: &[PlantPlacement]) -> Result<()> {
        self.set_plant_group(0, plants)
    }

    /// Place a group of plants (replacing the group's earlier set; other groups stay as they are).
    pub fn set_plant_group(&mut self, group: usize, plants: &[PlantPlacement]) -> Result<()> {
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        while self.imp.groups.len() <= group {
            self.imp.groups.push(Group::default());
        }
        let mut g = std::mem::take(&mut self.imp.groups[group]);
        Impostors::clear_group(&self.gpu, &mut g);
        const CELL: f32 = 128.0;
        let mut by_cell: std::collections::BTreeMap<(i32, i32), Vec<usize>> = Default::default();
        for (i, p) in plants.iter().enumerate() {
            let t = p.transform.translation;
            by_cell.entry(((t.x / CELL).floor() as i32, (t.z / CELL).floor() as i32)).or_default().push(i);
        }
        g.recs = Vec::with_capacity(plants.len());
        g.order = vec![0u32; plants.len()];
        let mut geoms = Vec::new();
        let mut aabb_bufs = Vec::new();
        let mut firsts = Vec::new();
        for idx in by_cell.values() {
            let first = g.recs.len() as u32;
            let mut boxes: Vec<f32> = Vec::with_capacity(idx.len() * 6);
            for &i in idx {
                let p = &plants[i];
                let info = &self.imp.infos[p.variant.0 as usize];
                let m = p.transform.matrix3;
                let t = p.transform.translation;
                g.order[i] = g.recs.len() as u32;
                g.recs.push(PlantGpu {
                    r0: [m.x_axis.x, m.y_axis.x, m.z_axis.x, t.x],
                    r1: [m.x_axis.y, m.y_axis.y, m.z_axis.y, t.y],
                    r2: [m.x_axis.z, m.y_axis.z, m.z_axis.z, t.z],
                    data: (p.variant.0 & 0xFFFF) | (p.seed << 16),
                });
                let (a, b) = (Vec3::new(info.bmin[0], info.bmin[1], info.bmin[2]), Vec3::new(info.bmax[0], info.bmax[1], info.bmax[2]));
                let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                for c in 0..8 {
                    let corner = Vec3::new(if c & 1 == 0 { a.x } else { b.x }, if c & 2 == 0 { a.y } else { b.y }, if c & 4 == 0 { a.z } else { b.z });
                    let q = p.transform.transform_point3(corner);
                    lo = lo.min(q);
                    hi = hi.max(q);
                }
                boxes.extend_from_slice(&[lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]);
            }
            let buf = self.gpu.upload_buffer(bytemuck::cast_slice(&boxes), vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR, "plant boxes")?;
            geoms.push((accel::aabb_geometry(buf.addr), idx.len() as u32));
            aabb_bufs.push(buf);
            firsts.push(first);
        }
        if !geoms.is_empty() {
            let blases = accel::build_blases(&self.gpu, &geoms)?;
            for ((a, b), f) in blases.into_iter().zip(aabb_bufs).zip(firsts) {
                g.cells.push((a, b, f));
            }
        }
        g.state = vec![0u32; plants.len()];
        self.imp.groups[group] = g;

        // the shared record and state buffers: every group, one after another
        let mut base = 0u32;
        let mut all: Vec<PlantGpu> = Vec::new();
        for g in self.imp.groups.iter_mut() {
            g.base = base;
            base += g.recs.len() as u32;
            all.extend_from_slice(&g.recs);
        }
        let data: &[u8] = if all.is_empty() { &[0u8; 64] } else { bytemuck::cast_slice(&all) };
        let mut old = std::mem::replace(&mut self.imp.plant_buf, self.gpu.upload_buffer(data, vk::BufferUsageFlags::STORAGE_BUFFER, "plants")?);
        self.gpu.destroy_buffer(&mut old);
        if (all.len() * 4) as u64 > self.imp.state_buf.size {
            let mut old = std::mem::replace(
                &mut self.imp.state_buf,
                self.gpu.create_buffer((all.len().max(16) * 4 * 3 / 2) as u64, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::CpuToGpu, "plant state")?,
            );
            self.gpu.destroy_buffer(&mut old);
        }
        self.imp.write_state();
        self.write_rt_set();
        Ok(())
    }

    /// Per plant of group 0 (in `set_plants` order): true when its real mesh is drawn (no impostor).
    pub fn set_plant_state(&mut self, mesh: &[bool]) -> Result<()> {
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        let Some(g) = self.imp.groups.get_mut(0) else { return Ok(()) };
        for (i, &m) in mesh.iter().enumerate().take(g.order.len()) {
            let k = g.order[i] as usize;
            g.state[k] = m as u32;
        }
        self.imp.write_state();
        Ok(())
    }

    pub fn set_impostors_visible(&mut self, on: bool) {
        self.imp.visible = on;
    }
}
