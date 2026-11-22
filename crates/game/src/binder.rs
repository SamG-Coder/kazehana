//! The physical binder: a 4-ring binder of 9-pocket sleeve sheets, held open in front of the
//! player. Cards are picked up by hand from the loose cards fanned below it and slid into a
//! pocket from the top; a sleeved card can be pulled out again. Pages turn on the rings.
//!
//! Hands (all three work the same binder, and can be mixed):
//! - mouse: click a card to pick it up and click a pocket to slide it in, or drag it there and
//!   let go. Drag a sleeved card out. Drop a card on a full pocket to swap them. Drag a page
//!   (anywhere but on a card) across to turn it, or flick it; the wheel turns pages too (and
//!   scrolls the loose cards when over them). Right click puts the card in hand back.
//! - controller: the left stick moves the hand (it settles on pockets and cards), the d-pad steps
//!   between them, Cross picks up / puts in (hold it to drag), Square quick-moves (a loose card
//!   into the next empty pocket, a sleeved one back out), Circle puts back / closes, L1 / R1
//!   turn pages and the right stick peels a page over by hand, Triangle closes.
//! - keyboard: arrows / WASD step, E (Enter, Space) picks up / puts in, Q quick-moves, Z / X
//!   turn pages (and stepping off a page's edge turns it), Esc puts back / closes.
//!
//! Models (quality-gated props, tools/binder/build_binder.py): `binder_ring4` (covers, spine,
//! ring mechanism) and `binder_sheet9` (the two films of one refill sheet). Both are sized from
//! the card (the card-templates session's slab), and their layout.json carries every dimension
//! used here, the card's included. Cards are the card-templates session's slabs (tools/cardtest)
//! with each card's own face from assets/cards/templates/cards/<id>_<finish>/.
//!
//! What sits where is `sim::binder::Binder` (saved with the player). This module is the view and
//! the hand: poses, animation, hit testing and input. Its state machine runs without a renderer
//! (tests); `frame` adds the GPU side.
//!
//! Frames: the binder's model space is the glTF's (+Y up, metres): the spine runs along Z, the
//! right-hand pages lie on +X, the top of the pages is -Z (away from the player). A sheet's own
//! space has its origin on the line through its holes, the pockets towards +X, its front +Y.

use akari::glam::{Affine3A, Mat3, Quat, Vec2, Vec3};
use akari::scene::MAT_CARD_FOIL;
use akari::textures::{file_key, TexJob, TexKind, TexSource};
use akari::{Camera, Instance, Material, MaterialId, MeshData, MeshId, Renderer};
use sim::binder::Binder;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};
use std::path::{Path, PathBuf};
use tcg::{CardDb, CardId, Collection, Pull, Rarity};

pub const BINDER_ID: &str = "binder_ring4";
pub const SHEET_ID: &str = "binder_sheet9";

const MM: f32 = 0.001;

// ------------------------------------------------------------------------------------- layout

/// The card slab (from the sheet's layout.json, which takes it from the card-templates session).
#[derive(Clone, Copy, Debug)]
pub struct CardSize {
    pub w: f32,
    pub h: f32,
    pub t: f32,
    pub corner: f32,
}

/// The binder's and the sheet's dimensions (their layout.json, written with the models).
#[derive(Clone, Debug)]
pub struct Layout {
    pub card: CardSize,
    /// a sleeved card sits this far off its pocket's bottom weld
    pub seat_gap: f32,
    pub board: Vec3,
    pub ring_r: f32,
    pub ring_cz: f32,
    pub sheet_x1: f32,
    pub sheet_h: f32,
    /// [row][col]: x0, x1, y0, y1 in the sheet's Blender frame (y up the page), metres
    pub pockets: [[[f32; 4]; 3]; 3],
    /// half the film gap: how far a pocket's films stand off the sheet's middle
    pub film: f32,
    /// thickness of one loaded sheet in a stack
    pub sheet_t: f32,
}

impl Layout {
    pub fn load() -> Self {
        let b: serde_json::Value = serde_json::from_str(include_str!("../../../assets/props/binder_ring4/layout.json")).expect("binder layout");
        let s: serde_json::Value = serde_json::from_str(include_str!("../../../assets/props/binder_sheet9/layout.json")).expect("sheet layout");
        let f = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0) as f32 * MM;
        let mut pockets = [[[0.0; 4]; 3]; 3];
        for (r, row) in pockets.iter_mut().enumerate() {
            for (c, p) in row.iter_mut().enumerate() {
                for (k, v) in p.iter_mut().enumerate() {
                    *v = f(&s["pockets_mm"][r][c][k]);
                }
            }
        }
        let card = CardSize { w: f(&s["card_mm"][0]), h: f(&s["card_mm"][1]), t: f(&s["card_mm"][2]), corner: f(&s["card_corner_mm"]) };
        let film = f(&s["film_gap_mm"]) * 0.5;
        Layout {
            card,
            seat_gap: f(&s["seat_gap_mm"]),
            board: Vec3::new(f(&b["board_mm"][0]), f(&b["board_mm"][1]), f(&b["board_mm"][2])),
            ring_r: f(&b["ring_r_mm"]),
            ring_cz: f(&b["ring_centre_z_mm"]),
            sheet_x1: f(&s["sheet_mm"][1]),
            sheet_h: f(&s["sheet_mm"][2]),
            pockets,
            film,
            // a loaded sheet in a stack: its pockets press a little flatter under the sheets above
            sheet_t: film * 2.0 + 0.1 * MM,
        }
    }

    /// Where a seated card's middle is, (x, z) in sheet space, front column.
    fn seat(&self, row: usize, col: usize) -> Vec2 {
        let [x0, x1, y0, _] = self.pockets[row][col];
        Vec2::new((x0 + x1) * 0.5, -(y0 + self.seat_gap + self.card.h * 0.5))
    }

    /// How far a card slides from the pocket's mouth to its seat.
    fn slide_len(&self, row: usize, col: usize) -> f32 {
        let [_, _, y0, y1] = self.pockets[row][col];
        (y1 - y0) - self.seat_gap
    }

    /// A pocket's corners (sheet space), for the focus marks.
    fn pocket_corners(&self, page: usize, pocket: usize) -> [Vec3; 4] {
        let (row, c) = pocket_rc(pocket);
        let front = page % 2 == 0;
        let col = if front { c } else { 2 - c };
        let [x0, x1, y0, y1] = self.pockets[row][col];
        let y = if front { self.film } else { -self.film };
        [Vec3::new(x0, y, -y1), Vec3::new(x1, y, -y1), Vec3::new(x1, y, -y0), Vec3::new(x0, y, -y0)]
    }
}

/// Which pocket of a page: (row, column as seen on that side).
fn pocket_rc(pocket: usize) -> (usize, usize) {
    (pocket / 3, pocket % 3)
}

