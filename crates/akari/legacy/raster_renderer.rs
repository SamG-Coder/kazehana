//! Vulkan 1.3 forward renderer: dynamic rendering, sync2, two frames in flight.
//!
//! One pass per frame: a fullscreen sky, then every `DrawItem` lit by the sun, the moon, a
//! hemisphere ambient term and up to `MAX_LIGHTS` point lights, with fog. Output goes straight to
//! an sRGB swapchain.

use crate::mesh::{MeshData, Vertex};
use anyhow::{anyhow, bail, Context, Result};
use ash::{khr, vk, Device, Entry, Instance};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec4};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::ffi::CStr;
use std::path::PathBuf;
use winit::window::Window;

pub const MAX_LIGHTS: usize = 32;
const FRAMES_IN_FLIGHT: usize = 2;
const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

/// Mirrors `Scene` in shaders/scene.glsl (std140).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SceneUniforms {
    pub view_proj: Mat4,
    pub inv_view_proj: Mat4,
    pub camera_pos: Vec4,
    pub sun_dir: Vec4,
    pub sun_color: Vec4,
    pub moon_dir: Vec4,
    pub sky_zenith: Vec4,
    pub sky_horizon: Vec4,
    pub ambient: Vec4,
    pub fog: Vec4,
    pub params: Vec4,
    pub light_pos: [Vec4; MAX_LIGHTS],
    pub light_color: [Vec4; MAX_LIGHTS],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Push {
    model: Mat4,
    tint: Vec4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshId(usize);

#[derive(Clone, Copy, Debug)]
pub struct DrawItem {
    pub mesh: MeshId,
    pub model: Mat4,
    /// sRGB multiplier in xyz, emissive strength in w.
    pub tint: Vec4,
}

impl DrawItem {
    pub fn new(mesh: MeshId, model: Mat4) -> Self {
        Self { mesh, model, tint: Vec4::new(1.0, 1.0, 1.0, 0.0) }
    }
}

struct Buffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: u64,
}

struct GpuMesh {
    vertices: Buffer,
    indices: Buffer,
    count: u32,
}

struct Frame {
    cmd: vk::CommandBuffer,
    image_ready: vk::Semaphore,
    fence: vk::Fence,
    ubo: Buffer,
    ubo_ptr: *mut u8,
    set: vk::DescriptorSet,
}

struct Swapchain {
    handle: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    views: Vec<vk::ImageView>,
    render_done: Vec<vk::Semaphore>,
    extent: vk::Extent2D,
    depth: vk::Image,
    depth_memory: vk::DeviceMemory,
    depth_view: vk::ImageView,
}

pub struct Renderer {
    _entry: Entry,
    instance: Instance,
    debug: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    surface_fn: khr::surface::Instance,
    surface: vk::SurfaceKHR,
    pdev: vk::PhysicalDevice,
    mem_props: vk::PhysicalDeviceMemoryProperties,
    device: Device,
    queue: vk::Queue,
    swapchain_fn: khr::swapchain::Device,
    format: vk::SurfaceFormatKHR,
    swapchain: Option<Swapchain>,
    cmd_pool: vk::CommandPool,
    set_layout: vk::DescriptorSetLayout,
    desc_pool: vk::DescriptorPool,
    layout: vk::PipelineLayout,
    mesh_pipeline: vk::Pipeline,
    sky_pipeline: vk::Pipeline,
    frames: Vec<Frame>,
    frame: usize,
    meshes: Vec<GpuMesh>,
    resize_pending: bool,
    screenshot: Option<PathBuf>,
    pub gpu_name: String,
}

impl Renderer {
    pub fn new(window: &Window) -> Result<Self> {
        unsafe { Self::create(window) }
    }

