//! Bottom/top level acceleration structures. (Ported from SmartImposter.)

use crate::gpu::{align_up, Buffer, Gpu};
use anyhow::Result;
use ash::vk;
use gpu_allocator::MemoryLocation;

pub struct Accel {
    pub raw: vk::AccelerationStructureKHR,
    pub buf: Buffer,
    pub addr: u64,
}

impl Accel {
    /// No acceleration structure (a mesh the scene never shows).
    pub fn none() -> Self {
        Accel { raw: vk::AccelerationStructureKHR::null(), buf: Buffer::none(), addr: 0 }
    }
}

pub fn triangles_geometry(pos_addr: u64, vcount: u32, idx_addr: u64) -> vk::AccelerationStructureGeometryKHR<'static> {
    vk::AccelerationStructureGeometryKHR::default()
        .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
        .flags(vk::GeometryFlagsKHR::OPAQUE)
        .geometry(vk::AccelerationStructureGeometryDataKHR {
            triangles: vk::AccelerationStructureGeometryTrianglesDataKHR::default()
                .vertex_format(vk::Format::R32G32B32_SFLOAT)
                .vertex_data(vk::DeviceOrHostAddressConstKHR { device_address: pos_addr })
                .vertex_stride(12)
                .max_vertex(vcount.saturating_sub(1))
                .index_type(vk::IndexType::UINT32)
                .index_data(vk::DeviceOrHostAddressConstKHR { device_address: idx_addr }),
        })
}

pub fn aabb_geometry(aabb_addr: u64) -> vk::AccelerationStructureGeometryKHR<'static> {
    vk::AccelerationStructureGeometryKHR::default()
        .geometry_type(vk::GeometryTypeKHR::AABBS)
        .flags(vk::GeometryFlagsKHR::OPAQUE)
        .geometry(vk::AccelerationStructureGeometryDataKHR {
            aabbs: vk::AccelerationStructureGeometryAabbsDataKHR::default()
                .data(vk::DeviceOrHostAddressConstKHR { device_address: aabb_addr })
                .stride(24),
        })
}

pub fn create_accel(gpu: &Gpu, ty: vk::AccelerationStructureTypeKHR, size: u64) -> Result<Accel> {
    let buf = gpu.create_buffer(size, vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR, MemoryLocation::GpuOnly, "accel")?;
    unsafe {
        let raw = gpu.accel_fn.create_acceleration_structure(
            &vk::AccelerationStructureCreateInfoKHR::default().buffer(buf.raw).size(size).ty(ty),
            None,
        )?;
        let addr = gpu
            .accel_fn
            .get_acceleration_structure_device_address(&vk::AccelerationStructureDeviceAddressInfoKHR::default().acceleration_structure(raw));
        Ok(Accel { raw, buf, addr })
    }
}

/// An acceleration structure placed in an existing buffer (pooled storage): only the AS object is
/// created; `offset` must be a multiple of 256.
pub fn create_accel_in(gpu: &Gpu, buffer: vk::Buffer, offset: u64, size: u64, ty: vk::AccelerationStructureTypeKHR) -> Result<(vk::AccelerationStructureKHR, u64)> {
    unsafe {
        let raw = gpu.accel_fn.create_acceleration_structure(
            &vk::AccelerationStructureCreateInfoKHR::default().buffer(buffer).offset(offset).size(size).ty(ty),
            None,
        )?;
        let addr = gpu
            .accel_fn
            .get_acceleration_structure_device_address(&vk::AccelerationStructureDeviceAddressInfoKHR::default().acceleration_structure(raw));
        Ok((raw, addr))
    }
}

/// Streamed BLASes are built to be compacted afterwards (globe/scene.rs).
const STREAM_FLAGS: vk::BuildAccelerationStructureFlagsKHR =
    vk::BuildAccelerationStructureFlagsKHR::from_raw(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE.as_raw() | vk::BuildAccelerationStructureFlagsKHR::ALLOW_COMPACTION.as_raw());

/// BLAS build sizes (storage, scratch) for one geometry.
pub fn blas_sizes(gpu: &Gpu, g: &vk::AccelerationStructureGeometryKHR, count: u32) -> (u64, u64) {
    let gs = [*g];
    let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
        .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
        .flags(STREAM_FLAGS)
        .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
        .geometries(&gs);
    let mut size = vk::AccelerationStructureBuildSizesInfoKHR::default();
    unsafe { gpu.accel_fn.get_acceleration_structure_build_sizes(vk::AccelerationStructureBuildTypeKHR::DEVICE, &info, &[count], &mut size) };
    (size.acceleration_structure_size, size.build_scratch_size)
}

