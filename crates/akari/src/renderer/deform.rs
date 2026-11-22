//! Deformable meshes: meshes whose vertices the game moves every frame (simulated cloth).
//!
//! Such a mesh keeps its position, normal and tangent streams in host-visible memory, and its
//! BLAS is built to be updated (refitted) rather than compacted. `Renderer::update_mesh` hands
//! the new vertices over; at the next `render` they are written into the mapped buffers once the
//! previous frame is done with them (one frame in flight), and the BLAS refit is recorded at the
//! head of that frame's command buffer, before the TLAS build. Every `REBUILD_EVERY` refits it
//! is built afresh instead, so a cloth that has moved far from its first shape still traces
//! quickly.
//!
//! Motion vectors: each deformable mesh has a twin mesh slot (its "history") that points at
//! last frame's positions with the same triangles. An instance of a deformable mesh that moved
//! is given the history as its previous mesh (rt.glsl `InstMotion.prevMesh`), so the denoiser
//! sees exact per-pixel motion of the cloth even with object motion off.

use super::{GpuMesh, MeshInfoGpu, Renderer};
use crate::accel::{self, Accel};
use crate::gpu::{align_up, Buffer};
use crate::scene::*;
use anyhow::{anyhow, Result};
use ash::vk;
use glam::Vec3;
use gpu_allocator::MemoryLocation;

/// A full BLAS build every this many refits.
const REBUILD_EVERY: u32 = 120;

pub(super) struct Deformable {
    /// the mesh and its history twin (renderer mesh indices)
    pub mesh: u32,
    pub history: u32,
    vcount: u32,
    tris: u32,
    /// last frame's positions (copied into the history buffer when new ones arrive)
    last: Vec<[f32; 3]>,
    /// vertices handed over for the next frame
    pending: Option<(Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>)>,
    /// moved in the frame being rendered (its instances get motion vectors)
    pub moved: bool,
    /// its BLAS is behind its vertices (refit at the head of the next recorded frame)
    stale: bool,
    refits: u32,
    scratch: Buffer,
    scratch_addr: u64,
    pos_addr: u64,
    idx_addr: u64,
}

const UPDATE_FLAGS: vk::BuildAccelerationStructureFlagsKHR =
    vk::BuildAccelerationStructureFlagsKHR::from_raw(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE.as_raw() | vk::BuildAccelerationStructureFlagsKHR::ALLOW_UPDATE.as_raw());

