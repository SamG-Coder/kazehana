//! Doors: any moving leaf in a model, found by its node's name.
//!
//! Name a door leaf's node (its origin on the hinge, or anywhere on a sliding leaf):
//!
//! ```text
//! kzdoor_hinge_<n|p><degrees>[_auto][_tag]     swings about its up axis: n = clockwise seen
//!                                              from above, p = anticlockwise
//! kzdoor_slide_<n|p><millimetres>[_auto][_tag] slides along its own x axis (n = towards -x)
//! ```
//!
//! e.g. `kzdoor_hinge_n95_back` (a door opening 95 degrees outwards), `kzdoor_slide_p1300_auto_r`
//! (the right leaf of an automatic shop door). A hand door opens and closes when the player uses
//! it (looking at it, within reach); an automatic one opens while the player is near it. A door
//! never closes into the player.

use crate::collision::{BodyId, Shape, World};
use crate::controller::FirstPerson;
use glam::{Affine3A, Quat, Vec3};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DoorKind {
    /// turns about the leaf's up axis by this many degrees (anticlockwise seen from above)
    Hinge(f32),
    /// moves along the leaf's x axis by this many metres
    Slide(f32),
}

/// A door node's name to its kind and whether it is automatic.
pub fn parse(node: &str) -> Option<(DoorKind, bool)> {
    let rest = node.strip_prefix("kzdoor_")?;
    let mut it = rest.split('_');
    let kind = it.next()?;
    let amount = it.next()?;
    let (sign, digits) = match amount.split_at_checked(1)? {
        ("n", d) => (-1.0, d),
        ("p", d) => (1.0, d),
        _ => return None,
    };
    let v: f32 = digits.parse().ok()?;
    let auto = it.any(|t| t == "auto");
    match kind {
        "hinge" => Some((DoorKind::Hinge(sign * v), auto)),
        "slide" => Some((DoorKind::Slide(sign * v / 1000.0), auto)),
        _ => None,
    }
}

pub struct Door {
    pub kind: DoorKind,
    pub auto: bool,
    /// the leaf's node in the world, closed
    pub closed: Affine3A,
    /// 0 closed .. 1 open, and where it is heading
    pub open: f32,
    pub target: f32,
    pub body: BodyId,
    /// the leaf's bounds in its own space
    pub min: Vec3,
    pub max: Vec3,
    /// the leaf's node name (kzdoor_...)
    pub name: String,
}

impl Door {
    /// The leaf's node in the world now.
    pub fn transform(&self) -> Affine3A {
        self.at(self.open)
    }

    fn at(&self, open: f32) -> Affine3A {
        // eased: it starts and stops gently
        let t = open * open * (3.0 - 2.0 * open);
        self.closed
            * match self.kind {
                DoorKind::Hinge(deg) => Affine3A::from_quat(Quat::from_rotation_y(deg.to_radians() * t)),
                DoorKind::Slide(d) => Affine3A::from_translation(Vec3::X * d * t),
            }
    }

    /// The middle of the leaf, closed (world).
    pub fn centre(&self) -> Vec3 {
        self.closed.transform_point3((self.min + self.max) * 0.5)
    }

    /// Open it if it is shut (or going shut), shut it if open.
    pub fn toggle(&mut self) {
        self.target = if self.target > 0.5 { 0.0 } else { 1.0 };
    }
}

#[derive(Default)]
pub struct Doors {
    pub doors: Vec<Door>,
}

impl Doors {
    /// A door from a node: its name says how it moves; `closed` is the node's world transform,
    /// `tris` the leaf's triangles in the node's space. None when the name is not a door's.
    pub fn add(&mut self, world: &mut World, node: &str, closed: Affine3A, tris: impl IntoIterator<Item = [Vec3; 3]>) -> Option<usize> {
        let (kind, auto) = parse(node)?;
        let shape: Arc<Shape> = Shape::new(tris);
        let (min, max) = (shape.min, shape.max);
        let body = world.add_moving(shape, closed);
        self.doors.push(Door { kind, auto, closed, open: 0.0, target: 0.0, body, min, max, name: node.to_string() });
        Some(self.doors.len() - 1)
    }

    /// Move the doors on: automatic ones open while the player is within `AUTO_REACH`; none
    /// closes into the player.
    pub fn update(&mut self, world: &mut World, player: &FirstPerson, dt: f32) {
        const AUTO_REACH: f32 = 1.9;
        let (a, b, r) = player.capsule();
        for d in &mut self.doors {
            if d.auto {
                let c = d.centre();
                let near = Vec3::new(c.x - player.feet.x, 0.0, c.z - player.feet.z).length() < AUTO_REACH && (c.y - player.feet.y).abs() < 2.5;
                d.target = if near { 1.0 } else { 0.0 };
            }
            if d.open == d.target {
                continue;
            }
            let rate = match d.kind {
                DoorKind::Hinge(_) => 1.1,
                DoorKind::Slide(_) => 1.4,
            };
            let next = if d.target > d.open { (d.open + rate * dt).min(d.target) } else { (d.open - rate * dt).max(d.target) };
            world.set_transform(d.body, d.at(next));
            if world.capsule_overlaps(d.body, a, b, r) {
                // (the player is in the way: wait)
                world.set_transform(d.body, d.at(d.open));
                continue;
            }
            d.open = next;
        }
    }

