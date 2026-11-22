//! The playable loop: walking the three levels while the calendar runs, and the screens for
//! talking, the shop, opening packs, the binder, selling to classmates, people and the calendar.
//! Time stops whenever a screen is open.

use crate::draw::{self, DIM, GOLD, INK};
use crate::npc::Placed;
use crate::world::{Interact, Level};
use akari::glam::Vec3;
use akari::{Camera, Rgba, Ui};
use sim::calendar::{events_on, Date, Event};
use sim::economy::{willingness, SaleResponse};
use sim::people::{Fact, Gender, Personality};
use sim::{GameState, Location, Phase};
use tcg::{CardDb, Pull};

/// Lessons as mini-games at the bell, and the year's end (sim::lessons).
#[path = "lesson.rs"]
pub mod lesson;

pub const MINUTES_PER_SECOND: f32 = 1.0;
pub const EYE: f32 = 1.42;
pub const SAVE_PATH: &str = "saves/slot1.json";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Binder,
    People,
    Calendar,
    /// the town map (M)
    Map,
    /// the journal's quests (J)
    Quests,
}

#[derive(Clone, Copy, Debug)]
pub enum Act {
    Chat(usize),
    Sell(usize),
    BuyPacks(u32),
    Shift,
    UncleTalk,
    Sleep,
    Save,
    Close,
}

pub struct Menu {
    pub title: String,
    pub subtitle: String,
    pub items: Vec<(String, Act, bool)>,
    pub sel: usize,
}

pub enum Mode {
    Explore,
    Dialogue { speaker: String, lines: Vec<String>, line: usize },
    Menu(Menu),
    Pack { pulls: Vec<Pull>, revealed: usize },
    Binder { sel: usize, sell_to: Option<usize> },
    Sell { student: usize, pull: Pull, price: u32, value: u32, result: Option<String>, counter: Option<u32>, done: bool },
    /// The journal open in your hands (time goes on; you just aren't walking).
    Journal { spread: crate::paper::Spread },
    /// A lesson (or a test) at the bell, played on screen; the period's clock runs with it.
    Lesson(Box<lesson::LessonView>),
    /// The closing ceremony: the year's over (page 0 the summary, then the report cards).
    YearEnd { page: usize },
    /// The town map, full screen (story.rs).
    Map,
    /// Your locker open: the timetable inside its door (story.rs).
    Locker,
}

pub struct Game {
    pub db: CardDb,
    pub state: GameState,
    pub levels: [Level; 3],
    /// the camera: the player's eyes while exploring (from `player`)
    pub cam: Camera,
    /// the player's body (play crate): walking, collision
    pub player: play::FirstPerson,
    /// the world as somewhere to walk: its solid shapes and doors (the town, set by the app once
    /// it is loaded; until then the floor is flat)
    pub world: Option<town::collide::Playable>,
    /// walking in the real town (the blockout levels' exits, zones and people are not used)
    pub in_town: bool,
    /// where the player starts (and is put back if they ever fall out of the world)
    pub start: (Vec3, f32),
    /// the story's messages (Uncle Kenji and the rest) are shown: off until there is a story
    pub story: bool,
    /// what the shop has out to buy: (model id, its middle in the world)
    pub stock: Vec<(String, Vec3)>,
    /// what is bound to each action (hints draw their buttons), and whose buttons are in hand
    pub prompts: play::Prompts,
    pub kit: play::Kit,
    pub mode: Mode,
    pub placed: Vec<Placed>,
    toasts: Vec<(String, f32)>,
    msg_seen: usize,
    minute_acc: f32,
    /// told this lesson to stay in your seat (not again till you've sat back down)
    told_to_sit: bool,
    /// where you stood before this frame's step (put back there when an exit won't let you out)
    before_exit: (Vec3, f32),
    /// Set when the level changed (the renderer resets its history).
    pub cut: bool,
    /// the physical binder (open in Mode::Binder)
    pub binder: crate::binder::BinderView,
    /// the wall calendar in Tsukumodō: where it hangs (set by the app), and how many months on
    /// it's been flipped
    pub calendar_at: Option<Vec3>,
    /// your desk in 1-A (its prop frame in the town; set by the app): your seat and your paper
    pub your_desk: Option<akari::glam::Affine3A>,
    /// places for people in the town's buildings: (room, spot, frame) (set by the app; crowd.rs)
    pub spots: Vec<(String, String, akari::glam::Affine3A)>,
    pub calendar_offset: i32,
    /// the story in the town: its places, the map, the waypoint (story.rs)
    pub story_rt: crate::story::Story,
}

/// What every action is bound to, for the hints: the play crate's (moving, looking, using)
/// and the screens' (main.rs binds the same keys and buttons).
pub fn game_prompts() -> play::Prompts {
    use akari::winit::keyboard::KeyCode::*;
    let mut b = play::input::Bindings::default();
    b.crouch = vec![ControlLeft, ControlRight];
    let mut p = play::Prompts::new(&b);
    for (act, keys, pad) in [
        ("confirm", &[KeyE][..], &["south"][..]),
        ("back", &[Escape][..], &["east"][..]),
        ("choose", &[ArrowUp, ArrowDown][..], &["dpad_up_down"][..]),
        ("browse", &[ArrowLeft, ArrowRight, ArrowUp, ArrowDown][..], &["dpad_up_down_left_right"][..]),
        ("sideways", &[ArrowLeft, ArrowRight][..], &["dpad_left_right"][..]),
        ("more", &[ShiftLeft][..], &["l1"][..]),
        ("binder", &[Tab][..], &["north"][..]),
        ("turn", &[KeyZ, KeyX][..], &["l1", "r1"][..]),
        ("quick", &[KeyQ][..], &["west"][..]),
        ("people", &[KeyP][..], &["select"][..]),
        ("calendar", &[KeyC][..], &["start"][..]),
        ("map", &[KeyM][..], &["touchpad"][..]),
        ("quests", &[KeyJ][..], &["r3"][..]),
        ("fast", &[KeyT][..], &["r2"][..]),
    ] {
        p.bind_keys(act, keys);
        p.bind_pad(act, pad);
    }
    p
}

/// A blockout level as somewhere to walk: its geometry and collider boxes, on a flat floor.
#[cfg(test)]
pub fn blockout_playable(lvl: &Level) -> town::collide::Playable {
    let mut pl = town::collide::Playable::new();
    pl.world.set_ground(|_, _| 0.0);
    for c in &lvl.colliders {
        pl.add_box(c.min, c.max);
    }
    for mesh in [&lvl.mesh, &lvl.glow, &lvl.neon].into_iter().flat_map(|m| m.groups.iter().map(|g| &g.1)) {
        let v = |i: u32| Vec3::from(mesh.positions[i as usize]);
        pl.add_triangles(mesh.indices.chunks_exact(3).map(|t| [v(t[0]), v(t[1]), v(t[2])]));
    }
    pl
}

pub fn level_index(l: Location) -> usize {
    match l {
        Location::Home => 0,
        Location::Path => 1,
        Location::School => 2,
    }
}

/// How the player refers to someone they may not know yet.
pub fn describe(st: &GameState, i: usize) -> String {
    let s = &st.students[i];
    if st.bonds[i].knows(Fact::Name) {
        s.given_name.clone()
    } else {
        let year = ["", "first-year", "second-year", "third-year"][s.grade as usize];
        format!("a {year} {}", if s.gender == Gender::Boy { "boy" } else { "girl" })
    }
}

fn smalltalk(p: Personality, plays: bool) -> &'static str {
    match (p, plays) {
        (Personality::Competitive, true) => "\"Did you see the new Mythics? I'm building a deck to win the shop tournament.\"",
        (Personality::Collector, true) => "\"I keep my holos in sleeves. Two sleeves, actually.\"",
        (Personality::Casual, true) => "\"I mostly play for fun. The dragon art this season is so cool.\"",
        (Personality::Haggler, true) => "\"Your uncle runs the card shop, right? Bet you get them cheap...\"",
        (Personality::Shy, _) => "\"...Oh. Um. Hi.\"",
        (Personality::Flashy, true) => "\"Check this out — full holo. Pulled it first pack.\"",
        (_, false) => "\"Cards? Nah. Did you finish the maths homework?\"",
    }
}