impl Renderer {
    /// Add a mesh whose vertices will change every frame (cloth). Its positions, normals and
    /// tangents are replaced with [`Renderer::update_mesh`]; its UVs, colours and triangles stay.
    pub fn add_deformable_mesh(&mut self, m: &MeshData) -> Result<MeshId> {
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        if m.indices.is_empty() || m.positions.is_empty() {
            return Err(anyhow!("mesh '{}' is empty", m.name));
        }
        let geo = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR;
        let mapped = |r: &Renderer, data: &[u8], usage: vk::BufferUsageFlags, name: &str| -> Result<Buffer> {
            if data.is_empty() {
                return Ok(Buffer::none());
            }
            let mut b = r.gpu.create_buffer(data.len() as u64, usage, MemoryLocation::CpuToGpu, name)?;
            b.mapped()[..data.len()].copy_from_slice(data);
            Ok(b)
        };
        let up = |r: &Renderer, data: &[u8], usage| -> Result<Buffer> { if data.is_empty() { Ok(Buffer::none()) } else { r.gpu.upload_buffer(data, usage, &m.name) } };
        let pos = mapped(self, bytemuck::cast_slice(&m.positions), geo, &m.name)?;
        let prev = mapped(self, bytemuck::cast_slice(&m.positions), vk::BufferUsageFlags::STORAGE_BUFFER, &m.name)?;
        let nrm = mapped(self, bytemuck::cast_slice(&m.normals), vk::BufferUsageFlags::STORAGE_BUFFER, &m.name)?;
        let tan = mapped(self, bytemuck::cast_slice(&m.tangents), vk::BufferUsageFlags::STORAGE_BUFFER, &m.name)?;
        let idx = up(self, bytemuck::cast_slice(&m.indices), geo)?;
        let uv = up(self, bytemuck::cast_slice(&m.uvs), vk::BufferUsageFlags::STORAGE_BUFFER)?;
        let col = up(self, bytemuck::cast_slice(&m.colors), vk::BufferUsageFlags::STORAGE_BUFFER)?;
        let mat = m.material.map_or(0, |x| x.0);
        let mflags = self.materials.get(mat as usize).map_or(0, |x| x.flags);
        let alpha = mflags & (MAT_ALPHA_MASK | MAT_GLASS) != 0;
        let glass = mflags & MAT_GLASS != 0;
        let info = MeshInfoGpu { addr: [pos.addr, nrm.addr, uv.addr, tan.addr, col.addr, idx.addr], material: mat, flags: 0 };
        let hinfo = MeshInfoGpu { addr: [prev.addr, nrm.addr, uv.addr, tan.addr, col.addr, idx.addr], material: mat, flags: 0 };
        let (vcount, tris) = (m.positions.len() as u32, (m.indices.len() / 3) as u32);
        let mut g = accel::triangles_geometry(pos.addr, vcount, idx.addr);
        if alpha {
            g.flags = vk::GeometryFlagsKHR::NO_DUPLICATE_ANY_HIT_INVOCATION;
        }
        // an updatable BLAS (not compacted), with scratch for builds and updates
        let gs = [g];
        let info_q = vk::AccelerationStructureBuildGeometryInfoKHR::default().ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL).flags(UPDATE_FLAGS).mode(vk::BuildAccelerationStructureModeKHR::BUILD).geometries(&gs);
        let mut size = vk::AccelerationStructureBuildSizesInfoKHR::default();
        unsafe { self.gpu.accel_fn.get_acceleration_structure_build_sizes(vk::AccelerationStructureBuildTypeKHR::DEVICE, &info_q, &[tris], &mut size) };
        let blas = accel::create_accel(&self.gpu, vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL, size.acceleration_structure_size)?;
        let align = self.gpu.rt.scratch_alignment.max(256);
        let scratch = self.gpu.create_buffer(size.build_scratch_size.max(size.update_scratch_size) + align, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::GpuOnly, "deformable scratch")?;
        let scratch_addr = align_up(scratch.addr, align);
        self.gpu.one_time(|cb| record_build(self, cb, blas.raw, vk::AccelerationStructureKHR::null(), scratch_addr, pos.addr, vcount, idx.addr, tris, alpha))?;
        let bounds = m.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p))));
        let counts = (m.positions.len(), m.indices.len());
        let id = self.meshes.len() as u32;
        let (pos_addr, idx_addr) = (pos.addr, idx.addr);
        self.meshes.push(GpuMesh { bufs: vec![pos, idx, nrm, uv, tan, col, prev], blas, info, alpha, glass, bounds, counts });
        // the history twin: last frame's positions, never instanced itself
        self.meshes.push(GpuMesh { bufs: Vec::new(), blas: Accel::none(), info: hinfo, alpha, glass, bounds, counts });
        self.deform.push(Deformable { mesh: id, history: id + 1, vcount, tris, last: m.positions.clone(), pending: None, moved: false, stale: false, refits: 0, scratch, scratch_addr, pos_addr, idx_addr });
        self.scene_dirty = true;
        Ok(MeshId(id))
    }

    /// New vertices for a deformable mesh (same count as it was made with). `tangents` may be
    /// empty if the mesh has none. Takes effect at the next frame.
    pub fn update_mesh(&mut self, id: MeshId, positions: &[[f32; 3]], normals: &[[f32; 3]], tangents: &[[f32; 4]]) -> Result<()> {
        let d = self.deform.iter_mut().find(|d| d.mesh == id.0).ok_or_else(|| anyhow!("mesh {} is not deformable", id.0))?;
        if positions.len() != d.vcount as usize || normals.len() != d.vcount as usize || !(tangents.is_empty() || tangents.len() == d.vcount as usize) {
            return Err(anyhow!("update_mesh: {} vertices given, the mesh has {}", positions.len(), d.vcount));
        }
        match &mut d.pending {
            Some((p, n, t)) => {
                p.clear();
                p.extend_from_slice(positions);
                n.clear();
                n.extend_from_slice(normals);
                t.clear();
                t.extend_from_slice(tangents);
            }
            None => d.pending = Some((positions.to_vec(), normals.to_vec(), tangents.to_vec())),
        }
        Ok(())
    }

    /// Write the handed-over vertices into the mapped buffers (the previous frame is done with
    /// them). Called by `render` after its fence wait. Marks what moved.
    pub(super) fn write_deforms(&mut self) {
        for d in &mut self.deform {
            d.moved = false;
            let Some((p, n, t)) = d.pending.take() else { continue };
            let m = &mut self.meshes[d.mesh as usize];
            // bufs: pos, idx, nrm, uv, tan, col, prev
            m.bufs[6].mapped()[..d.last.len() * 12].copy_from_slice(bytemuck::cast_slice(&d.last));
            m.bufs[0].mapped()[..p.len() * 12].copy_from_slice(bytemuck::cast_slice(&p));
            m.bufs[2].mapped()[..n.len() * 12].copy_from_slice(bytemuck::cast_slice(&n));
            if !t.is_empty() && m.bufs[4].size >= (t.len() * 16) as u64 {
                m.bufs[4].mapped()[..t.len() * 16].copy_from_slice(bytemuck::cast_slice(&t));
            }
            d.last.copy_from_slice(&p);
            d.moved = true;
            d.stale = true;
        }
    }

    /// Record the BLAS refits (or rebuilds) of the meshes that moved, at the head of the frame.
    pub(super) fn record_deforms(&mut self, cb: vk::CommandBuffer) {
        let mut any = false;
        for k in 0..self.deform.len() {
            if !self.deform[k].stale {
                continue;
            }
            any = true;
            let d = &self.deform[k];
            let m = &self.meshes[d.mesh as usize];
            let rebuild = d.refits >= REBUILD_EVERY;
            let src = if rebuild { vk::AccelerationStructureKHR::null() } else { m.blas.raw };
            record_build(self, cb, m.blas.raw, src, d.scratch_addr, d.pos_addr, d.vcount, d.idx_addr, d.tris, m.alpha);
            let d = &mut self.deform[k];
            d.refits = if rebuild { 0 } else { d.refits + 1 };
            d.stale = false;
        }
        if any {
            // the refits done before the TLAS is built over them
            self.gpu.memory_barrier(
                cb,
                vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_KHR,
                vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR | vk::PipelineStageFlags2::RAY_TRACING_SHADER_KHR,
                vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR,
            );
        }
    }

    /// Is this mesh deformable and did it move in this frame? Its history twin, if so.
    pub(super) fn moved_history(&self, mesh: u32) -> Option<u32> {
        self.deform.iter().find(|d| d.mesh == mesh && d.moved).map(|d| d.history)
    }

    pub(super) fn destroy_deforms(&mut self) {
        for mut d in std::mem::take(&mut self.deform) {
            self.gpu.destroy_buffer(&mut d.scratch);
        }
    }
}

/// Record a BLAS build (`src` null) or an update of `src` into `dst` (here always the same one).
#[allow(clippy::too_many_arguments)]
fn record_build(r: &Renderer, cb: vk::CommandBuffer, dst: vk::AccelerationStructureKHR, src: vk::AccelerationStructureKHR, scratch: u64, pos: u64, vcount: u32, idx: u64, tris: u32, alpha: bool) {
    let mut g = accel::triangles_geometry(pos, vcount, idx);
    if alpha {
        g.flags = vk::GeometryFlagsKHR::NO_DUPLICATE_ANY_HIT_INVOCATION;
    }
    let gs = [g];
    let update = src != vk::AccelerationStructureKHR::null();
    let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
        .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
        .flags(UPDATE_FLAGS)
        .mode(if update { vk::BuildAccelerationStructureModeKHR::UPDATE } else { vk::BuildAccelerationStructureModeKHR::BUILD })
        .src_acceleration_structure(src)
        .dst_acceleration_structure(dst)
        .geometries(&gs)
        .scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch });
    let range = [vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(tris)];
    unsafe { r.gpu.accel_fn.cmd_build_acceleration_structures(cb, &[info], &[&range[..]]) };
}
