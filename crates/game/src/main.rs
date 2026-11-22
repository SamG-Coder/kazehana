//! Kazehana: a 12-year-old's life in a small Japanese town, the uncle's card shop, and the monster
//! card game everyone at school plays. Ray traced with Akari.
//!
//! Controls: WASD move · Shift run · Ctrl crouch · Space jump · mouse look (click to capture,
//! Esc release) · E interact (people, counters, doors) · Tab binder · P people · C calendar ·
//! hold T to wait (time runs faster while you stand and wait; only sleep skips) · F12 screenshot. A PlayStation (or Xbox) controller walks and
//! looks with the sticks, L3 run, Circle crouch, Cross jump, Square interact (play crate).
//! In menus: arrows / WASD, E or Enter to confirm, Esc to go back.
//!
//! Command line (testing / screenshots): --date 2026-04-06 --time 07:30 --level home|path|school
//!   --pos x,y,z --yaw deg --pitch deg --seed N --packs N (start with N packs opened)
//!   --screen binder|people|calendar|pack|sell|shop|talk|lesson-<subject>|yearend --shot out.png --no-dlss

mod binder;
mod crowd;
mod paper;
mod draw;
mod game;
mod map;
mod npc;
mod story;
mod autoplay;
mod paint;
mod world;

use akari::glam::Vec3;
use akari::winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
use akari::{FrameDesc, Instance, Renderer, RendererOptions, Ui};
use game::{Game, Key, Mode};
use sim::clock::{self, GameTime};
use sim::{Date, GameState, Location};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tcg::CardDb;

#[derive(Default)]
struct Args {
    /// `--autoplay`: the first morning played through by itself, hidden, with screenshots
    /// (autoplay.rs)
    autoplay: bool,
    /// the story from its start: the first day, 6:45, in your room (`--story`; also a plain
    /// launch with no test arguments)
    story: bool,
    date: Option<Date>,
    time: Option<u32>,
    level: Option<Location>,
    pos: Option<Vec3>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    shot: Option<PathBuf>,
    screen: Option<String>,
    packs: u32,
    seed: u64,
    dlss: bool,
    /// Load authored levels that passed their automated checks but await visual review.
    review: bool,
    /// Draw this device's buttons (screenshots): keys, xbox, playstation.
    kit: Option<play::Kit>,
}

fn parse_args() -> Args {
    let mut a = Args { seed: 2026, dlss: true, ..Default::default() };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        let n = v.get(i + 1).cloned().unwrap_or_default();
        let mut take = true;
        match v[i].as_str() {
            "--date" => {
                let p: Vec<i32> = n.split('-').filter_map(|x| x.parse().ok()).collect();
                if p.len() == 3 {
                    a.date = Some(Date::new(p[0], p[1] as u32, p[2] as u32));
                }
            }
            "--time" => {
                let p: Vec<u32> = n.split(':').filter_map(|x| x.parse().ok()).collect();
                a.time = Some(p.first().copied().unwrap_or(7) * 60 + p.get(1).copied().unwrap_or(0));
            }
            "--level" => a.level = match n.as_str() { "home" => Some(Location::Home), "path" => Some(Location::Path), "school" => Some(Location::School), _ => None },
            "--pos" => {
                let p: Vec<f32> = n.split(',').filter_map(|x| x.parse().ok()).collect();
                if p.len() == 3 {
                    a.pos = Some(Vec3::new(p[0], p[1], p[2]));
                }
            }
            "--yaw" => a.yaw = n.parse::<f32>().ok().map(f32::to_radians),
            "--pitch" => a.pitch = n.parse::<f32>().ok().map(f32::to_radians),
            "--shot" => a.shot = Some(PathBuf::from(n)),
            "--screen" => a.screen = Some(n),
            "--packs" => a.packs = n.parse().unwrap_or(0),
            "--kit" => a.kit = match n.as_str() { "xbox" => Some(play::Kit::Xbox), "playstation" | "ps" => Some(play::Kit::PlayStation), _ => Some(play::Kit::Keys) },
            "--seed" => a.seed = n.parse().unwrap_or(2026),
            "--story" => {
                a.story = true;
                take = false;
            }
            "--autoplay" => {
                a.autoplay = true;
                a.story = true;
                // (headless, as a screenshot run: no input, time held, a hidden window)
                a.shot = Some(PathBuf::from("shots/slice/autoplay.png"));
                take = false;
            }
            "--review" => {
                a.review = true;
                take = false;
            }
            "--no-dlss" => {
                a.dlss = false;
                take = false;
            }
            _ => take = false,
        }
        i += if take { 2 } else { 1 };
    }
    // (a plain launch: the story)
    if v.is_empty() {
        a.story = true;
    }
    a
}