/// Record one BLAS build into `cb` (no waiting): geometry, primitive count, target, scratch address.
pub fn cmd_build_blas(gpu: &Gpu, cb: vk::CommandBuffer, g: &vk::AccelerationStructureGeometryKHR, count: u32, dst: vk::AccelerationStructureKHR, scratch: u64) {
    let gs = [*g];
    let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
        .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
        .flags(STREAM_FLAGS)
        .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
        .dst_acceleration_structure(dst)
        .geometries(&gs)
        .scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch });
    let range = [vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(count)];
    unsafe { gpu.accel_fn.cmd_build_acceleration_structures(cb, &[info], &[&range[..]]) };
}

pub fn destroy_accel(gpu: &Gpu, a: &mut Accel) {
    unsafe { gpu.accel_fn.destroy_acceleration_structure(a.raw, None) };
    gpu.destroy_buffer(&mut a.buf);
}

/// Build one BLAS per (geometry, primitive count), all in a single submission.
pub fn build_blases(gpu: &Gpu, geoms: &[(vk::AccelerationStructureGeometryKHR<'static>, u32)]) -> Result<Vec<Accel>> {
    let flags = vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE | vk::BuildAccelerationStructureFlagsKHR::ALLOW_COMPACTION;
    let mut out = Vec::new();
    let mut max_scratch = 0u64;
    for (g, count) in geoms {
        let gs = [*g];
        let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .flags(flags)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .geometries(&gs);
        let mut size = vk::AccelerationStructureBuildSizesInfoKHR::default();
        unsafe {
            gpu.accel_fn
                .get_acceleration_structure_build_sizes(vk::AccelerationStructureBuildTypeKHR::DEVICE, &info, &[*count], &mut size)
        };
        max_scratch = max_scratch.max(size.build_scratch_size);
        out.push(create_accel(gpu, vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL, size.acceleration_structure_size)?);
    }
    let align = gpu.rt.scratch_alignment.max(256);
    let mut scratch = gpu.create_buffer(max_scratch + align, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::GpuOnly, "blas scratch")?;
    let scratch_addr = align_up(scratch.addr, align);
    gpu.one_time(|cb| {
        for (i, (g, count)) in geoms.iter().enumerate() {
            let gs = [*g];
            let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                .flags(flags)
                .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
                .dst_acceleration_structure(out[i].raw)
                .geometries(&gs)
                .scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_addr });
            let range = [vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(*count)];
            unsafe { gpu.accel_fn.cmd_build_acceleration_structures(cb, &[info], &[&range[..]]) };
            // Scratch is reused: serialise the builds.
            gpu.memory_barrier(
                cb,
                vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_KHR,
                vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR | vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_KHR,
            );
        }
    })?;
    gpu.destroy_buffer(&mut scratch);
    compact(gpu, out)
}

/// Copy static BLASes (built with ALLOW_COMPACTION) into buffers of their compacted size and free
/// the originals: typically half the memory, the same tracing speed.
fn compact(gpu: &Gpu, mut built: Vec<Accel>) -> Result<Vec<Accel>> {
    if built.is_empty() {
        return Ok(built);
    }
    let before: u64 = built.iter().map(|a| a.buf.size).sum();
    let mut out = Vec::with_capacity(built.len());
    // in batches, so the originals and their copies never all coexist
    for chunk in built.chunks_mut(256) {
        let n = chunk.len() as u32;
        let pool = unsafe {
            gpu.device.create_query_pool(
                &vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::ACCELERATION_STRUCTURE_COMPACTED_SIZE_KHR).query_count(n),
                None,
            )?
        };
        let raws: Vec<vk::AccelerationStructureKHR> = chunk.iter().map(|a| a.raw).collect();
        gpu.one_time(|cb| unsafe {
            gpu.device.cmd_reset_query_pool(cb, pool, 0, n);
            gpu.accel_fn.cmd_write_acceleration_structures_properties(cb, &raws, vk::QueryType::ACCELERATION_STRUCTURE_COMPACTED_SIZE_KHR, pool, 0);
        })?;
        let mut sizes = vec![0u64; n as usize];
        unsafe { gpu.device.get_query_pool_results(pool, 0, &mut sizes, vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT)? };
        unsafe { gpu.device.destroy_query_pool(pool, None) };
        let mut compact: Vec<Accel> = Vec::with_capacity(chunk.len());
        for s in &sizes {
            compact.push(create_accel(gpu, vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL, (*s).max(256))?);
        }
        gpu.one_time(|cb| {
            for (src, dst) in chunk.iter().zip(&compact) {
                let info = vk::CopyAccelerationStructureInfoKHR::default().src(src.raw).dst(dst.raw).mode(vk::CopyAccelerationStructureModeKHR::COMPACT);
                unsafe { gpu.accel_fn.cmd_copy_acceleration_structure(cb, &info) };
            }
        })?;
        for a in chunk.iter_mut() {
            destroy_accel(gpu, a);
        }
        out.extend(compact);
    }
    let after: u64 = out.iter().map(|a| a.buf.size).sum();
    println!("BLAS compaction: {:.0} MB -> {:.0} MB", before as f64 / 1e6, after as f64 / 1e6);
    Ok(out)
}

