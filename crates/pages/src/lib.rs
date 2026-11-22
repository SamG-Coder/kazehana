//! Paper in the world. Information the player reads lives on real things (`docs/life_plan.md`):
//! the wall calendar, the journal in your bag. This draws their pages from the game state as
//! images; the game puts them on the objects as textures and draws them again when what's on them
//! changes.
//!
//! - [`calendar::month`]: a Japanese wall calendar's month (holidays and Sundays red, Saturdays
//!   blue, the town's events, release days, and your plans in pencil).
//! - [`journal`]: the journal's spreads (the diary, people you know, asks and plans).

pub mod calendar;
mod canvas;
pub mod journal;
pub mod school;

pub use canvas::{Canvas, Font, Rgba};
