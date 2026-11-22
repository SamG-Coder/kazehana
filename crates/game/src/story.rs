//! The story in the town (the vertical slice): the places a quest leads to, what finishes its
//! steps as you play, the map (M) with its waypoint and route, the compass, your locker with
//! the timetable inside its door, and the quests' page in the journal (J).
//!
//! The first day: you wake in your room in Tsukumodō at 6:45 with no money of your own. Get
//! changed at your closet, say goodbye to Uncle Kenji at the counter (the shop's already open:
//! the sign in the window says 営業中), walk to school (the map and the compass show the way),
//! put your bag in your locker at the back of 1-A and take your seat before homeroom.

use crate::game::{Game, Key, Menu, Mode};
use crate::map::{self, TownMap};
use akari::glam::{Affine3A, Vec3};
use akari::{Rgba, Ui};
use sim::quest::{Goal, Place};

#[derive(Default)]
pub struct Story {
    pub map: Option<TownMap>,
    pub centre: [f32; 2],
    pub zoom: f32,
    pub sel: usize,
    /// a waypoint you put on the map (over the quest's): where, and what it is
    pub pin: Option<([f32; 2], String)>,
    /// the way to the waypoint: the route, where it goes and where you were when it was found
    route: Option<(Vec<[f32; 2]>, [f32; 2], [f32; 2])>,
    /// your closet (its sliding doors' middle), Tsukumodō's counter (and which way its front
    /// faces), your locker in 1-A (and its door), the school's gate: in the world
    pub closet: Option<Vec3>,
    pub counter: Option<(Vec3, Vec3)>,
    pub locker: Option<(Vec3, Option<usize>)>,
    pub gate: [f32; 2],
    /// where to wake: your room (feet, yaw)
    pub bed: Option<(Vec3, f32)>,
    uploaded: bool,
}

fn plan(v: Vec3) -> [f32; 2] {
    [v.x, -v.z]
}

fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

impl Game {
    /// The town is loaded: build the map and find the story's places in it.
    pub fn setup_story(&mut self, town: &town::Town, home: Option<Affine3A>) {
        let st = &mut self.story_rt;
        st.map = Some(TownMap::build(town));
        st.gate = town.plan.school_gate;
        st.zoom = 1.0;
        let Some(p) = self.world.as_ref() else { return };
        let door_at = |key: &str| p.doors.doors.iter().enumerate().find(|(_, d)| p.label(d.body).contains(key)).map(|(i, d)| (i, d.centre()));
        st.closet = door_at("closet_mine").map(|x| x.1);
        // the counter: its body, and the way its front faces (towards the shop's door)
        for i in 0..p.world.body_count() {
            let b = play::collision::BodyId(i as u32);
            if p.label(b) == "shop_counter" {
                let at = p.world.transform(b);
                let front = home.map_or(Vec3::Z, |h| h.transform_vector3(-Vec3::Z).normalize());
                st.counter = Some((at.translation.into(), front));
            }
        }
        if let Some((_, _, f)) = self.spots.iter().find(|(r, s, _)| r == "1-A" && s == "locker") {
            st.locker = Some((f.translation.into(), door_at("school_lockers door kzdoor_hinge_n100_locker17").map(|x| x.0)));
        }
        // your room: between your closet and your room's door, facing the closet
        // (1.8 m out from the closet towards your room's door, facing the closet)
        if let (Some(c), Some((_, d))) = (st.closet, door_at("my_room")) {
            let dir = (d - c).with_y(0.0).normalize_or_zero();
            let at = c + dir * 1.8;
            let to = c - at;
            st.bed = Some((Vec3::new(at.x, c.y, at.z), to.x.atan2(-to.z)));
        }
    }

    /// Put the player in their room (the story's start).
    pub fn wake_in_room(&mut self) {
        if let Some((c, yaw)) = self.story_rt.bed {
            // (the closet door's middle is a metre up: the floor under it)
            let floor = self.world.as_ref().and_then(|p| p.world.raycast(c + Vec3::Y * 0.5, -Vec3::Y, 3.0)).map_or(c.y - 1.0, |h| h.point.y);
            self.place_player(Vec3::new(c.x, floor, c.z), yaw, 0.0);
        }
    }

    fn me(&self) -> [f32; 2] {
        plan(self.player.feet)
    }

