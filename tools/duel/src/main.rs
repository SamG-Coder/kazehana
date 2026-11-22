//! Mythbeasts Duel, 1v1 test: you against the AI at the play table in Tsukumodō.
//!
//! The rules are `tcg::battle` (docs/BATTLE_DESIGN.md). Everything is physical: the shop, its
//! oak play table, a neoprene playmat, and every card a real card slab (the cardgen templates)
//! lying on the mat or the table, gliding between places when it moves.
//!
//! Controls: click a card in your hand, then a lit zone / target. Click one of your monsters,
//! then an enemy to attack. Right click or Esc cancels. Space / End turn button ends your turn.
//! Right drag looks around, wheel leans in. N new duel, H help, F12 screenshot.
//!
//! Options: --deck Blaze|Torrent|Quake  --ai-deck ...  --level easy|normal|hard  --seed N
//!          --autoplay (AI vs AI in a hidden window, stills to assets/cards/duel_test/)  --no-dlss

mod hud;
mod view;

use akari::glam::{Affine3A, Quat, Vec3, Vec4};
use akari::winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use akari::*;
use anyhow::{Context, Result};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tcg::battle::ai::{self, Level};
use tcg::battle::{decks, Action, Duel, Effect, Event, Stats, Target, LEADER_HP, ZONES};
use tcg::{Attribute, CardDb, Kind};
use view::*;

// ----------------------------------------------------------------------------- presentation

/// One applied action, played back over time.
struct Step {
    events: Vec<Event>,
    after: Duel,
    keys_after: [Vec<u64>; 2],
    rekeys: Vec<(Key, Key)>,
    trap: Option<(usize, usize, tcg::CardId)>,
    attack: Option<(u32, Target)>,
    t: f32,
    hit_done: bool,
    switched: bool,
}

impl Step {
    fn trap_time(&self) -> f32 {
        if self.trap.is_some() { 0.85 } else { 0.0 }
    }
    fn switch_time(&self) -> f32 {
        if self.attack.is_some() { self.trap_time() + ATTACK * 0.62 } else { 0.0 }
    }
    fn hit_time(&self) -> f32 {
        if self.attack.is_some() { self.trap_time() + ATTACK * 0.38 } else { 0.25 }
    }
    fn end_time(&self) -> f32 {
        let cast = self.events.iter().any(|e| matches!(e, Event::Cast { .. }));
        let turn = self.events.iter().any(|e| matches!(e, Event::TurnStart { .. }));
        self.switch_time() + if cast { 1.05 } else if turn { 0.7 } else { 0.4 }
    }
}

/// How long an attack takes: rear up, dash, impact, settle back.
const ATTACK: f32 = 0.82;

/// The attacker's path at `k` (0..1 of `ATTACK`): offset from its zone and how far it rears up
/// (radians, its front edge rising). `to`: from its zone to the target; `flat`: that direction on
/// the table.
fn attack_curve(k: f32, to: Vec3, flat: Vec3) -> (Vec3, f32) {
    let ease = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let back = -flat * 0.022 + Vec3::Y * 0.04;
    let hit = to * 0.86 + Vec3::Y * 0.012;
    if k < 0.24 {
        let w = ease(k / 0.24);
        (back * w, 0.5 * w)
    } else if k < 0.38 {
        let d = ((k - 0.24) / 0.14).powi(2);
        (back.lerp(hit, d), 0.5 * (1.0 - d) - 0.18 * d)
    } else if k < 0.52 {
        let r = (k - 0.38) / 0.14;
        (hit - flat * 0.012 * (r * std::f32::consts::PI).sin(), -0.18 * (1.0 - r))
    } else {
        let r = ease((k - 0.52) / 0.48);
        (hit.lerp(Vec3::ZERO, r), 0.0)
    }
}

/// A fine glowing streak thrown out by a hit (it fades as it slows).
struct Spark {
    pos: Vec3,
    vel: Vec3,
    life: f32,
    max: f32,
    /// colour: `Fx` index (types, then white)
    mesh: usize,
}

/// A ring of light sweeping out over the mat, or a flash round a zone.
struct Wave {
    pos: Vec3,
    t: f32,
    max: f32,
    /// how far it sweeps (m)
    reach: f32,
    mesh: usize,
    /// a flash round the zone at `pos` instead of a sweeping ring
    flash: bool,
}

/// Where the controller's focus is when nothing is picked up: 0 = your hand, 1 = your monsters.
#[derive(Clone, Copy, PartialEq)]
struct Focus {
    row: usize,
    idx: usize,
}

struct Popup {
    pos: Vec3,
    text: String,
    color: Rgba,
    t: f32,
    big: bool,
    /// A number on a white pill, just under the ATK / HP tag of the card it happened to
    /// (`pos` is then the tag's anchor on the table).
    pill: bool,
    /// born above its anchor rather than below it
    above: bool,
}

/// Where a target's stat tag sits on the table (the chip in the HUD is drawn there).
fn tag_anchor(t: Target) -> Vec3 {
    match t {
        Target::Unit { side, zone } => monster_zone(side, zone) + Vec3::new(0.0, 0.002, if side == 0 { 0.054 } else { -0.054 }),
        Target::Leader { side } => leader_zone(side) + Vec3::new(0.0, 0.0, if side == 0 { 0.022 } else { -0.022 }),
    }
}

/// Where combat text for `t` is born: under the rival's stat tags; above your own cards (their
/// top edge on screen), so a hit on you reads where you are looking.
fn number_anchor(t: Target) -> Vec3 {
    match t {
        Target::Unit { side: 0, zone } => monster_zone(0, zone) + Vec3::new(0.0, 0.004, -0.05),
        Target::Leader { side: 0 } => leader_zone(0) + Vec3::new(0.0, 0.004, -0.05),
        _ => tag_anchor(t),
    }
}

fn mine(t: Target) -> bool {
    matches!(t, Target::Unit { side: 0, .. } | Target::Leader { side: 0 })
}

#[derive(Clone, Copy, PartialEq)]
enum Selected {
    None,
    Hand(usize),
    Unit(usize),
}

struct Args {
    deck: String,
    ai_deck: String,
    level: Level,
    seed: u64,
    autoplay: bool,
    dlss: bool,
}

fn parse_args() -> Args {
    let mut a = Args { deck: "Blaze".into(), ai_deck: "Torrent".into(), level: Level::Normal, seed: 7, autoplay: false, dlss: true };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        let next = v.get(i + 1).cloned().unwrap_or_default();
        match v[i].as_str() {
            "--deck" => {
                a.deck = next;
                i += 1;
            }
            "--ai-deck" => {
                a.ai_deck = next;
                i += 1;
            }
            "--level" => {
                a.level = match next.to_lowercase().as_str() {
                    "easy" => Level::Easy,
                    "hard" => Level::Hard,
                    _ => Level::Normal,
                };
                i += 1;
            }
            "--seed" => {
                a.seed = next.parse().unwrap_or(7);
                i += 1;
            }
            "--autoplay" => a.autoplay = true,
            "--no-dlss" => a.dlss = false,
            _ => {}
        }
        i += 1;
    }
    a
}

/// The duel's actions and their buttons (drawn as the main game draws them).
fn duel_prompts() -> play::Prompts {
    use akari::winit::keyboard::KeyCode as K;
    let mut p = play::Prompts::default();
    p.bind_mouse("pick", &["left"]);
    p.bind_pad("pick", &["south"]);
    p.bind_mouse("play", &["left"]);
    p.bind_pad("play", &["south"]);
    p.bind_mouse("back", &["right"]);
    p.bind_keys("back", &[K::Escape]);
    p.bind_pad("back", &["east"]);
    p.bind_keys("end", &[K::Space]);
    p.bind_pad("end", &["north"]);
    p.bind_pad("choose", &["dpad_left_right"]);
    p.bind_pad("row", &["dpad_up_down"]);
    p.bind_mouse("look", &["right"]);
    p.bind_pad("look", &["stick_right"]);
    p.bind_keys("new", &[K::KeyN]);
    p.bind_pad("new", &["start"]);
    p.bind_keys("quit", &[K::Escape]);
    p.bind_keys("help", &[K::KeyH]);
    p.bind_pad("help", &["select"]);
    p
}

fn type_color(a: Attribute) -> Rgba {
    let c = a.color();
    Rgba(c[0], c[1], c[2], 255)
}

struct App {
    args: Args,
    root: PathBuf,
    window: Option<Arc<Window>>,
    r: Option<Renderer>,
    scene: Option<Scene>,
    ui: Option<Ui>,
    db: CardDb,
    decks: [Vec<Stats>; 2],
    names: HashMap<tcg::CardId, String>,
    // the game
    game: Duel,
    display: Duel,
    keys: [Vec<u64>; 2],
    keys_display: [Vec<u64>; 2],
    serial: u64,
    steps: VecDeque<Step>,
    objs: HashMap<Key, Obj>,
    places: HashMap<Key, Place>,
    spell_show: HashMap<u64, f32>,
    popups: Vec<Popup>,
    log: VecDeque<String>,
    /// Recent messages and their ages (s).
    toasts: Vec<(String, f32)>,
    banner: (String, f32),
    ai_rng: tcg::Rng,
    ai_wait: f32,
    // input
    selected: Selected,
    hover_hand: Option<usize>,
    hover_card: Option<tcg::CardId>,
    cursor: (f64, f64),
    drag_right: bool,
    look: (f32, f32),
    lean: f32,
    show_help: bool,
    end_button: (f32, f32, f32, f32),
    /// Left button held since this cursor position (a pick-up), and whether it has moved enough
    /// to be a drag.
    press_at: Option<(f64, f64)>,
    dragging: bool,
    // effects
    sparks: Vec<Spark>,
    waves: Vec<Wave>,
    shakes: HashMap<Key, f32>,
    cam_shake: f32,
    fx_rng: u32,
    // controllers (crates/play, as in the main game)
    input: play::Input,
    prompts: play::Prompts,
    kit: play::Kit,
    pad_mode: bool,
    focus: Focus,
    target_i: usize,
    nav_cool: f32,
    /// --autoplay: frames left holding a card up for a still
    showcase: u32,
    attack_shots: u32,
    // clocks
    last: Instant,
    start: Instant,
    frames: u32,
    shot_request: Option<PathBuf>,
    shot_i: u32,
}

