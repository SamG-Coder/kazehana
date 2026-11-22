//! Akari's frame: a path traced image (pt.rgen) denoised and upscaled by DLSS Ray
//! Reconstruction, or accumulated by TAA, then auto-exposed, tonemapped, sharpened and presented.
//!
//! Per frame: sky map (sky.comp) -> TLAS build -> trace -> DLSS-RR | TAA -> exposure measure ->
//! resolve (tonemap) -> CAS sharpen -> blit to the swapchain. One frame in flight: the CPU waits
//! for the previous frame before touching anything the GPU reads, which also makes adding
//! meshes, materials and textures between frames safe.
//!
//! The device, acceleration structure, BC texture, DLSS, TAA, resolve and sharpen code is ported
//! from SmartImposter.

pub use impostor::{ImpostorVariant, PlantPlacement};

use crate::accel::{self, Accel, Tlas};
use crate::camera::Camera;
use crate::gpu::{align_up, Buffer, Gpu, Image, Swapchain};
use crate::scene::*;
use crate::textures::{self, TexJob};
use crate::ui::{Ui, UiVertex, ATLAS};
use anyhow::{anyhow, Context, Result};
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Affine3A, Vec3};
use gpu_allocator::MemoryLocation;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::time::Instant;

macro_rules! spv {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".spv"))
    };
}

/// Size of the bindless texture array (binding 20).
pub const MAX_TEXTURES: u32 = 4096;
const MAX_LIGHTS: usize = 4096;
const SKY_LUT: u32 = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
struct Globals {
    cam_pos: [f32; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    cam_fwd: [f32; 4],
    prev_pos: [f32; 4],
    prev_right: [f32; 4],
    prev_up: [f32; 4],
    prev_fwd: [f32; 4],
    sun_dir: [f32; 4],
    moon_dir: [f32; 4],
    sky: [f32; 4],
    fog: [f32; 4],
    frame: [u32; 4],
    opts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshInfoGpu {
    addr: [u64; 6], // pos, nrm, uv, tan, col, idx
    material: u32,
    flags: u32,
}

/// Bytes per instance in the object-motion buffer (rt.glsl `InstMotion`).
const MOTION_STRIDE: usize = 64;

struct GpuMesh {
    bufs: Vec<Buffer>,
    blas: Accel,
    info: MeshInfoGpu,
    alpha: bool,
    /// thin clear glass (MAT_GLASS): in its own instance mask, seen only by camera rays
    glass: bool,
    /// a water surface (MAT_WATER): in its own instance mask, which shadow rays skip
    water: bool,
    /// object-space box of the vertices (impostor bakes)
    bounds: (Vec3, Vec3),
    /// vertex and index counts: meshes with equal counts can stand in for each other's previous
    /// frame (object motion of pre-built deformation states)
    counts: (usize, usize),
}

pub struct RendererOptions {
    pub validation: bool,
    pub vsync: bool,
    /// DLSS Ray Reconstruction mode (index into dlss::MODES); None = TAA.
    pub dlss: Option<usize>,
    /// TAA path: rays per output pixel per axis.
    pub render_scale: f32,
    /// Indirect bounces after the first hit.
    pub bounces: u32,
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self { validation: false, vsync: true, dlss: Some(1), render_scale: 1.0, bounces: 2 }
    }
}

/// Everything the renderer needs for one frame.
pub struct FrameDesc<'a> {
    pub camera: &'a Camera,
    /// Towards the sun (world: +X east, +Y up, -Z north).
    pub sun_dir: Vec3,
    /// Sun illuminance above the atmosphere in renderer units (1 = full sun; 0 below the horizon).
    pub sun_illuminance: f32,
    pub moon_dir: Vec3,
    /// Moon illuminance (phase and altitude included); a full moon is ~2.5e-6 of the sun.
    pub moon_illuminance: f32,
    /// 0..1, star visibility.
    pub stars: f32,
    /// Camera altitude above sea level (m), for the atmosphere.
    pub altitude: f32,
    pub ground_albedo: f32,
    /// Aerial haze, 1 = clear day.
    pub haze: f32,
    pub instances: &'a [Instance],
    pub lights: &'a [Light],
    /// Seconds (animation clock).
    pub time: f32,
    /// Seconds since the last frame (exposure adaptation).
    pub dt: f32,
    /// Exposure compensation in stops.
    pub exposure_ev: f32,
    /// 0 = final image; 1 albedo, 2 normals.
    pub debug: u32,
    /// Return the final image as RGBA8.
    pub capture: bool,
    /// 2D overlay drawn over the final image (HUD, menus).
    pub ui: Option<&'a Ui>,
}

struct Compute {
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    pipe: vk::Pipeline,
}

struct RtPipeline {
    pipe: vk::Pipeline,
    rgen: vk::StridedDeviceAddressRegionKHR,
    miss: vk::StridedDeviceAddressRegionKHR,
    hit: vk::StridedDeviceAddressRegionKHR,
    sbt: Buffer,
}

struct Targets {
    cur: Image,
    depth: Image,
    hist: [Image; 2],
    /// TAA: samples averaged into each pixel since the camera last moved or something moved there
    count: Image,
    viewz: Image,
    motion: Image,
    nrm: Image,
    diff: Image,
    spec: Image,
    dlss_out: Image,
    ldr: Image,
    sharp: Image,
    readback: Buffer,
}

pub struct Renderer {
    pub gpu: Gpu,
    swap: Swapchain,
    vsync: bool,
    need_resize: bool,
    window_size: (u32, u32),
    // ray tracing
    rt_set_layout: vk::DescriptorSetLayout,
    rt_layout: vk::PipelineLayout,
    rt: RtPipeline,
    pool: vk::DescriptorPool,
    rt_set: vk::DescriptorSet,
    // compute passes
    sky: Compute,
    sky_set: vk::DescriptorSet,
    sky_lut: Image,
    exposure: Compute,
    exposure_set: vk::DescriptorSet,
    exposure_buf: Buffer,
    taa: Compute,
    taa_sets: [vk::DescriptorSet; 2],
    resolve: Compute,
    resolve_sets: [vk::DescriptorSet; 2],
    dlss_resolve_set: vk::DescriptorSet,
    sharpen: Compute,
    sharpen_set: vk::DescriptorSet,
    // 2D overlay
    ui_set_layout: vk::DescriptorSetLayout,
    ui_layout: vk::PipelineLayout,
    ui_pipe: vk::Pipeline,
    ui_set: vk::DescriptorSet,
    ui_atlas: Image,
    ui_atlas_version: u64,
    ui_verts: Buffer,
    // scene
    sampler: vk::Sampler,
    lut_sampler: vk::Sampler,
    textures: Vec<Image>,
    materials: Vec<MaterialGpu>,
    meshes: Vec<GpuMesh>,
    material_buf: Buffer,
    mesh_buf: Buffer,
    light_buf: Buffer,
    /// the lights' hash grid (lightgrid.rs), its capacity in words, and the lights it was built for
    light_grid: Buffer,
    light_grid_cap: usize,
    light_grid_for: Vec<LightGpu>,
    /// Object motion (`set_object_motion`): per-instance previous transform and mesh (rt.glsl
    /// `Motion`), last frame's instances, and the buffer's capacity in instances.
    object_motion: bool,
    motion_buf: Buffer,
    motion_cap: usize,
    prev_instances: Vec<Instance>,
    /// this frame's instances that moved by themselves (`set_moved`): (index, last frame's transform)
    moved: Vec<(usize, Affine3A)>,
    /// something moved by itself this frame (object motion written): the TAA's still-camera
    /// average then checks each pixel's depth (g.fog.z) (where a door was, the wall behind starts afresh)
    scene_moved: bool,
    scene_dirty: bool,
    tlas: Tlas,
    insts: Vec<vk::AccelerationStructureInstanceKHR>,
    /// plant impostors (renderer/impostor.rs)
    imp: impostor::Impostors,
    /// the lawn's baked views (crate::lawn; renderer/impostor.rs bake_lawn): its slot in the
    /// impostor infos, which shaders/lawn.glsl reads (globals fog.y = slot + 1)
    lawn: Option<u32>,
    /// meshes the game deforms every frame (renderer/deform.rs)
    deform: Vec<deform::Deformable>,
    // frame
    ubo: Buffer,
    tg: Targets,
    render_extent: vk::Extent2D,
    cb: vk::CommandBuffer,
    fence: vk::Fence,
    acquire_sem: vk::Semaphore,
    frame: u32,
    still: u32,
    parity: usize,
    taa_reset: bool,
    prev: Option<Globals>,
    pre_exposure: f32,
    exposure_valid: bool,
    /// Frames left in which exposure jumps straight to its target (after a cut).
    exposure_snap: u32,
    night_vision: f32,
    bounces: u32,
    render_scale: f32,
    dlss_ok: bool,
    dlss_mode: Option<usize>,
    last_frame: Instant,
    /// Absolute scene luminance the auto exposure measured last (renderer units).
    pub scene_luminance: f32,
}