    /// Where a quest's place is (plan).
    pub fn place_at(&self, p: Place) -> Option<[f32; 2]> {
        let st = &self.story_rt;
        match p {
            Place::YourCloset => st.closet.map(plan),
            Place::ShopCounter => st.counter.map(|c| plan(c.0)),
            Place::SchoolGate => Some(st.gate),
            Place::YourLocker => st.locker.map(|l| plan(l.0)),
            Place::YourSeat => self.your_desk.map(|d| plan(d.translation.into())),
        }
    }

    /// The waypoint: yours (on the map) or the tracked quest's. (where, colour, its name)
    pub fn waypoint(&self) -> Option<([f32; 2], Rgba, String)> {
        if let Some((at, name)) = &self.story_rt.pin {
            return Some((*at, map::C_PIN, name.clone()));
        }
        let (place, _, step) = self.state.quests.waypoint()?;
        Some((self.place_at(place)?, map::C_QUEST, step.text.clone()))
    }

    fn facing(&self, at: Vec3, reach: f32, cone: f32) -> bool {
        let to = at - self.cam.position;
        to.length() < reach && to.normalize_or_zero().dot(self.cam.forward()) > cone
    }

    fn near_counter(&self) -> bool {
        self.story_rt.counter.is_some_and(|(c, _)| (c - self.player.feet).with_y(0.0).length() < 2.6 && (c.y - self.player.feet.y).abs() < 1.5)
    }

    fn shop_open(&self) -> bool {
        (sim::clock::at::SHOP_OPEN..sim::clock::at::SHOP_CLOSE).contains(&self.state.time.minute)
    }

    /// What E does here in the story, if anything (before the game's own prompts).
    pub fn story_prompt(&self) -> Option<String> {
        if !self.in_town {
            return None;
        }
        let r = self.state.ready;
        let today = r.date == Some(self.state.time.date);
        if let Some(c) = self.story_rt.closet {
            if self.facing(c, 2.2, 0.55) && !(today && r.dressed) && self.state.time.school_day() {
                return Some("Get changed into your uniform".into());
            }
        }
        if let Some((l, _)) = self.story_rt.locker {
            if (l - self.player.feet).with_y(0.0).length() < 1.4 && (l.y - self.player.feet.y).abs() < 1.5 {
                return Some(if today && r.bag_stored { "Your locker".into() } else { "Open your locker".into() });
            }
        }
        if self.near_counter() && self.shop_open() {
            if today && r.dressed && !r.said_bye && self.state.time.school_day() && self.state.time.minute < sim::clock::at::HOMEROOM {
                return Some("Say goodbye to Uncle Kenji — \"Ittekimasu!\"".into());
            }
            return Some("Shop counter — Uncle Kenji".into());
        }
        None
    }

    /// E in the story: true if it did something.
    pub fn story_interact(&mut self) -> bool {
        let Some(p) = self.story_prompt() else { return false };
        if p.starts_with("Get changed") {
            let line = self.state.get_ready();
            self.mode = Mode::Dialogue { speaker: String::new(), lines: vec![line], line: 0 };
        } else if p.starts_with("Say goodbye") {
            let lines = self.state.say_goodbye();
            self.mode = Mode::Dialogue { speaker: "Uncle Kenji".into(), lines, line: 0 };
        } else if p.contains("locker") {
            if let Some((_, Some(d))) = self.story_rt.locker {
                if let Some(w) = self.world.as_mut() {
                    let door = &mut w.doors.doors[d];
                    if door.target < 0.5 {
                        door.toggle();
                    }
                }
            }
            let line = self.state.store_bag();
            self.state.say(line);
            self.mode = Mode::Locker;
        } else if p.starts_with("Shop counter") {
            self.counter_menu();
        } else {
            return false;
        }
        true
    }

    /// Tsukumodō's counter: packs, a shift, a word with Uncle Kenji.
    pub fn counter_menu(&mut self) {
        let cost = self.state.pack_cost();
        let money = self.state.player.money;
        let shift_ok = self.state.phase() == sim::Phase::Daytime || (self.shop_open() && !self.state.time.school_day());
        let season = tcg::season::theme(self.state.season).name;
        self.mode = Mode::Menu(Menu {
            title: "Tsukumodō counter".into(),
            subtitle: format!("{season} packs — ¥{cost} each with Uncle Kenji's discount (shelf price ¥{}). You have ¥{money}.", tcg::PACK_PRICE),
            items: vec![
                (format!("Buy a pack  (¥{cost})"), crate::game::Act::BuyPacks(1), money >= cost),
                ("Work a shift  (4 h, ¥600)".into(), crate::game::Act::Shift, shift_ok),
                ("Talk to Uncle Kenji".into(), crate::game::Act::UncleTalk, true),
                ("Leave".into(), crate::game::Act::Close, true),
            ],
            sel: 0,
        });
    }