    unsafe fn create(window: &Window) -> Result<Self> {
        let entry = Entry::load().context("Vulkan loader not found")?;
        let display = window.display_handle()?.as_raw();
        let mut extensions = ash_window::enumerate_required_extensions(display)?.to_vec();
        let validation = c"VK_LAYER_KHRONOS_validation";
        let want_validation = cfg!(debug_assertions) && std::env::var_os("AKARI_NO_VALIDATION").is_none();
        let have_validation = entry
            .enumerate_instance_layer_properties()?
            .iter()
            .any(|l| CStr::from_ptr(l.layer_name.as_ptr()) == validation);
        let layers = if want_validation && have_validation {
            extensions.push(ash::ext::debug_utils::NAME.as_ptr());
            vec![validation.as_ptr()]
        } else {
            vec![]
        };
        let app = vk::ApplicationInfo::default()
            .application_name(c"Kazehana")
            .engine_name(c"Akari")
            .api_version(vk::API_VERSION_1_3);
        let instance = entry.create_instance(
            &vk::InstanceCreateInfo::default()
                .application_info(&app)
                .enabled_extension_names(&extensions)
                .enabled_layer_names(&layers),
            None,
        )?;
        let mut debug = None;
        if !layers.is_empty() {
            let du = ash::ext::debug_utils::Instance::new(&entry, &instance);
            let info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
                .message_type(vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE)
                .pfn_user_callback(Some(debug_callback));
            let messenger = du.create_debug_utils_messenger(&info, None)?;
            debug = Some((du, messenger));
        }

        let surface_fn = khr::surface::Instance::new(&entry, &instance);
        let surface = ash_window::create_surface(&entry, &instance, display, window.window_handle()?.as_raw(), None)?;

        // Pick a GPU: prefer discrete, needs a graphics queue that can present.
        let mut best = None;
        for pdev in instance.enumerate_physical_devices()? {
            let props = instance.get_physical_device_properties(pdev);
            if props.api_version < vk::API_VERSION_1_3 {
                continue;
            }
            let qf = instance
                .get_physical_device_queue_family_properties(pdev)
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                        && surface_fn.get_physical_device_surface_support(pdev, i as u32, surface).unwrap_or(false)
                });
            let Some(qf) = qf else { continue };
            let score = if props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU { 2 } else { 1 };
            if best.as_ref().map_or(true, |(s, _, _, _)| score > *s) {
                let name = CStr::from_ptr(props.device_name.as_ptr()).to_string_lossy().into_owned();
                best = Some((score, pdev, qf as u32, name));
            }
        }
        let (_, pdev, qf, gpu_name) = best.ok_or_else(|| anyhow!("no Vulkan 1.3 GPU that can present"))?;
        let mem_props = instance.get_physical_device_memory_properties(pdev);

        let mut f13 = vk::PhysicalDeviceVulkan13Features::default().dynamic_rendering(true).synchronization2(true);
        let prio = [1.0];
        let qci = [vk::DeviceQueueCreateInfo::default().queue_family_index(qf).queue_priorities(&prio)];
        let dev_ext = [khr::swapchain::NAME.as_ptr()];
        let device = instance.create_device(
            pdev,
            &vk::DeviceCreateInfo::default().queue_create_infos(&qci).enabled_extension_names(&dev_ext).push_next(&mut f13),
            None,
        )?;
        let queue = device.get_device_queue(qf, 0);
        let swapchain_fn = khr::swapchain::Device::new(&instance, &device);

        let formats = surface_fn.get_physical_device_surface_formats(pdev, surface)?;
        let format = formats
            .iter()
            .copied()
            .find(|f| f.format == vk::Format::B8G8R8A8_SRGB && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .or_else(|| formats.iter().copied().find(|f| f.format == vk::Format::R8G8B8A8_SRGB))
            .unwrap_or(formats[0]);

        let cmd_pool = device.create_command_pool(
            &vk::CommandPoolCreateInfo::default().queue_family_index(qf).flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )?;

        // Descriptors: one uniform buffer per frame.
        let binding = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)];
        let set_layout = device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&binding), None)?;
        let pool_sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: FRAMES_IN_FLIGHT as u32 }];
        let desc_pool = device.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo::default().max_sets(FRAMES_IN_FLIGHT as u32).pool_sizes(&pool_sizes),
            None,
        )?;
        let push = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: std::mem::size_of::<Push>() as u32,
        }];
        let set_layouts = [set_layout];
        let layout = device.create_pipeline_layout(
            &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push),
            None,
        )?;

        let mut r = Renderer {
            _entry: entry,
            instance,
            debug,
            surface_fn,
            surface,
            pdev,
            mem_props,
            device,
            queue,
            swapchain_fn,
            format,
            swapchain: None,
            cmd_pool,
            set_layout,
            desc_pool,
            layout,
            mesh_pipeline: vk::Pipeline::null(),
            sky_pipeline: vk::Pipeline::null(),
            frames: Vec::new(),
            frame: 0,
            meshes: Vec::new(),
            resize_pending: false,
            screenshot: None,
            gpu_name,
        };
        r.mesh_pipeline = r.create_pipeline(
            include_bytes!(concat!(env!("OUT_DIR"), "/mesh.vert.spv")),
            include_bytes!(concat!(env!("OUT_DIR"), "/mesh.frag.spv")),
            true,
        )?;
        r.sky_pipeline = r.create_pipeline(
            include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv")),
            include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv")),
            false,
        )?;
        r.create_frames()?;
        let size = window.inner_size();
        r.create_swapchain(size.width, size.height)?;
        Ok(r)
    }

    unsafe fn create_pipeline(&self, vs: &[u8], fs: &[u8], mesh: bool) -> Result<vk::Pipeline> {
        let d = &self.device;
        let module = |bytes: &[u8]| -> Result<vk::ShaderModule> {
            let words = ash::util::read_spv(&mut std::io::Cursor::new(bytes))?;
            Ok(d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)?)
        };
        let (vsm, fsm) = (module(vs)?, module(fs)?);
        let stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vsm).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fsm).name(c"main"),
        ];
        let bindings = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let attrs = [
            vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32_SFLOAT, offset: 0 },
            vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32_SFLOAT, offset: 12 },
            vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32B32_SFLOAT, offset: 24 },
        ];
        let vertex_input = if mesh {
            vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&bindings).vertex_attribute_descriptions(&attrs)
        } else {
            vk::PipelineVertexInputStateCreateInfo::default()
        };
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(mesh)
            .depth_write_enable(mesh)
            .depth_compare_op(vk::CompareOp::LESS);
        let blend = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend);
        let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
        let color_formats = [self.format.format];
        let mut rendering = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_formats)
            .depth_attachment_format(DEPTH_FORMAT);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&ia)
            .viewport_state(&vp)
            .rasterization_state(&rs)
            .multisample_state(&ms)
            .depth_stencil_state(&ds)
            .color_blend_state(&cb)
            .dynamic_state(&dynamic)
            .layout(self.layout)
            .push_next(&mut rendering);
        let p = d
            .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
            .map_err(|(_, e)| anyhow!("pipeline creation failed: {e}"))?[0];
        d.destroy_shader_module(vsm, None);
        d.destroy_shader_module(fsm, None);
        Ok(p)
    }

    fn memory_type(&self, bits: u32, flags: vk::MemoryPropertyFlags) -> Result<u32> {
        (0..self.mem_props.memory_type_count)
            .find(|&i| bits & (1 << i) != 0 && self.mem_props.memory_types[i as usize].property_flags.contains(flags))
            .ok_or_else(|| anyhow!("no suitable memory type"))
    }

    unsafe fn create_buffer(&self, size: u64, usage: vk::BufferUsageFlags) -> Result<Buffer> {
        let buffer = self.device.create_buffer(&vk::BufferCreateInfo::default().size(size).usage(usage), None)?;
        let req = self.device.get_buffer_memory_requirements(buffer);
        let ty = self.memory_type(req.memory_type_bits, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)?;
        let memory = self.device.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(ty), None)?;
        self.device.bind_buffer_memory(buffer, memory, 0)?;
        Ok(Buffer { buffer, memory, size })
    }

    unsafe fn upload(&self, data: &[u8], usage: vk::BufferUsageFlags) -> Result<Buffer> {
        let b = self.create_buffer(data.len().max(4) as u64, usage)?;
        let ptr = self.device.map_memory(b.memory, 0, b.size, vk::MemoryMapFlags::empty())? as *mut u8;
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        self.device.unmap_memory(b.memory);
        Ok(b)
    }

    unsafe fn destroy_buffer(&self, b: &Buffer) {
        self.device.destroy_buffer(b.buffer, None);
        self.device.free_memory(b.memory, None);
    }

    unsafe fn create_frames(&mut self) -> Result<()> {
        let cmds = self.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default().command_pool(self.cmd_pool).command_buffer_count(FRAMES_IN_FLIGHT as u32),
        )?;
        let layouts = [self.set_layout; FRAMES_IN_FLIGHT];
        let sets = self.device.allocate_descriptor_sets(
            &vk::DescriptorSetAllocateInfo::default().descriptor_pool(self.desc_pool).set_layouts(&layouts),
        )?;
        for i in 0..FRAMES_IN_FLIGHT {
            let ubo = self.create_buffer(std::mem::size_of::<SceneUniforms>() as u64, vk::BufferUsageFlags::UNIFORM_BUFFER)?;
            let ubo_ptr = self.device.map_memory(ubo.memory, 0, ubo.size, vk::MemoryMapFlags::empty())? as *mut u8;
            let info = [vk::DescriptorBufferInfo { buffer: ubo.buffer, offset: 0, range: ubo.size }];
            let write = vk::WriteDescriptorSet::default()
                .dst_set(sets[i])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&info);
            self.device.update_descriptor_sets(&[write], &[]);
            self.frames.push(Frame {
                cmd: cmds[i],
                image_ready: self.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                fence: self.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)?,
                ubo,
                ubo_ptr,
                set: sets[i],
            });
        }
        Ok(())
    }

    unsafe fn create_swapchain(&mut self, width: u32, height: u32) -> Result<()> {
        let caps = self.surface_fn.get_physical_device_surface_capabilities(self.pdev, self.surface)?;
        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };
        if extent.width == 0 || extent.height == 0 {
            return Ok(()); // minimised
        }
        let old = self.swapchain.take();
        let mut count = caps.min_image_count + 1;
        if caps.max_image_count > 0 {
            count = count.min(caps.max_image_count);
        }
        let handle = self.swapchain_fn.create_swapchain(
            &vk::SwapchainCreateInfoKHR::default()
                .surface(self.surface)
                .min_image_count(count)
                .image_format(self.format.format)
                .image_color_space(self.format.color_space)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
                .pre_transform(caps.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(vk::PresentModeKHR::FIFO)
                .clipped(true)
                .old_swapchain(old.as_ref().map_or(vk::SwapchainKHR::null(), |s| s.handle)),
            None,
        )?;
        if let Some(old) = old {
            self.destroy_swapchain(old);
        }
        let images = self.swapchain_fn.get_swapchain_images(handle)?;
        let range = |aspect| vk::ImageSubresourceRange { aspect_mask: aspect, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 };
        let mut views = Vec::new();
        let mut render_done = Vec::new();
        for &img in &images {
            views.push(self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(img)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(self.format.format)
                    .subresource_range(range(vk::ImageAspectFlags::COLOR)),
                None,
            )?);
            render_done.push(self.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?);
        }
        let depth = self.device.create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(DEPTH_FORMAT)
                .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT),
            None,
        )?;
        let req = self.device.get_image_memory_requirements(depth);
        let ty = self.memory_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?;
        let depth_memory = self.device.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(ty), None)?;
        self.device.bind_image_memory(depth, depth_memory, 0)?;
        let depth_view = self.device.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(depth)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(DEPTH_FORMAT)
                .subresource_range(range(vk::ImageAspectFlags::DEPTH)),
            None,
        )?;
        self.swapchain = Some(Swapchain { handle, images, views, render_done, extent, depth, depth_memory, depth_view });
        Ok(())
    }

    unsafe fn destroy_swapchain(&self, s: Swapchain) {
        self.device.device_wait_idle().ok();
        for v in s.views {
            self.device.destroy_image_view(v, None);
        }
        for sem in s.render_done {
            self.device.destroy_semaphore(sem, None);
        }
        self.device.destroy_image_view(s.depth_view, None);
        self.device.destroy_image(s.depth, None);
        self.device.free_memory(s.depth_memory, None);
        self.swapchain_fn.destroy_swapchain(s.handle, None);
    }

    pub fn resize(&mut self) {
        self.resize_pending = true;
    }

    /// Current drawable size in pixels (0×0 when minimised).
    pub fn extent(&self) -> (u32, u32) {
        self.swapchain.as_ref().map_or((0, 0), |s| (s.extent.width, s.extent.height))
    }

    pub fn upload_mesh(&mut self, mesh: &MeshData) -> Result<MeshId> {
        unsafe {
            let vertices = self.upload(bytemuck::cast_slice(&mesh.vertices), vk::BufferUsageFlags::VERTEX_BUFFER)?;
            let indices = self.upload(bytemuck::cast_slice(&mesh.indices), vk::BufferUsageFlags::INDEX_BUFFER)?;
            self.meshes.push(GpuMesh { vertices, indices, count: mesh.indices.len() as u32 });
        }
        Ok(MeshId(self.meshes.len() - 1))
    }

    /// Save the next rendered frame as a PNG.
    pub fn screenshot(&mut self, path: impl Into<PathBuf>) {
        self.screenshot = Some(path.into());
    }

    pub fn render(&mut self, window: &Window, scene: &SceneUniforms, items: &[DrawItem]) -> Result<()> {
        unsafe { self.render_inner(window, scene, items) }
    }

    unsafe fn render_inner(&mut self, window: &Window, scene: &SceneUniforms, items: &[DrawItem]) -> Result<()> {
        if self.resize_pending || self.swapchain.is_none() {
            self.resize_pending = false;
            let size = window.inner_size();
            self.device.device_wait_idle()?;
            self.create_swapchain(size.width, size.height)?;
        }
        let Some(sc) = self.swapchain.as_ref() else { return Ok(()) };
        let f = &self.frames[self.frame];
        self.device.wait_for_fences(&[f.fence], true, u64::MAX)?;

        let (image_index, _) = match self.swapchain_fn.acquire_next_image(sc.handle, u64::MAX, f.image_ready, vk::Fence::null()) {
            Ok(r) => r,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.resize_pending = true;
                return Ok(());
            }
            Err(e) => bail!("acquire failed: {e}"),
        };
        self.device.reset_fences(&[f.fence])?;
        std::ptr::copy_nonoverlapping(bytemuck::bytes_of(scene).as_ptr(), f.ubo_ptr, std::mem::size_of::<SceneUniforms>());

        let image = sc.images[image_index as usize];
        let extent = sc.extent;
        let cmd = f.cmd;
        let d = &self.device;
        d.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
        d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;

        let color_range = vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 };
        let depth_range = vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::DEPTH, ..color_range };
        let barriers = [
            vk::ImageMemoryBarrier2::default()
                .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .image(image)
                .subresource_range(color_range),
            vk::ImageMemoryBarrier2::default()
                .src_stage_mask(vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS)
                .src_access_mask(vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE)
                .dst_stage_mask(vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS)
                .dst_access_mask(vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE | vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_READ)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                .image(sc.depth)
                .subresource_range(depth_range),
        ];
        d.cmd_pipeline_barrier2(cmd, &vk::DependencyInfo::default().image_memory_barriers(&barriers));

        let color = [vk::RenderingAttachmentInfo::default()
            .image_view(sc.views[image_index as usize])
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)];
        let depth = vk::RenderingAttachmentInfo::default()
            .image_view(sc.depth_view)
            .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent };
        d.cmd_begin_rendering(cmd, &vk::RenderingInfo::default().render_area(area).layer_count(1).color_attachments(&color).depth_attachment(&depth));
        d.cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: extent.width as f32, height: extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
        d.cmd_set_scissor(cmd, 0, &[area]);
        d.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[f.set], &[]);

        // Sky first (no depth), then the scene.
        d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.sky_pipeline);
        d.cmd_draw(cmd, 3, 1, 0, 0);

        d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.mesh_pipeline);
        let mut bound = None;
        for item in items {
            let m = &self.meshes[item.mesh.0];
            if bound != Some(item.mesh) {
                d.cmd_bind_vertex_buffers(cmd, 0, &[m.vertices.buffer], &[0]);
                d.cmd_bind_index_buffer(cmd, m.indices.buffer, 0, vk::IndexType::UINT32);
                bound = Some(item.mesh);
            }
            let push = Push { model: item.model, tint: item.tint };
            d.cmd_push_constants(cmd, self.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&push));
            d.cmd_draw_indexed(cmd, m.count, 1, 0, 0, 0);
        }
        d.cmd_end_rendering(cmd);

        // Optional screenshot: copy the image into a host buffer.
        let shot = self.screenshot.take();
        let readback = if shot.is_some() {
            let size = extent.width as u64 * extent.height as u64 * 4;
            let buf = self.create_buffer(size, vk::BufferUsageFlags::TRANSFER_DST)?;
            let to_src = [vk::ImageMemoryBarrier2::default()
                .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                .dst_stage_mask(vk::PipelineStageFlags2::COPY)
                .dst_access_mask(vk::AccessFlags2::TRANSFER_READ)
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .image(image)
                .subresource_range(color_range)];
            d.cmd_pipeline_barrier2(cmd, &vk::DependencyInfo::default().image_memory_barriers(&to_src));
            let region = vk::BufferImageCopy {
                buffer_offset: 0,
                buffer_row_length: 0,
                buffer_image_height: 0,
                image_subresource: vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 },
                image_offset: vk::Offset3D::default(),
                image_extent: vk::Extent3D { width: extent.width, height: extent.height, depth: 1 },
            };
            d.cmd_copy_image_to_buffer(cmd, image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, buf.buffer, &[region]);
            Some(buf)
        } else {
            None
        };

        let (old_layout, src_stage, src_access) = if readback.is_some() {
            (vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::PipelineStageFlags2::COPY, vk::AccessFlags2::TRANSFER_READ)
        } else {
            (vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT, vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
        };
        let to_present = [vk::ImageMemoryBarrier2::default()
            .src_stage_mask(src_stage)
            .src_access_mask(src_access)
            .dst_stage_mask(vk::PipelineStageFlags2::BOTTOM_OF_PIPE)
            .old_layout(old_layout)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .image(image)
            .subresource_range(color_range)];
        d.cmd_pipeline_barrier2(cmd, &vk::DependencyInfo::default().image_memory_barriers(&to_present));
        d.end_command_buffer(cmd)?;

        let wait = [f.image_ready];
        let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let cmds = [cmd];
        let signal = [sc.render_done[image_index as usize]];
        let submit = vk::SubmitInfo::default().wait_semaphores(&wait).wait_dst_stage_mask(&stages).command_buffers(&cmds).signal_semaphores(&signal);
        d.queue_submit(self.queue, &[submit], f.fence)?;

        let swapchains = [sc.handle];
        let indices = [image_index];
        let present = vk::PresentInfoKHR::default().wait_semaphores(&signal).swapchains(&swapchains).image_indices(&indices);
        match self.swapchain_fn.queue_present(self.queue, &present) {
            Ok(false) => {}
            Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.resize_pending = true,
            Err(e) => bail!("present failed: {e}"),
        }

        if let (Some(path), Some(buf)) = (shot, readback) {
            d.wait_for_fences(&[f.fence], true, u64::MAX)?;
            let ptr = d.map_memory(buf.memory, 0, buf.size, vk::MemoryMapFlags::empty())? as *const u8;
            let mut pixels = std::slice::from_raw_parts(ptr, buf.size as usize).to_vec();
            d.unmap_memory(buf.memory);
            self.destroy_buffer(&buf);
            if matches!(self.format.format, vk::Format::B8G8R8A8_SRGB | vk::Format::B8G8R8A8_UNORM) {
                for px in pixels.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
            }
            save_png(&path, extent.width, extent.height, &pixels)?;
        }

        self.frame = (self.frame + 1) % FRAMES_IN_FLIGHT;
        Ok(())
    }
}

fn save_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().ok();
            for m in std::mem::take(&mut self.meshes) {
                self.destroy_buffer(&m.vertices);
                self.destroy_buffer(&m.indices);
            }
            for f in std::mem::take(&mut self.frames) {
                self.destroy_buffer(&f.ubo);
                self.device.destroy_semaphore(f.image_ready, None);
                self.device.destroy_fence(f.fence, None);
            }
            if let Some(s) = self.swapchain.take() {
                self.destroy_swapchain(s);
            }
            self.device.destroy_pipeline(self.mesh_pipeline, None);
            self.device.destroy_pipeline(self.sky_pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device.destroy_descriptor_pool(self.desc_pool, None);
            self.device.destroy_descriptor_set_layout(self.set_layout, None);
            self.device.destroy_command_pool(self.cmd_pool, None);
            self.device.destroy_device(None);
            self.surface_fn.destroy_surface(self.surface, None);
            if let Some((du, m)) = self.debug.take() {
                du.destroy_debug_utils_messenger(m, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _ty: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let msg = CStr::from_ptr((*data).p_message).to_string_lossy();
    eprintln!("[vulkan {severity:?}] {msg}");
    vk::FALSE
}
