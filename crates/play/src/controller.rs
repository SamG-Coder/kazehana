//! The first-person controller: a standing capsule with its feet on the floor.
//!
//! Each step it turns with the look input, accelerates along the floor towards what the movement
//! asks (quick on the feet, little steering in the air), moves in short sub-steps, slides along
//! whatever it meets, and settles on the floor: up steps and small kerbs (`Settings::step`),
//! along slopes up to `max_slope`, falling off edges under gravity. Crouching lowers the eye and
//! the capsule (and it stays down under anything low). The eye follows the feet smoothly up and
//! down steps, and comes to rest exactly when nothing moves (a still image keeps accumulating).
//!
//! Ported from SmartImposter's walking camera (src/walk.rs), on real collision.

use crate::collision::{Contact, World};
use crate::input::Actions;
use glam::{Vec2, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// the body's radius (m)
    pub radius: f32,
    /// standing and crouched heights (m)
    pub height: f32,
    pub crouch_height: f32,
    /// eye heights above the feet (m)
    pub eye: f32,
    pub crouch_eye: f32,
    /// the highest step walked up without jumping (m)
    pub step: f32,
    /// the steepest floor that can be stood on (degrees)
    pub max_slope: f32,
    /// speeds (m/s)
    pub walk: f32,
    pub run: f32,
    pub crouch_speed: f32,
    /// take-off speed of a jump (m/s), gravity (m/s²)
    pub jump: f32,
    pub gravity: f32,
    /// how quickly the speed follows the stick, on the floor and in the air (1/s)
    pub grip: f32,
    pub air_grip: f32,
    /// look limits (radians)
    pub pitch_limit: f32,
}

impl Default for Settings {
    fn default() -> Self {
        // a 12-year-old: about 1.5 m tall
        Settings {
            radius: 0.26,
            height: 1.55,
            crouch_height: 1.05,
            eye: 1.42,
            crouch_eye: 0.92,
            step: 0.32,
            max_slope: 48.0,
            walk: 1.7,
            run: 4.6,
            crouch_speed: 0.9,
            jump: 3.4,
            gravity: 9.81,
            grip: 12.0,
            air_grip: 1.5,
            pitch_limit: 1.5,
        }
    }
}

pub struct FirstPerson {
    pub settings: Settings,
    /// the feet (bottom of the capsule), world
    pub feet: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
    pub crouched: bool,
    /// the eye's height above the feet now (between crouch_eye and eye)
    eye_h: f32,
    /// the eye's height in the world (smoothed over steps)
    eye_y: f32,
    contacts: Vec<Contact>,
}

impl FirstPerson {
    pub fn new(feet: Vec3, yaw: f32, settings: Settings) -> Self {
        FirstPerson { settings, feet, vel: Vec3::ZERO, yaw, pitch: 0.0, grounded: false, crouched: false, eye_h: settings.eye, eye_y: feet.y + settings.eye, contacts: Vec::new() }
    }

    /// Put the body somewhere at once (a cut: no smoothing, no speed).
    pub fn teleport(&mut self, feet: Vec3, yaw: f32) {
        self.feet = feet;
        self.vel = Vec3::ZERO;
        self.yaw = yaw;
        self.eye_y = feet.y + self.eye_h;
        self.grounded = false;
    }

    /// The eye, for the camera.
    pub fn eye(&self) -> Vec3 {
        Vec3::new(self.feet.x, self.eye_y, self.feet.z)
    }