impl Game {
    pub fn new(db: CardDb, state: GameState, levels: [Level; 3]) -> Self {
        let mut cam = Camera { fov_y: 65f32.to_radians(), ..Default::default() };
        let (p, yaw) = levels[level_index(state.location)].spawn_default();
        let player = play::FirstPerson::new(p, yaw, play::Settings { eye: EYE, ..Default::default() });
        cam.position = player.eye();
        cam.yaw = yaw;
        Self { db, state, levels, cam, player, world: None, in_town: false, start: (p, yaw), story: false, stock: Vec::new(), prompts: game_prompts(), kit: play::Kit::Keys, mode: Mode::Explore, placed: Vec::new(), toasts: Vec::new(), msg_seen: 0, minute_acc: 0.0, told_to_sit: false, before_exit: (p, yaw), cut: true, binder: crate::binder::BinderView::new(), calendar_at: None, calendar_offset: 0, your_desk: None, spots: Vec::new(), story_rt: Default::default() }
    }

    /// How far the clock is into its current minute (0..1): the sky moves smoothly by it.
    pub fn minute_fraction(&self) -> f32 {
        self.minute_acc.clamp(0.0, 1.0)
    }

    pub fn level(&self) -> &Level {
        &self.levels[level_index(self.state.location)]
    }