/// A card in a pocket, in its sheet's space. `u` is how far in it is: 0 with its bottom edge at
/// the pocket's mouth, 1 seated. `tilt` lifts its top towards you about its bottom edge (a card
/// held ready to go in), and `lift` raises it off the film.
fn card_in_sheet(l: &Layout, page: usize, pocket: usize, u: f32, tilt: f32, lift: f32) -> Affine3A {
    let (row, c) = pocket_rc(pocket);
    let front = page % 2 == 0;
    let col = if front { c } else { 2 - c };
    let seat = l.seat(row, col);
    let z = seat.y - l.slide_len(row, col) * (1.0 - u);
    // outside the pocket the card rides on the film; inside, between the films
    let out = (1.0 - u * 12.0).clamp(0.0, 1.0);
    let y = l.card.t * 0.5 + 0.01 * MM + out * (l.film + 0.15 * MM) + lift;
    let lie = Quat::from_rotation_x(-FRAC_PI_2);
    let x = if front { seat.x } else { -seat.x };
    let mut local = Affine3A::from_rotation_translation(lie, Vec3::new(x, y, z));
    if tilt != 0.0 {
        // pivot on the bottom edge (towards +z)
        let edge = Vec3::new(x, y, z + l.card.h * 0.5);
        local = Affine3A::from_translation(edge) * Affine3A::from_rotation_x(tilt) * Affine3A::from_translation(-edge) * local;
    }
    if front {
        local
    } else {
        // the back faces -Y: the same pose seen from the back
        Affine3A::from_rotation_z(PI) * local
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Frame-rate independent approach: the fraction to move this frame at `rate` per second.
fn approach(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

fn blend(a: Affine3A, b: Affine3A, k: f32) -> Affine3A {
    let ra = Quat::from_mat3(&Mat3::from(a.matrix3));
    let rb = Quat::from_mat3(&Mat3::from(b.matrix3));
    Affine3A::from_rotation_translation(ra.slerp(rb, k), Vec3::from(a.translation).lerp(b.translation.into(), k))
}

// ------------------------------------------------------------------------------------- state

/// What the hand is over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spot {
    /// a pocket of a visible page
    Pocket { page: usize, pocket: usize },
    /// a loose card in the fan (index into the loose list)
    Fan(usize),
    /// somewhere on a page that is not a card: grab here to turn it (true: the right-hand page)
    Page(bool),
    None,
}

#[derive(Clone, Copy, Debug)]
pub struct Held {
    pub pull: Pull,
    /// where it was taken from (it is already out of that sleeve)
    pub from: Option<(usize, usize)>,
    /// 0..1 while it slides up out of its pocket (it follows the hand after)
    extract: f32,
}

/// A card sliding into its pocket (it is already in the binder data).
#[derive(Clone, Copy, Debug)]
struct SlideIn {
    page: usize,
    pocket: usize,
    pull: Pull,
    t: f32,
    /// where it started (binder space): the hand's pose
    start: Affine3A,
}

/// A card going back to the loose cards.
#[derive(Clone, Copy, Debug)]
struct Fly {
    pull: Pull,
    from: Affine3A,
    t: f32,
}

#[derive(Clone, Copy, Debug)]
struct Turn {
    /// the sheet that turns; u = 0 lying on the right (spread = sheet), 1 on the left
    sheet: usize,
    u: f32,
    vel: f32,
    target: f32,
    /// held by the hand: (pointer x where the grab started, u then); None: on its own (spring)
    drag: Option<(f32, f32)>,
}

/// The button is down: what it went down on.
#[derive(Clone, Copy, Debug)]
struct Press {
    at: Vec2,
    /// moved far enough to be a drag
    dragged: bool,
    /// pressed on a page, not a card: dragging it turns the page (true: right-hand page)
    page: Option<bool>,
    /// it put a card down (the release does nothing more)
    done: bool,
}

const SLIDE_IN: f32 = 0.42;
const EXTRACT: f32 = 0.2;
const FLY: f32 = 0.32;
const OPEN_TIME: f32 = 0.45;
/// how far a waiting card's top is lifted (radians), its bottom edge at the pocket's mouth
const READY_TILT: f32 = 0.9;
/// sheets drawn on each stack (see `scene`): two sheets are four films, within the six a camera
/// ray may pass
const SHOWN: usize = 2;
/// page spring: stiffness (1/s^2) and damping ratio (a little under 1: the page lands with a soft bounce)
const PAGE_K: f32 = 70.0;
const PAGE_ZETA: f32 = 0.78;
/// a drag moves the hand this far (px) before it counts as a drag
const DRAG_PX: f32 = 10.0;
const FAN_SPACING: f32 = 0.043;

pub struct BinderView {
    pub layout: Layout,
    /// spread shown: 0 = the first sheet's front alone on the right
    pub spread: usize,
    turn: Option<Turn>,
    /// turns asked for while one is under way (fast flicking)
    queued: i32,
    slides: Vec<SlideIn>,
    flies: Vec<Fly>,
    pub held: Option<Held>,
    press: Option<Press>,
    /// keyboard / pad focus (also follows the pointer)
    pub focus: Spot,
    /// the pointer (screen pixels); `pointer_live`: the mouse or the stick moved it (else it
    /// follows the focus: keys and d-pad)
    pub pointer: Vec2,
    pointer_live: bool,
    /// the pad's hand is in use (draw its marker)
    pub pad_hand: bool,
    /// first loose card shown in the fan (smoothed in `fan_scroll`)
    fan_first: usize,
    fan_scroll: f32,
    /// each loose card's place in the fan (smoothed) and how far it is lifted (hover)
    fan_x: HashMap<(CardId, bool), f32>,
    fan_lift: HashMap<(CardId, bool), f32>,
    /// sleeved cards peeking up under the hand: (page, pocket) -> 0..1
    peek: HashMap<(usize, usize), f32>,
    /// 0..1 the binder coming up into view
    open_t: f32,
    pub screen: Vec2,
    /// the binder's place in the world (set when opened, from the player's eye and heading)
    rig: Affine3A,
    pub cam: Camera,
    eye: Vec3,
    yaw: f32,
    /// the held card's pose (smoothed) in binder space, and its sway
    held_pose: Option<Affine3A>,
    held_vel: Vec2,
    gpu: Option<Gpu>,
    /// show models that passed their checks but await approval (--review)
    pub review: bool,
    pub error: Option<String>,
    /// hold a page turn or a slide still (test screens)
    frozen: bool,
    /// object motion is on in the renderer (while open)
    motion: bool,
    /// the pad: Cross held last frame, and the d-pad direction held with its repeat timer
    pub pad_cross: bool,
    pub pad_repeat: Option<((i32, i32), f32)>,
}

impl Default for BinderView {
    fn default() -> Self {
        Self::new()
    }
}

impl BinderView {
    pub fn new() -> Self {
        BinderView {
            layout: Layout::load(),
            spread: 0,
            turn: None,
            queued: 0,
            slides: Vec::new(),
            flies: Vec::new(),
            held: None,
            press: None,
            focus: Spot::Fan(0),
            pointer: Vec2::ZERO,
            pointer_live: false,
            pad_hand: false,
            fan_first: 0,
            fan_scroll: 0.0,
            fan_x: HashMap::new(),
            fan_lift: HashMap::new(),
            peek: HashMap::new(),
            open_t: 0.0,
            screen: Vec2::new(1920.0, 1080.0),
            rig: Affine3A::IDENTITY,
            cam: Camera::default(),
            eye: Vec3::ZERO,
            yaw: 0.0,
            held_pose: None,
            held_vel: Vec2::ZERO,
            gpu: None,
            review: false,
            error: None,
            frozen: false,
            motion: false,
            pad_cross: false,
            pad_repeat: None,
        }
    }

    /// Open in front of the player: eye position and heading (radians, akari convention).
    pub fn open(&mut self, eye: Vec3, yaw: f32, cam: &Camera) {
        self.eye = eye;
        self.yaw = yaw;
        self.cam = *cam;
        self.open_t = 0.0;
        self.slides.clear();
        self.flies.clear();
        self.turn = None;
        self.queued = 0;
        self.press = None;
        self.held_pose = None;
        self.pointer_live = false;
        self.focus = Spot::Fan(0);
        self.fan_first = 0;
        self.fan_scroll = 0.0;
        self.fan_x.clear();
        self.fan_lift.clear();
        self.peek.clear();
        self.place();
    }

    /// Close: a card in hand goes back with the loose cards.
    pub fn close(&mut self) {
        self.frozen = false;
        self.held = None;
        self.slides.clear();
        self.flies.clear();
        self.turn = None;
        self.press = None;
    }

    /// Test screens: stop a page turn part-way.
    pub fn freeze_turn(&mut self, u: f32) {
        if let Some(t) = &mut self.turn {
            t.u = u;
            t.vel = 0.0;
            self.frozen = true;
        }
    }

    /// Test screens: stop a card part-way into its pocket.
    pub fn freeze_slide(&mut self, at: f32) {
        if let Some(s) = self.slides.last_mut() {
            s.t = at;
            self.frozen = true;
        }
    }

    #[allow(dead_code)]
    pub fn busy(&self) -> bool {
        !self.slides.is_empty() || self.held.is_some_and(|h| h.extract < 1.0)
    }

    // --------------------------------------------------------------------------- placement

    /// The binder in the world: in front of the eye, below it, tilted up towards the face.
    fn place(&mut self) {
        let pitch = -46f32.to_radians();
        let dist = 0.40;
        let k = ease_out(self.open_t);
        let up = (1.0 - k) * -0.10;
        let fwd = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());
        let dir = fwd * pitch.cos() + Vec3::Y * pitch.sin();
        let centre = self.eye + dir * dist + Vec3::Y * up;
        let tilt = FRAC_PI_2 + pitch + 0.02 - (1.0 - k) * 0.25;
        self.rig = Affine3A::from_translation(centre) * Affine3A::from_rotation_y(-self.yaw) * Affine3A::from_rotation_x(tilt) * Affine3A::from_translation(Vec3::new(0.0, -0.004, 0.012));
        // the camera looks at it a little below its middle (the loose cards fan out below)
        let look = centre - Vec3::Y * 0.035 + fwd * 0.03;
        let d = (look - self.eye).normalize();
        self.cam.position = self.eye;
        self.cam.yaw = self.yaw;
        self.cam.pitch = d.y.asin();
    }

    // --------------------------------------------------------------------------- sheets

    /// The hole's angle on the ring and the sheet's angle, lying in a stack.
    /// `layer` counts up from the board; `left`: on the front board.
    fn rest(&self, layer: usize, n: usize, left: bool) -> (f32, f32) {
        let l = &self.layout;
        let step = (35f32.to_radians() / (n.max(2) - 1) as f32).min(3.5f32.to_radians());
        let alpha = -55f32.to_radians() + layer as f32 * step;
        let hole = Vec2::new(alpha.cos(), alpha.sin()) * l.ring_r + Vec2::new(0.0, l.ring_cz);
        // the far edge rests on the stack below it
        let far = l.board.z + 0.35 * MM + (layer as f32 + 0.5) * l.sheet_t;
        let slope = ((hole.y - far) / (l.sheet_x1 * 0.8)).atan();
        if left { (PI - alpha, PI + slope) } else { (alpha, -slope) }
    }

    fn pose_at(&self, alpha: f32, theta: f32) -> Affine3A {
        let l = &self.layout;
        let hole = Vec3::new(alpha.cos() * l.ring_r, l.ring_cz + alpha.sin() * l.ring_r, 0.0);
        Affine3A::from_translation(hole) * Affine3A::from_rotation_z(theta)
    }

    /// Sheet `i`'s pose in binder space.
    fn sheet_pose(&self, i: usize, n: usize) -> Affine3A {
        if let Some(t) = self.turn {
            if i == t.sheet {
                let (a0, t0) = self.rest(n - 1 - i, n, false);
                let (a1, t1) = self.rest(i, n, true);
                let u = t.u.clamp(0.0, 1.0);
                // the hole runs over the top of the ring; the page lags a touch behind it at
                // the start and catches up as it falls (it is held at the rings, not the edge)
                let swing = u + 0.06 * (u * PI).sin();
                return self.pose_at(a0 + (a1 - a0) * u, t0 + (t1 - t0) * swing);
            }
            let (a, th) = if i < t.sheet { self.rest(i, n, true) } else { self.rest(n - 1 - i, n, false) };
            return self.pose_at(a, th);
        }
        let (a, th) = if i < self.spread { self.rest(i, n, true) } else { self.rest(n - 1 - i, n, false) };
        self.pose_at(a, th)
    }

    /// Pages showing now: (left page, right page).
    pub fn pages(&self, b: &Binder) -> (Option<usize>, Option<usize>) {
        let n = b.sheets.len();
        let s = self.spread.min(n);
        ((s > 0).then(|| 2 * (s - 1) + 1), (s < n).then(|| 2 * s))
    }

    // --------------------------------------------------------------------------- pointer

    fn ray(&self, px: Vec2) -> (Vec3, Vec3) {
        let c = &self.cam;
        let aspect = self.screen.x / self.screen.y.max(1.0);
        let f = c.forward();
        let r = f.cross(Vec3::Y).normalize();
        let u = r.cross(f);
        let t = (c.fov_y * 0.5).tan();
        let nx = (px.x / self.screen.x) * 2.0 - 1.0;
        let ny = 1.0 - (px.y / self.screen.y) * 2.0;
        (c.position, (f + r * nx * t * aspect + u * ny * t).normalize())
    }

    /// World point -> screen pixels.
    pub fn project(&self, p: Vec3) -> Option<Vec2> {
        let c = &self.cam;
        let f = c.forward();
        let r = f.cross(Vec3::Y).normalize();
        let u = r.cross(f);
        let d = p - c.position;
        let z = d.dot(f);
        if z < 0.01 {
            return None;
        }
        let t = (c.fov_y * 0.5).tan();
        let aspect = self.screen.x / self.screen.y.max(1.0);
        let nx = d.dot(r) / (z * t * aspect);
        let ny = d.dot(u) / (z * t);
        Some(Vec2::new((nx + 1.0) * 0.5 * self.screen.x, (1.0 - ny) * 0.5 * self.screen.y))
    }

    /// Where the pointer's ray meets the plane y = 0 of `frame` (frame space).
    fn hit_plane(&self, frame: Affine3A, px: Vec2) -> Option<Vec3> {
        let (o, d) = self.ray(px);
        let inv = frame.inverse();
        let o = inv.transform_point3(o);
        let d = inv.transform_vector3(d);
        if d.y.abs() < 1e-5 {
            return None;
        }
        let t = -o.y / d.y;
        (t > 0.0).then(|| o + d * t)
    }

    // --------------------------------------------------------------------------- the fan

    const FAN_SHOWN: usize = 12;

    /// The loose cards as the fan shows them: (index into `loose`, pull), leaving out the copies
    /// in the hand or on their way back.
    fn fan_cards(&self, loose: &[(Pull, u32)]) -> Vec<(usize, Pull)> {
        let mut away: Vec<Pull> = self.flies.iter().map(|f| f.pull).collect();
        if let Some(h) = self.held {
            away.push(h.pull);
        }
        let mut v = Vec::new();
        for (i, (p, n)) in loose.iter().enumerate() {
            let gone = away.iter().filter(|a| **a == *p).count() as u32;
            if *n > gone {
                v.push((i, *p));
            }
        }
        v
    }

    /// A fan slot `k` (0 = leftmost shown), binder space, lifted by `lift` (0..1, hover).
    fn fan_slot(&self, k: f32, lift: f32) -> Affine3A {
        let x = -0.235 + k * FAN_SPACING;
        let z = self.layout.board.y * 0.5 + 0.068 - lift * 0.024;
        let y = 0.022 + k * 0.45 * MM + lift * 0.008;
        let yaw = -(x * 0.25);
        Affine3A::from_translation(Vec3::new(x, y, z)) * Affine3A::from_rotation_y(yaw) * Affine3A::from_rotation_x(-FRAC_PI_2 + FAN_TILT)
    }

    /// Where loose card `pull` sits in the fan now (smoothed slot), or where it would go.
    fn fan_pose_of(&self, loose: &[(Pull, u32)], pull: Pull) -> Affine3A {
        let key = (pull.id, pull.foil);
        let k = self.fan_x.get(&key).copied().unwrap_or_else(|| {
            let i = loose.iter().position(|x| x.0 == pull).unwrap_or(0);
            i as f32 - self.fan_scroll
        });
        self.fan_slot(k.clamp(-1.0, Self::FAN_SHOWN as f32), 0.0)
    }

    // --------------------------------------------------------------------------- hit tests

    /// What's under a screen point.
    pub fn spot_at(&self, b: &Binder, loose: &[(Pull, u32)], px: Vec2) -> Spot {
        let n = b.sheets.len();
        let l = &self.layout;
        // the fan first (it is nearest the eye); the card on top of the pile wins
        let fan_frame = self.rig * Affine3A::from_translation(Vec3::new(0.0, 0.022, 0.0));
        if let Some(p) = self.hit_plane(fan_frame, px) {
            let z0 = l.board.y * 0.5 + 0.068;
            if (p.z - z0).abs() < l.card.h * 0.55 {
                let mut best = None;
                for (i, pull) in self.fan_cards(loose) {
                    let k = self.fan_x.get(&(pull.id, pull.foil)).copied().unwrap_or(i as f32 - self.fan_scroll);
                    if !(-0.5..Self::FAN_SHOWN as f32).contains(&k) {
                        continue;
                    }
                    let x = -0.235 + k * FAN_SPACING;
                    if (p.x - x).abs() < l.card.w * 0.5 {
                        best = Some(i);
                    }
                }
                if let Some(i) = best {
                    return Spot::Fan(i);
                }
            }
        }
        let (lp, rp) = self.pages(b);
        for (page, right) in [(rp, true), (lp, false)] {
            let Some(page) = page else { continue };
            let sheet = page / 2;
            let frame = self.rig * self.sheet_pose(sheet, n);
            let Some(p) = self.hit_plane(frame, px) else { continue };
            // (on the left the sheet is turned over: its x runs out to the left of the spine)
            let side_x = (self.rig.inverse() * frame).transform_point3(p).x;
            if right != (side_x > 0.0) {
                continue;
            }
            let (x, y) = (p.x, -p.z);
            if y.abs() > l.sheet_h * 0.5 + 0.012 || x > l.sheet_x1 + 0.012 {
                continue;
            }
            for r in 0..3 {
                for c in 0..3 {
                    let [x0, x1, y0, y1] = l.pockets[r][c];
                    if x >= x0 - 0.0012 && x <= x1 + 0.0012 && y >= y0 - 0.0012 && y <= y1 + 0.0012 {
                        let col = if page % 2 == 0 { c } else { 2 - c };
                        return Spot::Pocket { page, pocket: r * 3 + col };
                    }
                }
            }
            return Spot::Page(right);
        }
        Spot::None
    }

    /// Where the hand is in the world for a spot (to put the pointer there for keys and pads).
    fn spot_point(&self, b: &Binder, loose: &[(Pull, u32)], s: Spot) -> Option<Vec3> {
        let n = b.sheets.len();
        match s {
            Spot::Pocket { page, pocket } => {
                let pose = self.rig * self.sheet_pose(page / 2, n) * card_in_sheet(&self.layout, page, pocket, 1.0, 0.0, 0.0);
                Some(pose.translation.into())
            }
            Spot::Fan(i) => {
                let pull = loose.get(i)?.0;
                Some((self.rig * self.fan_pose_of(loose, pull)).translation.into())
            }
            _ => None,
        }
    }

    /// The focused pocket's corners on screen (the focus marks for keys and pads, and for the
    /// mouse while it carries a card).
    pub fn focus_quad(&self, b: &Binder) -> Option<[Vec2; 4]> {
        let Spot::Pocket { page, pocket } = self.focus else { return None };
        if self.turn.is_some() {
            return None;
        }
        let frame = self.rig * self.sheet_pose(page / 2, b.sheets.len());
        let c = self.layout.pocket_corners(page, pocket);
        let mut out = [Vec2::ZERO; 4];
        for (o, p) in out.iter_mut().zip(c) {
            *o = self.project(frame.transform_point3(p))?;
        }
        Some(out)
    }

    /// Show the focus marks (keys and pads always; the mouse while it carries a card).
    pub fn show_focus(&self) -> bool {
        !self.pointer_live || self.pad_hand || self.held.is_some()
    }

    // --------------------------------------------------------------------------- actions

    /// Turn a page: +1 forward (the right-hand page goes over), -1 back. While a page is still
    /// falling, the next turn is queued (flick through quickly).
    pub fn turn(&mut self, b: &Binder, dir: i32) -> bool {
        if self.held.is_some_and(|h| h.extract < 1.0) || !self.slides.is_empty() {
            return false;
        }
        let n = b.sheets.len();
        if let Some(t) = &mut self.turn {
            if t.drag.is_some() {
                return false;
            }
            let going = if t.target > 0.5 { 1 } else { -1 };
            if going != dir {
                // the other way: send this page back
                t.target = 1.0 - t.target;
                self.queued = 0;
            } else {
                let after = t.sheet as i32 + if dir > 0 { 1 } else { 0 } + self.queued;
                if (dir > 0 && after < n as i32) || (dir < 0 && after > 0) {
                    self.queued += dir;
                }
            }
            return true;
        }
        if dir > 0 && self.spread < n {
            self.turn = Some(Turn { sheet: self.spread, u: 0.0, vel: 1.2, target: 1.0, drag: None });
            true
        } else if dir < 0 && self.spread > 0 {
            self.turn = Some(Turn { sheet: self.spread - 1, u: 1.0, vel: -1.2, target: 0.0, drag: None });
            true
        } else {
            false
        }
    }

    /// Take the card under the hand (from the fan, or out of its sleeve).
    fn pick(&mut self, b: &mut Binder, loose: &[(Pull, u32)]) -> bool {
        match self.focus {
            Spot::Fan(i) => {
                let Some((pull, _)) = loose.get(i) else { return false };
                if !self.fan_cards(loose).iter().any(|x| x.0 == i) {
                    return false;
                }
                let from = self.fan_pose_of(loose, *pull);
                self.held = Some(Held { pull: *pull, from: None, extract: 1.0 });
                // lift it off the fan where it lay
                self.held_pose = Some(Affine3A::from_translation(Vec3::new(0.0, 0.01, -0.01)) * from);
                true
            }
            Spot::Pocket { page, pocket } => {
                let Some(pull) = b.take(page, pocket) else { return false };
                self.held = Some(Held { pull, from: Some((page, pocket)), extract: 0.0 });
                self.peek.remove(&(page, pocket));
                self.held_pose = Some(self.sheet_pose(page / 2, b.sheets.len()) * card_in_sheet(&self.layout, page, pocket, 1.0, 0.0, 0.0));
                true
            }
            _ => false,
        }
    }

    /// Put the card in hand where the hand is: into an empty pocket, swapped with a sleeved
    /// card, or back with the loose cards.
    fn put(&mut self, b: &mut Binder, col: &Collection, loose: &[(Pull, u32)]) {
        let Some(h) = self.held else { return };
        let start = self.held_pose.unwrap_or(Affine3A::IDENTITY);
        match self.focus {
            Spot::Pocket { page, pocket } if self.turn.is_none() => {
                // a full pocket: the card in it comes out into the hand
                let swapped = b.take(page, pocket);
                if b.put(col, page, pocket, h.pull) {
                    self.slides.push(SlideIn { page, pocket, pull: h.pull, t: 0.0, start });
                    self.held = None;
                    self.held_pose = None;
                    if let Some(out) = swapped {
                        self.held = Some(Held { pull: out, from: Some((page, pocket)), extract: 0.0 });
                        self.held_pose = Some(self.sheet_pose(page / 2, b.sheets.len()) * card_in_sheet(&self.layout, page, pocket, 1.0, 0.0, 0.0));
                    }
                } else if let Some(out) = swapped {
                    // (could not go in: leave the pocket as it was)
                    b.put(col, page, pocket, out);
                }
            }
            _ => self.drop_held_at(loose, start),
        }
    }

    fn drop_held_at(&mut self, _loose: &[(Pull, u32)], from: Affine3A) {
        if let Some(h) = self.held.take() {
            self.flies.push(Fly { pull: h.pull, from, t: 0.0 });
        }
        self.held_pose = None;
    }

    /// Put the card in hand back with the loose cards (right click, Esc, Circle).
    pub fn drop_held(&mut self) -> bool {
        if self.held.is_none() {
            return false;
        }
        let from = self.held_pose.unwrap_or(Affine3A::IDENTITY);
        self.drop_held_at(&[], from);
        true
    }

    /// One click of the hand (E, Cross tapped): pick up, put in or swap, or turn a page when on
    /// a page's empty part.
    pub fn click(&mut self, b: &mut Binder, col: &Collection, loose: &[(Pull, u32)]) {
        self.press(b, col, loose);
        self.release(b, col, loose);
    }

    /// The button went down (mouse left, Cross).
    pub fn press(&mut self, b: &mut Binder, col: &Collection, loose: &[(Pull, u32)]) {
        let mut p = Press { at: self.pointer, dragged: false, page: None, done: false };
        if let Some(t) = &mut self.turn {
            // catch a falling page
            if let Spot::Page(_) | Spot::None = self.focus {
                if self.held.is_none() && self.queued == 0 {
                    t.drag = Some((self.pointer.x, t.u));
                    t.vel = 0.0;
                    p.page = Some(t.target < 0.5);
                }
            }
            self.press = Some(p);
            return;
        }
        if self.held.is_some() {
            self.put(b, col, loose);
            p.done = true;
        } else if !self.pick(b, loose) {
            if let Spot::Page(right) = self.focus {
                p.page = Some(right);
            }
        }
        self.press = Some(p);
    }

    /// The button came up: a drag ends (a card dropped where the hand is, a page let go).
    pub fn release(&mut self, b: &mut Binder, col: &Collection, loose: &[(Pull, u32)]) {
        let Some(p) = self.press.take() else { return };
        if let Some(t) = &mut self.turn {
            if t.drag.take().is_some() {
                // let go: it falls the way it was going (a flick carries it over)
                t.target = if t.u + t.vel * 0.22 > 0.5 { 1.0 } else { 0.0 };
                return;
            }
        }
        if p.dragged && !p.done && self.held.is_some() {
            self.put(b, col, loose);
        } else if let (false, Some(right), None) = (p.dragged, p.page, self.held) {
            // a click on a page outside its pockets (its margin, the edge) turns it
            self.turn(b, if right { 1 } else { -1 });
        }
    }

    /// Quick move (Square, Q): a loose card into the next empty pocket of the open pages, a
    /// sleeved card back to the loose cards.
    pub fn quick(&mut self, b: &mut Binder, col: &Collection, loose: &[(Pull, u32)]) {
        if self.turn.is_some() {
            return;
        }
        if self.held.is_none() {
            match self.focus {
                Spot::Pocket { .. } => {
                    if self.pick(b, loose) {
                        // straight back to the fan once it is out
                        if let Some(h) = &mut self.held {
                            h.extract = 1.0;
                        }
                        self.drop_held();
                    }
                    return;
                }
                Spot::Fan(_) => {
                    if !self.pick(b, loose) {
                        return;
                    }
                }
                _ => return,
            }
        }
        let (lp, rp) = self.pages(b);
        let empty = [rp, lp].into_iter().flatten().flat_map(|page| (0..9).map(move |k| (page, k))).find(|&(page, k)| b.get(page, k).is_none());
        let keep = self.focus;
        match empty {
            Some((page, pocket)) => {
                self.focus = Spot::Pocket { page, pocket };
                self.put(b, col, loose);
                self.focus = keep;
            }
            None => {
                self.drop_held();
            }
        }
    }

    /// Move the focus by a step (arrows, d-pad): over the six pocket columns of the open
    /// pages, down to the fan, and over the edge to turn the page.
    pub fn step(&mut self, b: &Binder, loose: &[(Pull, u32)], dx: i32, dy: i32) {
        self.pointer_live = false;
        let fan: Vec<usize> = self.fan_cards(loose).into_iter().map(|x| x.0).collect();
        let (lp, rp) = self.pages(b);
        let at = |gx: i32, gy: i32| -> Option<Spot> {
            let page = if gx < 3 { lp? } else { rp? };
            let c = gx.rem_euclid(3) as usize;
            Some(Spot::Pocket { page, pocket: gy as usize * 3 + c })
        };
        let grid = |page: usize, pocket: usize| -> (i32, i32) {
            let (r, c) = pocket_rc(pocket);
            (if Some(page) == lp { c as i32 } else { 3 + c as i32 }, r as i32)
        };
        let fan_near = |gx: i32| -> Spot {
            // the shown fan card under that column
            let k = ((gx as f32 + 0.5) / 6.0 * Self::FAN_SHOWN as f32) as usize + self.fan_first;
            fan.iter().copied().min_by_key(|&i| (i as i64 - k as i64).abs()).map_or(Spot::None, Spot::Fan)
        };
        self.focus = match self.focus {
            Spot::Fan(i) => {
                if dy < 0 {
                    let pos = fan.iter().position(|&x| x == i).unwrap_or(0);
                    let k = pos.saturating_sub(self.fan_first) as f32 / Self::FAN_SHOWN as f32;
                    let gx = ((k * 6.0) as i32).clamp(0, 5);
                    at(gx, 2).or_else(|| at(3, 2)).or_else(|| at(2, 2)).unwrap_or(Spot::Fan(i))
                } else if fan.is_empty() {
                    Spot::None
                } else {
                    let pos = fan.iter().position(|&x| x >= i).unwrap_or(0) as i32;
                    Spot::Fan(fan[(pos + dx).clamp(0, fan.len() as i32 - 1) as usize])
                }
            }
            Spot::Pocket { page, pocket } => {
                let (gx, gy) = grid(page, pocket);
                let (nx, ny) = (gx + dx, gy + dy);
                if ny > 2 {
                    match fan_near(gx) {
                        Spot::None => Spot::Pocket { page, pocket },
                        s => s,
                    }
                } else if ny < 0 {
                    Spot::Pocket { page, pocket }
                } else if nx < 0 || nx > 5 || at(nx, ny).is_none() {
                    // over the edge: turn the page and land on the far side
                    let dir = if nx < 3 { -1 } else { 1 };
                    if self.turn(b, dir) {
                        let s = (self.spread as i32 + dir).max(0) as usize;
                        let n = b.sheets.len();
                        let (l2, r2) = ((s > 0).then(|| 2 * (s - 1) + 1), (s < n).then(|| 2 * s));
                        let (dest, c) = if dir > 0 { (l2.or(r2), 2) } else { (r2.or(l2), 0) };
                        let c = if dir > 0 && dest == r2 { 0 } else { c };
                        dest.map(|page| Spot::Pocket { page, pocket: ny as usize * 3 + c }).unwrap_or(Spot::Pocket { page, pocket })
                    } else {
                        Spot::Pocket { page, pocket }
                    }
                } else {
                    at(nx, ny).unwrap_or(Spot::Pocket { page, pocket })
                }
            }
            _ => at(3, 1).or_else(|| at(0, 1)).unwrap_or_else(|| fan.first().map_or(Spot::None, |&i| Spot::Fan(i))),
        };
        if let Spot::Fan(i) = self.focus {
            self.keep_in_fan(loose, i);
        }
    }

    fn keep_in_fan(&mut self, loose: &[(Pull, u32)], i: usize) {
        let fan: Vec<usize> = self.fan_cards(loose).into_iter().map(|x| x.0).collect();
        let pos = fan.iter().position(|&x| x == i).unwrap_or(0);
        if pos < self.fan_first {
            self.fan_first = pos;
        } else if pos >= self.fan_first + Self::FAN_SHOWN {
            self.fan_first = pos + 1 - Self::FAN_SHOWN;
        }
        self.fan_first = self.fan_first.min(fan.len().saturating_sub(Self::FAN_SHOWN));
    }

    pub fn scroll_fan(&mut self, loose: &[(Pull, u32)], d: i32) {
        let n = self.fan_cards(loose).len();
        self.fan_first = (self.fan_first as i32 + d).clamp(0, n.saturating_sub(Self::FAN_SHOWN) as i32) as usize;
    }

    /// The mouse moved.
    pub fn point(&mut self, b: &Binder, loose: &[(Pull, u32)], px: Vec2) {
        self.pointer = px;
        self.pointer_live = true;
        self.pad_hand = false;
        self.after_move(b, loose);
    }

    /// The pad's stick moves the hand: free, but it settles on the nearest pocket or card when
    /// the stick is let go (in `update`).
    pub fn nudge(&mut self, b: &Binder, loose: &[(Pull, u32)], d: Vec2) {
        self.pointer = (self.pointer + d).clamp(Vec2::ZERO, self.screen);
        self.pointer_live = true;
        self.pad_hand = true;
        self.after_move(b, loose);
    }

    fn after_move(&mut self, b: &Binder, loose: &[(Pull, u32)]) {
        if let Some(p) = &mut self.press {
            if !p.dragged && p.at.distance(self.pointer) > DRAG_PX {
                p.dragged = true;
                // a drag on a page's empty part takes hold of the page
                if let (Some(right), None, None) = (p.page, self.turn, self.held) {
                    let n = b.sheets.len();
                    let sheet = if right { (self.spread < n).then_some(self.spread) } else { self.spread.checked_sub(1) };
                    if let Some(sheet) = sheet {
                        let u = if right { 0.0 } else { 1.0 };
                        self.turn = Some(Turn { sheet, u, vel: 0.0, target: u, drag: Some((p.at.x, u)) });
                    }
                }
            }
        }
        let span = self.page_span();
        if let Some(t) = &mut self.turn {
            if let Some((x0, u0)) = t.drag {
                // the page follows the hand across the spread
                t.u = (u0 + (x0 - self.pointer.x) / span).clamp(0.0, 1.0);
                t.target = t.u;
                return;
            }
        }
        self.focus = self.spot_at(b, loose, self.pointer);
    }

    /// How far (px) the hand travels to carry a page from one side over to the other.
    fn page_span(&self) -> f32 {
        let a = self.project(self.rig.transform_point3(Vec3::new(self.layout.sheet_x1 * 0.75, 0.01, 0.0)));
        let b = self.project(self.rig.transform_point3(Vec3::new(-self.layout.sheet_x1 * 0.75, 0.01, 0.0)));
        a.zip(b).map_or(self.screen.x * 0.45, |(a, b)| (a.x - b.x).abs().max(100.0))
    }

    /// The right stick peels a page over by hand (x: -1 to turn forward, +1 back); let go and
    /// it falls the way it was going.
    pub fn peel(&mut self, b: &Binder, x: f32, dt: f32) {
        let n = b.sheets.len();
        if self.held.is_some() || !self.slides.is_empty() {
            return;
        }
        if x.abs() > 0.2 {
            if self.turn.is_none() {
                let forward = x < 0.0;
                let sheet = if forward { (self.spread < n).then_some(self.spread) } else { self.spread.checked_sub(1) };
                let Some(sheet) = sheet else { return };
                let u = if forward { 0.0 } else { 1.0 };
                self.turn = Some(Turn { sheet, u, vel: 0.0, target: u, drag: Some((0.0, u)) });
            }
            if let Some(t) = &mut self.turn {
                if t.drag.is_some() {
                    let from = if t.drag.unwrap().1 < 0.5 { 0.0 } else { 1.0 };
                    let want = if from < 0.5 { (-x).clamp(0.0, 1.0) } else { 1.0 - x.clamp(0.0, 1.0) };
                    let nu = t.u + (want - t.u) * approach(14.0, dt);
                    t.vel = (nu - t.u) / dt.max(1e-3);
                    t.u = nu;
                    t.target = nu;
                }
            }
        } else if let Some(t) = &mut self.turn {
            if t.drag.take().is_some() {
                t.target = if t.u + t.vel * 0.22 > 0.5 { 1.0 } else { 0.0 };
            }
        }
    }

    // --------------------------------------------------------------------------- update

    /// Advance the animations and place the binder and camera.
    pub fn update(&mut self, b: &Binder, loose: &[(Pull, u32)], dt: f32) {
        self.open_t = (self.open_t + dt / OPEN_TIME).min(1.0);
        let dt_all = dt;
        let dt = if self.frozen { 0.0 } else { dt };
        // the page: a spring towards where it is going; it lands on the stack with a small bounce
        if let Some(t) = &mut self.turn {
            if t.drag.is_none() && dt > 0.0 {
                let c = 2.0 * PAGE_K.sqrt() * PAGE_ZETA;
                let mut left = dt;
                while left > 0.0 {
                    let h = left.min(1.0 / 240.0);
                    let acc = PAGE_K * (t.target - t.u) - c * t.vel;
                    t.vel += acc * h;
                    t.u += t.vel * h;
                    if t.u < 0.0 {
                        t.u = 0.0;
                        t.vel = -t.vel * 0.18;
                    } else if t.u > 1.0 {
                        t.u = 1.0;
                        t.vel = -t.vel * 0.18;
                    }
                    left -= h;
                }
                // done once it lies still; a queued turn starts as this one lands
                let landed = (t.u - t.target).abs() < 0.003 && t.vel.abs() < 0.06;
                let nearly = (t.u - t.target).abs() < 0.12 && self.queued != 0;
                if landed || nearly {
                    self.spread = t.sheet + if t.target > 0.5 { 1 } else { 0 };
                    self.turn = None;
                    let q = self.queued;
                    self.queued = 0;
                    if q != 0 {
                        let dir = q.signum();
                        if self.turn(b, dir) {
                            self.queued = q - dir;
                        }
                    }
                }
            }
        }
        for s in &mut self.slides {
            s.t += dt / SLIDE_IN;
        }
        self.slides.retain(|s| s.t < 1.0 || self.frozen);
        for f in &mut self.flies {
            f.t += dt / FLY;
        }
        self.flies.retain(|f| f.t < 1.0);
        if let Some(h) = &mut self.held {
            h.extract = (h.extract + dt / EXTRACT).min(1.0);
        }
        self.place();
        // keys and pads: the hand follows the focus; a resting stick settles on the nearest spot
        let settle = !self.pointer_live || (self.pad_hand && self.press.is_none() && self.turn.is_none());
        if settle {
            if let Some(p) = self.spot_point(b, loose, self.focus).and_then(|p| self.project(p)) {
                let k = if self.pointer_live { approach(10.0, dt_all) } else { approach(22.0, dt_all) };
                self.pointer = self.pointer.lerp(p, k);
            }
        }
        // the fan: every card eases to its slot; the one under the hand rises
        let fan = self.fan_cards(loose);
        let first = self.fan_first.min(fan.len().saturating_sub(Self::FAN_SHOWN));
        self.fan_first = first;
        self.fan_scroll += (first as f32 - self.fan_scroll) * approach(14.0, dt_all);
        let hover = match self.focus {
            Spot::Fan(i) if self.held.is_none() => loose.get(i).map(|x| (x.0.id, x.0.foil)),
            _ => None,
        };
        for (k, (_, p)) in fan.iter().enumerate() {
            let key = (p.id, p.foil);
            let target = k as f32 - self.fan_scroll;
            let x = self.fan_x.entry(key).or_insert(target);
            *x += (target - *x) * approach(16.0, dt_all);
            let lift = self.fan_lift.entry(key).or_insert(0.0);
            let want = if hover == Some(key) { 1.0 } else { 0.0 };
            *lift += (want - *lift) * approach(18.0, dt_all);
        }
        // sleeved cards peek up out of their pocket under the hand
        let under = match self.focus {
            Spot::Pocket { page, pocket } if self.held.is_none() && self.turn.is_none() && b.get(page, pocket).is_some() => Some((page, pocket)),
            _ => None,
        };
        if let Some(k) = under {
            self.peek.entry(k).or_insert(0.0);
        }
        for (k, v) in self.peek.iter_mut() {
            let want = if Some(*k) == under { 1.0 } else { 0.0 };
            *v += (want - *v) * approach(16.0, dt_all);
        }
        self.peek.retain(|k, v| *v > 0.01 || Some(*k) == under);
    }

    // --------------------------------------------------------------------------- drawing

    /// Every card to draw (binder space) and the sheets' poses.
    fn scene(&self, b: &Binder, loose: &[(Pull, u32)]) -> (Vec<(Pull, Affine3A)>, Vec<Affine3A>) {
        let n = b.sheets.len();
        let l = &self.layout;
        let mut cards = Vec::new();
        let mut sheets = Vec::new();
        let base = self.turn.map_or(self.spread, |t| t.sheet);
        let sliding = |page: usize, pocket: usize| self.slides.iter().any(|s| s.page == page && s.pocket == pocket);
        for i in 0..n {
            // only the top sheets of each stack (and one turning) are drawn: a camera ray passes
            // at most six glass films (Akari's thin-glass limit); the cards of deeper sheets
            // could not be seen through the sheets above them anyway
            let turning = self.turn.is_some_and(|t| t.sheet == i);
            let right0 = if self.turn.is_some() { base + 1 } else { base };
            let near = turning || (i < base && i + SHOWN >= base) || (i >= right0 && i < right0 + SHOWN);
            if !near {
                continue;
            }
            let pose = self.sheet_pose(i, n);
            sheets.push(pose);
            for (page, side) in [(2 * i, &b.sheets[i].front), (2 * i + 1, &b.sheets[i].back)] {
                for (pocket, p) in side.iter().enumerate() {
                    let Some(p) = p else { continue };
                    if sliding(page, pocket) {
                        continue;
                    }
                    let peek = self.peek.get(&(page, pocket)).copied().unwrap_or(0.0);
                    let u = 1.0 - peek * 6.0 * MM / l.slide_len(0, 0);
                    cards.push((*p, pose * card_in_sheet(l, page, pocket, u, 0.0, 0.0)));
                }
            }
        }
        // cards going in: from the hand to the mouth, then down into the seat
        for s in &self.slides {
            let sheet = self.sheet_pose(s.page / 2, n);
            let ready = sheet * card_in_sheet(l, s.page, s.pocket, 0.0, READY_TILT, 0.0);
            let pose = if s.t < 0.22 {
                blend(s.start, ready, ease_out(s.t / 0.22))
            } else {
                let k = (s.t - 0.22) / 0.78;
                // the top comes down first, then it slides home (a hair past, and settles)
                let tilt = READY_TILT * (1.0 - smooth(0.0, 0.3, k));
                let u = ease_out(smooth(0.12, 1.0, k)) * 1.0;
                sheet * card_in_sheet(l, s.page, s.pocket, u.min(1.0), tilt, 0.0)
            };
            cards.push((s.pull, pose));
        }
        // the fan of loose cards
        for (_, p) in self.fan_cards(loose) {
            let key = (p.id, p.foil);
            let k = self.fan_x.get(&key).copied().unwrap_or(0.0);
            if !(-0.9..Self::FAN_SHOWN as f32 + 0.4).contains(&k) {
                continue;
            }
            let lift = self.fan_lift.get(&key).copied().unwrap_or(0.0);
            cards.push((p, self.fan_slot(k, lift)));
        }
        // cards flying back to the fan
        for f in &self.flies {
            let to = self.fan_pose_of(loose, f.pull);
            let k = ease_out(f.t);
            let arc = Affine3A::from_translation(Vec3::new(0.0, 0.03 * (k * PI).sin(), 0.0));
            cards.push((f.pull, arc * blend(f.from, to, k)));
        }
        (cards, sheets)
    }

    /// The held card's target pose (binder space): sliding out of its pocket, under the hand, or
    /// ready at the mouth of the pocket it is over.
    fn held_target(&self, b: &Binder) -> Option<Affine3A> {
        let h = self.held?;
        let n = b.sheets.len();
        let l = &self.layout;
        if let (Some((page, pocket)), true) = (h.from, h.extract < 1.0) {
            // out of the sleeve first: up to the mouth, then its top lifts
            let e = h.extract;
            let u = 1.0 - ease_out(smooth(0.0, 0.75, e));
            let tilt = READY_TILT * smooth(0.6, 1.0, e);
            return Some(self.sheet_pose(page / 2, n) * card_in_sheet(l, page, pocket, u, tilt, 0.0));
        }
        if let Spot::Pocket { page, pocket } = self.focus {
            if self.turn.is_none() {
                let sheet = self.sheet_pose(page / 2, n);
                return Some(if b.get(page, pocket).is_none() {
                    // ready to go in: bottom edge at the mouth, top lifted
                    sheet * card_in_sheet(l, page, pocket, 0.0, READY_TILT, 0.0)
                } else {
                    // over a full pocket: hovering, to swap
                    sheet * card_in_sheet(l, page, pocket, 0.62, 0.25, 0.022)
                });
            }
        }
        let hover = Affine3A::from_translation(Vec3::new(0.0, 0.045, 0.0));
        let p = self.hit_plane(self.rig * hover, self.pointer)?;
        // the hand holds the card by its lower half
        let at = hover.transform_point3(p) + Vec3::new(0.0, 0.0, -l.card.h * 0.22);
        let sway = Quat::from_rotation_z((-self.held_vel.x * 0.00012).clamp(-0.25, 0.25)) * Quat::from_rotation_x((self.held_vel.y * 0.0001).clamp(-0.2, 0.2));
        Some(Affine3A::from_rotation_translation(sway * Quat::from_rotation_x(-FRAC_PI_2 + 0.2), at))
    }

    // --------------------------------------------------------------------------- GPU

    /// Load what's needed, animate, and add this frame's instances. `cam` becomes the view.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, r: &mut Renderer, db: &CardDb, b: &Binder, loose: &[(Pull, u32)], dt: f32, out: &mut Vec<Instance>, cam: &mut Camera) -> anyhow::Result<()> {
        if !self.motion {
            // cards move on their own here (page turns, sleeves): exact per-object motion for DLSS
            r.set_object_motion(true);
            self.motion = true;
        }
        let (w, h) = r.extent();
        self.screen = Vec2::new(w as f32, h as f32);
        if self.gpu.is_none() {
            match Gpu::load(r, self.review, self.layout.card) {
                Ok(g) => self.gpu = Some(g),
                Err(e) => {
                    if self.error.is_none() {
                        eprintln!("binder: {e:#}");
                    }
                    self.error = Some(format!("{e:#}"));
                    return Ok(());
                }
            }
        }
        self.update(b, loose, dt);
        *cam = self.cam;
        let (cards, sheets) = self.scene(b, loose);
        // the held card follows the hand: exactly while it slides out of its sleeve, then on a
        // quick spring, swaying with the hand's speed
        let target = self.held_target(b);
        let extracting = self.held.is_some_and(|h| h.extract < 1.0 && h.from.is_some());
        self.held_pose = match (self.held_pose, target) {
            (_, Some(t)) if extracting => Some(t),
            (Some(p), Some(t)) => {
                let next = blend(p, t, approach(24.0, dt));
                let v = self.project(self.rig.transform_point3(next.translation.into())).zip(self.project(self.rig.transform_point3(p.translation.into()))).map(|(a, b)| (a - b) / dt.max(1e-3)).unwrap_or(Vec2::ZERO);
                self.held_vel = self.held_vel.lerp(v, approach(10.0, dt));
                Some(next)
            }
            (None, t) => t,
            (_, None) => None,
        };
        let g = self.gpu.as_mut().unwrap();
        for (m, t) in &g.binder {
            out.push(Instance { mesh: *m, transform: self.rig * *t });
        }
        for s in &sheets {
            for (m, t) in &g.sheet {
                out.push(Instance { mesh: *m, transform: self.rig * *s * *t });
            }
        }
        // faces load a few per frame (the card back shows until a face is ready)
        let mut budget = 3;
        let rig = self.rig;
        let mut draw = |pull: Pull, pose: Affine3A, out: &mut Vec<Instance>, budget: &mut usize, g: &mut Gpu| {
            for m in g.kit(r, pull, db.get(pull.id).rarity, budget) {
                out.push(Instance { mesh: m, transform: rig * pose });
            }
        };
        for (pull, pose) in cards {
            draw(pull, pose, out, &mut budget, g);
        }
        if let (Some(h), Some(p)) = (self.held, self.held_pose) {
            draw(h.pull, p, out, &mut budget, g);
        }
        Ok(())
    }

    /// Screen positions of the fan's visible cards with their loose counts (for the "x3" labels).
    pub fn fan_labels(&self, loose: &[(Pull, u32)]) -> Vec<(Vec2, u32, bool)> {
        let mut v = Vec::new();
        let away: Vec<Pull> = self.flies.iter().map(|f| f.pull).chain(self.held.map(|h| h.pull)).collect();
        for (i, p) in self.fan_cards(loose) {
            let key = (p.id, p.foil);
            let k = self.fan_x.get(&key).copied().unwrap_or(0.0);
            if !(-0.4..Self::FAN_SHOWN as f32 - 0.4).contains(&k) {
                continue;
            }
            let lift = self.fan_lift.get(&key).copied().unwrap_or(0.0);
            let n = loose[i].1 - away.iter().filter(|a| **a == p).count() as u32;
            let top = self.rig * self.fan_slot(k, lift) * Affine3A::from_translation(Vec3::new(self.layout.card.w * 0.36, self.layout.card.h * 0.5, 0.0));
            if let Some(px) = self.project(top.translation.into()) {
                v.push((px, n, self.focus == Spot::Fan(i)));
            }
        }
        v
    }

    /// Closed: hand the renderer back its usual motion (camera only).
    pub fn idle(&mut self, r: &mut Renderer) {
        if self.motion {
            r.set_object_motion(false);
            self.motion = false;
        }
    }

    pub fn loaded(&self) -> bool {
        self.gpu.is_some()
    }
}

