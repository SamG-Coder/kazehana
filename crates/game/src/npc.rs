//! Where people are. (Nobody is drawn yet: real characters come through the asset quality gate.)

use akari::glam::Vec3;
use sim::clock::at;
use sim::GameState;

/// A student standing somewhere in the current level.
#[derive(Clone, Copy)]
pub struct Placed {
    pub student: usize,
    pub pos: Vec3,
}

/// Uncle Kenji minds the counter during opening hours; otherwise he's in the living room.
pub fn uncle_spot(state: &GameState) -> (Vec3, f32) {
    if (at::SHOP_OPEN..at::SHOP_CLOSE).contains(&state.time.minute) {
        (Vec3::new(-2.0, 0.0, 3.6), 0.0)
    } else {
        (Vec3::new(-1.5, 0.0, 6.4), std::f32::consts::PI)
    }
}
