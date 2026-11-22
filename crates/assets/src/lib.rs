//! The asset quality gate (docs/ASSET_QUALITY.md): automated checks of glTF assets and the
//! approval registry. The game loads 3D models only through [`Registry::load_model`].

pub mod check;
pub mod registry;

pub use check::{check, Category, Level, Report};
pub use registry::{Registry, Status, REGISTRY_PATH};