/// Sun and moon for the renderer, from the town's real sky (sim::clock).
fn sky_frame(t: &GameTime, extra_minutes: f32) -> (Vec3, f32, Vec3, f32, f32) {
    let s = clock::sky_at(t, extra_minutes);
    let sun = Vec3::from(s.sun_dir).normalize();
    let moon = Vec3::from(s.moon_dir).normalize();
    let sun_i = if s.sun_elevation.to_degrees() > -18.0 { 1.0 } else { 0.0 };
    let moon_up = ((s.moon_elevation.to_degrees() + 1.0) / 6.0).clamp(0.0, 1.0);
    let moon_i = 2.5e-6 * s.moon_light * moon_up;
    let stars = (1.0 - ((s.sun_elevation.to_degrees() + 4.0) / 8.0).clamp(0.0, 1.0)).powi(2);
    (sun, sun_i, moon, moon_i, stars)
}

struct App {
    args: Args,
    game: Game,
    window: Option<Arc<Window>>,
    r: Option<Renderer>,
    ui: Option<Ui>,
    /// the town (town::scene): loaded with the renderer
    town: Option<town::Town>,
    world: Option<town::scene::TownWorld>,
    /// the calendar on the shop's wall and the journal in your hands (paper.rs)
    paper: Option<paper::Paper>,
    /// the townspeople drawn near you, from the schedule (crowd.rs)
    crowd: crowd::Crowd,
    /// this frame's instances
    instances: Vec<Instance>,
    keys: HashSet<KeyCode>,
    captured: bool,
    last: Instant,
    start: Instant,
    frames: u32,
    /// --autoplay: its driver, and the screenshot it asked for (name, the frame to take it)
    auto: Option<autoplay::Autoplay>,
    auto_shot: Option<(String, u32)>,
    shot: Option<PathBuf>,
    /// keyboard, mouse and gamepads (play crate)
    input: play::Input,
}

impl App {
    /// Load the town and put the player in Tsukumodō (or where --pos says).
    fn upload_world(&mut self) -> anyhow::Result<()> {
        let r = self.r.as_mut().unwrap();
        let town = town::Town::load(std::path::Path::new("assets/town"))?;
        let (world, playable) = town::scene::TownWorld::load(r, &town, true)?;
        self.game.world = Some(playable);
        self.game.stock = world.stock.clone();
        self.game.in_town = true;
        self.game.your_desk = world.your_desk;
        self.game.spots = world.spots.clone();
        self.game.state.location = Location::Home;
        let (feet, yaw) = town::scene::home_start(&town);
        self.game.start = (feet, yaw);
        if self.args.pos.is_none() {
            self.game.place_player(feet, self.args.yaw.unwrap_or(yaw), self.args.pitch.unwrap_or(0.0));
        }
        // the story: its places in the town, the map; a new story starts you in your room
        self.game.setup_story(&town, world.home);
        if self.args.story && self.args.pos.is_none() {
            self.game.wake_in_room();
            self.game.start = (self.game.player.feet, self.game.player.yaw);
        }
        let paper = paper::Paper::load(r, world.home);
        self.game.calendar_at = paper.calendar_at();
        if let Some(c) = self.game.calendar_at {
            let f = paper.calendar_facing().unwrap_or(Vec3::Z);
            println!("the calendar hangs at {:.2} {:.2} {:.2}, facing {:.2} {:.2} {:.2}", c.x, c.y, c.z, f.x, f.y, f.z);
        }
        self.paper = Some(paper);
        self.town = Some(town);
        self.world = Some(world);
        Ok(())
    }