    /// The door the player is looking at, within `reach` of the eye.
    pub fn looked_at(&self, world: &World, eye: Vec3, dir: Vec3, reach: f32) -> Option<usize> {
        let hit = world.raycast(eye, dir, reach)?;
        let body = hit.body?;
        self.doors.iter().position(|d| d.body == body && !d.auto)
    }

    /// Use the door being looked at (the player's interact); true if there was one.
    pub fn interact(&mut self, world: &World, eye: Vec3, dir: Vec3) -> bool {
        match self.looked_at(world, eye, dir, 2.2) {
            Some(i) => {
                self.doors[i].toggle();
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::Settings;
    use crate::input::Actions;
    use glam::Vec2;

    #[test]
    fn names() {
        assert_eq!(parse("kzdoor_hinge_n95_back"), Some((DoorKind::Hinge(-95.0), false)));
        assert_eq!(parse("kzdoor_slide_p1300_auto_r"), Some((DoorKind::Slide(1.3), true)));
        assert_eq!(parse("walls"), None);
        assert_eq!(parse("kzdoor_spin_p10"), None);
    }

    /// A wall across z = -3 with a 1 m opening, a hinged door in it.
    fn doorway() -> (World, Doors) {
        let mut w = World::new();
        w.set_ground(|_, _| 0.0);
        w.add(Shape::cuboid(Vec3::new(-5.0, 0.0, -3.1), Vec3::new(-0.5, 2.5, -2.9)), Affine3A::IDENTITY);
        w.add(Shape::cuboid(Vec3::new(0.5, 0.0, -3.1), Vec3::new(5.0, 2.5, -2.9)), Affine3A::IDENTITY);
        let mut doors = Doors::default();
        // the leaf from the hinge at x = -0.5 to x = 0.5, swinging away from the player (-z)
        let c =|i: u32| Vec3::new(if i & 1 == 0 { 0.0 } else { 1.0 }, if i & 2 == 0 { 0.0 } else { 2.0 }, if i & 4 == 0 { -0.02 } else { 0.02 });
        let quads = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
        let tris = quads.iter().flat_map(|q| [[c(q[0]), c(q[1]), c(q[2])], [c(q[0]), c(q[2]), c(q[3])]]);
        doors.add(&mut w, "kzdoor_hinge_p90_test", Affine3A::from_translation(Vec3::new(-0.5, 0.0, -3.0)), tris).unwrap();
        (w, doors)
    }

    #[test]
    fn a_door_blocks_until_opened() {
        let (mut w, mut doors) = doorway();
        let mut fp = FirstPerson::new(Vec3::ZERO, 0.0, Settings::default());
        let go = Actions { movement: Vec2::new(0.0, 1.0), ..Default::default() };
        for _ in 0..360 {
            fp.update(&go, &w, 1.0 / 120.0);
            doors.update(&mut w, &fp, 1.0 / 120.0);
        }
        assert!(fp.feet.z > -3.0 + 0.26 - 1e-3 && fp.feet.z < -2.5, "stopped at the closed door: {:?}", fp.feet);
        // look at it and use it: it swings open (away), and the way is free
        assert!(doors.interact(&w, fp.eye(), fp.look_dir()), "the door is in reach");
        for _ in 0..240 {
            doors.update(&mut w, &fp, 1.0 / 120.0);
        }
        assert_eq!(doors.doors[0].open, 1.0);
        for _ in 0..360 {
            fp.update(&go, &w, 1.0 / 120.0);
            doors.update(&mut w, &fp, 1.0 / 120.0);
        }
        assert!(fp.feet.z < -4.0, "through the doorway: {:?}", fp.feet);
    }

    #[test]
    fn automatic_doors_open_as_you_come() {
        let (mut w, mut doors) = doorway();
        doors.doors[0].auto = true;
        let mut fp = FirstPerson::new(Vec3::new(0.0, 0.0, 2.0), 0.0, Settings::default());
        let go = Actions { movement: Vec2::new(0.0, 1.0), ..Default::default() };
        for _ in 0..480 {
            fp.update(&go, &w, 1.0 / 120.0);
            doors.update(&mut w, &fp, 1.0 / 120.0);
        }
        assert!(fp.feet.z < -4.0, "walked straight through: {:?}", fp.feet);
        // and it shuts behind once the player has gone on
        for _ in 0..480 {
            fp.update(&go, &w, 1.0 / 120.0);
            doors.update(&mut w, &fp, 1.0 / 120.0);
        }
        assert_eq!(doors.doors[0].open, 0.0);
    }
}