    /// Each frame in the town: arrivals that finish steps, the route to the waypoint.
    pub fn story_update(&mut self) {
        if !self.in_town {
            return;
        }
        let me = self.me();
        if self.state.quests.waiting_for(Goal::Reached(Place::SchoolGate)) && dist2(me, self.story_rt.gate) < 14.0 {
            self.state.quest_event(Goal::Reached(Place::SchoolGate));
        }
        if self.seated() && self.state.quests.waiting_for(Goal::Seated) {
            self.state.quest_event(Goal::Seated);
        }
        // the route: found again when the waypoint changes or you've wandered off it
        let target = self.waypoint().map(|w| w.0);
        let st = &mut self.story_rt;
        match (target, st.map.as_ref()) {
            (Some(t), Some(m)) => {
                let stale = match &st.route {
                    Some((_, to, from)) => dist2(*to, t) > 1.0 || dist2(*from, me) > 25.0,
                    None => true,
                };
                if stale {
                    st.route = m.route(me, t).map(|r| (r, t, me));
                    if st.route.is_none() {
                        st.route = Some((vec![me, t], t, me));
                    }
                }
            }
            _ => st.route = None,
        }
    }

    // ------------------------------------------------------------------ the map

    pub fn open_map(&mut self) -> Mode {
        let me = self.me();
        let st = &mut self.story_rt;
        st.centre = me;
        st.zoom = st.zoom.clamp(0.3, 6.0);
        if st.zoom == 0.0 {
            st.zoom = 1.0;
        }
        Mode::Map
    }

    pub fn map_key(&mut self, k: Key) -> Mode {
        let me = self.me();
        let st = &mut self.story_rt;
        let n = st.map.as_ref().map_or(0, |m| m.spots.len());
        match k {
            Key::Up | Key::Down if n > 0 => {
                st.sel = if k == Key::Up { (st.sel + n - 1) % n } else { (st.sel + 1) % n };
                st.centre = st.map.as_ref().unwrap().spots[st.sel].at;
            }
            Key::Left => st.zoom = (st.zoom / 1.5).max(0.12),
            Key::Right => st.zoom = (st.zoom * 1.5).min(8.0),
            Key::Confirm if n > 0 => {
                let sp = st.map.as_ref().unwrap().spots[st.sel].clone();
                st.pin = match &st.pin {
                    Some((at, _)) if dist2(*at, sp.at) < 1.0 => None,
                    _ => Some((sp.at, sp.name)),
                };
                st.route = None;
            }
            Key::Calendar => {
                st.pin = None;
                st.route = None;
            }
            Key::Map | Key::Back => return Mode::Explore,
            Key::People => st.centre = me,
            _ => {}
        }
        Mode::Map
    }