/// How far the fan's cards lean back towards you (radians).
const FAN_TILT: f32 = 0.25;

// ------------------------------------------------------------------------------------- GPU side

enum Face {
    Ready([MeshId; 3]),
    /// no face baked for it (or it failed to load): the card back shows
    Missing,
}

struct Gpu {
    binder: Vec<(MeshId, Affine3A)>,
    sheet: Vec<(MeshId, Affine3A)>,
    back_kit: [MeshId; 3],
    back: MaterialId,
    edge: MaterialId,
    faces: HashMap<(CardId, bool), Face>,
    card: CardSize,
}

/// The finish a pulled card shows (card-templates session, docs/CARD_TEMPLATES.md and
/// tools/cardgen `foil_finish`): the plain print, or for a foil pull reverse holo on Commons and
/// Uncommons and holo on Rares and Mythics.
pub fn finish(foil: bool, rarity: Rarity) -> &'static str {
    match (foil, rarity) {
        (false, _) => "normal",
        (true, Rarity::Common | Rarity::Uncommon) => "reverse",
        (true, _) => "holo",
    }
}

/// The card's prebaked face (`cardgen --set N` and `--set N --finish foil`): its own finish, else
/// the plain print. Faces are never rendered while playing.
fn face_dir(pull: Pull, rarity: Rarity) -> Option<PathBuf> {
    let root = Path::new("assets/cards/templates/cards");
    [finish(pull.foil, rarity), "normal"].into_iter().map(|f| root.join(format!("{}_{f}", pull.id))).find(|d| d.join("base.png").exists())
}