impl App {
    fn new(args: Args, root: PathBuf) -> App {
        let db = CardDb::generate(1);
        let d0 = decks::starter_named(&db, 1, &args.deck).unwrap_or_else(|| decks::starter(&db, 1, Attribute::Flame, Attribute::Gale));
        let d1 = decks::starter_named(&db, 1, &args.ai_deck).unwrap_or_else(|| decks::starter(&db, 1, Attribute::Tide, Attribute::Radiant));
        let names = db.all().iter().map(|c| (c.id, c.name.clone())).collect();
        let (game, _) = Duel::new([d0.clone(), d1.clone()], (args.seed % 2) as usize, args.seed);
        let mut app = App {
            ai_rng: tcg::Rng::new(args.seed ^ 0xA1),
            args,
            root,
            window: None,
            r: None,
            scene: None,
            ui: None,
            db,
            decks: [d0, d1],
            names,
            display: game.clone(),
            game,
            keys: [Vec::new(), Vec::new()],
            keys_display: [Vec::new(), Vec::new()],
            serial: 0,
            steps: VecDeque::new(),
            objs: HashMap::new(),
            places: HashMap::new(),
            spell_show: HashMap::new(),
            popups: Vec::new(),
            log: VecDeque::new(),
            toasts: Vec::new(),
            banner: (String::new(), 0.0),
            ai_wait: 0.0,
            selected: Selected::None,
            hover_hand: None,
            hover_card: None,
            cursor: (0.0, 0.0),
            drag_right: false,
            look: (0.0, 0.0),
            lean: 0.0,
            show_help: true,
            end_button: (0.0, 0.0, 0.0, 0.0),
            press_at: None,
            dragging: false,
            sparks: Vec::new(),
            waves: Vec::new(),
            shakes: HashMap::new(),
            cam_shake: 0.0,
            fx_rng: 0x1234_5678,
            input: play::Input::new(true),
            prompts: duel_prompts(),
            kit: play::Kit::Keys,
            pad_mode: false,
            focus: Focus { row: 0, idx: 0 },
            target_i: 0,
            nav_cool: 0.0,
            showcase: 0,
            attack_shots: 0,
            last: Instant::now(),
            start: Instant::now(),
            frames: 0,
            shot_request: None,
            shot_i: 0,
        };
        app.reset_keys();
        app.announce_turn();
        app
    }

    fn reset_keys(&mut self) {
        for s in 0..2 {
            self.keys[s] = (0..self.game.sides[s].hand.len()).map(|_| self.next_serial()).collect();
        }
        self.keys_display = self.keys.clone();
        self.display = self.game.clone();
        self.objs.clear();
        // the opening hands are dealt from the decks
        for s in 0..2 {
            for &k in &self.keys[s] {
                let p = deck_zone(s) + Vec3::Y * 0.01;
                self.objs.insert(Key::Hand(k), Obj { pos: p, rot: lying(s, false), card: None, side: s, leaving: false, offset: Vec3::ZERO, tilt: Quat::IDENTITY });
            }
        }
    }

    fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    fn new_duel(&mut self) {
        self.args.seed = self.args.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 3;
        let (g, _) = Duel::new(self.decks.clone(), (self.args.seed % 2) as usize, self.args.seed);
        self.game = g;
        self.steps.clear();
        self.popups.clear();
        self.log.clear();
        self.selected = Selected::None;
        self.reset_keys();
        self.announce_turn();
    }

    fn name(&self, id: tcg::CardId) -> String {
        self.names.get(&id).cloned().unwrap_or_else(|| id.to_string())
    }

    fn announce_turn(&mut self) {
        let who = if self.game.active == 0 { "YOUR TURN" } else { "OPPONENT'S TURN" };
        self.banner = (who.into(), 1.3);
    }

    fn say(&mut self, s: String) {
        self.toasts.push((s.clone(), 0.0));
        if self.toasts.len() > 8 {
            self.toasts.remove(0);
        }
        self.log.push_back(s);
        while self.log.len() > 7 {
            self.log.pop_front();
        }
    }