    fn set_capture(&mut self, on: bool) {
        let Some(w) = &self.window else { return };
        if on {
            let _ = w.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| w.set_cursor_grab(CursorGrabMode::Locked));
        } else {
            let _ = w.set_cursor_grab(CursorGrabMode::None);
        }
        w.set_cursor_visible(!on);
        self.captured = on;
    }

    fn frame(&mut self, el: &ActiveEventLoop) -> anyhow::Result<()> {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let exploring = matches!(self.game.mode, Mode::Explore);
        if self.captured && matches!(self.game.mode, Mode::Binder { .. }) {
            self.set_capture(false); // (the binder is used with the mouse pointer)
        }
        self.input.keyboard().mouse_look = self.captured && exploring;
        // (in menus the buttons are the screens': the body is asked nothing, and Circle backing
        // out of a menu does not also crouch)
        let mut act = self.input.frame(dt, exploring && self.args.shot.is_none());
        if self.args.shot.is_some() {
            act = play::Actions::default();
        }
        // the controller: Square uses what is in front (E reaches the game as a key); on the
        // screens the d-pad chooses, Cross confirms, Circle goes back; Triangle, Create and
        // Options open the binder, people and calendar; R2 held runs time on
        let pad = *self.input.pad();
        use play::input::Button as B;
        if act.interact && act.device == play::input::Device::Gamepad {
            self.game.key(Key::Confirm, false);
        }
        let more = pad.held(B::L1);
        if self.args.shot.is_none() {
            if exploring {
                for (b, k) in [(B::Triangle, Key::Binder), (B::Create, Key::People), (B::Options, Key::Calendar)] {
                    if pad.pressed(b) {
                        self.game.key(k, false);
                    }
                }
            } else if !self.game.binder_open() {
                // (the binder reads the pad itself: binder_pad)
                for (b, k) in [(B::Up, Key::Up), (B::Down, Key::Down), (B::Left, Key::Left), (B::Right, Key::Right), (B::Cross, Key::Confirm), (B::Circle, Key::Back)] {
                    if pad.pressed(b) {
                        self.game.key(k, more);
                    }
                }
            }
        }
        if self.args.shot.is_none() {
            self.game.binder_pad(&pad, dt);
        }
        if self.args.kit.is_none() {
            self.game.kit = self.input.kit();
        }
        // KZ_AUTOWALK=1 with --shot: the player walks and looks round for 600 frames before the
        // shot (a test of playing in a place: frame times with KZ_PROFILE=1)
        let autowalk = self.args.shot.is_some() && std::env::var_os("KZ_AUTOWALK").is_some();
        if autowalk {
            let ph = self.frames as f32 / 60.0;
            act.movement = akari::glam::Vec2::new((ph * 0.9).sin() * 0.8, (ph * 0.6).cos() * 0.8);
            act.look = akari::glam::Vec2::new((ph * 2.1).sin() * 0.03, (ph * 3.3).cos() * 0.025);
        }
        let frozen = self.args.shot.is_some() && !autowalk;
        let fast = self.keys.contains(&KeyCode::KeyT) || pad.r2 > 0.5;
        let tp0 = Instant::now();
        self.game.update(if frozen { 0.0 } else { dt }, &act, fast);
        // in your seat at school, the camera is your eyes there
        self.game.seat_camera();
        let t_update = tp0.elapsed().as_secs_f64() * 1e3;

        let (sun, sun_i, moon, moon_i, stars) = sky_frame(&self.game.state.time, self.game.minute_fraction());
        let r = self.r.as_mut().unwrap();
        // the town round the camera: its plants listed again as it moves, its doors as they are
        let tp1 = Instant::now();
        if let (Some(w), Some(t)) = (self.world.as_mut(), self.town.as_ref()) {
            // the cloth (the noren) with the player in it
            w.cloth.step(if frozen { 0.0 } else { dt }, self.game.cam.position, &[town::cloth::person(&self.game.player)]);
            // Tsukumodō's sign: 営業中 in opening hours
            w.shop_open = (clock::at::SHOP_OPEN..clock::at::SHOP_CLOSE).contains(&self.game.state.time.minute);
            w.frame(r, t, self.game.cam.position, self.game.world.as_ref(), self.start.elapsed().as_secs_f32(), &mut self.instances)?;
        }
        let t_town = tp1.elapsed().as_secs_f64() * 1e3;
        // the calendar on the wall; the journal open in your hands (it takes the camera's pitch)
        if let Some(p) = self.paper.as_mut() {
            let held = if let Mode::Journal { spread } = self.game.mode {
                let (at, pitch) = paper::hold(self.game.cam.position, self.game.cam.yaw);
                self.game.cam.pitch = pitch;
                Some((spread, at))
            } else {
                None
            };
            // your paper on your desk at school
            let desk = self.game.desk_paper();
            p.frame(r, &self.game.state, self.game.calendar_offset, held, desk.as_ref(), &mut self.instances)?;
        }
        // the people the schedule puts near you
        self.crowd.frame(r, &self.game, if frozen { 0.0 } else { dt }, &mut self.instances)?;
        // the binder held open in front of you (it takes the camera)
        self.game.binder_frame(r, &mut self.instances, dt)?;
        if self.game.cut {
            r.reset_history();
            self.game.cut = false;
        }
        let (w, h) = r.extent();
        let ui = self.ui.as_mut().unwrap();
        let tp2 = Instant::now();
        ui.begin(w, h);
        self.game.draw(ui);
        let t_ui = tp2.elapsed().as_secs_f64() * 1e3;
        let tp3 = Instant::now();
        let shot_frame = if std::env::var_os("KZ_AUTOWALK").is_some() { 600 } else { 60 };
        if self.args.autoplay && self.frames > 30 {
            let a = self.auto.get_or_insert_with(autoplay::Autoplay::new);
            if let Some(name) = a.frame(&mut self.game) {
                self.auto_shot = Some((name, self.frames + 24));
                self.game.cut = true;
            }
            if a.done {
                el.exit();
            }
            if let Some((name, at)) = self.auto_shot.clone() {
                if self.frames == at {
                    self.shot = Some(PathBuf::from(format!("shots/slice/auto_{name}.png")));
                    self.auto_shot = None;
                }
            }
        }
        let capture = self.shot.is_some() || (self.args.shot.is_some() && !self.args.autoplay && self.frames == shot_frame);
        let desc = FrameDesc {
            camera: &self.game.cam,
            sun_dir: sun,
            sun_illuminance: sun_i,
            moon_dir: moon,
            moon_illuminance: moon_i,
            stars,
            altitude: self.game.cam.position.y + 20.0,
            ground_albedo: 0.12,
            haze: 1.0,
            instances: &self.instances,
            lights: self.world.as_ref().map_or(&[][..], |w| &w.lights[..]),
            time: self.start.elapsed().as_secs_f32(),
            dt,
            exposure_ev: 0.0,
            debug: 0,
            capture,
            ui: Some(ui),
        };
        if let Some(px) = r.render(&desc)? {
            let path = self.shot.take().or_else(|| self.args.shot.clone()).unwrap();
            save_png(&path, w, h, &px)?;
            println!("saved {}", path.display());
            if self.args.shot.is_some() && !self.args.autoplay {
                el.exit();
            }
        }
        self.frames += 1;
        if std::env::var_os("KZ_PROFILE").is_some() {
            // (per-frame CPU time of the game's own work vs the whole frame: KZ_PROFILE=1)
            eprintln!("frame {} total {:.2} ms  update {t_update:.2} town {t_town:.2} ui {t_ui:.2} render {:.2}", self.frames, now.elapsed().as_secs_f64() * 1e3, tp3.elapsed().as_secs_f64() * 1e3);
        }
        let t = self.game.state.time;
        if let Some(win) = &self.window {
            win.set_title(&format!("Kazehana — {} {:02}:{:02}", t.date, t.minute / 60, t.minute % 60));
            win.request_redraw();
        }
        Ok(())
    }

    fn key(&mut self, k: KeyCode, el: &ActiveEventLoop) {
        if self.game.binder_key(k) {
            return;
        }
        let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
        let exploring = matches!(self.game.mode, Mode::Explore);
        let key = match k {
            KeyCode::ArrowUp => Some(Key::Up),
            KeyCode::ArrowDown => Some(Key::Down),
            KeyCode::ArrowLeft => Some(Key::Left),
            KeyCode::ArrowRight => Some(Key::Right),
            KeyCode::KeyW if !exploring => Some(Key::Up),
            KeyCode::KeyS if !exploring => Some(Key::Down),
            KeyCode::KeyA if !exploring => Some(Key::Left),
            KeyCode::KeyD if !exploring => Some(Key::Right),
            KeyCode::KeyE | KeyCode::Enter => Some(Key::Confirm),
            KeyCode::Space if !exploring => Some(Key::Confirm), // (exploring: jump)
            KeyCode::Tab => Some(Key::Binder),
            KeyCode::KeyP => Some(Key::People),
            KeyCode::KeyC => Some(Key::Calendar),
            KeyCode::KeyM => Some(Key::Map),
            KeyCode::KeyJ => Some(Key::Quests),
            KeyCode::Escape | KeyCode::Backspace => {
                if exploring {
                    if self.captured { self.set_capture(false) } else { el.exit() }
                    None
                } else {
                    Some(Key::Back)
                }
            }
            KeyCode::F12 => {
                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                self.shot = Some(PathBuf::from(format!("shots/kazehana_{stamp}.png")));
                None
            }
            _ => None,
        };
        if let Some(key) = key {
            self.game.key(key, shift);
        }
    }

    /// --screen: open a screen straight away (screenshots and tests).
    fn open_screen(&mut self) {
        let Some(s) = self.args.screen.clone() else { return };
        let g = &mut self.game;
        match s.as_str() {
            s if s.starts_with("binder") => g.binder_screen(s),
            s if s.starts_with("lesson") => g.lesson_screen(s),
            "yearend" => {
                g.state.time = sim::GameTime::new(sim::Date::new(2027, 3, 24), clock::at::hm(10, 30));
                g.state.location = Location::School;
                g.update(0.0, &play::Actions::default(), false);
            }
            "people" => g.mode = Mode::Journal { spread: crate::paper::Spread::People },
            "quests" => g.mode = Mode::Journal { spread: crate::paper::Spread::Quests },
            "map" => g.mode = g.open_map(),
            "locker" => g.mode = Mode::Locker,
            "calendar" | "journal" => g.mode = Mode::Journal { spread: crate::paper::Spread::Plans },
            "pack" => {
                g.state.location = Location::Home;
                g.state.player.money += 300;
                if let Ok(p) = g.state.buy_pack(&g.db) {
                    g.mode = Mode::Pack { pulls: p, revealed: 4 };
                }
            }
            "sell" => {
                let owned = g.owned();
                if let Some((pull, _)) = owned.first() {
                    let i = g.state.students.iter().position(|s| s.plays_cards).unwrap_or(0);
                    let value = g.state.market.value(g.db.get(pull.id), pull.foil, g.state.season);
                    g.mode = Mode::Sell { student: i, pull: *pull, price: value + 40, value, result: None, counter: None, done: false };
                }
            }
            "talk" => {
                let i = g.state.students.iter().position(|s| s.plays_cards && s.grade == 1).unwrap_or(0);
                g.key(Key::Confirm, false); // (no-op unless facing someone)
                g.mode = Mode::Dialogue { speaker: game::describe(&g.state, i), lines: vec!["\"Hey — you're the new kid from the card shop, right? Is it true your uncle gets the new season early?\"".into()], line: 0 };
            }
            _ => {}
        }
    }
}