impl Gpu {
    fn load(r: &mut Renderer, review: bool, card: CardSize) -> anyhow::Result<Self> {
        let reg = assets::Registry::open(assets::REGISTRY_PATH)?;
        let get = |r: &mut Renderer, id: &str| if review { reg.load_for_review(r, id) } else { reg.load_model(r, id) };
        let binder = get(r, BINDER_ID)?;
        let sheet = get(r, SHEET_ID)?;
        let back = card_material(r, Path::new("assets/cards/templates/back"))?;
        let edge = r.add_material(Material::color([0.86, 0.85, 0.82], 0.7));
        let ids = r.add_meshes(&card_meshes(back, back, edge, card))?;
        Ok(Gpu {
            binder: binder.parts.iter().map(|p| (p.mesh, p.transform)).collect(),
            sheet: sheet.parts.iter().map(|p| (p.mesh, p.transform)).collect(),
            back_kit: [ids[0], ids[1], ids[2]],
            back,
            edge,
            faces: HashMap::new(),
            card,
        })
    }

    /// The card's meshes (front, back, edge); the plain back until its face is loaded.
    fn kit(&mut self, r: &mut Renderer, pull: Pull, rarity: Rarity, budget: &mut usize) -> [MeshId; 3] {
        let key = (pull.id, pull.foil);
        match self.faces.get(&key) {
            Some(Face::Ready(k)) => return *k,
            Some(Face::Missing) => return self.back_kit,
            None => {}
        }
        let Some(dir) = face_dir(pull, rarity) else {
            eprintln!("binder: no face baked for {} (run tools/cardgen --set {})", pull.id, pull.id.set);
            self.faces.insert(key, Face::Missing);
            return self.back_kit;
        };
        if *budget == 0 {
            return self.back_kit;
        }
        *budget -= 1;
        let kit = card_material(r, &dir).and_then(|front| {
            let ids = r.add_meshes(&card_meshes(front, self.back, self.edge, self.card))?;
            Ok([ids[0], ids[1], ids[2]])
        });
        match kit {
            Ok(k) => {
                self.faces.insert(key, Face::Ready(k));
                k
            }
            Err(e) => {
                eprintln!("binder: {}: {e:#}", dir.display());
                self.faces.insert(key, Face::Missing);
                self.back_kit
            }
        }
    }
}