fn scaled_extent(e: vk::Extent2D, scale: f32) -> vk::Extent2D {
    vk::Extent2D { width: ((e.width as f32 * scale).round() as u32).max(1), height: ((e.height as f32 * scale).round() as u32).max(1) }
}

fn world_to_view(g: &Globals) -> [f32; 16] {
    let v = |a: [f32; 4]| Vec3::new(a[0], a[1], a[2]);
    let (p, r, u, f) = (v(g.cam_pos), v(g.cam_right), v(g.cam_up), v(g.cam_fwd));
    [r.x, u.x, f.x, 0.0, r.y, u.y, f.y, 0.0, r.z, u.z, f.z, 0.0, -r.dot(p), -u.dot(p), -f.dot(p), 1.0]
}

fn view_to_clip(g: &Globals) -> [f32; 16] {
    let (sx, sy) = (1.0 / g.cam_right[3], 1.0 / g.cam_up[3]);
    let (n, f) = (0.02f32, 2.0e4f32);
    let a = f / (f - n);
    [sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, a, 1.0, 0.0, 0.0, -n * a, 0.0]
}

fn binding(b: u32, ty: vk::DescriptorType, count: u32, stages: vk::ShaderStageFlags) -> vk::DescriptorSetLayoutBinding<'static> {
    vk::DescriptorSetLayoutBinding::default().binding(b).descriptor_type(ty).descriptor_count(count).stage_flags(stages)
}