pub const INSTANCE_SIZE: usize = std::mem::size_of::<vk::AccelerationStructureInstanceKHR>();

/// Top level structure rebuilt from a persistently mapped instance buffer.
pub struct Tlas {
    pub accel: Accel,
    pub instances: Buffer,
    scratch: Buffer,
    scratch_addr: u64,
    pub capacity: u32,
}

impl Tlas {
    pub fn new(gpu: &Gpu, capacity: u32) -> Result<Self> {
        let capacity = capacity.max(16);
        let instances = gpu.create_buffer(
            capacity as u64 * INSTANCE_SIZE as u64,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
            MemoryLocation::CpuToGpu,
            "tlas instances",
        )?;
        let geom = [Self::geometry(instances.addr)];
        let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .geometries(&geom);
        let mut size = vk::AccelerationStructureBuildSizesInfoKHR::default();
        unsafe {
            gpu.accel_fn
                .get_acceleration_structure_build_sizes(vk::AccelerationStructureBuildTypeKHR::DEVICE, &info, &[capacity], &mut size)
        };
        let accel = create_accel(gpu, vk::AccelerationStructureTypeKHR::TOP_LEVEL, size.acceleration_structure_size)?;
        let align = gpu.rt.scratch_alignment.max(256);
        let scratch = gpu.create_buffer(size.build_scratch_size + align, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::GpuOnly, "tlas scratch")?;
        let scratch_addr = align_up(scratch.addr, align);
        Ok(Self { accel, instances, scratch, scratch_addr, capacity })
    }

    fn geometry(addr: u64) -> vk::AccelerationStructureGeometryKHR<'static> {
        vk::AccelerationStructureGeometryKHR::default()
            .geometry_type(vk::GeometryTypeKHR::INSTANCES)
            .flags(vk::GeometryFlagsKHR::OPAQUE)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                instances: vk::AccelerationStructureGeometryInstancesDataKHR::default()
                    .array_of_pointers(false)
                    .data(vk::DeviceOrHostAddressConstKHR { device_address: addr }),
            })
    }

    pub fn write(&mut self, insts: &[vk::AccelerationStructureInstanceKHR]) {
        assert!(insts.len() <= self.capacity as usize);
        let dst = self.instances.mapped();
        unsafe {
            std::ptr::copy_nonoverlapping(insts.as_ptr() as *const u8, dst.as_mut_ptr(), insts.len() * INSTANCE_SIZE);
        }
    }

    pub fn cmd_build(&self, gpu: &Gpu, cb: vk::CommandBuffer, count: u32) {
        let geom = [Self::geometry(self.instances.addr)];
        let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .dst_acceleration_structure(self.accel.raw)
            .geometries(&geom)
            .scratch_data(vk::DeviceOrHostAddressKHR { device_address: self.scratch_addr });
        let range = [vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(count)];
        gpu.memory_barrier(
            cb,
            vk::PipelineStageFlags2::HOST | vk::PipelineStageFlags2::RAY_TRACING_SHADER_KHR,
            vk::AccessFlags2::HOST_WRITE | vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR,
            vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
            vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR | vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_KHR,
        );
        unsafe { gpu.accel_fn.cmd_build_acceleration_structures(cb, &[info], &[&range[..]]) };
        gpu.memory_barrier(
            cb,
            vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
            vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_KHR,
            vk::PipelineStageFlags2::RAY_TRACING_SHADER_KHR,
            vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR,
        );
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        destroy_accel(gpu, &mut self.accel);
        gpu.destroy_buffer(&mut self.instances);
        gpu.destroy_buffer(&mut self.scratch);
    }
}

pub fn make_instance(
    m: &glam::Affine3A,
    custom: u32,
    mask: u8,
    sbt_offset: u32,
    flags: vk::GeometryInstanceFlagsKHR,
    blas_addr: u64,
) -> vk::AccelerationStructureInstanceKHR {
    let c = m.matrix3;
    let t = m.translation;
    vk::AccelerationStructureInstanceKHR {
        transform: vk::TransformMatrixKHR {
            matrix: [
                c.x_axis.x, c.y_axis.x, c.z_axis.x, t.x, //
                c.x_axis.y, c.y_axis.y, c.z_axis.y, t.y, //
                c.x_axis.z, c.y_axis.z, c.z_axis.z, t.z,
            ],
        },
        instance_custom_index_and_mask: vk::Packed24_8::new(custom & 0xFF_FFFF, mask),
        instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(sbt_offset, flags.as_raw() as u8),
        acceleration_structure_reference: vk::AccelerationStructureReferenceKHR { device_handle: blas_addr },
    }
}
