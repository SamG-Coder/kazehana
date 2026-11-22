//! `--autoplay`: the first morning played through by itself in the real game (a hidden window,
//! the real town, its collision and doors, the real prompts and quests), one step every few
//! seconds of frames, a screenshot at each (shots/slice/auto_NN_*.png), and what happened
//! printed. It checks that the slice holds together, as a player would go through it:
//!
//! wake in your room (6:45) → get changed at your closet → say goodbye to Uncle Kenji at the
//! counter (the shop open, the sign 営業中) → walk to school (the route on the map) → your locker
//! in 1-A (the timetable) → your seat before homeroom (8:30), the class arriving at their own
//! times → the bell.

use crate::game::{Game, Key, Mode};
use akari::glam::Vec3;
use sim::clock::at::hm;

pub struct Autoplay {
    step: usize,
    wait: u32,

    pub done: bool,
    pub failed: Vec<String>,
}

impl Autoplay {
    pub fn new() -> Self {
        Self { step: 0, wait: 40, done: false, failed: Vec::new() }
    }

    fn check(&mut self, ok: bool, what: &str) {
        println!("autoplay: {} {what}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            self.failed.push(what.to_string());
        }
    }

    /// Close whatever dialogue is open.
    fn dismiss(g: &mut Game) {
        for _ in 0..6 {
            if matches!(g.mode, Mode::Dialogue { .. }) {
                g.key(Key::Confirm, false);
            }
        }
    }

    fn face(g: &mut Game, feet: Vec3, look: Vec3) {
        let to = look - (feet + Vec3::Y * crate::game::EYE);
        let yaw = to.x.atan2(-to.z);
        let pitch = (to.y / to.with_y(0.0).length().max(0.01)).atan();
        g.place_player(feet, yaw, pitch);
    }