impl Renderer {
    pub fn new(window: &winit::window::Window, opts: RendererOptions) -> Result<Self> {
        let size = window.inner_size();
        let gpu = Gpu::new(window.display_handle()?.as_raw(), window.window_handle()?.as_raw(), opts.validation)?;
        println!("Akari: {} (SER {})", gpu.device_name, if gpu.ser { "on" } else { "off" });
        let dlss_ok = match crate::dlss::init(&gpu.entry, &gpu.instance, gpu.pdev, &gpu.device) {
            Ok(()) => true,
            Err(e) => {
                println!("DLSS Ray Reconstruction off: {e}");
                false
            }
        };
        let dlss_mode = if dlss_ok { opts.dlss } else { None };
        let swap = gpu.create_swapchain(size.width.max(1), size.height.max(1), vk::SwapchainKHR::null(), opts.vsync)?;

        unsafe {
            let d = &gpu.device;
            let rt_stages = vk::ShaderStageFlags::RAYGEN_KHR | vk::ShaderStageFlags::CLOSEST_HIT_KHR | vk::ShaderStageFlags::ANY_HIT_KHR | vk::ShaderStageFlags::MISS_KHR | vk::ShaderStageFlags::INTERSECTION_KHR;
            use vk::DescriptorType as T;
            let bindings = [
                binding(0, T::ACCELERATION_STRUCTURE_KHR, 1, rt_stages),
                binding(1, T::STORAGE_IMAGE, 1, rt_stages),
                binding(2, T::STORAGE_IMAGE, 1, rt_stages),
                binding(3, T::UNIFORM_BUFFER, 1, rt_stages),
                binding(4, T::STORAGE_BUFFER, 1, rt_stages),
                binding(5, T::STORAGE_BUFFER, 1, rt_stages),
                binding(6, T::STORAGE_BUFFER, 1, rt_stages),
                binding(7, T::COMBINED_IMAGE_SAMPLER, 1, rt_stages),
                binding(8, T::STORAGE_IMAGE, 1, rt_stages),
                binding(9, T::STORAGE_IMAGE, 1, rt_stages),
                binding(10, T::STORAGE_IMAGE, 1, rt_stages),
                binding(11, T::STORAGE_IMAGE, 1, rt_stages),
                binding(12, T::STORAGE_IMAGE, 1, rt_stages),
                // impostors (renderer/impostor.rs): variant info, plant records, plant state
                binding(13, T::STORAGE_BUFFER, 1, rt_stages),
                binding(14, T::STORAGE_BUFFER, 1, rt_stages),
                binding(15, T::STORAGE_BUFFER, 1, rt_stages),
                // object motion (rt.glsl Motion)
                binding(16, T::STORAGE_BUFFER, 1, rt_stages),
                // where the lights reach (renderer/lightgrid.rs, rt.glsl LightGrid)
                binding(17, T::STORAGE_BUFFER, 1, rt_stages),
                binding(20, T::COMBINED_IMAGE_SAMPLER, MAX_TEXTURES, rt_stages),
            ];
            let mut flags = vec![vk::DescriptorBindingFlags::empty(); bindings.len()];
            *flags.last_mut().unwrap() = vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT;
            let mut flag_info = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&flags);
            let rt_set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings).push_next(&mut flag_info), None)?;
            let rt_layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&[rt_set_layout]), None)?;
            let rt = Self::create_rt_pipeline(&gpu, rt_layout, if gpu.ser { spv!("pt_ser.rgen") } else { spv!("pt.rgen") })?;

            let sky = Self::compute(&gpu, spv!("sky.comp"), &[T::STORAGE_IMAGE, T::UNIFORM_BUFFER], 0)?;
            let exposure = Self::compute(&gpu, spv!("exposure.comp"), &[T::STORAGE_IMAGE, T::STORAGE_BUFFER], 0)?;
            let taa = Self::compute(&gpu, spv!("taa.comp"), &[T::STORAGE_IMAGE, T::STORAGE_IMAGE, T::STORAGE_IMAGE, T::STORAGE_IMAGE, T::UNIFORM_BUFFER, T::STORAGE_IMAGE, T::STORAGE_IMAGE], 4)?;
            let resolve = Self::compute(&gpu, spv!("resolve.comp"), &[T::STORAGE_IMAGE, T::STORAGE_IMAGE], 16)?;
            let sharpen = Self::compute(&gpu, spv!("sharpen.comp"), &[T::STORAGE_IMAGE, T::STORAGE_IMAGE], 4)?;

            let pool_sizes = [
                vk::DescriptorPoolSize { ty: T::ACCELERATION_STRUCTURE_KHR, descriptor_count: 2 },
                vk::DescriptorPoolSize { ty: T::STORAGE_IMAGE, descriptor_count: 64 },
                vk::DescriptorPoolSize { ty: T::UNIFORM_BUFFER, descriptor_count: 8 },
                vk::DescriptorPoolSize { ty: T::STORAGE_BUFFER, descriptor_count: 32 },
                vk::DescriptorPoolSize { ty: T::COMBINED_IMAGE_SAMPLER, descriptor_count: MAX_TEXTURES + 8 },
            ];
            let pool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(16).pool_sizes(&pool_sizes), None)?;
            let counts = [MAX_TEXTURES];
            let mut var = vk::DescriptorSetVariableDescriptorCountAllocateInfo::default().descriptor_counts(&counts);
            let rt_set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&[rt_set_layout]).push_next(&mut var))?[0];
            let alloc = |l: vk::DescriptorSetLayout| -> Result<vk::DescriptorSet> {
                Ok(d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&[l]))?[0])
            };
            let sky_set = alloc(sky.set_layout)?;
            let exposure_set = alloc(exposure.set_layout)?;
            let taa_sets = [alloc(taa.set_layout)?, alloc(taa.set_layout)?];
            let resolve_sets = [alloc(resolve.set_layout)?, alloc(resolve.set_layout)?];
            let dlss_resolve_set = alloc(resolve.set_layout)?;
            let sharpen_set = alloc(sharpen.set_layout)?;

            let sampler = d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::REPEAT)
                    .address_mode_v(vk::SamplerAddressMode::REPEAT)
                    .address_mode_w(vk::SamplerAddressMode::REPEAT)
                    .anisotropy_enable(true)
                    .max_anisotropy(16.0)
                    .max_lod(vk::LOD_CLAMP_NONE),
                None,
            )?;
            let lut_sampler = d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?;
            let sky_lut = gpu.create_image(SKY_LUT, SKY_LUT, 1, vk::Format::R16G16B16A16_SFLOAT, vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED, "sky lut")?;
            let exposure_buf = gpu.create_buffer(16, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::GpuToCpu, "exposure")?;
            let ubo = gpu.create_buffer(std::mem::size_of::<Globals>() as u64, vk::BufferUsageFlags::UNIFORM_BUFFER, MemoryLocation::CpuToGpu, "globals")?;
            let light_buf = gpu.create_buffer((MAX_LIGHTS * std::mem::size_of::<LightGpu>()) as u64, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::CpuToGpu, "lights")?;
            let light_grid_cap = 1 << 17;
            let mut light_grid = gpu.create_buffer((light_grid_cap * 4) as u64, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::CpuToGpu, "light grid")?;
            // (empty until the first lights: no cell lists anything)
            light_grid.mapped()[..4 * (4 + 8192 * 4)].fill(0);
            let tlas = Tlas::new(&gpu, 1024)?;
            let motion_cap = 1024;
            let motion_buf = gpu.create_buffer((16 + motion_cap * MOTION_STRIDE) as u64, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::CpuToGpu, "object motion")?;
            let imp = impostor::Impostors::new(&gpu)?;
            let render_extent = match dlss_mode {
                Some(m) => scaled_extent(swap.extent, crate::dlss::MODES[m].1),
                None => scaled_extent(swap.extent, opts.render_scale),
            };
            let tg = Self::create_targets(&gpu, swap.extent, render_extent)?;
            gpu.one_time(|cb| {
                gpu.image_barrier(cb, sky_lut.raw, vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL, vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE, vk::PipelineStageFlags2::ALL_COMMANDS, vk::AccessFlags2::SHADER_WRITE);
            })?;
            let cb = d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(gpu.cmd_pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1))?[0];
            let fence = d.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)?;
            let acquire_sem = d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;

            let (ui_set_layout, ui_layout, ui_pipe) = Self::create_ui_pipeline(&gpu)?;
            let ui_set = alloc(ui_set_layout)?;
            let ui_atlas = gpu.create_image(ATLAS, ATLAS, 1, vk::Format::R8G8B8A8_UNORM, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST, "ui atlas")?;
            gpu.one_time(|cb| {
                gpu.image_barrier(cb, ui_atlas.raw, vk::ImageLayout::UNDEFINED, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE, vk::PipelineStageFlags2::ALL_COMMANDS, vk::AccessFlags2::SHADER_READ);
            })?;
            let ui_verts = gpu.create_buffer(6 * 20000 * std::mem::size_of::<UiVertex>() as u64, vk::BufferUsageFlags::VERTEX_BUFFER, MemoryLocation::CpuToGpu, "ui vertices")?;
            {
                let info = [vk::DescriptorImageInfo::default().sampler(lut_sampler).image_view(ui_atlas.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
                let w = vk::WriteDescriptorSet::default().dst_set(ui_set).dst_binding(0).descriptor_type(T::COMBINED_IMAGE_SAMPLER).image_info(&info);
                d.update_descriptor_sets(&[w], &[]);
            }
            let mut r = Renderer {
                swap,
                vsync: opts.vsync,
                need_resize: false,
                window_size: (size.width, size.height),
                rt_set_layout,
                rt_layout,
                rt,
                pool,
                rt_set,
                sky,
                sky_set,
                sky_lut,
                exposure,
                exposure_set,
                exposure_buf,
                taa,
                taa_sets,
                resolve,
                resolve_sets,
                dlss_resolve_set,
                sharpen,
                sharpen_set,
                ui_set_layout,
                ui_layout,
                ui_pipe,
                ui_set,
                ui_atlas,
                ui_atlas_version: 0,
                ui_verts,
                sampler,
                lut_sampler,
                textures: Vec::new(),
                materials: Vec::new(),
                meshes: Vec::new(),
                material_buf: Buffer::none(),
                mesh_buf: Buffer::none(),
                light_buf,
                light_grid,
                light_grid_cap,
                light_grid_for: Vec::new(),
                object_motion: false,
                motion_buf,
                motion_cap,
                prev_instances: Vec::new(),
                moved: Vec::new(),
                scene_moved: false,
                scene_dirty: true,
                tlas,
                insts: Vec::new(),
                imp,
                lawn: None,
                deform: Vec::new(),
                ubo,
                tg,
                render_extent,
                cb,
                fence,
                acquire_sem,
                frame: 0,
                still: 0,
                parity: 0,
                taa_reset: true,
                prev: None,
                pre_exposure: 1.0,
                exposure_valid: false,
                exposure_snap: 0,
                night_vision: 0.0,
                bounces: opts.bounces,
                render_scale: opts.render_scale,
                dlss_ok,
                dlss_mode,
                last_frame: Instant::now(),
                scene_luminance: 0.0,
                gpu,
            };
            // A default material and a white texture so empty scenes still bind valid data.
            r.add_material(Material::default());
            r.sync_scene_buffers()?;
            r.write_compute_sets();
            r.write_rt_set();
            r.create_dlss_feature()?;
            Ok(r)
        }
    }

    unsafe fn compute(gpu: &Gpu, code: &[u8], types: &[vk::DescriptorType], push: u32) -> Result<Compute> {
        let d = &gpu.device;
        let b: Vec<_> = types.iter().enumerate().map(|(i, t)| binding(i as u32, *t, 1, vk::ShaderStageFlags::COMPUTE)).collect();
        let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&b), None)?;
        let ranges = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::COMPUTE, offset: 0, size: push }];
        let layouts = [set_layout];
        let mut info = vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts);
        if push > 0 {
            info = info.push_constant_ranges(&ranges);
        }
        let layout = d.create_pipeline_layout(&info, None)?;
        let module = gpu.shader_module(code)?;
        let stage = vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::COMPUTE).module(module).name(c"main");
        let pipe = d
            .create_compute_pipelines(vk::PipelineCache::null(), &[vk::ComputePipelineCreateInfo::default().stage(stage).layout(layout)], None)
            .map_err(|(_, e)| e)?[0];
        d.destroy_shader_module(module, None);
        Ok(Compute { set_layout, layout, pipe })
    }

    unsafe fn create_ui_pipeline(gpu: &Gpu) -> Result<(vk::DescriptorSetLayout, vk::PipelineLayout, vk::Pipeline)> {
        let d = &gpu.device;
        let b = [binding(0, vk::DescriptorType::COMBINED_IMAGE_SAMPLER, 1, vk::ShaderStageFlags::FRAGMENT)];
        let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&b), None)?;
        let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::VERTEX, offset: 0, size: 8 }];
        let sl = [set_layout];
        let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&sl).push_constant_ranges(&push), None)?;
        let (vs, fs) = (gpu.shader_module(spv!("ui.vert"))?, gpu.shader_module(spv!("ui.frag"))?);
        let stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
        ];
        let vb = [vk::VertexInputBindingDescription { binding: 0, stride: std::mem::size_of::<UiVertex>() as u32, input_rate: vk::VertexInputRate::VERTEX }];
        let va = [
            vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32_SFLOAT, offset: 0 },
            vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32_SFLOAT, offset: 8 },
            vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R8G8B8A8_UNORM, offset: 16 },
        ];
        let vi = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&vb).vertex_attribute_descriptions(&va);
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let blend = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let cbs = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend);
        let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
        let formats = [vk::Format::R8G8B8A8_UNORM];
        let mut rendering = vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&formats);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vi)
            .input_assembly_state(&ia)
            .viewport_state(&vp)
            .rasterization_state(&rs)
            .multisample_state(&ms)
            .color_blend_state(&cbs)
            .dynamic_state(&dynamic)
            .layout(layout)
            .push_next(&mut rendering);
        let pipe = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0];
        d.destroy_shader_module(vs, None);
        d.destroy_shader_module(fs, None);
        Ok((set_layout, layout, pipe))
    }

    /// Upload the glyph atlas if the UI rasterised new glyphs, and the frame's vertices.
    fn prepare_ui(&mut self, ui: &Ui) -> Result<u32> {
        if ui.font.version != self.ui_atlas_version {
            let mut staging = self.gpu.create_buffer(ui.font.atlas.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, MemoryLocation::CpuToGpu, "atlas staging")?;
            staging.mapped()[..ui.font.atlas.len()].copy_from_slice(&ui.font.atlas);
            let img = self.ui_atlas.raw;
            let g = &self.gpu;
            g.one_time(|cb| {
                g.image_barrier(cb, img, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::PipelineStageFlags2::ALL_COMMANDS, vk::AccessFlags2::SHADER_READ, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_WRITE);
                let region = vk::BufferImageCopy {
                    image_subresource: vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 },
                    image_extent: vk::Extent3D { width: ATLAS, height: ATLAS, depth: 1 },
                    ..Default::default()
                };
                unsafe { g.device.cmd_copy_buffer_to_image(cb, staging.raw, img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]) };
                g.image_barrier(cb, img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_WRITE, vk::PipelineStageFlags2::ALL_COMMANDS, vk::AccessFlags2::SHADER_READ);
            })?;
            self.gpu.destroy_buffer(&mut staging);
            self.ui_atlas_version = ui.font.version;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&ui.verts);
        if bytes.len() as u64 > self.ui_verts.size {
            let new = self.gpu.create_buffer(bytes.len() as u64 * 2, vk::BufferUsageFlags::VERTEX_BUFFER, MemoryLocation::CpuToGpu, "ui vertices")?;
            let mut old = std::mem::replace(&mut self.ui_verts, new);
            self.gpu.destroy_buffer(&mut old);
        }
        self.ui_verts.mapped()[..bytes.len()].copy_from_slice(bytes);
        Ok(ui.verts.len() as u32)
    }

    /// The ray tracing pipeline for a generation shader (the frame's path tracer, or the impostor
    /// bake). Hit groups: 0 triangles (hit.rchit + alpha.rahit), 1 impostors (impostor.rint +
    /// impostor.rchit; procedural instances use SBT record offset 1).
    unsafe fn create_rt_pipeline(gpu: &Gpu, layout: vk::PipelineLayout, rgen: &[u8]) -> Result<RtPipeline> {
        let modules = [
            gpu.shader_module(rgen)?,
            gpu.shader_module(spv!("miss.rmiss"))?,
            gpu.shader_module(spv!("shadow.rmiss"))?,
            gpu.shader_module(spv!("hit.rchit"))?,
            gpu.shader_module(spv!("alpha.rahit"))?,
            gpu.shader_module(spv!("impostor.rint"))?,
            gpu.shader_module(spv!("impostor.rchit"))?,
        ];
        let kinds = [
            vk::ShaderStageFlags::RAYGEN_KHR,
            vk::ShaderStageFlags::MISS_KHR,
            vk::ShaderStageFlags::MISS_KHR,
            vk::ShaderStageFlags::CLOSEST_HIT_KHR,
            vk::ShaderStageFlags::ANY_HIT_KHR,
            vk::ShaderStageFlags::INTERSECTION_KHR,
            vk::ShaderStageFlags::CLOSEST_HIT_KHR,
        ];
        let stages: Vec<_> = modules.iter().zip(kinds).map(|(m, k)| vk::PipelineShaderStageCreateInfo::default().stage(k).module(*m).name(c"main")).collect();
        let u = vk::SHADER_UNUSED_KHR;
        let group = |ty, general, closest, any| {
            vk::RayTracingShaderGroupCreateInfoKHR::default().ty(ty).general_shader(general).closest_hit_shader(closest).any_hit_shader(any).intersection_shader(u)
        };
        let groups = [
            group(vk::RayTracingShaderGroupTypeKHR::GENERAL, 0, u, u),
            group(vk::RayTracingShaderGroupTypeKHR::GENERAL, 1, u, u),
            group(vk::RayTracingShaderGroupTypeKHR::GENERAL, 2, u, u),
            group(vk::RayTracingShaderGroupTypeKHR::TRIANGLES_HIT_GROUP, u, 3, 4),
            group(vk::RayTracingShaderGroupTypeKHR::PROCEDURAL_HIT_GROUP, u, 6, u).intersection_shader(5),
        ];
        let info = vk::RayTracingPipelineCreateInfoKHR::default().stages(&stages).groups(&groups).max_pipeline_ray_recursion_depth(1).layout(layout);
        let pipe = gpu.rt_fn.create_ray_tracing_pipelines(vk::DeferredOperationKHR::null(), vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0];
        for m in modules {
            gpu.device.destroy_shader_module(m, None);
        }
        // SBT: raygen | miss, shadow miss | triangle hit, impostor hit
        let hs = gpu.rt.handle_size as u64;
        let stride = align_up(hs, gpu.rt.handle_alignment as u64);
        let ba = gpu.rt.base_alignment as u64;
        let off_miss = align_up(stride, ba);
        let off_hit = align_up(off_miss + 2 * stride, ba);
        let total = off_hit + 2 * stride;
        let handles = gpu.rt_fn.get_ray_tracing_shader_group_handles(pipe, 0, 5, 5 * hs as usize)?;
        let mut sbt = gpu.create_buffer(total + ba, vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR, MemoryLocation::CpuToGpu, "sbt")?;
        let base = align_up(sbt.addr, ba);
        let shift = (base - sbt.addr) as usize;
        {
            let m = sbt.mapped();
            for (g, off) in [(0usize, 0u64), (1, off_miss), (2, off_miss + stride), (3, off_hit), (4, off_hit + stride)] {
                let dst = shift + off as usize;
                m[dst..dst + hs as usize].copy_from_slice(&handles[g * hs as usize..(g + 1) * hs as usize]);
            }
        }
        let region = |off: u64, size: u64| vk::StridedDeviceAddressRegionKHR { device_address: base + off, stride, size };
        Ok(RtPipeline { pipe, rgen: region(0, stride), miss: region(off_miss, 2 * stride), hit: region(off_hit, 2 * stride), sbt })
    }

    fn create_targets(gpu: &Gpu, extent: vk::Extent2D, render: vk::Extent2D) -> Result<Targets> {
        let (w, h) = (render.width, render.height);
        let st = vk::ImageUsageFlags::STORAGE;
        let ss = vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED; // also read by NGX
        let f16 = vk::Format::R16G16B16A16_SFLOAT;
        let tg = Targets {
            cur: gpu.create_image(w, h, 1, f16, ss, "frame sample")?,
            depth: gpu.create_image(w, h, 1, vk::Format::R32G32_SFLOAT, st, "hit distance")?,
            hist: [gpu.create_image(w, h, 1, f16, st, "taa history 0")?, gpu.create_image(w, h, 1, f16, st, "taa history 1")?],
            count: gpu.create_image(w, h, 1, vk::Format::R32_SFLOAT, st, "taa samples")?,
            viewz: gpu.create_image(w, h, 1, vk::Format::R32_SFLOAT, ss, "view depth")?,
            motion: gpu.create_image(w, h, 1, vk::Format::R16G16_SFLOAT, ss, "motion")?,
            nrm: gpu.create_image(w, h, 1, f16, ss, "normal roughness")?,
            diff: gpu.create_image(w, h, 1, vk::Format::R8G8B8A8_UNORM, ss, "diffuse albedo")?,
            spec: gpu.create_image(w, h, 1, vk::Format::R8G8B8A8_UNORM, ss, "specular albedo")?,
            dlss_out: gpu.create_image(extent.width, extent.height, 1, f16, ss, "dlss output")?,
            ldr: gpu.create_image(extent.width, extent.height, 1, vk::Format::R8G8B8A8_UNORM, st | vk::ImageUsageFlags::TRANSFER_SRC, "tonemapped")?,
            sharp: gpu.create_image(extent.width, extent.height, 1, vk::Format::R8G8B8A8_UNORM, st | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::COLOR_ATTACHMENT, "sharpened")?,
            readback: gpu.create_buffer(extent.width as u64 * extent.height as u64 * 4, vk::BufferUsageFlags::TRANSFER_DST, MemoryLocation::GpuToCpu, "readback")?,
        };
        gpu.one_time(|cb| {
            for img in [&tg.cur, &tg.depth, &tg.hist[0], &tg.hist[1], &tg.count, &tg.viewz, &tg.motion, &tg.nrm, &tg.diff, &tg.spec, &tg.dlss_out, &tg.ldr, &tg.sharp] {
                gpu.image_barrier(cb, img.raw, vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL, vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE, vk::PipelineStageFlags2::ALL_COMMANDS, vk::AccessFlags2::SHADER_WRITE);
            }
        })?;
        Ok(tg)
    }

    fn destroy_targets(&mut self) {
        let g = &self.gpu;
        let tg = &mut self.tg;
        for img in [&mut tg.cur, &mut tg.depth, &mut tg.count, &mut tg.viewz, &mut tg.motion, &mut tg.nrm, &mut tg.diff, &mut tg.spec, &mut tg.dlss_out, &mut tg.ldr, &mut tg.sharp] {
            g.destroy_image(img);
        }
        for img in tg.hist.iter_mut() {
            g.destroy_image(img);
        }
        g.destroy_buffer(&mut tg.readback);
    }

    fn write_compute_sets(&self) {
        let info = |img: &Image| [vk::DescriptorImageInfo::default().image_view(img.view).image_layout(vk::ImageLayout::GENERAL)];
        let ubo = [vk::DescriptorBufferInfo::default().buffer(self.ubo.raw).range(vk::WHOLE_SIZE)];
        let expo = [vk::DescriptorBufferInfo::default().buffer(self.exposure_buf.raw).range(vk::WHOLE_SIZE)];
        fn img_w<'a>(set: vk::DescriptorSet, b: u32, i: &'a [vk::DescriptorImageInfo; 1]) -> vk::WriteDescriptorSet<'a> {
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(b).descriptor_type(vk::DescriptorType::STORAGE_IMAGE).image_info(i)
        }
        let (lut, cur, depth, ldr, sharp, dout) = (info(&self.sky_lut), info(&self.tg.cur), info(&self.tg.depth), info(&self.tg.ldr), info(&self.tg.sharp), info(&self.tg.dlss_out));
        let (h0, h1) = (info(&self.tg.hist[0]), info(&self.tg.hist[1]));
        let (motion, count) = (info(&self.tg.motion), info(&self.tg.count));
        let hist = [&h0, &h1];
        let mut writes = vec![
            img_w(self.sky_set, 0, &lut),
            vk::WriteDescriptorSet::default().dst_set(self.sky_set).dst_binding(1).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&ubo),
            img_w(self.exposure_set, 0, &cur),
            vk::WriteDescriptorSet::default().dst_set(self.exposure_set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&expo),
            img_w(self.dlss_resolve_set, 0, &dout),
            img_w(self.dlss_resolve_set, 1, &ldr),
            img_w(self.sharpen_set, 0, &ldr),
            img_w(self.sharpen_set, 1, &sharp),
        ];
        for p in 0..2 {
            let t = self.taa_sets[p];
            writes.push(img_w(t, 0, &cur));
            writes.push(img_w(t, 1, &depth));
            writes.push(img_w(t, 2, hist[p]));
            writes.push(img_w(t, 3, hist[1 - p]));
            writes.push(vk::WriteDescriptorSet::default().dst_set(t).dst_binding(4).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&ubo));
            writes.push(img_w(t, 5, &motion));
            writes.push(img_w(t, 6, &count));
            writes.push(img_w(self.resolve_sets[p], 0, hist[1 - p]));
            writes.push(img_w(self.resolve_sets[p], 1, &ldr));
        }
        unsafe { self.gpu.device.update_descriptor_sets(&writes, &[]) };
    }

    fn write_rt_set(&self) {
        let s = self.rt_set;
        let handles = [self.tlas.accel.raw];
        let mut as_info = vk::WriteDescriptorSetAccelerationStructureKHR::default().acceleration_structures(&handles);
        let info = |img: &Image| [vk::DescriptorImageInfo::default().image_view(img.view).image_layout(vk::ImageLayout::GENERAL)];
        let buf = |b: &Buffer| [vk::DescriptorBufferInfo::default().buffer(b.raw).range(vk::WHOLE_SIZE)];
        let (cur, depth, viewz, motion, nrm, diff, spec) =
            (info(&self.tg.cur), info(&self.tg.depth), info(&self.tg.viewz), info(&self.tg.motion), info(&self.tg.nrm), info(&self.tg.diff), info(&self.tg.spec));
        let (ubo, mats, meshes, lights) = (buf(&self.ubo), buf(&self.material_buf), buf(&self.mesh_buf), buf(&self.light_buf));
        let obj_motion = buf(&self.motion_buf);
        let grid = buf(&self.light_grid);
        let (imp_info, imp_plants, imp_state) = (buf(&self.imp.info_buf), buf(&self.imp.plant_buf), buf(&self.imp.state_buf));
        let lut = [vk::DescriptorImageInfo::default().sampler(self.lut_sampler).image_view(self.sky_lut.view).image_layout(vk::ImageLayout::GENERAL)];
        let tex: Vec<_> = self
            .textures
            .iter()
            .map(|t| vk::DescriptorImageInfo::default().sampler(self.sampler).image_view(t.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL))
            .collect();
        use vk::DescriptorType as T;
        let w = |b: u32, ty: T| vk::WriteDescriptorSet::default().dst_set(s).dst_binding(b).descriptor_type(ty);
        let mut writes = vec![
            w(0, T::ACCELERATION_STRUCTURE_KHR).descriptor_count(1).push_next(&mut as_info),
            w(1, T::STORAGE_IMAGE).image_info(&cur),
            w(2, T::STORAGE_IMAGE).image_info(&depth),
            w(3, T::UNIFORM_BUFFER).buffer_info(&ubo),
            w(4, T::STORAGE_BUFFER).buffer_info(&mats),
            w(5, T::STORAGE_BUFFER).buffer_info(&meshes),
            w(6, T::STORAGE_BUFFER).buffer_info(&lights),
            w(7, T::COMBINED_IMAGE_SAMPLER).image_info(&lut),
            w(8, T::STORAGE_IMAGE).image_info(&viewz),
            w(9, T::STORAGE_IMAGE).image_info(&motion),
            w(10, T::STORAGE_IMAGE).image_info(&nrm),
            w(11, T::STORAGE_IMAGE).image_info(&diff),
            w(12, T::STORAGE_IMAGE).image_info(&spec),
            w(13, T::STORAGE_BUFFER).buffer_info(&imp_info),
            w(14, T::STORAGE_BUFFER).buffer_info(&imp_plants),
            w(15, T::STORAGE_BUFFER).buffer_info(&imp_state),
            w(16, T::STORAGE_BUFFER).buffer_info(&obj_motion),
            w(17, T::STORAGE_BUFFER).buffer_info(&grid),
        ];
        if !tex.is_empty() {
            writes.push(w(20, T::COMBINED_IMAGE_SAMPLER).image_info(&tex));
        }
        unsafe { self.gpu.device.update_descriptor_sets(&writes, &[]) };
    }

    fn create_dlss_feature(&mut self) -> Result<()> {
        let Some(m) = self.dlss_mode else { return Ok(()) };
        let (r, o) = (self.render_extent, self.swap.extent);
        let res = self.gpu.one_time(|cb| crate::dlss::create(cb, (r.width, r.height), (o.width, o.height), m))?;
        match res {
            Ok(()) => println!("DLSS Ray Reconstruction: {} ({}x{} -> {}x{})", crate::dlss::MODES[m].0, r.width, r.height, o.width, o.height),
            Err(e) => {
                println!("{e}; falling back to TAA");
                self.dlss_mode = None;
                self.recreate_targets()?;
            }
        }
        Ok(())
    }

    fn recreate_targets(&mut self) -> Result<()> {
        unsafe { self.gpu.device.device_wait_idle()? };
        self.destroy_targets();
        self.render_extent = match self.dlss_mode {
            Some(m) => scaled_extent(self.swap.extent, crate::dlss::MODES[m].1),
            None => scaled_extent(self.swap.extent, self.render_scale),
        };
        self.tg = Self::create_targets(&self.gpu, self.swap.extent, self.render_extent)?;
        self.write_compute_sets();
        self.write_rt_set();
        self.taa_reset = true;
        self.create_dlss_feature()
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.window_size = (w, h);
        self.need_resize = true;
    }

    fn do_resize(&mut self) -> Result<()> {
        let (w, h) = self.window_size;
        if w == 0 || h == 0 {
            return Ok(());
        }
        unsafe { self.gpu.device.device_wait_idle()? };
        let new = self.gpu.create_swapchain(w, h, self.swap.raw, self.vsync)?;
        let same = new.extent == self.swap.extent;
        let mut old = std::mem::replace(&mut self.swap, new);
        self.gpu.destroy_swapchain(&mut old);
        if !same {
            self.recreate_targets()?;
        }
        self.need_resize = false;
        Ok(())
    }

    pub fn extent(&self) -> (u32, u32) {
        (self.swap.extent.width, self.swap.extent.height)
    }

    pub fn dlss_mode(&self) -> Option<usize> {
        self.dlss_mode
    }

    pub fn set_dlss(&mut self, mode: Option<usize>) -> Result<()> {
        let mode = if self.dlss_ok { mode } else { None };
        if mode != self.dlss_mode {
            self.dlss_mode = mode;
            self.recreate_targets()?;
        }
        Ok(())
    }

    pub fn set_bounces(&mut self, n: u32) {
        self.bounces = n.min(8);
    }

    // ------------------------------------------------------------------ scene

    /// Load textures (BC-compressed, cached). Returns their ids in order.
    pub fn add_textures(&mut self, jobs: &[TexJob]) -> Result<Vec<TextureId>> {
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        if self.textures.len() + jobs.len() > MAX_TEXTURES as usize {
            return Err(anyhow!("too many textures (max {MAX_TEXTURES})"));
        }
        let first = self.textures.len() as u32;
        let imgs = textures::load(&self.gpu, jobs)?;
        self.textures.extend(imgs);
        self.write_rt_set();
        Ok((0..jobs.len() as u32).map(|i| TextureId(first + i)).collect())
    }

    /// Put new contents in a texture (a page drawn again: the calendar's next day, the journal's
    /// new line). The old image is freed; materials using the id see the new one next frame.
    pub fn replace_texture(&mut self, id: TextureId, job: &TexJob) -> Result<()> {
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        let slot = id.0 as usize;
        if slot >= self.textures.len() {
            return Err(anyhow!("no texture {}", id.0));
        }
        let mut img = textures::load(&self.gpu, std::slice::from_ref(job))?.pop().ok_or_else(|| anyhow!("texture didn't load"))?;
        std::mem::swap(&mut self.textures[slot], &mut img);
        self.gpu.destroy_image(&mut img);
        self.write_rt_set();
        Ok(())
    }

    pub fn add_material(&mut self, m: Material) -> MaterialId {
        self.materials.push(m.gpu());
        self.scene_dirty = true;
        MaterialId(self.materials.len() as u32 - 1)
    }

    /// Change a material (e.g. a window's glow at night). Takes effect next frame.
    pub fn set_material(&mut self, id: MaterialId, m: Material) {
        if let Some(slot) = self.materials.get_mut(id.0 as usize) {
            *slot = m.gpu();
            self.scene_dirty = true;
        }
    }

    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    /// Upload meshes and build (compacted) BLASes for them, all in one go.
    pub fn add_meshes(&mut self, meshes: &[MeshData]) -> Result<Vec<MeshId>> {
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX)? };
        let geo = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR;
        let mut pending = Vec::new();
        let mut geoms = Vec::new();
        for m in meshes {
            if m.indices.is_empty() || m.positions.is_empty() {
                return Err(anyhow!("mesh '{}' is empty", m.name));
            }
            let mat = m.material.map_or(0, |x| x.0);
            let alpha = self.materials.get(mat as usize).is_some_and(|x| x.flags & (MAT_ALPHA_MASK | crate::scene::MAT_GLASS) != 0);
            let glass = self.materials.get(mat as usize).is_some_and(|x| x.flags & crate::scene::MAT_GLASS != 0);
            let water = self.materials.get(mat as usize).is_some_and(|x| x.flags & crate::scene::MAT_WATER != 0);
            let up = |data: &[u8], usage| -> Result<Buffer> {
                if data.is_empty() { Ok(Buffer::none()) } else { self.gpu.upload_buffer(data, usage, &m.name) }
            };
            let pos = up(bytemuck::cast_slice(&m.positions), geo)?;
            let idx = up(bytemuck::cast_slice(&m.indices), geo)?;
            let nrm = up(bytemuck::cast_slice(&m.normals), vk::BufferUsageFlags::STORAGE_BUFFER)?;
            let uv = up(bytemuck::cast_slice(&m.uvs), vk::BufferUsageFlags::STORAGE_BUFFER)?;
            let tan = up(bytemuck::cast_slice(&m.tangents), vk::BufferUsageFlags::STORAGE_BUFFER)?;
            let col = up(bytemuck::cast_slice(&m.colors), vk::BufferUsageFlags::STORAGE_BUFFER)?;
            let info = MeshInfoGpu { addr: [pos.addr, nrm.addr, uv.addr, tan.addr, col.addr, idx.addr], material: mat, flags: 0 };
            let mut g = accel::triangles_geometry(pos.addr, m.positions.len() as u32, idx.addr);
            if alpha {
                g.flags = vk::GeometryFlagsKHR::NO_DUPLICATE_ANY_HIT_INVOCATION;
            }
            geoms.push((g, (m.indices.len() / 3) as u32));
            let bounds = m.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p))));
            pending.push((vec![pos, idx, nrm, uv, tan, col], info, (alpha, glass, water), bounds, (m.positions.len(), m.indices.len())));
        }
        let blases = accel::build_blases(&self.gpu, &geoms)?;
        let first = self.meshes.len() as u32;
        for ((bufs, info, (alpha, glass, water), bounds, counts), blas) in pending.into_iter().zip(blases) {
            self.meshes.push(GpuMesh { bufs, blas, info, alpha, glass, water, bounds, counts });
        }
        self.scene_dirty = true;
        Ok((0..meshes.len() as u32).map(|i| MeshId(first + i)).collect())
    }

    fn sync_scene_buffers(&mut self) -> Result<()> {
        if !self.scene_dirty {
            return Ok(());
        }
        let mut old = [std::mem::replace(&mut self.material_buf, Buffer::none()), std::mem::replace(&mut self.mesh_buf, Buffer::none())];
        for b in old.iter_mut() {
            self.gpu.destroy_buffer(b);
        }
        self.material_buf = self.gpu.upload_buffer(bytemuck::cast_slice(&self.materials), vk::BufferUsageFlags::STORAGE_BUFFER, "materials")?;
        let infos: Vec<MeshInfoGpu> = self.meshes.iter().map(|m| m.info).collect();
        let dummy = [MeshInfoGpu { addr: [0; 6], material: 0, flags: 0 }];
        self.mesh_buf = self.gpu.upload_buffer(bytemuck::cast_slice(if infos.is_empty() { &dummy } else { &infos }), vk::BufferUsageFlags::STORAGE_BUFFER, "meshes")?;
        self.scene_dirty = false;
        self.write_rt_set();
        Ok(())
    }

    // ------------------------------------------------------------------ frame

    fn update_exposure(&mut self, dt: f32) {
        // last frame's measurement (its fence has signalled)
        let m: [f32; 4] = *bytemuck::from_bytes(&self.exposure_buf.mapped()[..16]);
        if !(m[1] > 0.0) || !m[0].is_finite() {
            return;
        }
        let measured = (2f32.powf(m[0]) / self.pre_exposure).max(1e-12); // absolute (renderer units)
        self.scene_luminance = measured;
        let l = measured.log2();
        // Dark scenes stay darker than mid grey (night should read as night), with a bluish
        // desaturation below lamp-lit levels (resolve.comp).
        let night = 1.0 - smoothstep(-21.0, -13.0, l);
        let key = 0.115 * (1.0 - night) + 0.035 * night;
        let target = (key / measured).clamp(1e-2, 1e9);
        let snap = !self.exposure_valid || self.exposure_snap > 0;
        self.exposure_snap = self.exposure_snap.saturating_sub(1);
        let rate = if snap { 1.0 } else { 1.0 - (-dt * 2.0).exp() };
        self.pre_exposure = (self.pre_exposure.ln() + (target.ln() - self.pre_exposure.ln()) * rate).exp();
        self.night_vision += (night - self.night_vision) * rate;
        self.exposure_valid = true;
    }

    /// Object motion vectors for moving and deforming objects (off by default).
    ///
    /// The denoiser (DLSS Ray Reconstruction / TAA) reprojects history with motion vectors; by
    /// default they only follow the camera, so anything that moves by itself leaves a short ghost
    /// trail. With this on, instance `i` of each frame is matched with instance `i` of the frame
    /// before: a changed transform, or a different mesh with the same vertex and index counts (a
    /// pre-built deformation state, e.g. a tearing pack strip), gives that object exact per-pixel
    /// motion. Only turn it on when the instance list keeps a stable order from frame to frame.
    pub fn set_object_motion(&mut self, on: bool) {
        self.object_motion = on;
        self.prev_instances.clear();
    }

    /// Instances of the next frame that moved by themselves, with where they were in the frame
    /// before: `(index in FrameDesc::instances, last frame's transform)` (same mesh). For things
    /// that move without object motion on (`set_object_motion`): doors, and the like, whose index
    /// the caller knows. Without it the denoiser reprojects their history with the camera's motion
    /// only and they leave a ghost.
    pub fn set_moved(&mut self, moved: &[(usize, Affine3A)]) {
        self.moved.clear();
        self.moved.extend_from_slice(moved);
    }

    /// Fill the object-motion buffer for this frame's instances (rt.glsl `Motion`).
    fn write_motion(&mut self, instances: &[Instance]) -> Result<()> {
        let moved = std::mem::take(&mut self.moved);
        let moved_n = moved.iter().filter(|m| m.0 < instances.len()).map(|m| m.0 + 1).max().unwrap_or(0);
        // deformable meshes that moved this frame (cloth) get their motion even with object
        // motion off: (instance, the mesh's history twin) (renderer/deform.rs)
        let deformed: Vec<(usize, u32)> = if self.deform.iter().any(|d| d.moved) { instances.iter().enumerate().filter_map(|(i, inst)| self.moved_history(inst.mesh.0).map(|h| (i, h))).collect() } else { Vec::new() };
        let n = if self.object_motion { instances.len() } else { deformed.last().map_or(0, |d| d.0 + 1).max(moved_n) };
        let mut any = false;

        if n > self.motion_cap {
            self.motion_cap = (n as f32 * 1.5) as usize;
            let mut old = std::mem::replace(&mut self.motion_buf, self.gpu.create_buffer((16 + self.motion_cap * MOTION_STRIDE) as u64, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::CpuToGpu, "object motion")?);
            self.gpu.destroy_buffer(&mut old);
            self.write_rt_set();
        }
        let mut data = vec![0u32; 4 + n * MOTION_STRIDE / 4];
        data[0] = n as u32;
        for (i, inst) in instances.iter().enumerate().take(n) {
            let hist = deformed.iter().find(|d| d.0 == i).map(|d| d.1);
            let told = if hist.is_none() { moved.iter().find(|m| m.0 == i).map(|m| m.1) } else { None };
            let (t, prev_mesh) = match (hist, self.prev_instances.get(i)) {
                (None, _) if told.is_some() => {
                    let t = told.unwrap();
                    if t == inst.transform {
                        continue;
                    }
                    (t, inst.mesh.0)
                }
                (Some(h), Some(p)) if p.mesh == inst.mesh => (p.transform, h),
                (Some(h), _) => (inst.transform, h),
                (None, Some(p)) => {
                    let cur = &self.meshes[inst.mesh.0 as usize];
                    let same_mesh = p.mesh == inst.mesh;
                    let stand_in = !same_mesh && self.meshes.get(p.mesh.0 as usize).is_some_and(|m| m.counts == cur.counts);
                    if !(stand_in || (same_mesh && p.transform != inst.transform)) {
                        continue;
                    }
                    (p.transform, p.mesh.0)
                }
                (None, None) => continue,
            };
            let t = &t;
            let (x, y, z, o) = (t.matrix3.x_axis, t.matrix3.y_axis, t.matrix3.z_axis, t.translation);
            let rows = [[x.x, y.x, z.x, o.x], [x.y, y.y, z.y, o.y], [x.z, y.z, z.z, o.z]];
            let e = 4 + i * MOTION_STRIDE / 4;
            for (r, row) in rows.iter().enumerate() {
                for (c, v) in row.iter().enumerate() {
                    data[e + r * 4 + c] = v.to_bits();
                }
            }
            data[e + 12] = prev_mesh;
            data[e + 13] = 1;
            any = true;
        }
        self.scene_moved = any;
        let bytes: &[u8] = bytemuck::cast_slice(&data);
        self.motion_buf.mapped()[..bytes.len()].copy_from_slice(bytes);
        self.prev_instances.clear();
        if self.object_motion {
            self.prev_instances.extend_from_slice(instances);
        }
        Ok(())
    }

    /// Render and present one frame. With `capture`, returns the final image (RGBA8).
    pub fn render(&mut self, f: &FrameDesc) -> Result<Option<Vec<u8>>> {
        let d = self.gpu.device.clone();
        unsafe { d.wait_for_fences(&[self.fence], true, u64::MAX).context("waiting for the previous frame")? };
        self.update_exposure(f.dt);
        if self.need_resize {
            self.do_resize()?;
        }
        if self.window_size.0 == 0 || self.window_size.1 == 0 {
            return Ok(None);
        }
        self.sync_scene_buffers()?;
        self.write_deforms();
        let ui_count = match f.ui {
            Some(ui) if !ui.verts.is_empty() => self.prepare_ui(ui)?,
            _ => 0,
        };

        // Instances -> TLAS (rebuilt every frame: the scene moves, characters walk).
        self.insts.clear();
        for inst in f.instances {
            let m = &self.meshes[inst.mesh.0 as usize];
            let flags = vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE
                | if m.alpha { vk::GeometryInstanceFlagsKHR::FORCE_NO_OPAQUE } else { vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE };
            // (mask 0x02: glass, only camera rays see it; 0x04: water, which shadow rays skip;
            // 0x01: everything else)
            let mask = if m.glass { 0x02 } else if m.water { 0x04 } else { 0x01 };
            self.insts.push(accel::make_instance(&inst.transform, inst.mesh.0, mask, 0, flags, m.blas.addr));
        }
        self.imp.push_instances(&mut self.insts);
        if self.insts.len() > self.tlas.capacity as usize {
            let cap = (self.insts.len() as f32 * 1.5) as u32;
            let mut old = std::mem::replace(&mut self.tlas, Tlas::new(&self.gpu, cap)?);
            old.destroy(&self.gpu);
            self.write_rt_set();
        }
        self.tlas.write(&self.insts);
        self.write_motion(f.instances)?;
        let lights: Vec<LightGpu> = f.lights.iter().take(MAX_LIGHTS).map(|l| l.gpu()).collect();
        if !lights.is_empty() {
            self.light_buf.mapped()[..lights.len() * std::mem::size_of::<LightGpu>()].copy_from_slice(bytemuck::cast_slice(&lights));
        }
        if bytemuck::cast_slice::<LightGpu, u8>(&lights) != bytemuck::cast_slice::<LightGpu, u8>(&self.light_grid_for) {
            let words = lightgrid::build(&lights);
            if words.len() > self.light_grid_cap {
                self.light_grid_cap = words.len().next_power_of_two();
                let mut old = std::mem::replace(&mut self.light_grid, self.gpu.create_buffer((self.light_grid_cap * 4) as u64, vk::BufferUsageFlags::STORAGE_BUFFER, MemoryLocation::CpuToGpu, "light grid")?);
                self.gpu.destroy_buffer(&mut old);
                self.write_rt_set();
            }
            self.light_grid.mapped()[..words.len() * 4].copy_from_slice(bytemuck::cast_slice(&words));
            self.light_grid_for = lights.clone();
        }

        let idx = match unsafe { self.gpu.swapchain_fn.acquire_next_image(self.swap.raw, u64::MAX, self.acquire_sem, vk::Fence::null()) } {
            Ok((i, _)) => i,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.need_resize = true;
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };
        unsafe { d.reset_fences(&[self.fence])? };

        // Globals.
        let (w, h) = self.extent();
        let (rw, rh) = (self.render_extent.width, self.render_extent.height);
        let cam = f.camera;
        let fwd = cam.forward();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let th = (cam.fov_y * 0.5).tan();
        let aspect = w as f32 / h as f32;
        let mut g = Globals {
            cam_pos: [cam.position.x, cam.position.y, cam.position.z, f.time],
            cam_right: [right.x, right.y, right.z, th * aspect],
            cam_up: [up.x, up.y, up.z, th],
            // texture LOD: an output pixel's footprint (DLSS upscales to it)
            cam_fwd: [fwd.x, fwd.y, fwd.z, (2.0 * th / h as f32).atan()],
            sun_dir: [f.sun_dir.x, f.sun_dir.y, f.sun_dir.z, f.sun_illuminance],
            moon_dir: [f.moon_dir.x, f.moon_dir.y, f.moon_dir.z, f.moon_illuminance],
            sky: [f.altitude, f.stars, f.ground_albedo, self.pre_exposure],
            fog: [f.haze, self.lawn.map_or(0.0, |k| k as f32 + 1.0), self.scene_moved as u32 as f32, 0.0],
            frame: [self.frame, 0, lights.len() as u32, f.debug],
            opts: [self.bounces, self.dlss_mode.is_some() as u32, 0, 0],
            ..Default::default()
        };
        let same_cam = self.prev.is_some_and(|p| p.cam_pos[..3] == g.cam_pos[..3] && p.cam_fwd[..3] == g.cam_fwd[..3]);
        self.still = if same_cam && self.dlss_mode.is_none() { self.still + 1 } else { 0 };
        g.frame[1] = self.still;
        let p = self.prev.unwrap_or(g);
        g.prev_pos = p.cam_pos;
        g.prev_right = p.cam_right;
        g.prev_up = p.cam_up;
        g.prev_fwd = p.cam_fwd;
        self.ubo.mapped()[..std::mem::size_of::<Globals>()].copy_from_slice(bytemuck::bytes_of(&g));

        let cb = self.cb;
        let swap_img = self.swap.images[idx as usize];
        let all = vk::PipelineStageFlags2::ALL_COMMANDS;
        let rw_access = vk::AccessFlags2::SHADER_READ | vk::AccessFlags2::SHADER_WRITE;
        unsafe {
            d.reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())?;
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            self.record_deforms(cb);
            self.tlas.cmd_build(&self.gpu, cb, self.insts.len() as u32);

            // the sky map for this frame's sun and moon
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.sky.pipe);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::COMPUTE, self.sky.layout, 0, &[self.sky_set], &[]);
            d.cmd_dispatch(cb, SKY_LUT / 8, SKY_LUT / 8, 1);
            self.gpu.memory_barrier(cb, all, vk::AccessFlags2::SHADER_WRITE | vk::AccessFlags2::SHADER_READ, all, rw_access);

            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::RAY_TRACING_KHR, self.rt.pipe);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::RAY_TRACING_KHR, self.rt_layout, 0, &[self.rt_set], &[]);
            self.gpu.rt_fn.cmd_trace_rays(cb, &self.rt.rgen, &self.rt.miss, &self.rt.hit, &vk::StridedDeviceAddressRegionKHR::default(), rw, rh, 1);
            self.gpu.memory_barrier(cb, all, vk::AccessFlags2::SHADER_WRITE | vk::AccessFlags2::SHADER_READ, all, rw_access);

            // auto exposure measurement (read back next frame)
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.exposure.pipe);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::COMPUTE, self.exposure.layout, 0, &[self.exposure_set], &[]);
            d.cmd_dispatch(cb, 1, 1, 1);

            let expo = 2f32.powf(f.exposure_ev);
            let tonemap = (f.debug == 0) as u32;
            if let Some(_mode) = self.dlss_mode {
                self.gpu.memory_barrier(cb, all, vk::AccessFlags2::SHADER_WRITE | vk::AccessFlags2::SHADER_READ | vk::AccessFlags2::MEMORY_WRITE, all, rw_access);
                let si = |img: &Image| crate::dlss::SiImage { image: img.raw, view: img.view, format: img.format, width: img.width, height: img.height };
                let (jx, jy) = crate::dlss::jitter(self.frame);
                let now = Instant::now();
                let e = crate::dlss::SiDlssEval {
                    color: si(&self.tg.cur),
                    output: si(&self.tg.dlss_out),
                    depth: si(&self.tg.viewz),
                    motion: si(&self.tg.motion),
                    normal_rough: si(&self.tg.nrm),
                    diff_albedo: si(&self.tg.diff),
                    spec_albedo: si(&self.tg.spec),
                    // NGX wants the projection offset: the opposite of where the samples sit
                    // (measured in SmartImposter)
                    jitter_x: -(jx - 0.5),
                    jitter_y: -(jy - 0.5),
                    render_w: rw,
                    render_h: rh,
                    reset: self.taa_reset as i32,
                    world_to_view: world_to_view(&g),
                    view_to_clip: view_to_clip(&g),
                    frame_time_ms: (now - self.last_frame).as_secs_f32() * 1000.0,
                };
                self.last_frame = now;
                self.taa_reset = false;
                if let Err(err) = crate::dlss::evaluate(cb, &e) {
                    eprintln!("{err}");
                }
                self.gpu.memory_barrier(cb, all, vk::AccessFlags2::SHADER_WRITE | vk::AccessFlags2::MEMORY_WRITE, all, rw_access);
                let push: [u32; 4] = [1.0f32.to_bits(), self.night_vision.to_bits(), expo.to_bits(), tonemap];
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.resolve.pipe);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::COMPUTE, self.resolve.layout, 0, &[self.dlss_resolve_set], &[]);
                d.cmd_push_constants(cb, self.resolve.layout, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::cast_slice(&push));
                d.cmd_dispatch(cb, w.div_ceil(8), h.div_ceil(8), 1);
            } else {
                let p = self.parity;
                let reset = [self.taa_reset as u32];
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.taa.pipe);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::COMPUTE, self.taa.layout, 0, &[self.taa_sets[p]], &[]);
                d.cmd_push_constants(cb, self.taa.layout, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::cast_slice(&reset));
                d.cmd_dispatch(cb, rw.div_ceil(8), rh.div_ceil(8), 1);
                self.gpu.memory_barrier(cb, all, vk::AccessFlags2::SHADER_WRITE, all, rw_access);
                self.taa_reset = false;
                self.parity = 1 - p;
                let push: [u32; 4] = [(rw as f32 / w as f32).to_bits(), self.night_vision.to_bits(), expo.to_bits(), tonemap];
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.resolve.pipe);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::COMPUTE, self.resolve.layout, 0, &[self.resolve_sets[p]], &[]);
                d.cmd_push_constants(cb, self.resolve.layout, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::cast_slice(&push));
                d.cmd_dispatch(cb, w.div_ceil(8), h.div_ceil(8), 1);
            }
            self.gpu.memory_barrier(cb, all, vk::AccessFlags2::SHADER_WRITE, all, rw_access);
            let sharp = [if f.debug == 0 { 0.35f32 } else { 0.0 }];
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.sharpen.pipe);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::COMPUTE, self.sharpen.layout, 0, &[self.sharpen_set], &[]);
            d.cmd_push_constants(cb, self.sharpen.layout, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::cast_slice(&sharp));
            d.cmd_dispatch(cb, w.div_ceil(8), h.div_ceil(8), 1);
            if ui_count > 0 {
                self.gpu.memory_barrier(cb, all, vk::AccessFlags2::SHADER_WRITE, vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT, vk::AccessFlags2::COLOR_ATTACHMENT_READ | vk::AccessFlags2::COLOR_ATTACHMENT_WRITE);
                let att = [vk::RenderingAttachmentInfo::default()
                    .image_view(self.tg.sharp.view)
                    .image_layout(vk::ImageLayout::GENERAL)
                    .load_op(vk::AttachmentLoadOp::LOAD)
                    .store_op(vk::AttachmentStoreOp::STORE)];
                let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } };
                d.cmd_begin_rendering(cb, &vk::RenderingInfo::default().render_area(area).layer_count(1).color_attachments(&att));
                d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
                d.cmd_set_scissor(cb, 0, &[area]);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.ui_pipe);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.ui_layout, 0, &[self.ui_set], &[]);
                d.cmd_push_constants(cb, self.ui_layout, vk::ShaderStageFlags::VERTEX, 0, bytemuck::cast_slice(&[w as f32, h as f32]));
                d.cmd_bind_vertex_buffers(cb, 0, &[self.ui_verts.raw], &[0]);
                d.cmd_draw(cb, ui_count, 1, 0, 0);
                d.cmd_end_rendering(cb);
                self.gpu.memory_barrier(cb, vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT, vk::AccessFlags2::COLOR_ATTACHMENT_WRITE, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_READ);
            } else {
                self.gpu.memory_barrier(cb, all, vk::AccessFlags2::SHADER_WRITE, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_READ);
            }

            self.gpu.image_barrier(cb, swap_img, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::NONE, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_WRITE);
            let layers = vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 };
            let corner = vk::Offset3D { x: w as i32, y: h as i32, z: 1 };
            d.cmd_blit_image(
                cb,
                self.tg.sharp.raw,
                vk::ImageLayout::GENERAL,
                swap_img,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::ImageBlit { src_subresource: layers, src_offsets: [vk::Offset3D::default(), corner], dst_subresource: layers, dst_offsets: [vk::Offset3D::default(), corner] }],
                vk::Filter::NEAREST,
            );
            if f.capture {
                d.cmd_copy_image_to_buffer(
                    cb,
                    self.tg.sharp.raw,
                    vk::ImageLayout::GENERAL,
                    self.tg.readback.raw,
                    &[vk::BufferImageCopy { image_subresource: layers, image_extent: vk::Extent3D { width: w, height: h, depth: 1 }, ..Default::default() }],
                );
            }
            self.gpu.image_barrier(cb, swap_img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::PRESENT_SRC_KHR, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_WRITE, vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE);
            d.end_command_buffer(cb)?;

            let waits = [self.acquire_sem];
            let stages = [vk::PipelineStageFlags::TRANSFER];
            let cbs = [cb];
            let signals = [self.swap.done[idx as usize]];
            d.queue_submit(self.gpu.queue, &[vk::SubmitInfo::default().wait_semaphores(&waits).wait_dst_stage_mask(&stages).command_buffers(&cbs).signal_semaphores(&signals)], self.fence)?;
            let swaps = [self.swap.raw];
            let idxs = [idx];
            match self.gpu.swapchain_fn.queue_present(self.gpu.queue, &vk::PresentInfoKHR::default().wait_semaphores(&signals).swapchains(&swaps).image_indices(&idxs)) {
                Ok(false) => {}
                Ok(true) => {
                    let caps = self.gpu.surface_fn.get_physical_device_surface_capabilities(self.gpu.pdev, self.gpu.surface)?;
                    if caps.current_extent != self.swap.extent {
                        self.need_resize = true;
                    }
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.need_resize = true,
                Err(e) => return Err(e.into()),
            }
        }
        self.prev = Some(g);
        self.frame = self.frame.wrapping_add(1);
        if f.capture {
            unsafe { d.wait_for_fences(&[self.fence], true, u64::MAX)? };
            let n = (w * h * 4) as usize;
            return Ok(Some(self.tg.readback.mapped()[..n].to_vec()));
        }
        Ok(None)
    }

    /// Forget the temporal history (after a teleport or a level change).
    pub fn reset_history(&mut self) {
        self.taa_reset = true;
        self.prev = None;
        self.prev_instances.clear();
        // the measurement in flight still belongs to the old view: snap for a few frames
        self.exposure_snap = 3;
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let d = self.gpu.device.clone();
            let _ = d.device_wait_idle();
            if self.dlss_ok {
                crate::dlss::shutdown();
            }
            self.destroy_targets();
            self.destroy_deforms();
            for mut m in std::mem::take(&mut self.meshes) {
                accel::destroy_accel(&self.gpu, &mut m.blas);
                for b in m.bufs.iter_mut() {
                    self.gpu.destroy_buffer(b);
                }
            }
            for mut t in std::mem::take(&mut self.textures) {
                self.gpu.destroy_image(&mut t);
            }
            self.tlas.destroy(&self.gpu);
            self.imp.destroy(&self.gpu);
            for b in [&mut self.material_buf, &mut self.mesh_buf, &mut self.light_buf, &mut self.light_grid, &mut self.motion_buf, &mut self.ubo, &mut self.exposure_buf, &mut self.rt.sbt] {
                self.gpu.destroy_buffer(b);
            }
            self.gpu.destroy_image(&mut self.sky_lut);
            self.gpu.destroy_image(&mut self.ui_atlas);
            self.gpu.destroy_buffer(&mut self.ui_verts);
            d.destroy_pipeline(self.ui_pipe, None);
            d.destroy_pipeline_layout(self.ui_layout, None);
            d.destroy_descriptor_set_layout(self.ui_set_layout, None);
            d.destroy_sampler(self.sampler, None);
            d.destroy_sampler(self.lut_sampler, None);
            d.destroy_pipeline(self.rt.pipe, None);
            for c in [&self.sky, &self.exposure, &self.taa, &self.resolve, &self.sharpen] {
                d.destroy_pipeline(c.pipe, None);
                d.destroy_pipeline_layout(c.layout, None);
                d.destroy_descriptor_set_layout(c.set_layout, None);
            }
            d.destroy_pipeline_layout(self.rt_layout, None);
            d.destroy_descriptor_set_layout(self.rt_set_layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_fence(self.fence, None);
            d.destroy_semaphore(self.acquire_sem, None);
            self.gpu.destroy_swapchain(&mut self.swap);
            // (the device, allocator and instance are torn down with `gpu`)
        }
    }
}

mod deform;
mod impostor;
mod lightgrid;