/// A card face as a material (tools/cardtest `template_material`): print, emboss normal, ORM,
/// and the foil mask in the emissive slot under MAT_CARD_FOIL.
fn card_material(r: &mut Renderer, dir: &Path) -> anyhow::Result<MaterialId> {
    let f = |n: &str| dir.join(n);
    for n in ["base.png", "normal.png", "orm.png", "foil.png"] {
        anyhow::ensure!(f(n).exists(), "missing {}", f(n).display());
    }
    let job = |n: &str, kind| TexJob { key: file_key(&f(n)), name: format!("{} {n}", dir.display()), kind, source: TexSource::File(f(n)) };
    let t = r.add_textures(&[job("base.png", TexKind::Color), job("normal.png", TexKind::Normal), job("orm.png", TexKind::Data), job("foil.png", TexKind::Data)])?;
    Ok(r.add_material(Material {
        base_color: [1.0; 4],
        roughness: 1.0,
        metallic: 1.0,
        base_tex: Some(t[0]),
        normal_tex: Some(t[1]),
        mr_tex: Some(t[2]),
        occlusion_tex: Some(t[2]),
        emissive_tex: Some(t[3]),
        normal_scale: 1.0,
        flags: MAT_CARD_FOIL,
        ..Default::default()
    }))
}