    /// Before each frame: the next step when it's time. Returns a screenshot name to take now.
    pub fn frame(&mut self, g: &mut Game) -> Option<String> {
        if self.done {
            return None;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return None;
        }
        self.wait = 45;
        let q = |g: &Game| g.state.quests.waypoint().map(|w| format!("{:?}", w.0)).unwrap_or_else(|| "-".into());
        let step = self.step;
        self.step += 1;
        match step {
            0 => {
                self.check(g.state.player.money == 0, "you start with no money");
                self.check(g.state.time.minute == hm(6, 45) || g.state.time.minute <= hm(6, 50), "it's a quarter to seven");
                self.check(g.prompt().is_some_and(|p| p.contains("uniform")), &format!("in your room, facing your closet: \"{}\"", g.prompt().unwrap_or_default()));
                return Some("01_room".into());
            }
            1 => {
                g.key(Key::Confirm, false);
                Self::dismiss(g);
                self.check(g.state.ready.dressed, "changed into your uniform");
                self.check(q(g) == "ShopCounter", &format!("the next step is the counter ({})", q(g)));
                g.mode = g.open_map();
                return Some("02_map_to_counter".into());
            }
            2 => {
                g.mode = Mode::Explore;
                g.state.time.minute = hm(7, 32);
                let Some((c, front)) = g.story_rt.counter else {
                    self.check(false, "the counter is in the town");
                    return None;
                };
                let feet = Vec3::new(c.x + front.x * 0.9, g.player.feet.y.min(c.y + 0.2), c.z + front.z * 0.9);
                Self::face(g, feet, c + Vec3::Y * 1.0);
                return Some("03_counter".into());
            }
            3 => {
                let p = g.prompt().unwrap_or_default();
                self.check(p.contains("goodbye"), &format!("at the counter: \"{p}\""));
                g.key(Key::Confirm, false);
                return Some("04_goodbye".into());
            }
            4 => {
                Self::dismiss(g);
                self.check(g.state.ready.said_bye, "said goodbye to Uncle Kenji");
                self.check(q(g) == "SchoolGate", &format!("the next step is the walk to school ({})", q(g)));
                // outside, looking back at the shop: the sign
                return None;
            }
            5 => {
                // the walk: half way along the route to the gate, with the compass
                g.state.time.minute = hm(8, 2);
                let me = [g.player.feet.x, -g.player.feet.z];
                let route = g.story_rt.map.as_ref().and_then(|m| m.route(me, g.story_rt.gate));
                self.check(route.is_some(), "there's a way to school by road");
                if let Some(r) = route {
                    let mid = r[r.len() / 2];
                    let next = r[(r.len() / 2 + 4).min(r.len() - 1)];
                    let y = g.world.as_ref().and_then(|w| w.world.ground(mid[0], -mid[1])).unwrap_or(g.player.feet.y);
                    let to = Vec3::new(next[0] - mid[0], 0.0, -(next[1] - mid[1]));
                    g.place_player(Vec3::new(mid[0], y, -mid[1]), to.x.atan2(-to.z), 0.0);
                }
                return Some("05_walk".into());
            }
            6 => {
                // at the school's gate
                g.state.time.minute = hm(8, 12);
                let gate = g.story_rt.gate;
                let y = g.world.as_ref().and_then(|w| w.world.ground(gate[0], -gate[1])).unwrap_or(g.player.feet.y);
                let school = g.your_desk.map(|d| d.translation).unwrap_or_default();
                let to = Vec3::new(school.x - gate[0], 0.0, school.z + gate[1]);
                g.place_player(Vec3::new(gate[0], y, -gate[1]), to.x.atan2(-to.z), 0.05);
                return Some("06_gate".into());
            }
            7 => {
                self.check(q(g) == "YourLocker", &format!("at the gate, the next step is your locker ({})", q(g)));
                g.state.time.minute = hm(8, 18);
                let Some((l, _)) = g.story_rt.locker else {
                    self.check(false, "your locker is in 1-A");
                    return None;
                };
                // (the locker spot faces the cabinet)
                let spot = g.spots.iter().find(|(r, s, _)| r == "1-A" && s == "locker").map(|x| x.2);
                // (the spot faces the class: the lockers are behind it)
                let look = spot.map_or(l, |f| f.transform_point3(Vec3::new(0.0, 0.55, -1.0)));
                Self::face(g, l, look);
                let p = g.prompt().unwrap_or_default();
                self.check(p.contains("locker"), &format!("by your locker: \"{p}\""));
                return Some("07_classroom".into());
            }
            8 => {
                g.key(Key::Confirm, false);
                self.check(g.state.ready.bag_stored, "your bag's in your locker");
                self.check(matches!(g.mode, Mode::Locker), "your timetable inside the door");
                return Some("08_timetable".into());
            }
            9 => {
                g.key(Key::Confirm, false);
                self.check(q(g) == "YourSeat", &format!("the next step is your seat ({})", q(g)));
                g.state.time.minute = hm(8, 22);
                if let Some((feet, _, yaw)) = g.seat() {
                    let d = g.your_desk.unwrap();
                    let side = d.transform_point3(Vec3::new(0.0, 0.0, 0.9));
                    g.place_player(Vec3::new(side.x, feet.y, side.z), yaw, 0.0);
                }
                let p = g.prompt().unwrap_or_default();
                self.check(p.contains("Sit"), &format!("by your desk: \"{p}\""));
                g.key(Key::Confirm, false);
                return None;
            }
            10 => {
                self.check(g.seated(), "sitting at your desk");
                let first = g.state.quests.get(sim::quest::QuestId::FirstDay).map(|q| q.done()).unwrap_or(false);
                self.check(first, "the first day's quest is done");
                // who's in by now (each at their own time)
                let here = sim::presence::in_homeroom(sim::school::MY_CLASS, g.state.time.date, g.state.time.minute);
                let seated = here.iter().filter(|p| matches!(p.spot, sim::presence::Spot::Seat(_))).count();
                println!("autoplay: at {}:{:02} {seated} of 16 classmates are in", g.state.time.minute / 60, g.state.time.minute % 60);
                return Some("09_seated".into());
            }
            11 => {
                g.state.time.minute = hm(8, 31);
                let here = sim::presence::in_homeroom(sim::school::MY_CLASS, g.state.time.date, g.state.time.minute);
                let seated = here.iter().filter(|p| matches!(p.spot, sim::presence::Spot::Seat(_))).count();
                let teacher = here.iter().any(|p| p.spot == sim::presence::Spot::Front);
                println!("autoplay: at 8:31 {seated} of 16 classmates are in, the teacher {}", if teacher { "at the front" } else { "not here" });
                self.check(seated >= 15 && teacher, "homeroom: the class and the teacher");
                return Some("10_homeroom".into());
            }
            _ => {
                println!("autoplay: {} failed", self.failed.len());
                for f in &self.failed {
                    println!("autoplay:   {f}");
                }
                self.done = true;
            }
        }
        None
    }
}
