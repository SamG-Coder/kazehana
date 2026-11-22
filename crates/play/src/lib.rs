//! Playing in the world, independent of the renderer:
//!
//! - [`input`]: keyboard and mouse, PlayStation controllers (DualSense, DualShock 4, read from
//!   their own HID reports) and other gamepads (gilrs), merged into one [`input::Actions`] a frame.
//! - [`collision`]: solid shapes (triangle meshes, placed rigidly, some of them moving) and the
//!   queries a body needs: capsule contacts and rays.
//! - [`controller`]: the first-person controller: walk, run, crouch, jump, gravity, steps, slopes,
//!   sliding along walls.
//! - [`cloth`]: hanging fabric (noren, curtains) simulated with XPBD: weight, folds, air,
//!   people walking through.
//! - [`door`]: generic doors (hinged or sliding, opened by hand or automatically), found in
//!   models by their node names.
//!
//! Frames: world space is the renderer's (+Y up, metres). The camera convention is akari's: yaw
//! 0 looks along -Z, positive yaw turns towards +X; pitch up is positive.

pub mod cloth;
pub mod collision;
pub mod controller;
pub mod door;
pub mod input;

pub use cloth::{Air, Cloth, Collider};
pub use collision::{BodyId, Shape, World};
pub use controller::{FirstPerson, Settings};
pub use door::{Door, DoorKind, Doors};
pub use input::{Actions, Glyph, Input, Kit, Prompts};