    pub fn draw_map(&mut self, ui: &mut Ui) {
        let s = ui.scale();
        let me = self.me();
        let wp = self.waypoint();
        let quest = self.state.quests.waypoint().and_then(|(p, _, _)| self.place_at(p));
        let yaw = self.player.yaw;
        let st = &mut self.story_rt;
        let Some(m) = st.map.as_mut() else { return };
        if !st.uploaded {
            m.upload(ui);
            st.uploaded = true;
        }
        let (w, h) = (ui.width, ui.height);
        let pin = st.pin.as_ref().map(|(at, _)| (*at, map::C_PIN));
        let route = st.route.as_ref().map(|r| r.0.as_slice());
        m.draw(ui, [0.0, 0.0, w, h], st.centre, st.zoom, (me, yaw), pin, route, quest);
        // the legend: the places, the waypoint, how far
        let (px, py, pw) = (28.0 * s, 28.0 * s, 430.0 * s);
        let rows = m.spots.len();
        let ph = 120.0 * s + rows as f32 * 30.0 * s;
        ui.rect(px, py, pw, ph, Rgba(14, 16, 18, 220));
        ui.rect(px, py, pw, 4.0 * s, crate::draw::ACCENT);
        ui.text(px + 18.0 * s, py + 16.0 * s, 28.0 * s, Rgba(240, 242, 245, 255), "Kazehana-chō");
        ui.text(px + 18.0 * s, py + 52.0 * s, 15.0 * s, Rgba(160, 166, 172, 255), &format!("{} · {:02}:{:02}", self.state.time.date, self.state.time.minute / 60, self.state.time.minute % 60));
        for (i, sp) in m.spots.iter().enumerate() {
            let y = py + 84.0 * s + i as f32 * 30.0 * s;
            if i == st.sel {
                ui.rect(px + 8.0 * s, y - 3.0 * s, pw - 16.0 * s, 28.0 * s, Rgba(255, 255, 255, 28));
                ui.rect(px + 8.0 * s, y - 3.0 * s, 4.0 * s, 28.0 * s, crate::draw::ACCENT);
            }
            map::badge(ui, [px + 32.0 * s, y + 11.0 * s], 10.0 * s, Rgba(24, 28, 32, 255), sp.mark, Rgba(235, 238, 240, 255));
            let d = dist2(me, sp.at);
            ui.text(px + 52.0 * s, y, 18.0 * s, Rgba(228, 230, 234, 255), &sp.name);
            let dt = if d >= 1000.0 { format!("{:.1} km", d / 1000.0) } else { format!("{:.0} m", d) };
            ui.text_right(px + pw - 18.0 * s, y + 1.0 * s, 15.0 * s, Rgba(150, 156, 162, 255), &dt);
            if st.pin.as_ref().is_some_and(|(at, _)| dist2(*at, sp.at) < 1.0) {
                map::diamond(ui, [px + pw - 92.0 * s, y + 11.0 * s], 7.0 * s, map::C_PIN);
            }
        }
        // the waypoint's line at the bottom
        if let Some((at, col, name)) = wp {
            let route_len: f32 = st.route.as_ref().map_or(dist2(me, at), |r| r.0.windows(2).map(|w| dist2(w[0], w[1])).sum());
            let mins = (route_len / 75.0).ceil();
            let t = format!("{name} — {:.0} m by road, about {mins:.0} min on foot", route_len);
            let tw = ui.measure(20.0 * s, &t) + 60.0 * s;
            let (bx, by) = (w * 0.5 - tw * 0.5, h - 120.0 * s);
            ui.rect(bx, by, tw, 44.0 * s, Rgba(14, 16, 18, 220));
            map::diamond(ui, [bx + 24.0 * s, by + 22.0 * s], 8.0 * s, col);
            ui.text(bx + 44.0 * s, by + 10.0 * s, 20.0 * s, Rgba(240, 242, 245, 255), &t);
        }
        crate::draw::hints(ui, &self.buttons_pub(), "{choose} Place · {confirm} Set / clear waypoint · {sideways} Zoom · {people} Back to you · {calendar} Clear waypoint · {back} Close");
    }

    /// The compass at the top of the screen and the tracked step under it (exploring the town).
    pub fn draw_compass(&self, ui: &mut Ui) {
        if !self.in_town {
            return;
        }
        let me = self.me();
        let mut marks = Vec::new();
        if let Some((at, col, _)) = self.waypoint() {
            let d = dist2(me, at);
            let label = if d >= 1000.0 { format!("{:.1} km", d / 1000.0) } else { format!("{:.0} m", d) };
            marks.push((map::bearing(me, at), col, label));
        }
        map::compass(ui, self.player.yaw, &marks);
        // the tracked step, under the compass
        if let Some((q, step)) = self.state.quests.tracked().next() {
            let s = ui.scale();
            let by = step.by.map(|m| format!("  ·  by {}:{:02}", m / 60, m % 60)).unwrap_or_default();
            let t = format!("{}  —  {}{by}", q.title, step.text);
            let tw = ui.measure(17.0 * s, &t);
            let (cx, y) = (ui.width * 0.5, 96.0 * s);
            ui.rect(cx - tw * 0.5 - 16.0 * s, y - 4.0 * s, tw + 32.0 * s, 30.0 * s, Rgba(10, 12, 14, 150));
            map::diamond(ui, [cx - tw * 0.5 - 4.0 * s, y + 11.0 * s], 5.0 * s, map::C_QUEST);
            ui.text(cx - tw * 0.5 + 8.0 * s, y, 17.0 * s, Rgba(240, 242, 245, 255), &t);
        }
    }

    // ------------------------------------------------------------------ your locker