    /// Apply an action to the game and queue its playback.
    fn perform(&mut self, action: Action) {
        let side = self.game.active;
        let hand_idx = match action {
            Action::Summon { card, .. } | Action::Ascend { card, .. } | Action::Cast { card, .. } | Action::SetTrap { card, .. } | Action::Equip { card, .. } => Some(card),
            _ => None,
        };
        let played = hand_idx.map(|i| self.game.sides[side].hand[i]);
        let events = match self.game.apply(action) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("rejected {action:?}: {e}");
                return;
            }
        };
        let removed = hand_idx.map(|i| self.keys[side].remove(i));
        let mut rekeys = Vec::new();
        let mut trap = None;
        let mut attack = None;
        for e in &events {
            match *e {
                Event::Summon { uid, card, .. } => {
                    if let Some(k) = removed {
                        rekeys.push((Key::Hand(k), Key::Unit(uid)));
                    }
                    let who = if side == 0 { "You summon" } else { "Opponent summons" };
                    self.say(format!("{who} {}", self.name(card)));
                }
                Event::Ascend { uid, card, from, .. } => {
                    if let Some(k) = removed {
                        rekeys.push((Key::Hand(k), Key::Unit(uid)));
                    }
                    self.say(format!("{} ascends into {}!", self.name(from), self.name(card)));
                    self.banner = (format!("ASCEND!  {}", self.name(card)), 1.4);
                }
                Event::Cast { card, .. } => {
                    if let Some(k) = removed {
                        rekeys.push((Key::Hand(k), Key::Spell(k)));
                        self.spell_show.insert(k, 0.0);
                    }
                    let who = if side == 0 { "You cast" } else { "Opponent casts" };
                    self.say(format!("{who} {}", self.name(card)));
                }
                Event::SetTrap { side: s, slot } => {
                    if let Some(k) = removed {
                        rekeys.push((Key::Hand(k), Key::Trap(s, slot)));
                    }
                    self.say(if s == 0 { "You set a card face down".into() } else { "Opponent sets a card face down".into() });
                }
                Event::Equip { side: s, zone, card } => {
                    if let (Some(k), Some(u)) = (removed, self.game.unit(s, zone)) {
                        rekeys.push((Key::Hand(k), Key::Relic(u.uid)));
                    }
                    self.say(format!("{} {}", if s == 0 { "You equip" } else { "Opponent equips" }, self.name(card)));
                }
                Event::Draw { side: s, .. } => {
                    let k = self.next_serial();
                    self.keys[s].push(k);
                }
                Event::Returned { side: s, uid, card, .. } => {
                    let k = self.next_serial();
                    self.keys[s].push(k);
                    rekeys.push((Key::Unit(uid), Key::Hand(k)));
                    self.say(format!("{} returns to the hand", self.name(card)));
                }
                Event::Attack { uid, target, .. } => attack = Some((uid, target)),
                Event::TrapSprung { side: s, slot, card } => {
                    trap = Some((s, slot, card));
                    self.say(format!("TRAP! {}", self.name(card)));
                }
                Event::KnockOut { card, .. } => self.say(format!("{} is knocked out", self.name(card))),
                Event::Win { side: w } => {
                    self.say(match w {
                        Some(0) => "You win!".into(),
                        Some(_) => "The opponent wins.".into(),
                        None => "A draw.".into(),
                    });
                }
                _ => {}
            }
        }
        let _ = played;
        // keep the key lists the same length as the hands (a full hand burns a returned card)
        for s in 0..2 {
            let n = self.game.sides[s].hand.len();
            self.keys[s].truncate(n);
            while self.keys[s].len() < n {
                let k = self.next_serial();
                self.keys[s].push(k);
            }
        }
        self.steps.push_back(Step { events, after: self.game.clone(), keys_after: self.keys.clone(), rekeys, trap, attack, t: 0.0, hit_done: false, switched: false });
    }

    /// The centre of the card `t` names, where it is right now (mid-lunge included).
    fn card_centre(&self, d: &Duel, t: Target) -> Vec3 {
        match t {
            Target::Unit { side, zone } => d.unit(side, zone).and_then(|u| self.objs.get(&Key::Unit(u.uid))).map(|o| o.pos + o.offset).unwrap_or_else(|| monster_zone(side, zone)),
            Target::Leader { side } => leader_zone(side),
        }
    }

    fn target_pos(&self, d: &Duel, t: Target) -> Vec3 {
        match t {
            Target::Leader { side } => leader_zone(side),
            Target::Unit { side, zone } => {
                let _ = d;
                monster_zone(side, zone)
            }
        }
    }

    /// Advance the playback queue.
    fn play_steps(&mut self, dt: f32) {
        let Some(mut st) = self.steps.pop_front() else { return };
        st.t += dt;
        // a sprung trap turns face up first
        if let Some((s, slot, card)) = st.trap {
            if st.t < st.trap_time() + 0.42 {
                if let Some(o) = self.objs.get_mut(&Key::Trap(s, slot)) {
                    o.card = Some(card);
                    let k = (st.t / 0.3).min(1.0);
                    o.offset = Vec3::Y * (0.03 * k);
                }
            }
        }
        // the attacker rears up, dashes at its target, strikes and settles back
        if let Some((uid, target)) = st.attack {
            let t0 = st.trap_time();
            let k = (st.t - t0) / ATTACK;
            if (0.0..1.0).contains(&k) {
                if let Some(o) = self.objs.get(&Key::Unit(uid)).copied() {
                    let to = self.target_pos(&self.display, target) - o.pos;
                    let flat = Vec3::new(to.x, 0.0, to.z).normalize_or_zero();
                    let (off, rear) = attack_curve(k, Vec3::new(to.x, 0.0, to.z), flat);
                    let axis = flat.cross(Vec3::Y).normalize_or_zero();
                    if let Some(o) = self.objs.get_mut(&Key::Unit(uid)) {
                        o.offset = off;
                        o.tilt = if axis == Vec3::ZERO { Quat::IDENTITY } else { Quat::from_axis_angle(axis, -rear) };
                    }
                }
            } else if let Some(o) = self.objs.get_mut(&Key::Unit(uid)) {
                o.offset = Vec3::ZERO;
                o.tilt = Quat::IDENTITY;
            }
        }
        if !st.switched && st.t >= st.switch_time() {
            st.switched = true;
            for (from, to) in &st.rekeys {
                if let Some(o) = self.objs.remove(from) {
                    self.objs.insert(*to, Obj { offset: Vec3::ZERO, ..o });
                }
            }
            self.display = st.after.clone();
            self.keys_display = st.keys_after.clone();
            for e in &st.events {
                match *e {
                    Event::Summon { side, zone, card, .. } => {
                        let a = self.db.try_get(card).map(|c| c.attribute).unwrap_or(Attribute::Flame);
                        let m = Attribute::ALL.iter().position(|x| *x == a).unwrap_or(0);
                        self.burst(monster_zone(side, zone) + Vec3::Y * 0.006, m, 14, 0.55);
                    }
                    Event::Ascend { side, zone, card, .. } => {
                        let a = self.db.try_get(card).map(|c| c.attribute).unwrap_or(Attribute::Flame);
                        let m = Attribute::ALL.iter().position(|x| *x == a).unwrap_or(0);
                        self.burst(monster_zone(side, zone) + Vec3::Y * 0.006, m, 40, 1.0);
                        self.burst(monster_zone(side, zone) + Vec3::Y * 0.006, 6, 20, 0.8);
                        self.cam_shake = self.cam_shake.max(0.2);
                    }
                    _ => {}
                }
            }
            for e in &st.events {
                if let Event::TurnStart { .. } = e {
                    self.announce_turn();
                }
            }
        }
        if !st.hit_done && st.t >= st.hit_time() {
            st.hit_done = true;
            let pre = if st.switched { &st.after } else { &self.display };
            let mut pops = Vec::new();
            // the colour of the blow: the attacker's type, else the spell's
            let blow = st
                .attack
                .and_then(|(uid, _)| (0..2).flat_map(|sd| self.display.sides[sd].units().map(|(_, u)| (u.uid, u.card.attribute)).collect::<Vec<_>>()).find(|(u, _)| *u == uid).map(|(_, a)| a))
                .or_else(|| st.events.iter().find_map(|e| if let Event::Cast { card, .. } = e { self.db.try_get(*card).map(|c| c.attribute) } else { None }))
                .unwrap_or(Attribute::Flame);
            let mut bursts: Vec<(Vec3, usize, usize)> = Vec::new();
            let mut shakes: Vec<(Key, f32)> = Vec::new();
            for e in &st.events {
                match *e {
                    Event::Damage { target, amount, super_hit } => {
                        let p = self.target_pos(pre, target) + Vec3::Y * 0.02;
                        // combat text, MMO style: your hits gold, hits on you red; a SUPER hit is a
                        // bigger number with its tag
                        let theirs = matches!(target, Target::Unit { side: 1, .. } | Target::Leader { side: 1 });
                        let col = if theirs { Rgba(255, 212, 64, 255) } else { Rgba(255, 64, 52, 255) };
                        let _ = p;
                        pops.push(Popup { pos: self.card_centre(pre, target), text: amount.to_string(), color: col, t: 0.0, big: super_hit, pill: true, above: false });
                        let ti = Attribute::ALL.iter().position(|a| *a == blow).unwrap_or(0);
                        bursts.push((p, ti, if super_hit { 16 } else { 9 }));
                        if super_hit {
                            bursts.push((p, 6, 8));
                        }
                        match target {
                            Target::Unit { side, zone } => {
                                if let Some(u) = pre.unit(side, zone) {
                                    shakes.push((Key::Unit(u.uid), 0.4));
                                }
                            }
                            Target::Leader { side } => shakes.push((Key::Leader(side), 0.45)),
                        }
                        self.cam_shake = self.cam_shake.max(if super_hit { 0.32 } else { 0.18 });
                    }
                    Event::Heal { target, amount } => pops.push(Popup { pos: self.card_centre(pre, target), text: format!("+{amount}"), color: Rgba(96, 240, 120, 255), t: 0.0, big: false, pill: true, above: false }),
                    Event::Buff { side, zone, atk, hp } => {
                        let t = match (atk, hp) {
                            (a, 0) if a > 0 => format!("+{a} ATK"),
                            (a, 0) => format!("{a} ATK"),
                            (0, h) => format!("+{h} HP"),
                            (a, h) => format!("+{a}/+{h}"),
                        };
                        pops.push(Popup { pos: self.card_centre(pre, Target::Unit { side, zone }), text: t, color: Rgba(120, 190, 255, 255), t: 0.0, big: false, pill: true, above: false });
                    }
                    Event::KnockOut { side, zone, .. } => pops.push(Popup { pos: monster_zone(side, zone) + Vec3::new(0.0, 0.07, 0.0), text: "KO!".into(), color: Rgba(255, 255, 255, 255), t: -0.18, big: true, pill: false, above: false }),
                    Event::AttackCancelled { side, zone } => pops.push(Popup { pos: monster_zone(side, zone) + Vec3::Y * 0.04, text: "BLOCKED".into(), color: Rgba(150, 220, 255, 255), t: 0.0, big: true, pill: false, above: false }),
                    Event::TrapSprung { side, slot, card } => {
                        pops.push(Popup { pos: trap_zone(side, slot) + Vec3::Y * 0.05, text: "TRAP!".into(), color: Rgba(255, 120, 220, 255), t: 0.0, big: true, pill: false, above: false });
                        self.banner = (format!("TRAP!  {}", self.name(card)), 1.4);
                    }
                    Event::Win { side } => {
                        self.banner = (
                            match side {
                                Some(0) => "YOU WIN!".into(),
                                Some(_) => "YOU LOSE".into(),
                                None => "DRAW".into(),
                            },
                            999.0,
                        );
                    }
                    _ => {}
                }
            }
            self.popups.extend(pops);
            for (p, m, n) in bursts {
                self.burst(p, m, n, 1.0);
            }
            for (k, t) in shakes {
                self.shakes.insert(k, t);
            }
        }
        if st.t < st.end_time() {
            self.steps.push_front(st);
        }
    }

    /// Move every card object towards its place.
    fn animate(&mut self, dt: f32) {
        layout(&self.display, &self.keys_display, if self.selected == Selected::None || matches!(self.selected, Selected::Hand(_)) { self.hover_hand.or(match self.selected {
            Selected::Hand(i) => Some(i),
            _ => None,
        }) } else { None }, &mut self.places);
        // a dragged hand card follows the cursor, held a little above the table
        if let (Selected::Hand(i), true) = (self.selected, self.dragging) {
            let snap = self.hovered_option(true).as_ref().and_then(App::action_spot);
            if let (Some(&key), Some(p)) = (self.keys_display[0].get(i), self.plane_point(0.045)) {
                if let Some(pl) = self.places.get_mut(&Key::Hand(key)) {
                    match snap {
                        // over a spot it can go: it settles just above it, ready to drop
                        Some(z) => {
                            pl.pos = z + Vec3::Y * 0.018;
                            pl.rot = lying(0, true);
                        }
                        None => {
                            pl.pos = p;
                            pl.rot = Quat::from_rotation_x(0.25) * lying(0, true);
                        }
                    }
                }
            }
        }
        let k = 1.0 - (-dt * 11.0).exp();
        // spells on show above the mat, then off to the discard pile
        let show_at = mat_origin() + Vec3::new(0.0, 0.16, 0.12);
        let show_rot = Quat::from_rotation_x(-0.6);
        for (s, t) in self.spell_show.iter_mut() {
            *t += dt;
            let _ = s;
        }
        let mut gone = Vec::new();
        for (key, o) in self.objs.iter_mut() {
            let target = match self.places.get(key) {
                Some(p) => {
                    o.card = p.card.or(if matches!(key, Key::Trap(..)) { o.card.filter(|_| o.offset.y > 0.0) } else { None });
                    o.leaving = false;
                    Some((p.pos, p.rot))
                }
                None => match key {
                    Key::Spell(k) if self.spell_show.get(k).is_some_and(|t| *t < 0.95) => Some((show_at, show_rot)),
                    _ => {
                        o.leaving = true;
                        None
                    }
                },
            };
            let (tp, tr) = match target {
                Some(t) => t,
                None => {
                    let d = discard_zone(o.side) + Vec3::Y * 0.01;
                    if o.pos.distance(d) < 0.004 {
                        gone.push(*key);
                    }
                    (d, lying(o.side, true))
                }
            };
            // an arc when travelling far
            let dist = o.pos.distance(tp);
            let arc = Vec3::Y * (dist * 0.35).min(0.07);
            o.pos += (tp + arc * (dist > 0.01) as i32 as f32 - o.pos) * k;
            if dist < 0.0015 {
                o.pos = tp;
            }
            o.rot = o.rot.slerp(tr, k).normalize();
        }
        for g in gone {
            self.objs.remove(&g);
            if let Key::Spell(k) = g {
                self.spell_show.remove(&k);
            }
        }
        // new objects: hand cards come off their deck, everything else appears in place
        let missing: Vec<(Key, Place)> = self.places.iter().filter(|(k, _)| !self.objs.contains_key(k)).map(|(k, p)| (*k, *p)).collect();
        for (key, p) in missing {
            let mut o = spawn_from(&p);
            if let Key::Hand(_) = key {
                let top = self.display.sides[p.side].deck.len();
                o.pos = deck_zone(p.side) + Vec3::Y * (0.002 + top as f32 * CARD_T);
                o.rot = lying(p.side, false);
            }
            self.objs.insert(key, o);
        }
        // offsets relax (hits, trap lifts)
        for o in self.objs.values_mut() {
            o.offset *= 1.0 - (dt * 6.0).min(1.0) * 0.5;
        }
        for p in self.popups.iter_mut() {
            p.t += dt;
        }
        self.popups.retain(|p| p.t < 1.3);
        if self.banner.1 > 0.0 {
            self.banner.1 -= dt;
        }
    }

    // ------------------------------------------------------------------ camera and picking

    fn eye(&self) -> (Vec3, Vec3) {
        let (e, f) = self.eye_steady();
        if self.cam_shake <= 0.0 {
            return (e, f);
        }
        let t = self.start.elapsed().as_secs_f32();
        let a = self.cam_shake.min(0.3);
        (e + Vec3::new((t * 61.0).sin(), (t * 47.0).cos(), (t * 53.0).sin()) * 0.006 * a, f)
    }

    fn eye_steady(&self) -> (Vec3, Vec3) {
        let base_eye = TABLE + Vec3::new(0.0, 0.5, 0.66);
        let look_at = TABLE + Vec3::new(0.0, 0.0, 0.035);
        let dir = (look_at - base_eye).normalize();
        let eye = base_eye + dir * self.lean;
        let yaw = Quat::from_rotation_y(self.look.0);
        let fwd = yaw * dir;
        let right = fwd.cross(Vec3::Y).normalize();
        let fwd = Quat::from_axis_angle(right, self.look.1) * fwd;
        (eye, fwd)
    }

    fn ray(&self, cx: f64, cy: f64) -> Option<(Vec3, Vec3)> {
        let win = self.window.as_ref()?;
        let sz = win.inner_size();
        let (w, h) = (sz.width.max(1) as f32, sz.height.max(1) as f32);
        let cam = self.camera();
        let fwd = cam.forward();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let th = (cam.fov_y * 0.5).tan();
        let nx = (cx as f32 / w) * 2.0 - 1.0;
        let ny = 1.0 - (cy as f32 / h) * 2.0;
        Some((cam.position, (fwd + right * nx * th * (w / h) + up * ny * th).normalize()))
    }

    fn camera(&self) -> Camera {
        let (eye, fwd) = self.eye();
        let mut c = Camera::default();
        c.fov_y = 42f32.to_radians();
        c.near = 0.02;
        c.position = eye;
        c.pitch = fwd.y.asin();
        c.yaw = fwd.x.atan2(-fwd.z);
        c
    }

    fn to_screen(&self, p: Vec3, w: u32, h: u32) -> Option<(f32, f32)> {
        let c = self.camera().view_proj(w as f32 / h as f32) * Vec4::new(p.x, p.y, p.z, 1.0);
        if c.w <= 0.0 {
            return None;
        }
        Some(((c.x / c.w * 0.5 + 0.5) * w as f32, (c.y / c.w * 0.5 + 0.5) * h as f32))
    }

    /// Which of your hand cards is under the cursor. Cards are picked by where they rest (so a
    /// card lifting on hover doesn't slip out from under the cursor and flicker), and the hovered
    /// card also by where it is lifted to.
    fn pick_hand(&self) -> Option<usize> {
        let (o, d) = self.ray(self.cursor.0, self.cursor.1)?;
        let n = self.keys_display[0].len();
        let hit = |pos: Vec3, rot: Quat| -> Option<f32> {
            let inv = Affine3A::from_rotation_translation(rot, pos).inverse();
            let (lo, ld) = (inv.transform_point3(o), inv.transform_vector3(d));
            if ld.z.abs() < 1e-6 {
                return None;
            }
            let t = -lo.z / ld.z;
            let p = lo + ld * t;
            (t > 0.0 && p.x.abs() < CARD_W * 0.5 && p.y.abs() < CARD_H * 0.5).then_some(t)
        };
        // the hovered card keeps the hover while the cursor is on it, resting or lifted
        if let Some(i) = self.hover_hand.filter(|&i| i < n) {
            let (rp, rr) = hand_place(0, i, n, false);
            let (lp, lr) = hand_place(0, i, n, true);
            if hit(rp, rr).is_some() || hit(lp, lr).is_some() {
                return Some(i);
            }
        }
        let mut best: Option<(f32, usize)> = None;
        for i in 0..n {
            let (p, r) = hand_place(0, i, n, false);
            if let Some(t) = hit(p, r) {
                if best.map_or(true, |b| t < b.0) {
                    best = Some((t, i));
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The board target under the cursor (mat zones).
    fn pick_board(&self) -> Option<Target> {
        let (o, d) = self.ray(self.cursor.0, self.cursor.1)?;
        let m = mat_origin();
        if d.y.abs() < 1e-6 {
            return None;
        }
        let t = (m.y - o.y) / d.y;
        if t <= 0.0 {
            return None;
        }
        let p = o + d * t;
        let hit = |c: Vec3| (p.x - c.x).abs() < ZONE_MM.0 * 0.0005 && (p.z - c.z).abs() < ZONE_MM.1 * 0.0005;
        for side in 0..2 {
            for z in 0..ZONES {
                if hit(monster_zone(side, z)) {
                    return Some(Target::Unit { side, zone: z });
                }
            }
            if hit(leader_zone(side)) {
                return Some(Target::Leader { side });
            }
        }
        None
    }

    fn pick_trap_slot(&self) -> Option<usize> {
        let (o, d) = self.ray(self.cursor.0, self.cursor.1)?;
        let m = mat_origin();
        let t = (m.y - o.y) / d.y;
        let p = o + d * t;
        (0..2).find(|&s| {
            let c = trap_zone(0, s);
            (p.x - c.x).abs() < ZONE_MM.0 * 0.0005 && (p.z - c.z).abs() < ZONE_MM.1 * 0.0005
        })
    }

    fn my_turn(&self) -> bool {
        !self.args.autoplay && self.game.active == 0 && self.steps.is_empty() && self.game.result.is_none()
    }

    /// Actions available for the current selection.
    fn options(&self) -> Vec<Action> {
        let legal = self.game.legal_actions();
        match self.selected {
            Selected::Hand(i) => legal
                .into_iter()
                .filter(|a| matches!(a, Action::Summon { card, .. } | Action::Ascend { card, .. } | Action::Cast { card, .. } | Action::SetTrap { card, .. } | Action::Equip { card, .. } if *card == i))
                .collect(),
            Selected::Unit(z) => legal.into_iter().filter(|a| matches!(a, Action::Attack { zone, .. } if *zone == z)).collect(),
            Selected::None => Vec::new(),
        }
    }

    fn rand(&mut self) -> f32 {
        self.fx_rng ^= self.fx_rng << 13;
        self.fx_rng ^= self.fx_rng >> 17;
        self.fx_rng ^= self.fx_rng << 5;
        (self.fx_rng & 0xFFFFFF) as f32 / 16_777_216.0
    }

    /// The light of a hit at `p` (`mesh`: a type colour, 6 white): a ring sweeping out over the
    /// mat, a flash round the zone, and `n` fine streaks thrown low and outwards. `power` scales
    /// the reach and speed.
    fn burst(&mut self, p: Vec3, mesh: usize, n: usize, power: f32) {
        let floor = Vec3::new(p.x, mat_origin().y + 0.0009, p.z);
        self.waves.push(Wave { pos: floor, t: 0.0, max: 0.5, reach: 0.075 * power, mesh, flash: false });
        // the zone flash sits on the nearest zone centre
        let zones: Vec<Vec3> = (0..2).flat_map(|s| (0..ZONES).map(move |z| monster_zone(s, z)).chain([leader_zone(s)])).collect();
        if let Some(z) = zones.iter().copied().min_by(|a, b| a.distance(floor).partial_cmp(&b.distance(floor)).unwrap()) {
            if z.distance(floor) < 0.06 {
                self.waves.push(Wave { pos: z + Vec3::Y * 0.0009, t: 0.0, max: 0.35, reach: 0.0, mesh, flash: true });
            }
        }
        for _ in 0..n {
            let a = self.rand() * std::f32::consts::TAU;
            let up = 0.08 + self.rand() * 0.3;
            let sp = (0.35 + self.rand() * 0.6) * power;
            let vel = Vec3::new(a.cos() * sp, up * power, a.sin() * sp);
            let max = 0.22 + self.rand() * 0.22;
            self.sparks.push(Spark { pos: p, vel, life: max, max, mesh });
        }
    }

    /// Is the hand card `i` playable right now (on your turn)?
    fn playable(&self, i: usize) -> bool {
        self.game.legal_actions().iter().any(|a| matches!(a, Action::Summon { card, .. } | Action::Ascend { card, .. } | Action::Cast { card, .. } | Action::SetTrap { card, .. } | Action::Equip { card, .. } if *card == i))
    }

    /// The action the pointer (or the controller's focus) would make now.
    fn hovered_option(&self, dragging: bool) -> Option<Action> {
        let opts: Vec<Action> = self.options();
        if self.pad_mode {
            return opts.get(self.target_i % opts.len().max(1)).copied();
        }
        let board = self.pick_board();
        let slot = self.pick_trap_slot();
        let hand = if dragging { None } else { self.pick_hand() };
        let on_mat = self.over_mat();
        opts.into_iter().find(|a| match *a {
            Action::Summon { zone, .. } | Action::Ascend { zone, .. } | Action::Equip { zone, .. } => board == Some(Target::Unit { side: 0, zone }),
            Action::Cast { target: Some(t), .. } => board == Some(t),
            Action::Cast { target: None, card } => {
                if dragging {
                    on_mat
                } else {
                    hand == Some(card) || board.is_some()
                }
            }
            Action::SetTrap { slot: s, .. } => slot == Some(s),
            Action::Attack { target, .. } => board == Some(target),
            _ => false,
        })
    }

    /// Where on the mat an action lands (for its glow and for a card snapping into place).
    fn action_spot(a: &Action) -> Option<Vec3> {
        match *a {
            Action::Summon { zone, .. } | Action::Ascend { zone, .. } | Action::Equip { zone, .. } => Some(monster_zone(0, zone)),
            Action::Cast { target: Some(Target::Unit { side, zone }), .. } | Action::Attack { target: Target::Unit { side, zone }, .. } => Some(monster_zone(side, zone)),
            Action::Cast { target: Some(Target::Leader { side }), .. } | Action::Attack { target: Target::Leader { side }, .. } => Some(leader_zone(side)),
            Action::SetTrap { slot, .. } => Some(trap_zone(0, slot)),
            _ => None,
        }
    }

    /// Controller input (crates/play): move the focus, pick up, play, back, end the turn.
    fn pad_input(&mut self, dt: f32) {
        let act = self.input.frame(dt, false);
        let pad = *self.input.pad();
        if act.device == play::input::Device::Gamepad {
            if !self.pad_mode {
                self.pad_mode = true;
                self.dragging = false;
                self.press_at = None;
            }
            self.kit = act.kit;
        }
        if !self.pad_mode {
            return;
        }
        // right stick looks around
        self.look.0 = (self.look.0 - pad.right.x * dt * 1.1).clamp(-0.6, 0.6);
        self.look.1 = (self.look.1 + pad.right.y * dt * 0.8).clamp(-0.35, 0.3);
        use play::input::Button as B;
        if self.game.result.is_some() {
            if pad.pressed(B::Options) || pad.pressed(B::Cross) {
                self.new_duel();
            }
            return;
        }
        if pad.pressed(B::Create) {
            self.show_help = !self.show_help;
        }
        if !self.my_turn() {
            return;
        }
        // left / right / up / down from the d-pad, or a flick of the left stick
        self.nav_cool -= dt;
        let mut dx = pad.pressed(B::Right) as i32 - pad.pressed(B::Left) as i32 + pad.pressed(B::R1) as i32 - pad.pressed(B::L1) as i32;
        let mut dy = pad.pressed(B::Up) as i32 - pad.pressed(B::Down) as i32;
        if self.nav_cool <= 0.0 && pad.left.length() > 0.6 {
            if pad.left.x.abs() > pad.left.y.abs() {
                dx = pad.left.x.signum() as i32;
            } else {
                dy = pad.left.y.signum() as i32;
            }
            self.nav_cool = 0.22;
        }
        if pad.left.length() < 0.3 {
            self.nav_cool = 0.0;
        }
        if pad.pressed(B::Triangle) {
            self.selected = Selected::None;
            self.perform(Action::EndTurn);
            return;
        }
        match self.selected {
            Selected::None => {
                let hand_n = self.game.sides[0].hand.len();
                let units: Vec<usize> = self.game.sides[0].units().map(|(z, _)| z).collect();
                if dy > 0 && !units.is_empty() {
                    self.focus = Focus { row: 1, idx: 0 };
                } else if dy < 0 {
                    self.focus = Focus { row: 0, idx: 0 };
                }
                let n = if self.focus.row == 0 { hand_n } else { units.len() };
                if n == 0 {
                    self.focus = Focus { row: if hand_n > 0 { 0 } else { 1 }, idx: 0 };
                    return;
                }
                self.focus.idx = ((self.focus.idx as i32 + dx).rem_euclid(n as i32)) as usize;
                if pad.pressed(B::Cross) {
                    self.target_i = 0;
                    self.selected = if self.focus.row == 0 { Selected::Hand(self.focus.idx) } else { Selected::Unit(units[self.focus.idx]) };
                }
            }
            _ => {
                let n = self.options().len();
                if n > 0 {
                    self.target_i = ((self.target_i as i32 + dx + dy).rem_euclid(n as i32)) as usize;
                }
                if pad.pressed(B::Circle) {
                    self.selected = Selected::None;
                } else if pad.pressed(B::Cross) && n > 0 {
                    let a = self.options()[self.target_i % n];
                    self.selected = Selected::None;
                    self.focus.idx = 0;
                    self.perform(a);
                }
            }
        }
    }

    /// The hand card the controller has its focus on.
    fn pad_hand_focus(&self) -> Option<usize> {
        match self.selected {
            Selected::Hand(i) => Some(i),
            Selected::None if self.focus.row == 0 => Some(self.focus.idx),
            _ => None,
        }
    }

    /// Where the cursor ray meets the mat's plane (raised by `lift`).
    fn plane_point(&self, lift: f32) -> Option<Vec3> {
        let (o, d) = self.ray(self.cursor.0, self.cursor.1)?;
        let y = mat_origin().y + lift;
        if d.y.abs() < 1e-6 {
            return None;
        }
        let t = (y - o.y) / d.y;
        (t > 0.0).then(|| o + d * t)
    }

    fn over_mat(&self) -> bool {
        self.plane_point(0.0).is_some_and(|p| {
            let m = mat_origin();
            (p.x - m.x).abs() < MAT_MM.0 * 0.0005 && (p.z - m.z).abs() < MAT_MM.1 * 0.0005
        })
    }

    /// Complete the selected card's or monster's action at the cursor. `dragging`: a drop (spells
    /// with no target need the mat, not the hand).
    fn try_complete(&mut self, dragging: bool) -> bool {
        match self.hovered_option(dragging) {
            Some(a) => {
                self.selected = Selected::None;
                self.perform(a);
                true
            }
            None => false,
        }
    }

    fn over_end_button(&self) -> bool {
        let (bx, by, bw, bh) = self.end_button;
        let (cx, cy) = (self.cursor.0 as f32, self.cursor.1 as f32);
        cx >= bx && cx <= bx + bw && cy >= by && cy <= by + bh
    }

    /// Left button down: pick up a hand card or a monster (drag), or finish a click-click play.
    fn press(&mut self) {
        if !self.my_turn() {
            return;
        }
        if self.over_end_button() {
            self.selected = Selected::None;
            self.perform(Action::EndTurn);
            return;
        }
        // finishing a click-to-select play comes first (e.g. equip onto a monster)
        if self.selected != Selected::None && self.try_complete(false) {
            return;
        }
        self.press_at = Some(self.cursor);
        self.dragging = false;
        if let Some(i) = self.pick_hand() {
            self.selected = Selected::Hand(i);
            return;
        }
        if let Some(Target::Unit { side: 0, zone }) = self.pick_board() {
            if self.game.unit(0, zone).is_some() {
                self.selected = Selected::Unit(zone);
                return;
            }
        }
        self.selected = Selected::None;
        self.press_at = None;
    }

    /// Left button up: drop what is dragged (a plain click keeps its selection).
    fn release(&mut self) {
        if self.press_at.take().is_some() && self.dragging && !self.try_complete(true) {
            // dropped somewhere it can't go: it goes back
            self.selected = Selected::None;
        }
        self.dragging = false;
    }

    fn draw_hud(&mut self, w: u32, h: u32) {
        use hud::*;
        let time = self.start.elapsed().as_secs_f32();
        let opts = if self.my_turn() { self.options() } else { Vec::new() };
        // the attack arrow while dragging a monster
        let arrow_ends = match (self.selected, self.dragging, self.pad_mode) {
            (Selected::Unit(z), true, false) => self.to_screen(monster_zone(0, z) + Vec3::Y * 0.01, w, h).map(|a| (a, (self.cursor.0 as f32, self.cursor.1 as f32))),
            (Selected::Unit(z), _, true) => {
                let tgt = self.hovered_option(false).as_ref().and_then(App::action_spot);
                match (self.to_screen(monster_zone(0, z) + Vec3::Y * 0.01, w, h), tgt.and_then(|t| self.to_screen(t, w, h))) {
                    (Some(a), Some(b)) => Some((a, b)),
                    _ => None,
                }
            }
            _ => None,
        };
        struct Chip {
            x: f32,
            y: f32,
            atk: i32,
            hp: i32,
            max: i32,
            tag: String,
            color: Rgba,
        }
        let mut chips = Vec::new();
        for side in 0..2 {
            for (z, u) in self.display.sides[side].units() {
                let edge = if side == 0 { 0.054 } else { -0.054 };
                if let Some((x, y)) = self.to_screen(monster_zone(side, z) + Vec3::new(0.0, 0.002, edge), w, h) {
                    let kw = [tcg::battle::Keyword::Guard, tcg::battle::Keyword::Flying, tcg::battle::Keyword::Pierce, tcg::battle::Keyword::Swift].into_iter().find(|k| u.has(*k)).map(|k| k.name().to_uppercase());
                    let mut tag = kw.unwrap_or_default();
                    if u.sick && !u.has(tcg::battle::Keyword::Swift) {
                        tag = if tag.is_empty() { "RESTING".into() } else { format!("{tag}  ·  RESTING") };
                    }
                    chips.push(Chip { x, y, atk: u.atk(), hp: u.hp(), max: u.max_hp(), tag, color: type_color(u.card.attribute) });
                }
            }
        }
        let pops: Vec<(Option<(f32, f32)>, String, Rgba, f32, bool, bool, bool)> = self
            .popups
            .iter()
            .filter(|p| p.t >= 0.0)
            .map(|p| (self.to_screen(if p.pill { p.pos } else { p.pos + Vec3::Y * (p.t * 0.03) }, w, h), p.text.clone(), p.color, p.t, p.big, p.pill, p.above))
            .collect();
        let (me, foe) = (&self.display.sides[0], &self.display.sides[1]);
        let me_counts = format!("Hand {}  ·  Deck {}  ·  Traps {}", me.hand.len(), me.deck.len(), self.display.trap_count(0));
        let foe_counts = format!("Hand {}  ·  Deck {}  ·  Traps {}", foe.hand.len(), foe.deck.len(), self.display.trap_count(1));
        let (me_hp, foe_hp, me_sp, foe_sp) = (me.leader, foe.leader, (me.spark, me.max_spark), (foe.spark, foe.max_spark));
        let active = self.display.active;
        let tip = self.hover_card.and_then(|id| self.tooltip(id));
        let toasts_v: Vec<(String, f32)> = self.toasts.clone();
        let banner = self.banner.clone();
        let my_turn = self.my_turn();
        let over_end = self.over_end_button();
        let thinking = !self.args.autoplay && self.game.active == 1 && self.game.result.is_none();
        let over = self.game.result.is_some() && self.steps.is_empty();
        let level = self.args.level.name();
        let deck_names = (self.args.deck.clone(), self.args.ai_deck.clone());
        let show_help = self.show_help;
        let dragging = self.dragging;
        let pad_kit = if self.pad_mode { Some(self.kit) } else { None };
        let prompts = self.prompts.clone();
        let picked = self.selected != Selected::None;
        let hint_sel = match self.selected {
            Selected::Hand(_) if opts.is_empty() => Some("Not enough Spark, or nowhere to play this card"),
            Selected::Unit(_) if opts.is_empty() => Some("This monster can't attack right now"),
            _ => None,
        };
        let Some(ui) = self.ui.as_mut() else { return };
        ui.begin(w, h);
        let s = ui.scale();
        let (fw, fh) = (w as f32, h as f32);

        for c in &chips {
            let cw = 104.0 * s;
            let (x, y) = (c.x - cw * 0.5, c.y - 15.0 * s);
            ui.rect(x + 3.0 * s, y + 4.0 * s, cw, 30.0 * s, Rgba(0, 0, 0, 70));
            ui.rect(x, y, cw, 30.0 * s, PANEL);
            ui.rect(x, y, cw, 3.0 * s, c.color);
            ui.text(x + 10.0 * s, y + 8.0 * s, 13.0 * s, DIM, "ATK");
            ui.text(x + 38.0 * s, y + 3.0 * s, 22.0 * s, GOLD, &c.atk.to_string());
            ui.text(x + 58.0 * s, y + 8.0 * s, 13.0 * s, DIM, "HP");
            ui.text(x + 80.0 * s, y + 3.0 * s, 22.0 * s, if c.hp < c.max { ACCENT } else { INK }, &c.hp.to_string());
            if !c.tag.is_empty() {
                let tw = ui.measure(13.0 * s, &c.tag) + 14.0 * s;
                ui.rect(c.x - tw * 0.5, y + 33.0 * s, tw, 20.0 * s, PANEL.alpha(0.8));
                ui.text_centered(c.x, y + 35.0 * s, 13.0 * s, DIM, &c.tag);
            }
        }
        if let Some((a, b)) = arrow_ends {
            arrow(ui, a, b, ACCENT);
        }
        for (pos, text, c, t, big, pill, above) in pops {
            let Some((x, y)) = pos else { continue };
            let a = (1.0 - (t - 0.8).max(0.0) * 2.0).clamp(0.0, 1.0);
            if pill {
                // floating combat text (as in WoW): born just under the stat tag with a scale punch,
                // then it floats up and drifts aside, fading at the end; a thick dark outline
                // keeps it readable on anything
                let life = 1.2;
                let k = (t / life).clamp(0.0, 1.0);
                let ease = 1.0 - (1.0 - k).powi(3);
                let drift = if text.len() % 2 == 0 { 1.0 } else { -1.0 } * (x as i32 % 3 - 1) as f32;
                let punch = 1.0 + if big { 1.2 } else { 0.7 } * (1.0 - (t / 0.14).min(1.0)).powi(2);
                let size = if big { 44.0 } else { 34.0 } * s * punch;
                let fade = (1.0 - ((t - 0.85) / 0.35).max(0.0)).clamp(0.0, 1.0);
                let tw = ui.measure(size, &text);
                let born = if above { -size * 1.1 } else { -size * 0.55 };
                let (tx, ty) = (x - tw * 0.5 + drift * 8.0 * s * ease, y + born - 64.0 * s * ease);
                let o = (if big { 3.2 } else { 2.6 }) * s;
                let dark = Rgba(16, 8, 6, (235.0 * fade) as u8);
                for i in 0..12 {
                    let a = i as f32 / 12.0 * std::f32::consts::TAU;
                    ui.text(tx + a.cos() * o, ty + a.sin() * o, size, dark, &text);
                }
                ui.text(tx, ty, size, c.alpha(fade), &text);
                if big {
                    let ls = 18.0 * s;
                    let lw = ui.measure(ls, "SUPER!");
                    for i in 0..8 {
                        let a = i as f32 / 8.0 * std::f32::consts::TAU;
                        ui.text(tx + tw * 0.5 - lw * 0.5 + a.cos() * 2.0 * s, ty - ls * 0.9 + a.sin() * 2.0 * s, ls, dark, "SUPER!");
                    }
                    ui.text(tx + tw * 0.5 - lw * 0.5, ty - ls * 0.9, ls, c.alpha(fade), "SUPER!");
                }
                continue;
            }
            let size = if big { 36.0 } else { 32.0 } * s * (1.0 + 0.3 * (1.0 - (t * 6.0).min(1.0)));
            let tw = ui.measure(size, &text);
            ui.text_shadow(x - tw * 0.5, y - size, size, c.alpha(a), &text);
        }
        // the two Leaders
        leader_panel(ui, 28.0 * s, 26.0 * s, "RIVAL", &format!("{} deck  ·  {level}", deck_names.1), foe_hp, LEADER_HP, foe_sp, &foe_counts, active == 1);
        leader_panel(ui, 28.0 * s, fh - 236.0 * s, "YOU", &format!("{} deck", deck_names.0), me_hp, LEADER_HP, me_sp, &me_counts, active == 0);
        toasts(ui, &toasts_v, fh - 250.0 * s);
        // end turn
        let (bw, bh) = (230.0 * s, 64.0 * s);
        let (bx, by) = (fw - bw - 28.0 * s, fh - bh - 92.0 * s);
        ui.rect(bx + 4.0 * s, by + 6.0 * s, bw, bh, Rgba(0, 0, 0, 70));
        let fill = if my_turn && over_end { Rgba(60, 44, 24, 240) } else { PANEL };
        ui.rect(bx, by, bw, bh, fill);
        ui.rect(bx, by, bw, 3.0 * s, if my_turn { GOLD } else { DIM.alpha(0.5) });
        let label = if thinking { "RIVAL'S TURN" } else { "END TURN" };
        let col = if my_turn { INK } else { DIM.alpha(0.6) };
        ui.text_shadow(bx + 20.0 * s, by + 18.0 * s, 26.0 * s, col, label);
        if my_turn {
            let gh = 32.0 * s;
            let gw = glyphs_width(ui, &prompts.glyphs("end", pad_kit.unwrap_or(play::Kit::Keys)), gh);
            draw_glyphs(ui, &prompts.glyphs("end", pad_kit.unwrap_or(play::Kit::Keys)), bx + bw - gw - 16.0 * s, by + 16.0 * s, gh);
        }
        self.end_button = (bx, by, bw, bh);
        // card details
        if let Some(lines) = tip {
            let tw = 420.0 * s;
            let mut th = 26.0 * s;
            for (l, big) in &lines {
                th += if *big { 34.0 } else { 24.0 } * s * (1.0 + (ui.measure(16.0 * s, l) / (tw - 36.0 * s)).floor());
            }
            let (tx, ty) = (fw - tw - 28.0 * s, 26.0 * s);
            panel(ui, tx, ty, tw, th, GOLD);
            let mut yy = ty + 14.0 * s;
            for (l, big) in &lines {
                let size = if *big { 26.0 } else { 16.0 } * s;
                let used = ui.text_wrapped(tx + 18.0 * s, yy, tw - 36.0 * s, size, if *big { INK } else { DIM }, l);
                yy += used.max(if *big { 34.0 } else { 24.0 } * s);
            }
        }
        if let Some(m) = hint_sel {
            let tw = ui.measure(20.0 * s, m) + 40.0 * s;
            panel(ui, fw * 0.5 - tw * 0.5, fh - 170.0 * s, tw, 44.0 * s, ACCENT);
            ui.text_centered(fw * 0.5, fh - 160.0 * s, 20.0 * s, INK, m);
        }
        // banners
        if banner.1 > 0.0 && !banner.0.is_empty() && !over {
            let a = banner.1.min(1.0);
            let size = 34.0 * s;
            let bwid = ui.measure(size, &banner.0) + 80.0 * s;
            let y = 40.0 * s;
            ui.rect(fw * 0.5 - bwid * 0.5, y, bwid, size + 28.0 * s, PANEL.alpha(a));
            ui.rect(fw * 0.5 - bwid * 0.5, y, bwid, 3.0 * s, GOLD.alpha(a));
            ui.text_centered(fw * 0.5, y + 12.0 * s, size, INK.alpha(a), &banner.0);
        }
        if over {
            ui.rect(0.0, 0.0, fw, fh, Rgba(6, 8, 14, 140));
            let (pw, ph) = (560.0 * s, 190.0 * s);
            panel(ui, fw * 0.5 - pw * 0.5, fh * 0.36, pw, ph, GOLD);
            ui.text_centered(fw * 0.5, fh * 0.36 + 30.0 * s, 64.0 * s, if me_hp > 0 { GOLD } else { INK }, &banner.0);
            ui.text_centered(fw * 0.5, fh * 0.36 + 120.0 * s, 20.0 * s, DIM, &format!("You {}  ·  Rival {}", me_hp.max(0), foe_hp.max(0)));
        }
        // the hints line: the buttons of whatever is in hand (crates/play prompts)
        let kit = pad_kit.unwrap_or(play::Kit::Keys);
        let items: Vec<(&str, &str)> = if over {
            vec![("new", "New duel"), ("quit", "Quit")]
        } else if dragging {
            vec![("pick", "Release on a lit spot"), ("back", "Cancel")]
        } else if my_turn && picked {
            vec![("choose", "Choose a spot"), ("play", "Play here"), ("back", "Put back")]
        } else if my_turn {
            let mut v = vec![("choose", "Choose"), ("row", "Hand / field"), ("pick", "Pick up"), ("end", "End turn"), ("look", "Look")];
            if show_help {
                v.push(("help", "Hide hints"));
            }
            v
        } else {
            vec![("look", "Look around"), ("new", "New duel")]
        };
        let items: Vec<(Vec<play::Glyph>, &str)> = items.into_iter().map(|(a, t)| (prompts.glyphs(a, kit), t)).filter(|(g, _)| !g.is_empty()).collect();
        hints_glyphs(ui, &items);

    }

    fn tooltip(&self, id: tcg::CardId) -> Option<Vec<(String, bool)>> {
        let def = self.db.try_get(id)?;
        let st = tcg::battle::stats(def);
        let mut v = vec![(def.name.clone(), true)];
        v.push((format!("{:?} {:?}  ·  {}  ·  cost {}", def.rarity, def.kind, def.attribute, st.cost), false));
        match def.kind {
            Kind::Monster => {
                v.push((format!("ATK {}   HP {}", st.atk, st.hp), false));
                if let Some(k) = st.keyword {
                    v.push((format!("{}: {}", k.name(), k.rule()), false));
                }
                match st.ascend {
                    tcg::battle::Ascend::May => v.push((format!("Can Ascend onto one of your monsters for {} Spark.", st.ascend_cost), false)),
                    tcg::battle::Ascend::Must => v.push((format!("Ascend only: play onto one of your monsters ({} Spark).", st.ascend_cost), false)),
                    _ => {}
                }
                let beats: Vec<String> = Attribute::ALL.iter().filter(|d| tcg::battle::beats(def.attribute, **d)).map(|d| d.to_string()).collect();
                v.push((format!("Strong against {}  (+2)", beats.join(", ")), false));
            }
            _ => v.push((st.effect.text(), false)),
        }
        if let Effect::Equip { .. } = st.effect {
            v.push(("Equip to one of your monsters.".into(), false));
        }
        Some(v)
    }

    // ------------------------------------------------------------------ frame

    fn frame(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let now = Instant::now();
        let dt = if self.args.autoplay { 1.0 / 60.0 } else { (now - self.last).as_secs_f32().min(0.05) };
        self.last = now;
        self.frames += 1;

        // AI turns (and both sides in autoplay)
        if self.args.autoplay && self.showcase > 0 {
            self.showcase -= 1;
            if self.showcase == 1 {
                self.shot_i += 1;
                self.shot_request = Some(self.root.join(format!("assets/cards/duel_test/duel_{:02}_pick.png", self.shot_i)));
            }
            if self.showcase == 0 {
                self.selected = Selected::None;
            }
        } else if self.args.autoplay && self.game.active == 0 && self.steps.is_empty() && self.game.result.is_none() && self.frames % 600 == 300 {
            // hold up a card (or a ready monster) so the glows can be checked
            let card = (0..self.game.sides[0].hand.len()).find(|&i| self.playable(i));
            let unit = (0..ZONES).find(|&z| !self.game.attack_targets(z).is_empty());
            self.selected = match (unit, card) {
                (Some(z), _) if self.frames % 1200 == 300 => Selected::Unit(z),
                (_, Some(i)) => Selected::Hand(i),
                (Some(z), None) => Selected::Unit(z),
                _ => Selected::None,
            };
            if self.selected != Selected::None {
                self.pad_mode = true;
                self.target_i = 0;
                self.showcase = 50;
            }
        }
        let ai_side = (self.args.autoplay && self.showcase == 0) || (!self.args.autoplay && self.game.active == 1);
        if ai_side && self.steps.is_empty() && self.game.result.is_none() {
            self.ai_wait += dt;
            if self.ai_wait > if self.args.autoplay { 0.35 } else { 0.6 } {
                self.ai_wait = 0.0;
                let lv = if self.args.autoplay && self.game.active == 0 { Level::Normal } else { self.args.level };
                let a = ai::choose(&self.game, lv, &mut self.ai_rng);
                self.perform(a);
            }
        }
        self.play_steps(dt);
        for t in self.toasts.iter_mut() {
            t.1 += dt;
        }
        self.pad_input(dt);
        if self.my_turn() {
            self.hover_hand = if self.pad_mode { self.pad_hand_focus() } else { self.pick_hand() };
        } else {
            self.hover_hand = None;
        }
        let board_focus = if self.pad_mode {
            match (self.selected, self.focus.row) {
                (Selected::None, 1) => self.game.sides[0].units().nth(self.focus.idx).map(|(z, _)| Target::Unit { side: 0, zone: z }),
                (Selected::None, _) => None,
                _ => self.hovered_option(false).and_then(|a| match a {
                    Action::Attack { target, .. } | Action::Cast { target: Some(target), .. } => Some(target),
                    _ => None,
                }),
            }
        } else {
            self.pick_board()
        };
        self.hover_card = self.hover_hand.and_then(|i| self.display.sides[0].hand.get(i).map(|c| c.id)).or_else(|| match board_focus {
            Some(Target::Unit { side, zone }) => self.display.unit(side, zone).map(|u| u.card.id),
            _ => None,
        });
        // sparks fly and fall; shakes die away
        for w in self.waves.iter_mut() {
            w.t += dt;
        }
        self.waves.retain(|w| w.t < w.max);
        for sp in self.sparks.iter_mut() {
            sp.vel += Vec3::new(0.0, -1.2, 0.0) * dt;
            sp.vel *= 1.0 - dt * 4.0;
            sp.pos += sp.vel * dt;
            sp.pos.y = sp.pos.y.max(mat_origin().y + 0.0018);
            sp.life -= dt;
        }
        self.sparks.retain(|s| s.life > 0.0);
        for t in self.shakes.values_mut() {
            *t -= dt;
        }
        self.shakes.retain(|_, t| *t > 0.0);
        self.cam_shake = (self.cam_shake - dt).max(0.0);
        self.animate(dt);

        if std::env::var("DUEL_FXTEST").is_ok() && self.args.autoplay {
            if self.frames == 90 {
                self.shot_request = Some(self.root.join("assets/cards/duel_test/fxtest.png"));
            }
            if self.frames > 92 {
                el.exit();
                return Ok(());
            }
        }
        // an attack caught mid-dash and at the impact (scripted test)
        if self.args.autoplay && self.attack_shots < 4 {
            if let Some(st) = self.steps.front() {
                if st.attack.is_some() {
                    let k = (st.t - st.trap_time()) / ATTACK;
                    let want = if self.attack_shots % 2 == 0 { 0.30 } else { 0.42 };
                    if (want..want + 0.025).contains(&k) && self.shot_request.is_none() {
                        self.attack_shots += 1;
                        self.shot_request = Some(self.root.join(format!("assets/cards/duel_test/attack_{}.png", self.attack_shots)));
                    }
                }
            }
        }
        // scripted test stills
        if self.args.autoplay {
            let f = self.frames;
            if f % 420 == 0 || (self.game.result.is_some() && self.steps.is_empty() && self.shot_i < 99) {
                self.shot_i += 1;
                self.shot_request = Some(self.root.join(format!("assets/cards/duel_test/duel_{:02}.png", self.shot_i)));
                if self.game.result.is_some() {
                    self.shot_i = 99;
                }
            }
            if self.shot_i >= 99 && self.shot_request.is_none() {
                el.exit();
                return Ok(());
            }
            if f > 60 * 60 * 8 {
                el.exit();
                return Ok(());
            }
        }

        // which of your hand cards are playable (the rest are greyed out on your turn)
        let my_turn = self.my_turn() || self.showcase > 0;
        let n_hand = self.keys_display[0].len();
        let playable: Vec<bool> = (0..n_hand).map(|i| my_turn && self.playable(i)).collect();
        let hand_index: HashMap<Key, usize> = self.keys_display[0].iter().enumerate().map(|(i, k)| (Key::Hand(*k), i)).collect();
        let hot = self.hovered_option(self.dragging);
        let opts = if my_turn { self.options() } else { Vec::new() };
        let mut jitter = Vec::new();
        for (k, t) in &self.shakes {
            let a = (self.frames as f32 * 2.3 + (k as *const Key as usize % 7) as f32).sin();
            let b = (self.frames as f32 * 3.1).cos();
            jitter.push((*k, Vec3::new(a, 0.0, b) * 0.004 * (t / 0.4).min(1.0)));
        }
        let sc = self.scene.as_ref().unwrap();
        let mut inst = sc.statics.clone();
        for (key, o) in &self.objs {
            let dim = my_turn && hand_index.get(key).is_some_and(|&i| !playable.get(i).copied().unwrap_or(true));
            let j = jitter.iter().find(|(k, _)| k == key).map(|(_, v)| *v).unwrap_or(Vec3::ZERO);
            card_instances(sc, o.card, dim, Affine3A::from_rotation_translation(o.tilt * o.rot, o.pos + o.offset + j), &mut inst);
        }
        let fx = &sc.fx;
        let ring_y = Vec3::Y * 0.0007;
        // a gold ring round each playable hand card, on the table
        if my_turn {
            for i in 0..n_hand {
                if playable[i] {
                    let (p, r) = hand_place(0, i, n_hand, false);
                    let focused = self.hover_hand == Some(i) || self.selected == Selected::Hand(i);
                    let at = Vec3::new(p.x, TABLE.y + 0.0006, p.z);
                    let t = Affine3A::from_rotation_translation(r * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), at);
                    inst.push(Instance { mesh: fx.card[if focused { 1 } else { 0 }], transform: t });
                }
            }
        }
        // where the picked-up card or monster can go: own spots green, enemies red, the one under
        // the pointer bright
        for a in &opts {
            // a spell with no target lights everything it will touch
            if let Action::Cast { target: None, card } = *a {
                let eff = self.game.sides[0].hand.get(card).map(|c| c.effect);
                let m = if Some(*a) == hot { fx.zone[2] } else if matches!(eff, Some(tcg::battle::Effect::Volley(_))) { fx.zone[1] } else { fx.zone[0] };
                let spots: Vec<Vec3> = match eff {
                    Some(tcg::battle::Effect::Volley(_)) => self.game.sides[1].units().map(|(z, _)| monster_zone(1, z)).collect(),
                    _ => vec![leader_zone(0)],
                };
                for z in spots {
                    inst.push(Instance { mesh: m, transform: Affine3A::from_translation(z + ring_y) });
                }
                continue;
            }
            let Some(z) = App::action_spot(a) else { continue };
            let enemy = matches!(a, Action::Attack { .. } | Action::Cast { target: Some(Target::Unit { side: 1, .. } | Target::Leader { side: 1 }), .. });
            let m = if Some(*a) == hot { fx.zone[2] } else if enemy { fx.zone[1] } else { fx.zone[0] };
            inst.push(Instance { mesh: m, transform: Affine3A::from_translation(z + ring_y) });
        }
        // monsters ready to attack glow softly when nothing is picked up
        if my_turn && self.selected == Selected::None {
            for zn in 0..ZONES {
                if !self.game.attack_targets(zn).is_empty() {
                    inst.push(Instance { mesh: fx.zone[3], transform: Affine3A::from_translation(monster_zone(0, zn) + ring_y) });
                }
            }
        }
        if let Selected::Unit(zn) = self.selected {
            inst.push(Instance { mesh: fx.zone[2], transform: Affine3A::from_translation(monster_zone(0, zn) + ring_y) });
        }
        if std::env::var("DUEL_FXTEST").is_ok() {
            for side in 0..2 {
                for zn in 0..ZONES {
                    inst.push(Instance { mesh: fx.zone[(side + zn) % 4], transform: Affine3A::from_translation(monster_zone(side, zn) + ring_y) });
                }
            }
        }
        let level = |k: f32| ((k.clamp(0.0, 1.0) * view::FX_LEVELS as f32).ceil() as usize).clamp(1, view::FX_LEVELS) - 1;
        for sp in &self.sparks {
            let k = sp.life / sp.max;
            // a streak along its flight, shortening as it slows
            let dir = sp.vel.normalize_or(Vec3::Z);
            let len = (0.35 + sp.vel.length() * 1.2).min(1.4) * (0.4 + 0.6 * k);
            let t = Affine3A::from_scale_rotation_translation(Vec3::new(1.0, 1.0, len), Quat::from_rotation_arc(Vec3::Z, dir), sp.pos);
            inst.push(Instance { mesh: fx.streak[sp.mesh][level(k * k)], transform: t });
        }
        for w in &self.waves {
            let k = w.t / w.max;
            let fade = 1.0 - k;
            if w.flash {
                inst.push(Instance { mesh: fx.flash[w.mesh][level(fade * fade)], transform: Affine3A::from_translation(w.pos) });
            } else {
                let r = 0.012 + w.reach * (1.0 - (1.0 - k).powi(3));
                let t = Affine3A::from_scale_rotation_translation(Vec3::new(2.0 * r, 1.0, 2.0 * r), Quat::IDENTITY, w.pos);
                inst.push(Instance { mesh: fx.wave[w.mesh][level(fade)], transform: t });
            }
        }
        let lights = sc.lights.clone();
        let (w, h) = self.r.as_ref().unwrap().extent();
        self.draw_hud(w, h);
        let cam = self.camera();
        let capture = self.shot_request.is_some();
        let desc = FrameDesc {
            camera: &cam,
            sun_dir: Vec3::new(-0.35, 0.55, 0.75).normalize(),
            sun_illuminance: 1.0,
            moon_dir: Vec3::new(0.3, 0.6, -0.7).normalize(),
            moon_illuminance: 0.0,
            stars: 0.0,
            altitude: 40.0,
            ground_albedo: 0.2,
            haze: 1.0,
            instances: &inst,
            lights: &lights,
            time: (now - self.start).as_secs_f32(),
            dt,
            exposure_ev: 0.0,
            debug: 0,
            capture,
            ui: self.ui.as_ref(),
        };
        let r = self.r.as_mut().unwrap();
        if let Some(px) = r.render(&desc)? {
            let path = self.shot_request.take().unwrap();
            save_png(&path, w, h, &px)?;
            println!("saved {}", path.display());
        }
        if let Some(win) = &self.window {
            win.request_redraw();
        }
        Ok(())
    }
}

fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let headless = self.args.autoplay;
        let attrs = Window::default_attributes().with_title("Mythbeasts Duel").with_inner_size(PhysicalSize::new(1920, 1080)).with_visible(!headless).with_active(!headless);
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let opts = RendererOptions { validation: false, vsync: !headless, dlss: if self.args.dlss { Some(0) } else { None }, render_scale: 1.0, bounces: 3 };
        let mut r = match Renderer::new(&window, opts) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("renderer: {e:#}");
                el.exit();
                return;
            }
        };
        r.set_object_motion(true);
        let cards: Vec<Stats> = self.decks.iter().flatten().copied().collect();
        match view::build(&mut r, &self.root, &cards) {
            Ok(s) => self.scene = Some(s),
            Err(e) => {
                eprintln!("scene: {e:#}");
                el.exit();
                return;
            }
        }
        self.ui = Ui::new().map_err(|e| eprintln!("ui: {e:#}")).ok();
        if let Some(ui) = self.ui.as_mut() {
            if let Err(e) = ui.load_images(&self.root.join("assets/ui/buttons")) {
                eprintln!("button drawings: {e:#}");
            }
        }
        self.r = Some(r);
        self.window = Some(window.clone());
        self.last = Instant::now();
        window.request_redraw();
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.args.autoplay && self.r.is_some() {
            if let Err(e) = self.frame(el) {
                eprintln!("frame: {e:#}");
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                if let Some(r) = &mut self.r {
                    r.resize(s.width, s.height);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Left if down => self.press(),
                    MouseButton::Left => self.release(),
                    MouseButton::Right => {
                        self.drag_right = down;
                        if down {
                            self.selected = Selected::None;
                            self.press_at = None;
                            self.dragging = false;
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (dx, dy) = ((position.x - self.cursor.0) as f32, (position.y - self.cursor.1) as f32);
                if self.drag_right {
                    self.look.0 = (self.look.0 - dx * 0.002).clamp(-0.6, 0.6);
                    self.look.1 = (self.look.1 - dy * 0.002).clamp(-0.35, 0.3);
                }
                if (position.x - self.cursor.0).abs() + (position.y - self.cursor.1).abs() > 2.0 {
                    self.pad_mode = false;
                }
                self.cursor = (position.x, position.y);
                if let Some((px, py)) = self.press_at {
                    if (position.x - px).abs() + (position.y - py).abs() > 6.0 {
                        self.dragging = true;
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    akari::winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    akari::winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 60.0,
                };
                self.lean = (self.lean + d * 0.04).clamp(-0.2, 0.45);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(k) = event.physical_key else { return };
                self.input.keyboard().key(k, event.state == ElementState::Pressed, event.repeat);
                if event.state == ElementState::Pressed {
                    self.pad_mode = false;
                }
                if event.state != ElementState::Pressed || event.repeat {
                    return;
                }
                match k {
                    KeyCode::Escape => {
                        if self.selected != Selected::None {
                            self.selected = Selected::None;
                        } else {
                            el.exit();
                        }
                    }
                    KeyCode::Space | KeyCode::KeyE => {
                        if self.my_turn() {
                            self.selected = Selected::None;
                            self.perform(Action::EndTurn);
                        }
                    }
                    KeyCode::KeyN => self.new_duel(),
                    KeyCode::KeyH => self.show_help = !self.show_help,
                    KeyCode::F12 => {
                        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                        self.shot_request = Some(self.root.join(format!("assets/cards/duel_test/shot_{stamp}.png")));
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                if self.r.is_some() && !self.args.autoplay {
                    if let Err(e) = self.frame(el) {
                        eprintln!("frame: {e:#}");
                        el.exit();
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let args = parse_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    // canonicalize() gives a \\?\ path on Windows, which NGX can't search for the DLSS DLL
    let root = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\").to_string());
    std::env::set_current_dir(&root)?;
    let event_loop = EventLoop::new().context("event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(args, root);
    event_loop.run_app(&mut app)?;
    drop(app.r.take());
    Ok(())
}