    fn buttons(&self) -> draw::Buttons<'_> {
        draw::Buttons { prompts: &self.prompts, kit: self.kit }
    }

    pub fn buttons_pub(&self) -> draw::Buttons<'_> {
        self.buttons()
    }

    /// Put the player somewhere (a cut): feet at `feet`, facing `yaw`.
    pub fn place_player(&mut self, feet: Vec3, yaw: f32, pitch: f32) {
        self.player.teleport(feet, yaw);
        self.player.pitch = pitch;
        self.sync_cam();
    }

    fn sync_cam(&mut self) {
        self.cam.position = self.player.eye();
        self.cam.yaw = self.player.yaw;
        self.cam.pitch = self.player.pitch;
    }

    /// The stock the player is looking at (a booster box, a single on its stand), within reach.
    fn facing_stock(&self) -> Option<usize> {
        let (eye, dir) = (self.cam.position, self.cam.forward());
        self.stock
            .iter()
            .enumerate()
            .filter_map(|(i, (_, p))| {
                let to = *p - eye;
                let along = to.dot(dir);
                // (within reach, and the look passes within 12 cm of it)
                (along > 0.2 && along < 2.0 && (to - dir * along).length() < 0.12).then_some((i, along))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    }

    /// The single card on a stand in the showcase: which card it is (each stand its own, by
    /// where it stands) and its price.
    fn single_at(&self, i: usize) -> Option<(tcg::Pull, u32)> {
        let (id, p) = &self.stock[i];
        let (rarity, foil) = match id.as_str() {
            "single_stand_gold" => (tcg::Rarity::Rare, false),
            "single_stand_holo" => (tcg::Rarity::Rare, true),
            "single_stand_obsidian" => (tcg::Rarity::Mythic, true),
            _ => return None,
        };
        let ids = self.db.set_rarity(self.state.season, rarity);
        if ids.is_empty() {
            return None;
        }
        let h = ((p.x * 73.0) as i64 ^ (p.z * 131.0) as i64 ^ (p.y * 17.0) as i64).unsigned_abs() as usize;
        let card = ids[h % ids.len()];
        let price = self.state.market.value(self.db.get(card), foil, self.state.season);
        Some((tcg::Pull { id: card, foil }, price))
    }

    /// The door the player is looking at in this level, within reach.
    fn facing_door(&self) -> Option<usize> {
        let p = self.world.as_ref()?;
        p.doors.looked_at(&p.world, self.cam.position, self.cam.forward(), 2.2)
    }

    fn say(&mut self, m: impl Into<String>) {
        self.state.say(m);
    }

    // ------------------------------------------------------------------ explore

    /// Per-frame update: `act` = the player's input (play crate), `fast_time` = held to wait
    /// (stand still and let time pass, faster; nothing ever skips but sleep).
    pub fn update(&mut self, dt: f32, act: &play::Actions, fast_time: bool) {
        for t in self.toasts.iter_mut() {
            t.1 += dt;
        }
        self.toasts.retain(|t| t.1 < 9.0);
        let n = self.state.messages.len();
        if self.msg_seen > n {
            self.msg_seen = 0;
        }
        for m in self.state.messages[self.msg_seen..].to_vec() {
            if self.story {
                println!("[{}] {m}", self.state.time);
                self.toasts.push((m, 0.0));
            }
        }
        self.msg_seen = n;

        if self.year_check() {
            return; // (the closing ceremony: the game's over)
        }
        self.town_location();
        self.story_update();
        if matches!(self.mode, Mode::Lesson(_)) {
            self.lesson_update(dt);
            return;
        }
        let reading = matches!(self.mode, Mode::Journal { .. });
        if !matches!(self.mode, Mode::Explore) && !reading {
            return; // time stops in menus (the binder, a sale, a pack)
        }
        // waiting: hold to stop where you are and let time go by; walking off ends whatever you're
        // doing (lessons, the counter, a snack...)
        let moving = act.movement.length() > 0.25 || act.jump;
        let waiting = matches!(self.state.doing(), Some(sim::life::Activity::Wait));
        if fast_time && self.state.doing().is_none() && !moving {
            let _ = self.state.start(sim::life::Activity::Wait);
        } else if (waiting && (!fast_time || moving)) || (!waiting && moving && self.state.doing().is_some()) {
            // (in a lesson you stay in your seat: raise your hand to ask for the toilet)
            match self.state.can_get_up() {
                Err(why) if !waiting => {
                    if !self.told_to_sit {
                        self.say(why);
                        self.told_to_sit = true;
                    }
                }
                _ => {
                    self.told_to_sit = false;
                    if let Some(line) = self.state.stop() {
                        if !waiting {
                            self.say(line);
                        }
                    }
                }
            }
        }
        self.minute_acc += dt * MINUTES_PER_SECOND * self.state.pace();
        let whole = self.minute_acc.floor();
        if whole >= 1.0 {
            self.minute_acc -= whole;
            self.state.advance(whole as u32, &self.db);
        }
        // seated at the bell: the lesson starts
        self.lesson_check();
        let li = level_index(self.state.location);
        if dt > 0.0 && self.state.doing().is_none() && !reading {
            let _ = li;
            self.before_exit = (self.player.feet, self.player.yaw);
            match self.world.as_mut() {
                Some(p) => {
                    self.player.update(act, &p.world, dt);
                    p.doors.update(&mut p.world, &self.player, dt);
                }
                None => {
                    // (no level collision yet: a flat floor)
                    let mut flat = play::World::new();
                    flat.set_ground(|_, _| 0.0);
                    self.player.update(act, &flat, dt);
                }
            }
            if self.player.feet.y < -50.0 {
                // (fell out of the world: back to the start)
                let (p, yaw) = self.start;
                self.player.teleport(p, yaw);
            }
        }
        self.sync_cam();
        let pos = self.player.feet;
        if self.in_town {
            return; // (the town is one place: no exits)
        }
        if let Some(exit) = self.level().exits.iter().find(|e| e.area.contains_xz(pos)).copied() {
            // once you're in school, you're in till it's out (or you sneak off from the toilet)
            if self.state.location == Location::School && exit.to != Location::School {
                match self.state.leave_school(exit.to) {
                    Err(why) => {
                        self.say(why);
                        let (back, yaw) = self.before_exit;
                        self.player.teleport(back, yaw);
                        self.sync_cam();
                        return;
                    }
                    Ok(line) if !line.is_empty() => self.say(line),
                    Ok(_) => {}
                }
            }
            self.state.location = exit.to;
            self.player.teleport(exit.spawn, exit.yaw);
            self.sync_cam();
            self.cut = true;
        }
    }

    /// The student in front of the player within reach.
    fn facing_student(&self) -> Option<usize> {
        let eye = self.cam.position;
        let fwd = self.cam.forward_flat();
        self.placed
            .iter()
            .filter_map(|p| {
                let to = p.pos + Vec3::Y * 1.2 - eye;
                let flat = Vec3::new(to.x, 0.0, to.z);
                let d = flat.length();
                (d < 2.2 && (to.y).abs() < 2.0 && flat.normalize_or_zero().dot(fwd) > 0.75).then_some((p.student, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    }

    fn zone(&self) -> Option<Interact> {
        let p = self.cam.position;
        if self.in_town {
            return None; // (the blockout's zones are not in the town)
        }
        self.level().zones.iter().find(|(a, _)| a.contains_xz(p)).map(|z| z.1)
    }

    fn near_uncle(&self) -> bool {
        self.state.location == Location::Home && {
            let (u, _) = crate::npc::uncle_spot(&self.state);
            (u - self.cam.position).with_y(0.0).length() < 2.6
        }
    }

    /// What E does right now, as a prompt.
    pub fn prompt(&self) -> Option<String> {
        if self.state.can_get_up().is_err() {
            return Some("In a lesson: raise your hand to ask for the toilet".into());
        }
        if let Some(p) = self.desk_prompt() {
            return Some(p);
        }
        if self.state.doing().is_none() && self.facing_student().is_none() {
            if let Some(p) = self.story_prompt() {
                return Some(p);
            }
        }
        if let Some(a) = self.state.doing() {
            return Some(match a {
                sim::life::Activity::Wait => "Waiting... (let go to stop)".into(),
                a => format!("{} (move to stop)", capitalise(&a.doing())),
            });
        }
        if let Some(i) = self.facing_student() {
            return Some(format!("Talk to {}", describe(&self.state, i)));
        }
        if self.facing_calendar() {
            return Some(if self.calendar_offset == 3 { "Calendar: back to this month".into() } else { "Calendar: turn the page".into() });
        }
        if let Some(i) = self.facing_stock() {
            return Some(match self.single_at(i) {
                Some((pull, price)) => format!("Buy {}{} (¥{price})", self.db.get(pull.id).name, if pull.foil { ", holo" } else { "" }),
                None => format!("Buy a booster pack (¥{})", self.state.pack_cost()),
            });
        }
        if let Some(d) = self.facing_door() {
            let p = self.world.as_ref().unwrap();
            return Some(if p.doors.doors[d].target > 0.5 { "Close the door".into() } else { "Open the door".into() });
        }
        match self.zone() {
            Some(Interact::ShopCounter) => Some(if self.near_uncle() { "Shop counter — Uncle Kenji".into() } else { "Shop counter".into() }),
            Some(Interact::Bed) => Some("Your futon".into()),
            Some(Interact::Classroom) => {
                let ph = self.state.phase();
                if self.state.time.school_day() && matches!(ph, Phase::Morning | Phase::MorningClass | Phase::Lunch | Phase::AfternoonClass) && !self.class_done() {
                    Some("Take your seat for class".into())
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn class_done(&self) -> bool {
        let m = self.state.time.minute;
        if m < sim::clock::at::LUNCH { self.state.attended[0] } else { self.state.attended[1] }
    }

    /// Looking at the wall calendar, close enough to read it.
    fn facing_calendar(&self) -> bool {
        self.calendar_at.is_some_and(|c| {
            let to = c - self.cam.position;
            to.length() < 2.2 && to.normalize_or_zero().dot(self.cam.forward()) > 0.8
        })
    }

    fn interact(&mut self) {
        // seated in a lesson: E raises your hand ("May I go to the toilet?")
        if self.state.can_get_up().is_err() {
            let line = match self.state.ask_toilet() {
                Ok(l) | Err(l) => l,
            };
            self.say(line);
            return;
        }
        // by your desk at school: sit down
        if self.sit_at_desk() {
            return;
        }
        // the story's places: your closet, the counter, your locker
        if self.facing_student().is_none() && self.story_interact() {
            return;
        }
        if self.facing_calendar() && self.facing_student().is_none() {
            // flip on a month (three ahead, then back to this one)
            self.calendar_offset = (self.calendar_offset + 1) % 4;
            return;
        }
        // the shop's stock: a pack from a box, or a single off its stand
        if self.facing_student().is_none() {
            if let Some(i) = self.facing_stock() {
                match self.single_at(i) {
                    Some((pull, price)) => {
                        if self.state.player.money >= price {
                            self.state.player.money -= price;
                            self.state.player.collection.add(pull);
                            self.mode = Mode::Pack { pulls: vec![pull], revealed: 1 };
                        }
                    }
                    None => self.run(Act::BuyPacks(1)),
                }
                return;
            }
        }
        if self.facing_student().is_none() {
            if let Some(d) = self.facing_door() {
                self.world.as_mut().unwrap().doors.doors[d].toggle();
                return;
            }
        }
        if let Some(i) = self.facing_student() {
            let s = &self.state.students[i];
            let b = &self.state.bonds[i];
            let title = if b.knows(Fact::Name) { s.full_name() } else { describe(&self.state, i) };
            let sub = if b.knows(Fact::Name) { format!("{} · {} (rank {})", s.class_name(), b.title(), b.rank()) } else { "You haven't been introduced.".into() };
            let can_sell = self.state.player.collection.total() > 0;
            self.mode = Mode::Menu(Menu {
                title,
                subtitle: sub,
                items: vec![("Chat".into(), Act::Chat(i), true), ("Offer a card".into(), Act::Sell(i), can_sell), ("Leave".into(), Act::Close, true)],
                sel: 0,
            });
            return;
        }
        match self.zone() {
            Some(Interact::ShopCounter) => {
                let cost = self.state.pack_cost();
                let money = self.state.player.money;
                let shift_ok = self.state.phase() == Phase::Daytime;
                let season = tcg::season::theme(self.state.season).name;
                self.mode = Mode::Menu(Menu {
                    title: "Tsukumodō counter".into(),
                    subtitle: format!("{season} packs — ¥{cost} each with Uncle Kenji's discount (shelf price ¥{})", tcg::PACK_PRICE),
                    items: vec![
                        (format!("Buy a pack  (¥{cost})"), Act::BuyPacks(1), money >= cost),
                        (format!("Buy five packs  (¥{})", cost * 5), Act::BuyPacks(5), money >= cost * 5),
                        ("Work a shift  (4 h, ¥600)".into(), Act::Shift, shift_ok),
                        ("Talk to Uncle Kenji".into(), Act::UncleTalk, self.near_uncle()),
                        ("Leave".into(), Act::Close, true),
                    ],
                    sel: 0,
                });
            }
            Some(Interact::Bed) => {
                self.mode = Mode::Menu(Menu {
                    title: "Your futon".into(),
                    subtitle: "Sleep until 6:30. Bed before 22:00 for full energy.".into(),
                    items: vec![("Sleep".into(), Act::Sleep, true), ("Save the game".into(), Act::Save, true), ("Leave".into(), Act::Close, true)],
                    sel: 0,
                });
            }
            Some(Interact::Classroom) if self.prompt().is_some() => {
                let r = self.state.enter_classroom(&self.db);
                self.mode = Mode::Dialogue { speaker: "Class 1-A".into(), lines: vec![r], line: 0 };
            }
            _ => {}
        }
    }

    fn run(&mut self, act: Act) {
        match act {
            Act::Close => self.mode = Mode::Explore,
            Act::Chat(i) => {
                let before = self.state.bonds[i].rank();
                let r = self.state.talk(i);
                let s = &self.state.students[i];
                let mut lines = vec![smalltalk(s.personality, s.plays_cards).to_string()];
                if r != "You chatted for a while." || self.state.bonds[i].rank() == before {
                    lines.push(r);
                }
                let speaker = describe(&self.state, i);
                self.mode = Mode::Dialogue { speaker, lines, line: 0 };
            }
            Act::Sell(i) => self.open_binder(Some(i)),
            Act::BuyPacks(n) => {
                let mut pulls = Vec::new();
                for _ in 0..n {
                    match self.state.buy_pack(&self.db) {
                        Ok(p) => pulls.extend(p),
                        Err(e) => {
                            self.say(e);
                            break;
                        }
                    }
                }
                self.mode = if pulls.is_empty() { Mode::Explore } else { Mode::Pack { pulls, revealed: 0 } };
            }
            Act::Shift => {
                let r = self.state.work_shift(&self.db);
                self.mode = Mode::Dialogue { speaker: "Uncle Kenji".into(), lines: vec![r, "\"Good work. The kids trust you behind the counter.\"".into()], line: 0 };
            }
            Act::UncleTalk => {
                let today = self.state.time.date;
                let first = self.state.uncle.last_talk != Some(today);
                if first {
                    self.state.uncle.add(2);
                    self.state.uncle.last_talk = Some(today);
                }
                let disc = (sim::economy::uncle_discount(self.state.uncle_rank()) * 100.0).round();
                let next = next_release(today);
                let lines = vec![
                    format!("\"You get {disc:.0}% off packs. Don't tell the other kids.\""),
                    format!("\"Next season hits the shelves on {} {} {}. Release days get busy — I could use a hand.\"", next.weekday().kanji(), next.day, next.month_name()),
                    "\"Your classmates will pay more for a card they need. Get to know them first — and don't rip anyone off, this town talks.\"".into(),
                ];
                self.mode = Mode::Dialogue { speaker: "Uncle Kenji".into(), lines, line: 0 };
            }
            Act::Sleep => {
                let r = self.state.sleep(&self.db);
                self.cut = true;
                self.mode = Mode::Dialogue { speaker: String::new(), lines: vec![r], line: 0 };
            }
            Act::Save => {
                let r = match self.state.save(std::path::Path::new(SAVE_PATH)) {
                    Ok(()) => "Game saved.".to_string(),
                    Err(e) => format!("Save failed: {e}"),
                };
                self.mode = Mode::Dialogue { speaker: String::new(), lines: vec![r], line: 0 };
            }
        }
    }

    /// The binder's list: (pull, copies) sorted by season, rarity (best first), number.
    pub fn owned(&self) -> Vec<(Pull, u32)> {
        let mut v = Vec::new();
        for o in self.state.player.collection.iter() {
            if o.foil > 0 {
                v.push((Pull { id: o.id, foil: true }, o.foil));
            }
            if o.normal > 0 {
                v.push((Pull { id: o.id, foil: false }, o.normal));
            }
        }
        v.sort_by_key(|(p, _)| {
            let c = self.db.get(p.id);
            (std::cmp::Reverse(c.id.set), std::cmp::Reverse(c.rarity), !p.foil, c.id.number)
        });
        v
    }

    // ------------------------------------------------------------------ the binder

    /// Open the binder in front of the player (choosing a card to offer when `sell_to`).
    pub fn open_binder(&mut self, sell_to: Option<usize>) {
        self.binder.open(self.player.eye(), self.player.yaw, &self.cam);
        self.mode = Mode::Binder { sel: 0, sell_to };
    }

    /// The loose cards (owned, not in a sleeve) in the binder's order: newest season, best first.
    pub fn loose(&self) -> Vec<(Pull, u32)> {
        let p = &self.state.player;
        self.owned().into_iter().filter_map(|(pull, _)| Some((pull, p.binder.loose_count(&p.collection, pull))).filter(|x| x.1 > 0)).collect()
    }

    /// The card under (or in) the hand.
    fn binder_pick(&self, loose: &[(Pull, u32)]) -> Option<Pull> {
        use crate::binder::Spot;
        if let Some(h) = self.binder.held {
            return Some(h.pull);
        }
        match self.binder.focus {
            Spot::Fan(i) => loose.get(i).map(|x| x.0),
            Spot::Pocket { page, pocket } => self.state.player.binder.get(page, pocket),
            _ => None,
        }
    }

    pub fn binder_open(&self) -> bool {
        matches!(self.mode, Mode::Binder { .. })
    }

    /// The binder's frame: animate, load what it needs, add its instances and take the camera.
    pub fn binder_frame(&mut self, r: &mut akari::Renderer, out: &mut Vec<akari::Instance>, dt: f32) -> anyhow::Result<()> {
        if !self.binder_open() {
            self.binder.idle(r);
            return Ok(());
        }
        let loose = self.loose();
        self.binder.frame(r, &self.db, &self.state.player.binder, &loose, dt, out, &mut self.cam)
    }

    /// The mouse moved (window pixels).
    pub fn binder_pointer(&mut self, x: f32, y: f32) {
        if self.binder_open() {
            let loose = self.loose();
            self.binder.point(&self.state.player.binder, &loose, akari::glam::Vec2::new(x, y));
        }
    }

    /// The hand's button (mouse left, Cross) went down or up. Choosing a card to offer, a
    /// press offers the card under the hand.
    fn binder_hand(&mut self, down: bool) {
        let Mode::Binder { sell_to, .. } = self.mode else { return };
        if sell_to.is_some() {
            if down {
                self.key(Key::Confirm, false);
            }
            return;
        }
        let loose = self.loose();
        let p = &mut self.state.player;
        if down {
            self.binder.press(&mut p.binder, &p.collection, &loose);
        } else {
            self.binder.release(&mut p.binder, &p.collection, &loose);
        }
    }

    /// Left mouse button (down / up); the right button puts the card in hand back.
    pub fn binder_button(&mut self, left: bool, down: bool) {
        if !self.binder_open() {
            return;
        }
        if !left {
            if down {
                self.binder.drop_held();
            }
            return;
        }
        self.binder_hand(down);
    }

    /// Mouse wheel (+ away from you): over the loose cards it scrolls them, elsewhere it turns pages.
    pub fn binder_wheel(&mut self, d: f32) {
        if !self.binder_open() || d == 0.0 {
            return;
        }
        let loose = self.loose();
        if matches!(self.binder.focus, crate::binder::Spot::Fan(_)) {
            self.binder.scroll_fan(&loose, if d > 0.0 { -1 } else { 1 });
        } else {
            self.binder.turn(&self.state.player.binder, if d > 0.0 { 1 } else { -1 });
        }
    }

    /// Quick move: a loose card into the next empty pocket, a sleeved card back out.
    fn binder_quick(&mut self) {
        if !matches!(self.mode, Mode::Binder { sell_to: None, .. }) {
            return;
        }
        let loose = self.loose();
        let p = &mut self.state.player;
        self.binder.quick(&mut p.binder, &p.collection, &loose);
    }

    /// Keys the binder handles itself (page turning, quick move); true when used.
    pub fn binder_key(&mut self, k: akari::winit::keyboard::KeyCode) -> bool {
        use akari::winit::keyboard::KeyCode::*;
        if !self.binder_open() {
            return false;
        }
        match k {
            KeyZ | PageUp => {
                self.binder.turn(&self.state.player.binder, -1);
            }
            KeyX | PageDown => {
                self.binder.turn(&self.state.player.binder, 1);
            }
            KeyQ => self.binder_quick(),
            _ => return false,
        }
        true
    }

    /// The controller in the binder: the left stick moves the hand (it settles on the nearest
    /// pocket or card), the d-pad steps (held: repeats), Cross is the hand's button (hold to
    /// drag), Square quick-moves, Circle puts back / closes, L1 / R1 turn pages, the right stick
    /// peels a page over, Triangle closes. (main.rs leaves these buttons to the binder.)
    pub fn binder_pad(&mut self, pad: &play::input::gamepad::Pad, dt: f32) {
        use play::input::Button as B;
        if !self.binder_open() || !pad.connected {
            return;
        }
        if pad.pressed(B::Triangle) {
            self.key(Key::Binder, false);
            return;
        }
        if pad.pressed(B::Circle) {
            self.key(Key::Back, false);
            return;
        }
        let selling = matches!(self.mode, Mode::Binder { sell_to: Some(_), .. });
        for (b, d) in [(B::L1, -1), (B::R1, 1)] {
            if pad.pressed(b) {
                self.binder.turn(&self.state.player.binder, d);
            }
        }
        if pad.pressed(B::Square) {
            self.binder_quick();
        }
        // the d-pad: a step, then repeats while held
        let dir = (pad.held(B::Right) as i32 - pad.held(B::Left) as i32, pad.held(B::Down) as i32 - pad.held(B::Up) as i32);
        let loose = self.loose();
        if dir == (0, 0) {
            self.binder.pad_repeat = None;
        } else {
            let fire = match &mut self.binder.pad_repeat {
                Some((d, t)) if *d == dir => {
                    *t -= dt;
                    if *t <= 0.0 {
                        *t += 0.11;
                        true
                    } else {
                        false
                    }
                }
                _ => {
                    self.binder.pad_repeat = Some((dir, 0.32));
                    true
                }
            };
            if fire {
                let (dx, dy) = if dir.0 != 0 { (dir.0, 0) } else { (0, dir.1) };
                self.binder.step(&self.state.player.binder, &loose, dx, dy);
            }
        }
        // the stick moves the hand; faster the further it is pushed
        if pad.left.length() > 0.08 {
            let s = self.binder.screen.y * (0.35 + 0.9 * pad.left.length());
            let d = akari::glam::Vec2::new(pad.left.x, -pad.left.y) * s * dt;
            self.binder.nudge(&self.state.player.binder, &loose, d);
        }
        // Cross is the hand's button: tap = click, hold = drag
        let cross = pad.held(B::Cross);
        if cross != self.binder.pad_cross {
            self.binder.pad_cross = cross;
            if !selling || cross {
                self.binder_hand(cross);
            }
        }
        if !selling {
            self.binder.peel(&self.state.player.binder, pad.right.x, dt);
        }
    }

    /// Test screens: "binder" as it is, "binder-filled" with the loose cards sleeved in order,
    /// "binder-hold" with a card in hand over an empty pocket, "binder-turn" mid page turn,
    /// "binder-insert" a card half-way into its pocket, "binder-out" one sliding out.
    pub fn binder_screen(&mut self, which: &str) {
        use crate::binder::Spot;
        self.open_binder(None);
        if which == "binder" {
            return;
        }
        let col = self.state.player.collection.clone();
        let b = &mut self.state.player.binder;
        let mut slot = 0;
        for (pull, n) in b.loose(&col) {
            for _ in 0..n {
                // fill the first sheet's front and back and part of the second, leaving gaps
                if slot >= 2 * sim::binder::POCKETS + 5 {
                    break;
                }
                let (page, pocket) = (slot / 9, slot % 9);
                if !(page == 0 && pocket == 4) && !(page == 2 && pocket >= 3) {
                    b.put(&col, page, pocket, pull);
                }
                slot += 1;
            }
        }
        let loose = self.loose();
        let b = &mut self.state.player.binder;
        let v = &mut self.binder;
        match which {
            "binder-filled" => {
                v.turn(b, 1);
            }
            "binder-hold" => {
                v.focus = Spot::Fan(1.min(loose.len().saturating_sub(1)));
                v.click(b, &col, &loose);
                v.focus = Spot::Pocket { page: 0, pocket: 4 };
            }
            "binder-insert" => {
                v.focus = Spot::Fan(0);
                v.click(b, &col, &loose);
                v.focus = Spot::Pocket { page: 0, pocket: 4 };
                v.click(b, &col, &loose);
                v.freeze_slide(0.5);
            }
            "binder-out" => {
                v.focus = Spot::Pocket { page: 0, pocket: 1 };
                v.click(b, &col, &loose);
            }
            "binder-turn" => {
                v.turn(b, 1);
                v.freeze_turn(0.42);
            }
            _ => {}
        }
    }

    pub fn key(&mut self, k: Key, shift: bool) {
        let mode = std::mem::replace(&mut self.mode, Mode::Explore);
        self.mode = match mode {
            Mode::Explore => match k {
                Key::Confirm => {
                    self.interact();
                    return;
                }
                Key::Binder => {
                    self.open_binder(None);
                    return;
                }
                // your journal, from your bag: the people in it, or your plans
                Key::People => Mode::Journal { spread: crate::paper::Spread::People },
                Key::Calendar => Mode::Journal { spread: crate::paper::Spread::Plans },
                Key::Quests => Mode::Journal { spread: crate::paper::Spread::Quests },
                Key::Map if self.in_town => self.open_map(),
                _ => Mode::Explore,
            },
            Mode::Map => self.map_key(k),
            Mode::Locker => self.locker_key(k),
            Mode::Dialogue { speaker, lines, line } => match k {
                Key::Confirm if line + 1 < lines.len() => Mode::Dialogue { speaker, lines, line: line + 1 },
                Key::Confirm | Key::Back => Mode::Explore,
                _ => Mode::Dialogue { speaker, lines, line },
            },
            Mode::Menu(mut m) => match k {
                Key::Up => {
                    m.sel = (m.sel + m.items.len() - 1) % m.items.len();
                    Mode::Menu(m)
                }
                Key::Down => {
                    m.sel = (m.sel + 1) % m.items.len();
                    Mode::Menu(m)
                }
                Key::Back => Mode::Explore,
                Key::Confirm => {
                    let (_, act, ok) = m.items[m.sel].clone();
                    if ok {
                        self.run(act);
                        return;
                    }
                    Mode::Menu(m)
                }
                _ => Mode::Menu(m),
            },
            Mode::Pack { pulls, revealed } => match k {
                Key::Confirm if revealed < pulls.len() => Mode::Pack { pulls, revealed: revealed + 1 },
                Key::Back if revealed < pulls.len() => Mode::Pack { revealed: pulls.len(), pulls },
                Key::Confirm | Key::Back => Mode::Explore,
                _ => Mode::Pack { pulls, revealed },
            },
            Mode::Binder { sel, sell_to } => {
                let loose = self.loose();
                let b = &mut self.state.player.binder;
                let v = &mut self.binder;
                let step = |v: &mut crate::binder::BinderView, b: &sim::binder::Binder, dx, dy| v.step(b, &loose, dx, dy);
                match k {
                    Key::Left => step(v, b, -1, 0),
                    Key::Right => step(v, b, 1, 0),
                    Key::Up => step(v, b, 0, -1),
                    Key::Down => step(v, b, 0, 1),
                    Key::Back if v.drop_held() => {}
                    Key::Back | Key::Binder => {
                        v.close();
                        return; // (mode is already Explore)
                    }
                    Key::Confirm => match sell_to {
                        // selling: offer the card under the hand (or in it)
                        Some(i) => {
                            if let Some(pull) = self.binder_pick(&loose) {
                                self.binder.close();
                                let value = self.state.market.value(self.db.get(pull.id), pull.foil, self.state.season);
                                self.mode = Mode::Sell { student: i, pull, price: value, value, result: None, counter: None, done: false };
                                return;
                            }
                        }
                        None => v.click(b, &self.state.player.collection, &loose),
                    },
                    _ => {}
                }
                Mode::Binder { sel, sell_to }
            }
            Mode::Sell { student, pull, price, value, result, counter, done } => {
                let step = if shift { 100 } else { 10 };
                match (k, done, counter) {
                    (Key::Confirm | Key::Back, true, _) => Mode::Explore,
                    (Key::Confirm, false, Some(c)) => self.offer(student, pull, c, value),
                    (Key::Back, false, Some(_)) => Mode::Sell { student, pull, price, value, result: Some("You turn down the counter-offer.".into()), counter: None, done: true },
                    (Key::Left | Key::Down, false, None) => Mode::Sell { student, pull, price: price.saturating_sub(step).max(10), value, result, counter, done },
                    (Key::Right | Key::Up, false, None) => Mode::Sell { student, pull, price: price + step, value, result, counter, done },
                    (Key::Confirm, false, None) => self.offer(student, pull, price, value),
                    (Key::Back, false, None) => {
                        self.open_binder(Some(student));
                        return;
                    }
                    _ => Mode::Sell { student, pull, price, value, result, counter, done },
                }
            }
            Mode::Lesson(v) => self.lesson_key(v, k),
            Mode::YearEnd { page } => self.year_key(page, k),
            Mode::Journal { spread: crate::paper::Spread::Quests } if self.quests_key(k) => Mode::Journal { spread: crate::paper::Spread::Quests },
            Mode::Journal { spread } => match k {
                Key::Left | Key::Up => Mode::Journal { spread: spread.turn(-1) },
                Key::Right | Key::Down | Key::Confirm => Mode::Journal { spread: spread.turn(1) },
                Key::Back | Key::People | Key::Calendar | Key::Quests => Mode::Explore,
                _ => Mode::Journal { spread },
            },
        };
    }

    fn offer(&mut self, student: usize, pull: Pull, price: u32, value: u32) -> Mode {
        let who = describe(&self.state, student);
        let out = self.state.sell(&self.db, student, pull, price);
        let (result, counter, done) = match out.response {
            SaleResponse::Accept(p) => {
                let mood = if out.reputation_delta < 0 { " They look like they feel ripped off." } else if out.bond_delta > 1 { " They're really happy with the deal." } else { "" };
                (format!("{who} buys it for ¥{p}.{mood}"), None, true)
            }
            SaleResponse::Counter(c) => (format!("{who}: \"How about ¥{c}?\"  (Enter: accept · Esc: decline)"), Some(c), false),
            SaleResponse::Refuse(why) => (format!("{who} {why}."), None, true),
        };
        Mode::Sell { student, pull, price, value, result: Some(result), counter, done }
    }

    // ------------------------------------------------------------------ UI

    pub fn draw(&mut self, ui: &mut Ui) {
        let s = ui.scale();
        let in_menu = !matches!(self.mode, Mode::Explore | Mode::Dialogue { .. });
        if matches!(self.mode, Mode::Map) {
            self.draw_map(ui);
            return;
        }
        if !in_menu {
            let loc = if self.in_town { "Kazehana-chō" } else { self.level().name };
            draw::hud(ui, &self.state, loc);
            self.draw_compass(ui);
            draw::toasts(ui, &self.toasts);
        }
        match &self.mode {
            Mode::Explore => {
                if let Some(p) = self.prompt() {
                    draw::prompt(ui, &self.buttons(), "interact", &p);
                }
                draw::hints(ui, &self.buttons(), "{move} Move · {look} Look · {run} Run · {binder} Binder · {map} Map · {quests} Quests · {people} Journal · {fast} Hold: wait");
            }
            Mode::Dialogue { speaker, lines, line } => {
                let (w, h) = (ui.width * 0.62, 170.0 * s);
                let (x, y) = (ui.width * 0.19, ui.height - h - 70.0 * s);
                draw::panel(ui, x, y, w, h);
                if !speaker.is_empty() {
                    let nw = ui.measure(24.0 * s, speaker) + 32.0 * s;
                    ui.rect(x + 20.0 * s, y - 22.0 * s, nw, 40.0 * s, draw::ACCENT);
                    ui.text(x + 36.0 * s, y - 16.0 * s, 24.0 * s, INK, speaker);
                }
                ui.text_wrapped(x + 32.0 * s, y + 34.0 * s, w - 64.0 * s, 26.0 * s, INK, &lines[*line]);
                let more = if line + 1 < lines.len() { "{confirm} ▼" } else { "{confirm} Close" };
                draw::hint_right(ui, &self.buttons(), more, x + w - 24.0 * s, y + h - 40.0 * s);
            }
            Mode::Menu(m) => {
                let w = 560.0 * s;
                let ih = 50.0 * s;
                let h = 120.0 * s + ih * m.items.len() as f32;
                let (x, y) = (ui.width * 0.5 - w * 0.5, ui.height * 0.5 - h * 0.5);
                draw::panel(ui, x, y, w, h);
                ui.text(x + 28.0 * s, y + 18.0 * s, 30.0 * s, INK, &m.title);
                ui.text_wrapped(x + 28.0 * s, y + 58.0 * s, w - 56.0 * s, 17.0 * s, DIM, &m.subtitle);
                for (k, (label, _, ok)) in m.items.iter().enumerate() {
                    let iy = y + 104.0 * s + ih * k as f32;
                    if k == m.sel {
                        ui.rect(x + 16.0 * s, iy, w - 32.0 * s, ih - 6.0 * s, Rgba(255, 255, 255, 30));
                        ui.rect(x + 16.0 * s, iy, 5.0 * s, ih - 6.0 * s, draw::ACCENT);
                    }
                    ui.text(x + 36.0 * s, iy + 9.0 * s, 24.0 * s, if *ok { INK } else { Rgba(120, 118, 114, 255) }, label);
                }
                draw::hints(ui, &self.buttons(), "{choose} Choose · {confirm} Confirm · {back} Back");
            }
            Mode::Pack { pulls, revealed } => self.draw_pack(ui, pulls, *revealed),
            Mode::Binder { sell_to, .. } => self.draw_binder(ui, *sell_to),
            Mode::Sell { student, pull, price, value, result, counter, done } => self.draw_sell(ui, *student, *pull, *price, *value, result.as_deref(), *counter, *done),
            Mode::Journal { .. } => {}
            Mode::Lesson(v) => self.draw_lesson(ui, v),
            Mode::YearEnd { page } => self.draw_year_end(ui, *page),
            Mode::Map => {}
            Mode::Locker => self.draw_locker(ui),
        }
    }

    fn draw_sell(&self, ui: &mut Ui, student: usize, pull: Pull, price: u32, value: u32, result: Option<&str>, counter: Option<u32>, done: bool) {
        let s = ui.scale();
        draw::dim(ui, 0.86);
        let c = self.db.get(pull.id);
        let st = &self.state.students[student];
        let b = &self.state.bonds[student];
        let who = describe(&self.state, student);
        let cw = 330.0 * s;
        let (cx, cy) = (ui.width * 0.5 - 520.0 * s, ui.height * 0.5 - draw::card_h(cw) * 0.5);
        draw::card(ui, cx, cy, cw, c, pull.foil, false);
        let (px, py) = (ui.width * 0.5 - 120.0 * s, cy);
        draw::panel(ui, px, py, 640.0 * s, draw::card_h(cw));
        ui.text(px + 28.0 * s, py + 20.0 * s, 30.0 * s, INK, &format!("Selling to {who}"));
        if b.knows(Fact::Name) {
            ui.text(px + 28.0 * s, py + 62.0 * s, 18.0 * s, DIM, &format!("{} · {} · {:?}", st.class_name(), b.title(), st.personality));
        }
        ui.text(px + 28.0 * s, py + 110.0 * s, 22.0 * s, DIM, &format!("Market value ~¥{value}"));
        if b.knows(Fact::Wallet) {
            ui.text(px + 28.0 * s, py + 142.0 * s, 22.0 * s, DIM, &format!("They have about ¥{} on them", st.wallet / 100 * 100));
        }
        // their interest, as body language (exact numbers stay hidden)
        let max = willingness(st, c, pull.foil, value, b.rank());
        let interest = if !st.plays_cards { "Doesn't seem to care about cards." } else if max as f32 > value as f32 * 1.5 { "Their eyes light up." } else if max >= value { "They look interested." } else if max > 0 { "They look unsure." } else { "They shrug." };
        ui.text(px + 28.0 * s, py + 180.0 * s, 22.0 * s, INK, interest);
        ui.text(px + 28.0 * s, py + 240.0 * s, 20.0 * s, DIM, "Your asking price");
        ui.text(px + 28.0 * s, py + 268.0 * s, 64.0 * s, GOLD, &format!("¥{price}"));
        let ratio = price as f32 / value.max(1) as f32;
        let fair = if ratio > 1.3 { ("Steep — could hurt your reputation", draw::ACCENT) } else if ratio < 0.8 { ("A bargain for them", Rgba(120, 220, 150, 255)) } else { ("Fair", INK) };
        ui.text(px + 28.0 * s, py + 350.0 * s, 20.0 * s, fair.1, fair.0);
        if let Some(r) = result {
            ui.text_wrapped(px + 28.0 * s, py + 400.0 * s, 580.0 * s, 22.0 * s, INK, r);
        }
        draw::hints(
            ui,
            &self.buttons(),
            if done {
                "{confirm} Continue"
            } else if counter.is_some() {
                "{confirm} Accept their offer · {back} Decline"
            } else {
                "{choose} Price ±¥10 (Shift ±¥100) · {confirm} Make the offer · {back} Pick another card"
            },
        );
    }

    fn draw_pack(&self, ui: &mut Ui, pulls: &[Pull], revealed: usize) {
        let s = ui.scale();
        draw::dim(ui, 0.82);
        let season = tcg::season::theme(self.state.season).name;
        ui.text_centered(ui.width * 0.5, 50.0 * s, 40.0 * s, INK, &format!("{season} — {} pack{}", pulls.len() / tcg::PACK_SIZE, if pulls.len() > tcg::PACK_SIZE { "s" } else { "" }));
        // show the current pack's six cards (the pack of the next card to reveal)
        let pack = (revealed.min(pulls.len() - 1)) / tcg::PACK_SIZE;
        let row = &pulls[pack * tcg::PACK_SIZE..((pack + 1) * tcg::PACK_SIZE).min(pulls.len())];
        let cw = 250.0 * s;
        let gap = 22.0 * s;
        let total = row.len() as f32 * cw + (row.len() as f32 - 1.0) * gap;
        let (x0, y) = (ui.width * 0.5 - total * 0.5, ui.height * 0.5 - draw::card_h(cw) * 0.5);
        let mut value = 0;
        for (k, p) in row.iter().enumerate() {
            let idx = pack * tcg::PACK_SIZE + k;
            let x = x0 + k as f32 * (cw + gap);
            if idx < revealed {
                let c = self.db.get(p.id);
                let shine = matches!(c.rarity, tcg::Rarity::Rare | tcg::Rarity::Mythic) || p.foil;
                if shine {
                    let col = if p.foil { GOLD } else { draw::rarity_color(c.rarity) };
                    ui.rect(x - 10.0 * s, y - 10.0 * s, cw + 20.0 * s, draw::card_h(cw) + 20.0 * s, col.alpha(0.35));
                }
                draw::card(ui, x, y, cw, c, p.foil, idx + 1 == revealed);
                let v = self.state.market.value(c, p.foil, self.state.season);
                value += v;
                ui.text_centered(x + cw * 0.5, y + draw::card_h(cw) + 14.0 * s, 18.0 * s, draw::rarity_color(c.rarity), &format!("{} · ~¥{v}", draw::rarity_name(c.rarity)));
            } else {
                draw::card_back(ui, x, y, cw);
            }
        }
        if revealed >= pulls.len() {
            ui.text_centered(ui.width * 0.5, ui.height - 140.0 * s, 26.0 * s, GOLD, &format!("Market value of this pack: ~¥{value}"));
        }
        draw::hints(ui, &self.buttons(), if revealed < pulls.len() { "{confirm} Reveal the next card · {back} Reveal all" } else { "{confirm} Done" });
    }

    /// Over the physical binder: what you're doing, the card under your hand, and the hints.
    fn draw_binder(&self, ui: &mut Ui, sell_to: Option<usize>) {
        use crate::binder::Spot;
        let s = ui.scale();
        let v = &self.binder;
        let b = &self.state.player.binder;
        let col = &self.state.player.collection;
        let loose = self.loose();
        if let Some(e) = &v.error {
            draw::dim(ui, 0.86);
            ui.text(70.0 * s, 46.0 * s, 30.0 * s, INK, "The binder can't be shown");
            ui.text_wrapped(70.0 * s, 100.0 * s, ui.width - 140.0 * s, 18.0 * s, DIM, e);
            draw::hints(ui, &self.buttons(), "{back} Back");
            return;
        }
        if !v.loaded() {
            return;
        }
        // a soft vignette keeps the binder the subject
        ui.gradient(0.0, 0.0, ui.width, 120.0 * s, Rgba(0, 0, 0, 150), Rgba(0, 0, 0, 0));
        let title = match sell_to {
            Some(i) => format!("Offer a card to {}", describe(&self.state, i)),
            None => "Binder".to_string(),
        };
        ui.text_shadow(48.0 * s, 30.0 * s, 30.0 * s, INK, &title);
        let (lp, rp) = v.pages(b);
        let pages = match (lp, rp) {
            (Some(l), Some(r)) => format!("Pages {}–{} of {}", l + 1, r + 1, b.pages()),
            (Some(l), None) => format!("Page {} of {}", l + 1, b.pages()),
            (None, Some(r)) => format!("Page {} of {}", r + 1, b.pages()),
            _ => String::new(),
        };
        let loose_n: u32 = loose.iter().map(|x| x.1).sum();
        ui.text_shadow(48.0 * s, 70.0 * s, 18.0 * s, DIM, &format!("{pages}  ·  {} in sleeves  ·  {} loose", b.total(), loose_n));
        if col.total() == 0 {
            ui.text_shadow(48.0 * s, 98.0 * s, 18.0 * s, DIM, "No cards yet. Uncle Kenji sells packs at the shop counter.");
        }
        // how many of each loose card, over its corner in the fan
        for (p, n, focus) in v.fan_labels(&loose) {
            if n > 1 || focus {
                let t = format!("×{n}");
                let w = ui.measure(16.0 * s, &t) + 12.0 * s;
                ui.rect(p.x - w * 0.5, p.y - 24.0 * s, w, 22.0 * s, Rgba(10, 10, 14, 200));
                ui.text_centered(p.x, p.y - 22.0 * s, 16.0 * s, if focus { GOLD } else { INK }, &t);
            }
        }
        // the card under the hand
        let under = v.held.map(|h| h.pull).or(match v.focus {
            Spot::Fan(i) => loose.get(i).map(|x| x.0),
            Spot::Pocket { page, pocket } => b.get(page, pocket),
            _ => None,
        });
        if let Some(p) = under {
            let c = self.db.get(p.id);
            let w = 380.0 * s;
            let (x, y) = (ui.width - w - 40.0 * s, 26.0 * s);
            draw::panel(ui, x, y, w, 112.0 * s);
            ui.text(x + 18.0 * s, y + 12.0 * s, 24.0 * s, INK, &c.name);
            ui.text(x + 18.0 * s, y + 46.0 * s, 17.0 * s, draw::rarity_color(c.rarity), &format!("{}{} · {:?} · {}", draw::rarity_name(c.rarity), if p.foil { " Holo" } else { "" }, c.kind, c.attribute));
            let value = self.state.market.value(c, p.foil, self.state.season);
            ui.text(x + 18.0 * s, y + 74.0 * s, 17.0 * s, DIM, &format!("{}  ·  ~¥{value}  ·  own {}", c.id, col.get(p.id).map_or(0, |o| if p.foil { o.foil } else { o.normal })));
            if let Some(i) = sell_to {
                let st = &self.state.students[i];
                let knows = self.state.bonds[i].knows(Fact::Wants);
                let wants = knows && st.wants.contains(&p.id);
                let hint = if wants { "They're hunting for this one!" } else if knows { "Not on their want list." } else { "You don't know what they're after yet." };
                ui.text(x + 18.0 * s, y + 120.0 * s, 17.0 * s, if wants { GOLD } else { DIM }, hint);
            }
        }
        // the focused pocket: corner marks (keys and pads; the mouse while it carries a card)
        if v.show_focus() {
            if let Some(q) = v.focus_quad(b) {
                let full = b.get(match v.focus { Spot::Pocket { page, .. } => page, _ => 0 }, match v.focus { Spot::Pocket { pocket, .. } => pocket, _ => 0 }).is_some();
                let c = if v.held.is_some() && full { Rgba(240, 170, 90, 230) } else { GOLD };
                let (l, t) = (q[0].x.min(q[3].x), q[0].y.min(q[1].y));
                let (r, btm) = (q[1].x.max(q[2].x), q[2].y.max(q[3].y));
                let (k, w) = (16.0 * s, 3.0 * s);
                for (x, y, dx, dy) in [(l, t, 1.0, 1.0), (r, t, -1.0, 1.0), (r, btm, -1.0, -1.0), (l, btm, 1.0, -1.0)] {
                    let hx = if dx > 0.0 { x } else { x - k };
                    let vy = if dy > 0.0 { y } else { y - k };
                    ui.rect(hx, if dy > 0.0 { y } else { y - w }, k, w, c);
                    ui.rect(if dx > 0.0 { x } else { x - w }, vy, w, k, c);
                }
            }
        }
        // the pad's hand (the mouse has its own cursor)
        if v.pad_hand && v.held.is_none() {
            let p = v.pointer;
            ui.rect(p.x - 7.0 * s, p.y - 7.0 * s, 14.0 * s, 14.0 * s, Rgba(10, 10, 14, 160));
            ui.rect(p.x - 4.0 * s, p.y - 4.0 * s, 8.0 * s, 8.0 * s, GOLD);
        }
        let hint = if sell_to.is_some() {
            "{browse} Choose · {confirm} Offer this card · {turn} Turn page · {back} Back"
        } else if v.held.is_some() {
            match v.focus {
                Spot::Pocket { page, pocket } if b.get(page, pocket).is_some() => "{browse} Move · {confirm} Swap · {quick} Quick place · {turn} Turn page · {back} Put back",
                _ => "{browse} Move · {confirm} Slide in · {quick} Quick place · {turn} Turn page · {back} Put back",
            }
        } else {
            match v.focus {
                Spot::Pocket { page, pocket } if b.get(page, pocket).is_some() => "{browse} Move · {confirm} Pull out · {quick} Back to loose · {turn} Turn page · {back} Close",
                Spot::Fan(_) => "{browse} Move · {confirm} Pick up · {quick} Quick place · {turn} Turn page · {back} Close",
                _ => "{browse} Move · {turn} Turn page · {back} Close",
            }
        };
        draw::hints(ui, &self.buttons(), hint);
    }
}

fn next_release(today: Date) -> Date {
    let mut d = today.add_days(1);
    loop {
        if events_on(d).iter().any(|e| matches!(e, Event::SetRelease(_))) {
            return d;
        }
        d = d.add_days(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::Placed;

    fn new_game() -> Game {
        let db = CardDb::generate(12);
        let state = GameState::new(7, &db);
        let levels = [crate::world::build_home(), crate::world::build_path(), crate::world::build_school()];
        Game::new(db, state, levels)
    }

    #[test]
    fn shop_pack_binder_and_sell() {
        let mut g = new_game();
        // stand at the counter on a Saturday afternoon
        g.state.time = sim::GameTime::new(Date::new(2026, 4, 11), 14 * 60);
        g.state.location = Location::Home;
        g.cam.position = Vec3::new(-2.0, EYE, 1.4);
        assert!(g.prompt().unwrap().contains("Shop counter"));
        g.key(Key::Confirm, false);
        assert!(matches!(g.mode, Mode::Menu(_)));
        g.key(Key::Down, false); // "Buy five packs"
        g.key(Key::Confirm, false);
        let Mode::Pack { ref pulls, .. } = g.mode else { panic!("expected the pack screen") };
        assert_eq!(pulls.len(), 30);
        g.key(Key::Back, false); // reveal all
        g.key(Key::Confirm, false); // done
        assert!(matches!(g.mode, Mode::Explore));
        assert_eq!(g.state.player.collection.total(), 30);
        let money_after_packs = g.state.player.money;
        assert_eq!(money_after_packs, 3000 - 5 * g.state.pack_cost());

        // a classmate who plays, standing right in front of us
        let i = (0..g.state.students.len()).find(|&i| g.state.students[i].plays_cards && g.state.students[i].wallet > 500).unwrap();
        g.state.students[i].wallet = 5000;
        let fwd = g.cam.forward_flat();
        g.placed = vec![Placed { student: i, pos: g.cam.position - Vec3::Y * EYE + fwd * 1.2 }];
        assert!(g.prompt().unwrap().starts_with("Talk to"));
        g.key(Key::Confirm, false);
        g.key(Key::Confirm, false); // Chat
        assert!(matches!(g.mode, Mode::Dialogue { .. }));
        g.key(Key::Confirm, false);
        g.key(Key::Confirm, false);
        assert!(matches!(g.mode, Mode::Explore));
        assert!(g.state.bonds[i].points > 0);

        // offer them the card they want most if we have it, else our best card, at market value
        g.key(Key::Confirm, false);
        g.key(Key::Down, false); // "Offer a card"
        g.key(Key::Confirm, false);
        assert!(matches!(g.mode, Mode::Binder { sell_to: Some(_), .. }));
        g.key(Key::Confirm, false);
        let Mode::Sell { value, .. } = g.mode else { panic!("expected the sell screen") };
        // ask a bargain price: anyone who plays should take it or counter
        for _ in 0..(value / 10) {
            g.key(Key::Left, false);
        }
        g.key(Key::Confirm, false);
        match &g.mode {
            Mode::Sell { done: true, result: Some(r), .. } => assert!(r.contains("buys") || r.contains("interested") || r.contains("care"), "{r}"),
            Mode::Sell { counter: Some(_), .. } => {
                g.key(Key::Confirm, false);
            }
            _ => panic!("unexpected sell state"),
        }
    }

    #[test]
    fn walls_hold_in_every_level() {
        let mut g = new_game();
        for loc in [Location::Home, Location::Path, Location::School] {
            g.state.location = loc;
            g.world = Some(blockout_playable(g.level()));
            let (p, yaw) = g.level().spawn_default();
            for k in 0..8 {
                // walk (running) for 8 s in each of eight directions from the spawn
                g.place_player(p, yaw + k as f32 * std::f32::consts::FRAC_PI_4, 0.0);
                let go = play::Actions { movement: akari::glam::Vec2::new(0.0, 1.0), run: true, ..Default::default() };
                for _ in 0..(8 * 60) {
                    g.update(1.0 / 60.0, &go, false);
                    if g.state.location != loc {
                        break; // (walked out through an exit)
                    }
                    let f = g.player.feet;
                    for c in &g.level().colliders {
                        let inside = f.x > c.min.x + 0.05 && f.x < c.max.x - 0.05 && f.z > c.min.z + 0.05 && f.z < c.max.z - 0.05 && c.min.y < f.y + 1.0 && c.max.y > f.y + 0.5;
                        assert!(!inside, "{loc:?}: walked into a collider at {f:?}");
                    }
                }
                g.state.location = loc;
            }
        }
    }

    #[test]
    fn a_school_morning() {
        let mut g = new_game();
        g.state.time = sim::GameTime::new(Date::new(2026, 4, 7), 8 * 60 + 15);
        g.state.location = Location::School;
        g.cam.position = Vec3::new(20.0, EYE, -5.0); // inside classroom 1-A
        assert!(g.prompt().unwrap().contains("class"));
        g.key(Key::Confirm, false);
        assert!(matches!(g.mode, Mode::Dialogue { .. }));
        assert_eq!(g.state.time.minute, 8 * 60 + 15, "sitting down skips nothing");
        g.key(Key::Confirm, false);
        // the lesson plays out, faster than walking about
        let t = g.state.time.minute;
        g.update(5.0, &play::Actions::default(), false);
        let pace = sim::life::Activity::Class { focus: sim::life::Focus::Listen }.pace() as u32;
        assert_eq!(g.state.time.minute, t + 5 * pace);
        // in the middle of a lesson you can't just get up: you raise your hand for the toilet
        let walk = play::Actions { movement: akari::glam::Vec2::new(0.0, 1.0), ..Default::default() };
        g.update(0.0, &walk, false);
        assert!(g.state.doing().is_some(), "still in your seat");
        assert!(g.prompt().unwrap().contains("toilet"));
        // (the 2nd period's lesson is on screen by now: there the people key raises your hand)
        g.key(if matches!(g.mode, Mode::Lesson(_)) { Key::People } else { Key::Confirm }, false);
        assert!(g.state.doing().is_none(), "let out to the toilet");
        let t = g.state.time.minute;
        g.update(5.0, &play::Actions::default(), false);
        assert_eq!(g.state.time.minute, t + 5);
        // the journal open in your hands: time goes on, you don't walk
        g.key(Key::Calendar, false);
        assert!(matches!(g.mode, Mode::Journal { .. }));
        let feet = g.player.feet;
        g.update(5.0, &walk, false);
        assert_eq!(g.state.time.minute, t + 10);
        assert_eq!(g.player.feet, feet);
        g.key(Key::Back, false);
        assert!(matches!(g.mode, Mode::Explore));
    }

    /// At school during the day, the gate won't let you out (but a trip to the toilet will).
    #[test]
    fn the_school_gate_during_lessons() {
        let mut g = new_game();
        g.state.time = sim::GameTime::new(Date::new(2026, 4, 7), 10 * 60);
        g.state.location = Location::School;
        let gate = Vec3::new(0.0, 0.0, 46.0);
        g.player.teleport(Vec3::new(0.0, 0.0, 40.0), 0.0);
        g.update(0.0, &play::Actions::default(), false);
        g.player.teleport(gate, 0.0);
        g.update(0.0, &play::Actions::default(), false);
        assert_eq!(g.state.location, Location::School, "turned back at the gate");
        // after school, out you go
        g.state.time = sim::GameTime::new(Date::new(2026, 4, 7), 16 * 60 + 30);
        g.player.teleport(gate, 0.0);
        g.update(0.0, &play::Actions::default(), false);
        assert_eq!(g.state.location, Location::Path);
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