    pub fn locker_key(&mut self, k: Key) -> Mode {
        match k {
            Key::Back | Key::Confirm => {
                // (close the door again)
                if let Some((_, Some(d))) = self.story_rt.locker {
                    if let Some(w) = self.world.as_mut() {
                        let door = &mut w.doors.doors[d];
                        if door.target > 0.5 {
                            door.toggle();
                        }
                    }
                }
                Mode::Explore
            }
            _ => Mode::Locker,
        }
    }

    /// Your timetable, taped inside your locker's door: the week's six periods.
    pub fn draw_locker(&self, ui: &mut Ui) {
        let s = ui.scale();
        crate::draw::dim(ui, 0.55);
        let (w, h) = (1020.0 * s, 640.0 * s);
        let (x, y) = (ui.width * 0.5 - w * 0.5, ui.height * 0.5 - h * 0.5 - 20.0 * s);
        // the sheet: off-white paper, a strip of tape at the top
        ui.rect(x + 6.0 * s, y + 8.0 * s, w, h, Rgba(0, 0, 0, 90));
        ui.rect(x, y, w, h, Rgba(246, 243, 234, 255));
        ui.rect(x + w * 0.5 - 70.0 * s, y - 14.0 * s, 140.0 * s, 30.0 * s, Rgba(236, 226, 190, 200));
        let ink = Rgba(40, 38, 36, 255);
        let dim = Rgba(110, 104, 96, 255);
        ui.text(x + 36.0 * s, y + 26.0 * s, 34.0 * s, ink, "1年A組  時間割");
        ui.text(x + 36.0 * s, y + 70.0 * s, 17.0 * s, dim, "Class 1-A timetable · homeroom 8:30 · lessons from 8:50");
        let days = [("月", "Mon"), ("火", "Tue"), ("水", "Wed"), ("木", "Thu"), ("金", "Fri")];
        let (gx, gy) = (x + 36.0 * s, y + 110.0 * s);
        const LC: f32 = 130.0; // the periods' column
        let (cw, ch) = ((w - 72.0 * s - LC * s) / 5.0, 76.0 * s);
        let line = Rgba(170, 162, 150, 255);
        for (d, (jp, en)) in days.iter().enumerate() {
            let cx = gx + LC * s + d as f32 * cw;
            ui.text(cx + 12.0 * s, gy + 4.0 * s, 22.0 * s, ink, &format!("{jp}  {en}"));
        }
        for p in 1..=6u8 {
            let ry = gy + 36.0 * s + (p as f32 - 1.0) * ch;
            ui.rect(gx, ry, w - 72.0 * s, 1.5 * s, line);
            ui.text(gx + 8.0 * s, ry + 10.0 * s, 26.0 * s, ink, &format!("{p}"));
            if let Some((a, b)) = sim::school::PERIODS.get(p as usize - 1) {
                ui.text(gx + 34.0 * s, ry + 16.0 * s, 14.0 * s, dim, &format!("{}:{:02}–{}:{:02}", a / 60, a % 60, b / 60, b % 60));
            }
            for d in 0..5u8 {
                let cx = gx + LC * s + d as f32 * cw;
                match sim::school::my_lesson(d, p) {
                    Some(l) => {
                        use sim::school::Subject as S;
                        let short = match l.subject {
                            S::TechHome => "tech & home ec.",
                            S::Integrated => "integrated",
                            S::ClassActivities => "class time",
                            S::Moral => "moral ed.",
                            x => x.name(),
                        };
                        ui.text(cx + 12.0 * s, ry + 8.0 * s, 22.0 * s, ink, l.subject.kanji());
                        ui.text(cx + 12.0 * s, ry + 40.0 * s, 13.0 * s, dim, short);
                    }
                    None => {
                        ui.text(cx + 12.0 * s, ry + 20.0 * s, 16.0 * s, dim, "—");
                    }
                }
            }
        }
        crate::draw::hints(ui, &self.buttons_pub(), "{confirm} Close the locker");
    }

    // ------------------------------------------------------------------ the journal's quests

    pub fn quests_key(&mut self, k: Key) -> bool {
        let n = self.state.quests.list.len();
        if n == 0 {
            return false;
        }
        let q = &mut self.state.quests;
        match k {
            Key::Up => q.sel = (q.sel + n - 1) % n,
            Key::Down => q.sel = (q.sel + 1) % n,
            Key::Confirm => {
                let i = q.sel.min(n - 1);
                let on = !q.list[i].active;
                q.set_active(i, on);
                self.story_rt.route = None;
            }
            _ => return false,
        }
        true
    }
}
