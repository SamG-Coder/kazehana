//! Kazehana life simulation: calendar, clock and sky, students and bonds, economy and save data.
//! No graphics; everything here is deterministic and unit-tested.

pub mod binder;
pub mod calendar;
pub mod clock;
pub mod economy;
pub mod items;
pub mod life;
pub mod people;
pub mod state;
pub mod town;

pub use calendar::{Date, Weekday, START_DATE};
pub use clock::{GameTime, Phase, Sky};
pub use state::{GameState, Location};
pub use town::{HouseKind, HouseSlot, Household, Town};
