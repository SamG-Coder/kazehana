//! `--doortour <model>`: every door of a building, photographed in the real game (hidden
//! window): from each side of the doorway, closed and then swung open, so a door that opens the
//! wrong way, into a wall or a stair, or with no room to walk through, shows. Shots go to
//! shots/doors/<model>/NN_<side>_<closed|open>.png and a list of the doors to
//! shots/doors/<model>/doors.txt.

use crate::game::Game;
use akari::glam::Vec3;

pub struct DoorTour {
    model: String,
    doors: Vec<usize>,
    k: usize,
    phase: u8,
    wait: u32,
    pub done: bool,
}

impl DoorTour {
    pub fn new(model: &str) -> Self {
        Self { model: model.to_string(), doors: Vec::new(), k: 0, phase: 0, wait: 40, done: false }
    }

    fn set(g: &mut Game, d: usize, open: f32) {
        let Some(p) = g.world.as_mut() else { return };
        let door = &mut p.doors.doors[d];
        door.open = open;
        door.target = open;
        let (b, t) = (door.body, door.transform());
        p.world.set_transform(b, t);
    }

    /// Before each frame: the next view when it's time; a screenshot name to take now.
    pub fn frame(&mut self, g: &mut Game) -> Option<String> {
        if self.done {
            return None;
        }
        g.hold_still = true;
        if self.wait > 0 {
            self.wait -= 1;
            return None;
        }
        self.wait = 14;
        let Some(p) = g.world.as_ref() else {
            self.done = true;
            return None;
        };
        if self.doors.is_empty() {
            let key = format!("{} door ", self.model);
            self.doors = (0..p.doors.doors.len()).filter(|&i| p.label(p.doors.doors[i].body).starts_with(&key)).collect();
            let list: Vec<String> = self.doors.iter().enumerate().map(|(n, &i)| format!("{n:02} {}", p.label(p.doors.doors[i].body))).collect();
            let _ = std::fs::create_dir_all(format!("shots/doors/{}", self.model));
            let _ = std::fs::write(format!("shots/doors/{}/doors.txt", self.model), list.join("\n"));
            println!("doortour: {} doors in {}", self.doors.len(), self.model);
            if self.doors.is_empty() {
                self.done = true;
            }
            return None;
        }
        if self.k >= self.doors.len() {
            self.done = true;
            return None;
        }
        let d = self.doors[self.k];
        let door = &p.doors.doors[d];
        let (lo, hi) = (door.min, door.max);
        let size = hi - lo;
        let thin = if size.x < size.z { Vec3::X } else { Vec3::Z };
        let mid = door.closed.transform_point3((lo + hi) * 0.5);
        let through = door.closed.transform_vector3(thin).normalize_or_zero();
        let floor = door.closed.transform_point3(Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5)).y;
        // (phases: side a closed, side a open, side b closed, side b open)
        let side = if self.phase < 2 { -1.0 } else { 1.0 };
        let open = self.phase % 2 == 1;
        // close every door of this building, then this one as the phase says
        for &o in &self.doors {
            Self::set(g, o, 0.0);
        }
        Self::set(g, d, if open { 1.0 } else { 0.0 });
        // stand back from the opening (less if a wall is in the way behind you)
        let back = (size.y.max(1.0) * 0.9).clamp(1.4, 2.2);
        let eye_h = 1.45;
        let mut dist = back;
        if let Some(w) = g.world.as_ref() {
            let o = Vec3::new(mid.x, floor + eye_h, mid.z) + through * (side * 0.3);
            if let Some(h) = w.world.raycast(o, through * side, back + 0.4) {
                // (a wall close behind: stand just in front of it, never through it)
                dist = (h.t - 0.35).clamp(0.0, back);
            }
        }
        let feet = Vec3::new(mid.x, floor, mid.z) + through * (side * (dist + 0.3));
        let look = Vec3::new(mid.x, floor + size.y.min(2.2) * 0.5, mid.z) - (feet + Vec3::Y * crate::game::EYE);
        let yaw = look.x.atan2(-look.z);
        let pitch = (look.y / look.with_y(0.0).length().max(0.01)).atan();
        if std::env::var_os("KZ_TOUR_DEBUG").is_some() {
            println!("door {} side {side}: mid {mid:?} through {through:?} floor {floor:.2} dist {dist:.2} feet {feet:?} size {size:?}", self.k);
        }
        g.place_player(feet, yaw, pitch);
        g.cut = true;
        let name = format!("{}/{:02}_{}_{}", self.model, self.k, if side < 0.0 { "a" } else { "b" }, if open { "open" } else { "closed" });
        self.phase += 1;
        if self.phase == 4 {
            self.phase = 0;
            self.k += 1;
        }
        Some(name)
    }
}