/// The card slab: front face (+Z), back face (mirrored u) and the white-core edge
/// (tools/cardtest `card_meshes`).
fn card_meshes(front: MaterialId, back: MaterialId, edge: MaterialId, card: CardSize) -> Vec<MeshData> {
    let (cw, ch) = (card.w, card.h);
    let ru = card.corner / cw;
    let rv = card.corner / ch;
    let mut ol = Vec::new();
    for (cx, cy, a0) in [(0.5 - ru, -(0.5 - rv), -0.5f32), (0.5 - ru, 0.5 - rv, 0.0), (-(0.5 - ru), 0.5 - rv, 0.5), (-(0.5 - ru), -(0.5 - rv), 1.0)] {
        for k in 0..=8 {
            let a = (a0 + k as f32 / 8.0 * 0.5) * PI;
            ol.push((cx + a.cos() * ru + 0.5, 0.5 - (cy + a.sin() * rv)));
        }
    }
    let n = ol.len();
    let pt = |u: f32, v: f32, z: f32| [(u - 0.5) * cw, (0.5 - v) * ch, z];
    let z = card.t * 0.5;
    let face = |name: &str, mat: MaterialId, zf: f32, nz: f32, mirror: bool| {
        let mut m = MeshData { name: name.into(), material: Some(mat), ..Default::default() };
        let uv = |u: f32, v: f32| if mirror { [1.0 - u, v] } else { [u, v] };
        m.positions.push(pt(0.5, 0.5, zf));
        m.normals.push([0.0, 0.0, nz]);
        m.uvs.push(uv(0.5, 0.5));
        for &(u, v) in &ol {
            m.positions.push(pt(u, v, zf));
            m.normals.push([0.0, 0.0, nz]);
            m.uvs.push(uv(u, v));
        }
        for i in 0..n as u32 {
            let (a, b) = (1 + i, 1 + (i + 1) % n as u32);
            if nz > 0.0 { m.indices.extend([0, a, b]) } else { m.indices.extend([0, b, a]) }
        }
        m.generate_tangents();
        m
    };
    let mut meshes = vec![face("card front", front, z, 1.0, false), face("card back", back, -z, -1.0, true)];
    let mut e = MeshData { name: "card edge".into(), material: Some(edge), ..Default::default() };
    for i in 0..n {
        let (u0, v0) = ol[i];
        let (u1, v1) = ol[(i + 1) % n];
        let (p0, p1) = (pt(u0, v0, 0.0), pt(u1, v1, 0.0));
        let d = Vec3::new(p1[0] - p0[0], p1[1] - p0[1], 0.0);
        let nrm = Vec3::new(d.y, -d.x, 0.0).normalize_or_zero();
        let base = e.positions.len() as u32;
        for (p, zz) in [(p0, z), (p0, -z), (p1, -z), (p1, z)] {
            e.positions.push([p[0], p[1], zz]);
            e.normals.push(nrm.to_array());
            e.uvs.push([0.0, 0.0]);
        }
        e.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    meshes.push(e);
    meshes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (BinderView, Binder, Collection) {
        let mut col = Collection::default();
        for n in 1..=5 {
            col.add(Pull { id: CardId { set: 1, number: n }, foil: false });
        }
        let mut v = BinderView::new();
        v.open(Vec3::new(0.0, 1.42, 0.0), 0.0, &Camera { fov_y: 65f32.to_radians(), ..Default::default() });
        let b = Binder::default();
        v.update(&b, &b.loose(&col), 1.0);
        (v, b, col)
    }

    fn settle(v: &mut BinderView, b: &Binder, col: &Collection) {
        for _ in 0..240 {
            v.update(b, &b.loose(col), 1.0 / 60.0);
        }
    }

    fn screen_of(v: &BinderView, b: &Binder, col: &Collection, s: Spot) -> Vec2 {
        v.project(v.spot_point(b, &b.loose(col), s).unwrap()).unwrap()
    }

    #[test]
    fn click_to_pick_up_and_click_to_put_in_then_pull_out() {
        let (mut v, mut b, col) = setup();
        let loose = b.loose(&col);
        v.focus = Spot::Fan(2);
        v.click(&mut b, &col, &loose);
        let card = loose[2].0;
        assert_eq!(v.held.map(|h| h.pull), Some(card));
        v.focus = Spot::Pocket { page: 0, pocket: 4 };
        v.click(&mut b, &col, &loose);
        assert!(v.held.is_none());
        assert_eq!(b.get(0, 4), Some(card));
        settle(&mut v, &b, &col);
        assert!(!v.busy());
        // and out again: it slides up out of the sleeve into the hand
        let loose = b.loose(&col);
        v.click(&mut b, &col, &loose);
        assert_eq!(b.get(0, 4), None);
        assert_eq!(v.held.map(|h| h.pull), Some(card));
        assert!(v.busy());
        settle(&mut v, &b, &col);
        assert!(!v.busy());
    }

    #[test]
    fn drag_and_drop_with_the_mouse() {
        let (mut v, mut b, col) = setup();
        let from = screen_of(&v, &b, &col, Spot::Fan(1));
        let to = screen_of(&v, &b, &col, Spot::Pocket { page: 0, pocket: 8 });
        let loose = b.loose(&col);
        v.point(&b, &loose, from);
        assert_eq!(v.focus, Spot::Fan(1));
        v.press(&mut b, &col, &loose);
        let card = v.held.unwrap().pull;
        for k in 1..=10 {
            v.point(&b, &loose, from.lerp(to, k as f32 / 10.0));
        }
        assert_eq!(v.focus, Spot::Pocket { page: 0, pocket: 8 });
        v.release(&mut b, &col, &loose);
        assert_eq!(b.get(0, 8), Some(card));
        assert!(v.held.is_none());
        // drag it out again and drop it on the table: it goes back to the loose cards
        settle(&mut v, &b, &col);
        let loose = b.loose(&col);
        v.point(&b, &loose, to);
        v.press(&mut b, &col, &loose);
        settle(&mut v, &b, &col);
        v.point(&b, &loose, Vec2::new(10.0, 10.0));
        let l2 = b.loose(&col);
        v.release(&mut b, &col, &l2);
        assert_eq!(b.total(), 0);
        assert!(v.held.is_none());
    }

    #[test]
    fn dropping_on_a_full_pocket_swaps() {
        let (mut v, mut b, col) = setup();
        let loose = b.loose(&col);
        assert!(b.put(&col, 0, 0, loose[0].0));
        let loose = b.loose(&col);
        v.focus = Spot::Fan(0);
        v.click(&mut b, &col, &loose);
        let a = v.held.unwrap().pull;
        v.focus = Spot::Pocket { page: 0, pocket: 0 };
        v.click(&mut b, &col, &loose);
        assert_eq!(b.get(0, 0), Some(a));
        assert_eq!(v.held.map(|h| h.pull), Some(Pull { id: CardId { set: 1, number: 1 }, foil: false }));
    }

    #[test]
    fn quick_move_fills_the_next_empty_pocket() {
        let (mut v, mut b, col) = setup();
        let loose = b.loose(&col);
        v.focus = Spot::Fan(0);
        v.quick(&mut b, &col, &loose);
        assert_eq!(b.get(0, 0), Some(loose[0].0));
        v.focus = Spot::Pocket { page: 0, pocket: 0 };
        settle(&mut v, &b, &col);
        let l2 = b.loose(&col);
        v.quick(&mut b, &col, &l2);
        assert_eq!(b.get(0, 0), None);
        assert!(v.held.is_none(), "it went back to the loose cards");
    }

    #[test]
    fn pages_turn_on_a_spring_and_queue_when_flicked() {
        let (mut v, b, col) = setup();
        assert_eq!(v.pages(&b), (None, Some(0)));
        assert!(v.turn(&b, 1));
        settle(&mut v, &b, &col);
        assert_eq!(v.spread, 1);
        assert_eq!(v.pages(&b), (Some(1), Some(2)));
        // three quick presses: three pages
        v.turn(&b, 1);
        v.turn(&b, 1);
        v.turn(&b, 1);
        settle(&mut v, &b, &col);
        settle(&mut v, &b, &col);
        assert_eq!(v.spread, 4);
        assert!(v.turn(&b, -1));
        settle(&mut v, &b, &col);
        assert_eq!(v.spread, 3);
    }

    #[test]
    fn dragging_a_page_turns_it_and_a_short_drag_lets_it_fall_back() {
        let (mut v, mut b, col) = setup();
        let loose = b.loose(&col);
        // grab the right-hand page by its margin, outside the pockets
        let right = v.project((v.rig * v.sheet_pose(0, b.sheets.len())).transform_point3(Vec3::new(v.layout.sheet_x1 + 0.004, 0.0, 0.0))).unwrap();
        v.point(&b, &loose, right);
        assert_eq!(v.focus, Spot::Page(true));
        v.press(&mut b, &col, &loose);
        v.point(&b, &loose, right - Vec2::new(60.0, 0.0));
        settle(&mut v, &b, &col);
        v.release(&mut b, &col, &loose);
        settle(&mut v, &b, &col);
        assert_eq!(v.spread, 0, "a short drag falls back");
        v.point(&b, &loose, right);
        v.press(&mut b, &col, &loose);
        for k in 1..=20 {
            v.point(&b, &loose, right - Vec2::new(k as f32 * 50.0, 0.0));
            v.update(&b, &loose, 1.0 / 60.0);
        }
        v.release(&mut b, &col, &loose);
        settle(&mut v, &b, &col);
        assert_eq!(v.spread, 1);
    }

    #[test]
    fn the_pointer_finds_pockets_and_the_fan() {
        let (v, b, col) = setup();
        let loose = b.loose(&col);
        for pocket in [0, 4, 8] {
            let s = screen_of(&v, &b, &col, Spot::Pocket { page: 0, pocket });
            assert!(s.x > 0.0 && s.x < v.screen.x && s.y > 0.0 && s.y < v.screen.y, "pocket {pocket} off screen at {s}");
            assert_eq!(v.spot_at(&b, &loose, s), Spot::Pocket { page: 0, pocket });
        }
        let s = screen_of(&v, &b, &col, Spot::Fan(1));
        assert_eq!(v.spot_at(&b, &loose, s), Spot::Fan(1));
    }

    #[test]
    fn keys_walk_the_pockets_and_turn_at_the_edge() {
        let (mut v, b, col) = setup();
        let loose = b.loose(&col);
        v.step(&b, &loose, 0, -1); // up from the fan into the pages
        assert!(matches!(v.focus, Spot::Pocket { page: 0, .. }));
        for _ in 0..3 {
            v.step(&b, &loose, 1, 0);
        }
        assert!(v.turn.is_some(), "stepping off the right edge turns the page");
    }

    #[test]
    fn sheets_hang_on_the_rings() {
        let (v, b, _) = setup();
        let n = b.sheets.len();
        for i in 0..n {
            let hole = Vec3::from(v.sheet_pose(i, n).translation);
            let d = Vec2::new(hole.x, hole.y - v.layout.ring_cz).length();
            assert!((d - v.layout.ring_r).abs() < 1e-5);
        }
    }

    #[test]
    fn pockets_fit_the_card() {
        let l = Layout::load();
        for row in l.pockets {
            for [x0, x1, y0, y1] in row {
                assert!(x1 - x0 > l.card.w + 2.0 * MM && x1 - x0 < l.card.w + 5.0 * MM);
                assert!(y1 - y0 > l.card.h + 3.0 * MM && y1 - y0 < l.card.h + 6.0 * MM);
            }
        }
        assert!(l.film * 2.0 > 2.0 * l.card.t);
    }

    #[test]
    fn foil_finish_follows_rarity() {
        assert_eq!(finish(false, Rarity::Mythic), "normal");
        assert_eq!(finish(true, Rarity::Common), "reverse");
        assert_eq!(finish(true, Rarity::Uncommon), "reverse");
        assert_eq!(finish(true, Rarity::Rare), "holo");
        assert_eq!(finish(true, Rarity::Mythic), "holo");
    }
}
