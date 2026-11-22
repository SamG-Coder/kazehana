//! Akari (明かり): a small, clean, hardware ray traced engine for Kazehana.
//!
//! - [`renderer`]: the frame: path tracing, DLSS Ray Reconstruction / TAA, exposure, tonemap
//! - [`scene`]: materials, meshes, instances and lights handed to the renderer
//! - [`gltf_load`]: glTF 2.0 models (Fab downloads, Blender exports)
//! - [`mesh`]: procedural geometry for greybox and test scenes
//! - [`camera`]: first-person camera
//! - `gpu`, `accel`, `textures`, `bc7`, `dlss`: Vulkan plumbing (ported from SmartImposter)

pub mod accel;
pub mod bc7;
pub mod camera;
pub mod dlss;
pub mod gltf_load;
pub mod gpu;
pub mod mesh;
pub mod renderer;
pub mod scene;
pub mod textures;
pub mod ui;

pub use camera::Camera;
pub use glam;
pub use gltf_load::{LoadOptions, Model, Part};
pub use mesh::MeshBuilder;
pub use renderer::{FrameDesc, ImpostorVariant, PlantPlacement, Renderer, RendererOptions};
pub use scene::{Instance, Light, Material, MaterialId, MeshData, MeshId, TextureId};
pub use ui::{Rgba, Ui};
pub use winit;