    /// Where the eye looks (unit).
    pub fn look_dir(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), -self.yaw.cos() * self.pitch.cos())
    }

    fn height(&self) -> f32 {
        if self.crouched { self.settings.crouch_height } else { self.settings.height }
    }

    /// The body's capsule: the segment its spheres run along (the bottom one raised by the step
    /// height, so steps and kerbs are walked up rather than walked into), and the radius.
    pub fn capsule(&self) -> (Vec3, Vec3, f32) {
        let s = &self.settings;
        let lo = s.step + s.radius;
        let hi = (self.height() - s.radius).max(lo);
        (self.feet + Vec3::Y * lo, self.feet + Vec3::Y * hi, s.radius)
    }

    /// One frame.
    pub fn update(&mut self, a: &Actions, world: &World, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let s = self.settings;
        self.yaw = (self.yaw + a.look.x).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + a.look.y).clamp(-s.pitch_limit, s.pitch_limit);
        let near = |rate: f32| 1.0 - (-rate * dt).exp();

        // crouch (stand up only where there is room)
        if a.crouch {
            self.crouched = true;
        } else if self.crouched {
            let top = self.feet + Vec3::Y * (s.crouch_height - 0.05);
            if world.raycast(top, Vec3::Y, s.height - s.crouch_height + 0.05).is_none() {
                self.crouched = false;
            }
        }

        // ---- along the floor
        let (fwd, right) = (Vec2::new(self.yaw.sin(), -self.yaw.cos()), Vec2::new(self.yaw.cos(), self.yaw.sin()));
        let wish = fwd * a.movement.y + right * a.movement.x;
        let speed = if self.crouched { s.crouch_speed } else if a.run { s.run } else { s.walk };
        let mut v = Vec2::new(self.vel.x, self.vel.z);
        v += (wish * speed - v) * near(if self.grounded { s.grip } else { s.air_grip });
        if wish == Vec2::ZERO && self.grounded && v.length() < 0.02 {
            v = Vec2::ZERO;
        }
        self.vel.x = v.x;
        self.vel.z = v.y;
        let travel = v.length() * dt;
        let n = ((travel / (s.radius * 0.5)).ceil() as usize).clamp(1, 16);
        for _ in 0..n {
            self.feet += Vec3::new(self.vel.x, 0.0, self.vel.z) * (dt / n as f32);
            self.slide(world);
        }

        // ---- up and down
        let want = if self.crouched { s.crouch_eye } else { s.eye };
        self.eye_h += (want - self.eye_h) * near(10.0);
        if (want - self.eye_h).abs() < 1e-3 {
            self.eye_h = want;
        }
        if self.grounded && a.jump && !self.crouched {
            self.vel.y = s.jump;
            self.grounded = false;
        }
        if self.grounded {
            // stay on the floor: up a step, down a step or a slope; off an edge: fall
            match self.floor(world, s.step * 2.0) {
                Some(f) => {
                    self.feet.y = f;
                    self.vel.y = 0.0;
                }
                None => self.grounded = false,
            }
        }
        if !self.grounded {
            self.vel.y -= s.gravity * dt;
            if self.vel.y > 0.0 {
                // a ceiling stops the rise
                let top = self.feet + Vec3::Y * (self.height() - 0.02);
                if world.raycast(top, Vec3::Y, self.vel.y * dt + 0.02).is_some() {
                    self.vel.y = 0.0;
                }
            }
            self.feet.y += self.vel.y * dt;
            if self.vel.y <= 0.0 {
                if let Some(f) = self.floor(world, 0.0) {
                    if f >= self.feet.y - 1e-4 {
                        self.feet.y = f;
                        self.vel.y = 0.0;
                        self.grounded = true;
                    }
                }
            }
        }

        // the eye: smoothly over steps on the floor, directly in the air
        let target = self.feet.y + self.eye_h;
        if self.grounded {
            self.eye_y += (target - self.eye_y) * near(16.0);
            if (target - self.eye_y).abs() < 1e-4 {
                self.eye_y = target;
            }
        } else {
            self.eye_y = target;
        }
    }

    /// Push the body out of whatever it has moved into, sliding along it.
    fn slide(&mut self, world: &World) {
        let s = self.settings;
        let cos_max = s.max_slope.to_radians().cos();
        for _ in 0..4 {
            let (a, b, r) = self.capsule();
            self.contacts.clear();
            world.capsule_contacts(a, b, r, &mut self.contacts);
            let mut push = Vec3::ZERO;
            for c in &self.contacts {
                // (a floor at step height or below is stood on, not pushed against)
                if c.normal.y > cos_max && c.point.y <= self.feet.y + s.step + 0.1 {
                    continue;
                }
                // out sideways (the floor carries the body; a low ceiling is not climbed into)
                let mut n = Vec3::new(c.normal.x, 0.0, c.normal.z);
                if n.length_squared() < 1e-4 {
                    n = Vec3::new(self.feet.x - c.point.x, 0.0, self.feet.z - c.point.z);
                }
                let Some(n) = n.try_normalize() else { continue };
                // the depth sideways: how far out along n the capsule must go to clear it
                let depth = c.depth / n.dot(c.normal).abs().max(0.3);
                let need = depth.min(s.radius) - push.dot(n);
                if need > 0.0 {
                    push += n * need;
                }
            }
            if push.length_squared() < 1e-10 {
                break;
            }
            self.feet += push;
            // the speed into it is taken, the rest slides past
            let pn = push.normalize();
            let into = self.vel.dot(pn);
            if into < 0.0 {
                self.vel -= pn * into;
            }
        }
    }

    /// The floor under the feet: the highest walkable surface found by rays down from step
    /// height, reaching `below` under the feet (the centre and four points round it).
    fn floor(&self, world: &World, below: f32) -> Option<f32> {
        let s = self.settings;
        let cos_max = s.max_slope.to_radians().cos();
        let o = self.feet + Vec3::Y * (s.step + 0.01);
        let reach = s.step + 0.01 + below;
        let k = s.radius * 0.6;
        let mut best: Option<f32> = None;
        for d in [Vec3::ZERO, Vec3::X * k, -Vec3::X * k, Vec3::Z * k, -Vec3::Z * k] {
            if let Some(h) = world.raycast(o + d, Vec3::NEG_Y, reach) {
                if h.normal.y > cos_max {
                    best = Some(best.map_or(h.point.y, |b: f32| b.max(h.point.y)));
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::Shape;
    use glam::Affine3A;

    fn walk(fp: &mut FirstPerson, w: &World, a: Actions, secs: f32) {
        for _ in 0..(secs * 120.0) as u32 {
            fp.update(&a, w, 1.0 / 120.0);
        }
    }

    fn fwd() -> Actions {
        Actions { movement: Vec2::new(0.0, 1.0), ..Default::default() }
    }

    fn flat() -> World {
        let mut w = World::new();
        w.set_ground(|_, _| 0.0);
        w
    }

    #[test]
    fn lands_walks_and_comes_to_rest() {
        let w = flat();
        let mut fp = FirstPerson::new(Vec3::new(0.0, 3.0, 0.0), 0.0, Settings::default());
        walk(&mut fp, &w, Actions::default(), 1.5);
        assert!(fp.grounded && fp.feet.y.abs() < 1e-4);
        walk(&mut fp, &w, fwd(), 2.0);
        assert!(fp.feet.z < -2.5, "walked forward (yaw 0 looks along -z): {:?}", fp.feet);
        walk(&mut fp, &w, Actions::default(), 1.0);
        let rest = (fp.feet, fp.eye());
        walk(&mut fp, &w, Actions::default(), 0.3);
        assert_eq!((fp.feet, fp.eye()), rest, "at rest exactly");
    }

    #[test]
    fn walls_stop_and_slide() {
        let mut w = flat();
        // a wall across the way at z = -2, and one at 45 degrees further on
        w.add(Shape::cuboid(Vec3::new(-5.0, 0.0, -2.2), Vec3::new(5.0, 3.0, -2.0)), Affine3A::IDENTITY);
        let mut fp = FirstPerson::new(Vec3::ZERO, 0.0, Settings::default());
        walk(&mut fp, &w, fwd(), 3.0);
        assert!(fp.feet.z > -2.0 + 0.26 - 1e-3 && fp.feet.z < -1.6, "stopped at the wall: {:?}", fp.feet);
        // walking into it at an angle slides along it
        fp.yaw = 0.6;
        let x0 = fp.feet.x;
        walk(&mut fp, &w, fwd(), 2.0);
        assert!(fp.feet.x > x0 + 1.0 && fp.feet.z > -2.0 + 0.25, "slid along: {:?}", fp.feet);
    }

    #[test]
    fn steps_up_but_not_walls() {
        let mut w = flat();
        // a 0.2 m step at z <= -1, then a 0.6 m ledge at z <= -3
        w.add(Shape::cuboid(Vec3::new(-5.0, 0.0, -10.0), Vec3::new(5.0, 0.2, -1.0)), Affine3A::IDENTITY);
        w.add(Shape::cuboid(Vec3::new(-5.0, 0.0, -10.0), Vec3::new(5.0, 0.8, -3.0)), Affine3A::IDENTITY);
        let mut fp = FirstPerson::new(Vec3::ZERO, 0.0, Settings::default());
        walk(&mut fp, &w, Actions::default(), 0.5);
        walk(&mut fp, &w, fwd(), 3.0);
        assert!((fp.feet.y - 0.2).abs() < 1e-3, "up the step: {:?}", fp.feet);
        assert!(fp.feet.z > -3.0, "not up the ledge: {:?}", fp.feet);
        // but a jump gets up it
        walk(&mut fp, &w, Actions { movement: Vec2::new(0.0, 1.0), jump: true, ..Default::default() }, 0.02);
        walk(&mut fp, &w, fwd(), 1.5);
        assert!((fp.feet.y - 0.8).abs() < 1e-3, "jumped up: {:?}", fp.feet);
    }

    #[test]
    fn falls_off_edges_and_crouches_under() {
        let mut w = World::new();
        // a platform 1 m up, nothing beyond z < -2 but the ground 0 far below... a floor at 0
        w.add(Shape::cuboid(Vec3::new(-5.0, 0.0, -2.0), Vec3::new(5.0, 1.0, 5.0)), Affine3A::IDENTITY);
        w.set_ground(|_, _| 0.0);
        let mut fp = FirstPerson::new(Vec3::new(0.0, 1.0, 0.0), 0.0, Settings::default());
        walk(&mut fp, &w, Actions::default(), 0.3);
        assert!((fp.feet.y - 1.0).abs() < 1e-3);
        walk(&mut fp, &w, fwd(), 2.5);
        assert!(fp.feet.y.abs() < 1e-3 && fp.grounded, "fell off: {:?}", fp.feet);
        // a low beam: crouched, under it; it stays crouched beneath it
        let mut w = flat();
        w.add(Shape::cuboid(Vec3::new(-5.0, 1.2, -3.0), Vec3::new(5.0, 1.5, -2.0)), Affine3A::IDENTITY);
        let mut fp = FirstPerson::new(Vec3::ZERO, 0.0, Settings::default());
        walk(&mut fp, &w, Actions::default(), 0.3);
        walk(&mut fp, &w, fwd(), 2.5);
        assert!(fp.feet.z > -2.0, "blocked standing: {:?}", fp.feet);
        let crouch = Actions { movement: Vec2::new(0.0, 1.0), crouch: true, ..Default::default() };
        walk(&mut fp, &w, crouch, 0.9);
        assert!(fp.feet.z < -2.2, "crouched under: {:?}", fp.feet);
        walk(&mut fp, &w, Actions::default(), 0.5);
        assert!(fp.crouched, "no room to stand under the beam");
    }
}