fn save_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> anyhow::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

impl App {
    fn headless_frame(&mut self, el: &ActiveEventLoop) {
        if let Err(e) = self.frame(el) {
            eprintln!("frame: {e:#}");
            el.exit();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let headless = self.args.shot.is_some();
        // automated runs stay invisible and never take focus
        let attrs = Window::default_attributes().with_title("Kazehana").with_inner_size(PhysicalSize::new(1920, 1080)).with_visible(!headless).with_active(!headless);
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let opts = RendererOptions { dlss: if self.args.dlss { Some(1) } else { None }, vsync: self.args.shot.is_none(), ..Default::default() };
        match Renderer::new(&window, opts) {
            Ok(r) => self.r = Some(r),
            Err(e) => {
                eprintln!("renderer: {e:#}");
                el.exit();
                return;
            }
        }
        match Ui::new() {
            Ok(mut u) => {
                // the controller and mouse buttons the hints draw (tools/buttons)
                if let Err(e) = u.load_images(std::path::Path::new("assets/ui/buttons")) {
                    eprintln!("buttons: {e:#}");
                }
                self.ui = Some(u)
            }
            Err(e) => {
                eprintln!("ui: {e:#}");
                el.exit();
                return;
            }
        }
        if let Err(e) = self.upload_world() {
            eprintln!("world: {e:#}");
            el.exit();
            return;
        }
        self.open_screen();
        self.window = Some(window.clone());
        self.last = Instant::now();
        window.request_redraw();
    }

    /// Headless runs: a hidden window gets no redraw events, so frames are driven from here.
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.args.shot.is_some() && self.r.is_some() {
            self.headless_frame(el);
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
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.input.keyboard().release_all();
                self.set_capture(false);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed && button == MouseButton::Left && !self.captured && matches!(self.game.mode, Mode::Explore) {
                    self.set_capture(true);
                } else if matches!(button, MouseButton::Left | MouseButton::Right) {
                    self.game.binder_button(button == MouseButton::Left, state == ElementState::Pressed);
                }
            }
            WindowEvent::CursorMoved { position, .. } => self.game.binder_pointer(position.x as f32, position.y as f32),
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    akari::winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    akari::winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.game.binder_wheel(d);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if !event.repeat || matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::ArrowUp | KeyCode::ArrowDown) {
                                self.key(code, el);
                            }
                            self.keys.insert(code);
                        }
                        ElementState::Released => {
                            self.keys.remove(&code);
                        }
                    }
                    self.input.keyboard().key(code, event.state == ElementState::Pressed, event.repeat);
                }
            }
            WindowEvent::RedrawRequested if self.r.is_some() => {
                if let Err(e) = self.frame(el) {
                    eprintln!("frame: {e:#}");
                    el.exit();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.input.keyboard().mouse_motion(delta.0, delta.1);
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = parse_args();
    let db = CardDb::load_or_generate(std::path::Path::new("assets/cards/sets"), 12);
    // every student lives with their family in a house on the town plan (tools/blender/town_plan.py)
    let houses = sim::town::load_houses(std::path::Path::new("assets/town/town.json")).unwrap_or_else(|e| {
        eprintln!("town plan not found ({e}); using a stand-in town");
        sim::town::fallback_houses()
    });
    let mut state = if args.story { GameState::new_story(args.seed, &db) } else { GameState::with_houses(args.seed, &db, houses) };
    if let Some(d) = args.date {
        state.time.date = d;
        state.season = sim::calendar::released_season(d);
    }
    if let Some(t) = args.time {
        state.time.minute = t;
    }
    if let Some(l) = args.level {
        state.location = l;
    }
    if args.packs > 0 {
        let loc = state.location;
        state.location = Location::Home;
        state.player.money += args.packs * 300;
        for _ in 0..args.packs {
            let _ = state.buy_pack(&db);
        }
        state.location = loc;
        // and get to know a few classmates for the people screen
        for i in (0..state.students.len()).filter(|&i| state.students[i].grade == 1).take(6) {
            state.bonds[i].add(30 + i as i32 * 9);
        }
    }
    let levels = [world::build_home(), world::build_path(), world::build_school()];
    let mut game = Game::new(db, state, levels);
    // the story's messages (Uncle Kenji, the quests) are shown in the story
    game.story = args.story;
    game.binder.review = args.review;
    if args.pos.is_some() || args.yaw.is_some() || args.pitch.is_some() {
        // (--pos is where the eye is)
        let feet = args.pos.map_or(game.player.feet, |p| p - Vec3::Y * game::EYE);
        let (yaw, pitch) = (args.yaw.unwrap_or(game.player.yaw), args.pitch.unwrap_or(0.0));
        game.place_player(feet, yaw, pitch);
    }
    // (C opens the calendar: crouch is Ctrl only)
    if let Some(k) = args.kit {
        game.kit = k;
    }
    let mut input = play::Input::new(args.shot.is_none());
    input.keyboard().bindings.crouch = vec![KeyCode::ControlLeft, KeyCode::ControlRight];
    let el = EventLoop::new()?;
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        args,
        game,
        window: None,
        r: None,
        ui: None,
        town: None,
        world: None,
        paper: None,
        crowd: crowd::Crowd::new(),
        instances: Vec::new(),
        keys: HashSet::new(),
        captured: false,
        last: Instant::now(),
        start: Instant::now(),
        frames: 0,
        auto: None,
        auto_shot: None,
        shot: None,
        input,
    };
    el.run_app(&mut app)?;
    drop(app.r.take());
    Ok(())
}
