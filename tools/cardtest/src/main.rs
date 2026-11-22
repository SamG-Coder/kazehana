//! Akari card test: the Mythbeasts templates as real ray-traced objects.
//!
//! Every card is a 63 x 88 x 0.31 mm slab with rounded corners: front (base / normal / ORM from
//! tools/cardgen), back (the shared back) and a white-core edge. The 14 fronts and a face-up
//! back lie on a play mat; a hero card stands at the front and turns on a turntable so the
//! emboss, metallic ink and foil can be judged under the path tracer's sun, sky and a desk lamp.
//!
//! Interactive: WASD/QE move, mouse look (click to capture, Esc release), Space pause the
//! turntable, Left/Right step the hero card, 0-2 debug views, F12 screenshot.
//! `--gate` renders the fixed shot list to assets/cards/templates/engine_test/ and exits.
//! Options: --hero front_holo  --night  --no-dlss
//!
//! Booster packs (P, or start with --pack [--format booster6]): a sealed pack floats in front of
//! you. Grab it at a TEAR notch and drag across the top to peel the strip off; the cards slide
//! up, click (or Space) to draw them one by one. Tab changes the pack format, N opens a new pack.
//! `--pack-test` scripts a whole opening and saves stills to assets/cards/packs/engine_test/.

use akari::glam::{Affine3A, Quat, Vec3};
use akari::textures::{file_key, TexJob, TexKind, TexSource};
use akari::winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use akari::*;
use anyhow::{Context, Result};

mod pack;
use pack::{Phase, PackRig};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const CARD_W: f32 = 0.063;
const CARD_H: f32 = 0.088;
const CARD_T: f32 = 0.00031;
const CORNER: f32 = 0.00294;

const TEMPLATES: [&str; 14] = [
    "front_common", "front_uncommon", "front_rare", "front_mythic", "front_holo", "front_reverse", "front_fullart",
    "front_gold", "front_cosmos", "front_ice", "front_rainbow", "front_glass", "front_sakura", "front_obsidian",
];

fn card_point(u: f32, v: f32, z: f32) -> [f32; 3] {
    [(u - 0.5) * CARD_W, (0.5 - v) * CARD_H, z]
}

/// The card slab as three meshes: front face, back face, edge.
fn card_meshes(front: MaterialId, back: MaterialId, edge: MaterialId) -> Vec<MeshData> {
    let ol = outline_ccw();
    let n = ol.len();
    let z = CARD_T * 0.5;
    let face = |name: &str, mat: MaterialId, zf: f32, nz: f32, mirror: bool| {
        let mut m = MeshData { name: name.into(), material: Some(mat), ..Default::default() };
        let uvof = |u: f32, v: f32| if mirror { [1.0 - u, v] } else { [u, v] };
        m.positions.push(card_point(0.5, 0.5, zf));
        m.normals.push([0.0, 0.0, nz]);
        m.uvs.push(uvof(0.5, 0.5));
        for &(u, v) in &ol {
            m.positions.push(card_point(u, v, zf));
            m.normals.push([0.0, 0.0, nz]);
            m.uvs.push(uvof(u, v));
        }
        for i in 0..n as u32 {
            let a = 1 + i;
            let b = 1 + (i + 1) % n as u32;
            if nz > 0.0 { m.indices.extend([0, a, b]) } else { m.indices.extend([0, b, a]) }
        }
        m.generate_tangents();
        m
    };
    let mut meshes = vec![face("card front", front, z, 1.0, false), face("card back", back, -z, -1.0, true)];
    // Back texture: viewed from behind, image-right is world -x, so mirror u.
    let mut e = MeshData { name: "card edge".into(), material: Some(edge), ..Default::default() };
    for i in 0..n {
        let (u0, v0) = ol[i];
        let (u1, v1) = ol[(i + 1) % n];
        let p0 = card_point(u0, v0, 0.0);
        let p1 = card_point(u1, v1, 0.0);
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

/// Counter-clockwise (seen from +z, the front) outline in UV space.
fn outline_ccw() -> Vec<(f32, f32)> {
    let ru = CORNER / CARD_W;
    let rv = CORNER / CARD_H;
    let mut pts = Vec::new();
    // In world space (x right, y up) walk: bottom-right corner, top-right, top-left, bottom-left.
    // A world point (x, y) maps to u = x/W + 0.5, v = 0.5 - y/H.
    let corners = [(0.5 - ru, -(0.5 - rv), -0.5f32), (0.5 - ru, 0.5 - rv, 0.0), (-(0.5 - ru), 0.5 - rv, 0.5), (-(0.5 - ru), -(0.5 - rv), 1.0)];
    for (cx, cy, a0) in corners {
        for k in 0..=8 {
            let a = (a0 + k as f32 / 8.0 * 0.5) * std::f32::consts::PI;
            let x = cx + a.cos() * ru;
            let y = cy + a.sin() * rv;
            pts.push((x + 0.5, 0.5 - y));
        }
    }
    pts
}

struct CardKit {
    /// (front, back, edge) mesh ids per template, plus the back-only kit.
    meshes: Vec<[MeshId; 3]>,
}

fn template_material(r: &mut Renderer, dir: &Path) -> Result<MaterialId> {
    let f = |n: &str| dir.join(n);
    for n in ["base.png", "normal.png", "orm.png"] {
        anyhow::ensure!(f(n).exists(), "missing {} (run tools/cardgen first)", f(n).display());
    }
    let jobs = vec![
        TexJob { key: file_key(&f("base.png")), name: format!("{} base", dir.display()), kind: TexKind::Color, source: TexSource::File(f("base.png")) },
        TexJob { key: file_key(&f("normal.png")), name: format!("{} normal", dir.display()), kind: TexKind::Normal, source: TexSource::File(f("normal.png")) },
        TexJob { key: file_key(&f("orm.png")), name: format!("{} orm", dir.display()), kind: TexKind::Data, source: TexSource::File(f("orm.png")) },
    ];
    #[allow(unused_mut)]
    let mut jobs = jobs;
    // With the Akari foil patch: the foil mask rides in the emissive slot under MAT_CARD_FOIL.
    #[cfg(feature = "foil")]
    jobs.push(TexJob { key: file_key(&f("foil.png")), name: format!("{} foil", dir.display()), kind: TexKind::Data, source: TexSource::File(f("foil.png")) });
    let t = r.add_textures(&jobs)?;
    #[allow(unused_mut)]
    let mut m = Material {
        base_color: [1.0; 4],
        roughness: 1.0,
        metallic: 1.0,
        base_tex: Some(t[0]),
        normal_tex: Some(t[1]),
        mr_tex: Some(t[2]),
        occlusion_tex: Some(t[2]),
        normal_scale: 1.0,
        ..Default::default()
    };
    #[cfg(feature = "foil")]
    {
        m.emissive_tex = Some(t[3]);
        m.flags |= akari::scene::MAT_CARD_FOIL;
    }
    Ok(r.add_material(m))
}

struct Scene {
    statics: Vec<Instance>,
    hero_meshes: [MeshId; 3],
    hero_index: usize,
    kits: CardKit,
    lamp: Light,
}

fn build_scene(r: &mut Renderer, root: &Path, hero: &str) -> Result<Scene> {
    let t0 = Instant::now();
    let tdir = root.join("assets/cards/templates");
    let back = template_material(r, &tdir.join("back"))?;
    let edge = r.add_material(Material::color([0.86, 0.85, 0.82], 0.7));
    let mut all_meshes = Vec::new();
    for t in TEMPLATES {
        let front = template_material(r, &tdir.join(t))?;
        all_meshes.extend(card_meshes(front, back, edge));
    }
    // A back-up card: same back on both faces.
    all_meshes.extend(card_meshes(back, back, edge));

    // Table, play mat and a backdrop wall.
    let wood = r.add_material(Material::color([0.20, 0.12, 0.07], 0.42));
    let mat = r.add_material(Material::color([0.07, 0.08, 0.14], 0.95));
    let wall = r.add_material(Material::color([0.35, 0.33, 0.31], 0.9));
    let mut table = MeshBuilder::new("table", wood, 1.0);
    table.aabb(Vec3::new(-1.2, -0.04, -0.8), Vec3::new(1.2, 0.0, 0.8));
    let mut playmat = MeshBuilder::new("playmat", mat, 1.0);
    playmat.aabb(Vec3::new(-0.34, 0.0, -0.36), Vec3::new(0.34, 0.002, -0.02));
    let mut wallb = MeshBuilder::new("wall", wall, 1.0);
    wallb.aabb(Vec3::new(-2.0, -0.04, -1.2), Vec3::new(2.0, 1.6, -1.15));
    all_meshes.push(table.finish());
    all_meshes.push(playmat.finish());
    all_meshes.push(wallb.finish());

    let ids = r.add_meshes(&all_meshes)?;
    let mut kits = Vec::new();
    for k in 0..=TEMPLATES.len() {
        kits.push([ids[k * 3], ids[k * 3 + 1], ids[k * 3 + 2]]);
    }
    let base = (TEMPLATES.len() + 1) * 3;
    let mut statics: Vec<Instance> = (0..3).map(|i| Instance { mesh: ids[base + i], transform: Affine3A::IDENTITY }).collect();

    // Lay the 14 fronts + one face-up back on the mat in a 5 x 3 grid.
    let lie = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for k in 0..15 {
        let (col, row) = ((k % 5) as f32, (k / 5) as f32);
        let x = (col - 2.0) * 0.072;
        let zc = -0.30 + row * 0.098;
        let jitter = ((k as f32 * 12.9898).sin() * 43758.545).fract() - 0.5;
        let rot = Quat::from_rotation_y(jitter * 0.12) * lie;
        let kit = if k < 14 { kits[k] } else { kits[14] };
        let t = Affine3A::from_rotation_translation(rot, Vec3::new(x, 0.002 + CARD_T * 0.5 + 0.0002, zc));
        for m in kit {
            statics.push(Instance { mesh: m, transform: t });
        }
    }
    let hero_index = TEMPLATES.iter().position(|t| *t == hero).unwrap_or(4);
    let lamp = Light {
        position: Vec3::new(-0.22, 0.38, 0.22),
        radius: 0.035,
        intensity: Vec3::new(1.0, 0.82, 0.62) * 0.05,
        range: 6.0,
        spot: None,
    };
    println!("scene built in {:.1}s", t0.elapsed().as_secs_f32());
    Ok(Scene { statics, hero_meshes: kits[hero_index], hero_index, kits: CardKit { meshes: kits }, lamp })
}

/// Hero card standing on the table at the front, turned by `yaw` and leaning back a little.
fn hero_transform(yaw: f32) -> Affine3A {
    let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-0.18);
    Affine3A::from_rotation_translation(rot, Vec3::new(0.0, CARD_H * 0.5 * 0.985 + 0.0005, 0.06))
}

struct Shot {
    name: &'static str,
    hero: Option<&'static str>,
    yaw: f32,
    cam: Vec3,
    look: Vec3,
}

fn shots() -> Vec<Shot> {
    let hero_cam = Vec3::new(0.0, 0.075, 0.27);
    let hero_look = Vec3::new(0.0, 0.045, 0.06);
    let mut v = vec![Shot { name: "overview", hero: Some("front_holo"), yaw: 0.35, cam: Vec3::new(0.0, 0.42, 0.36), look: Vec3::new(0.0, 0.0, -0.12) }];
    for (hero, tag) in [("front_holo", "holo"), ("front_gold", "gold"), ("front_obsidian", "obsidian"), ("front_ice", "ice"), ("front_mythic", "mythic"), ("front_rainbow", "rainbow")] {
        for (i, yaw) in [-0.55f32, 0.0, 0.55].into_iter().enumerate() {
            let name: &'static str = Box::leak(format!("hero_{tag}_{i}").into_boxed_str());
            v.push(Shot { name, hero: Some(hero), yaw, cam: hero_cam, look: hero_look });
        }
    }
    v.push(Shot { name: "hero_back", hero: Some("front_holo"), yaw: std::f32::consts::PI + 0.3, cam: hero_cam, look: hero_look });
    v
}

fn aim(cam: &mut Camera, from: Vec3, to: Vec3) {
    let d = (to - from).normalize();
    cam.position = from;
    cam.pitch = d.y.asin();
    cam.yaw = d.x.atan2(-d.z);
}

struct Args {
    gate: bool,
    /// Drive the inspector like a user (pick, rotate, move lamp), capture the window, quit.
    selftest: bool,
    hero: String,
    night: bool,
    dlss: bool,
    /// Start in booster-pack mode with this format.
    pack: bool,
    format: String,
    /// Script a pack opening (hidden window) and save stills.
    pack_test: bool,
}

fn parse_args() -> Args {
    let mut a = Args { gate: false, selftest: false, hero: "front_holo".into(), night: false, dlss: true, pack: false, format: "booster6".into(), pack_test: false };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        match v[i].as_str() {
            "--gate" => a.gate = true,
            "--selftest" => a.selftest = true,
            "--night" => a.night = true,
            "--no-dlss" => a.dlss = false,
            "--pack" => a.pack = true,
            "--pack-test" => {
                a.pack = true;
                a.pack_test = true;
            }
            "--format" => {
                a.format = v.get(i + 1).cloned().unwrap_or_default();
                i += 1;
            }
            "--hero" => {
                a.hero = v.get(i + 1).cloned().unwrap_or_default();
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    a
}

/// Where the inspected card floats, above the table and in front of the cards on the mat.
const INSPECT: Vec3 = Vec3::new(0.0, 0.125, 0.06);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lighting {
    /// Dim room + bright desk lamp you can move (best for judging shine).
    Lamp,
    /// Late-afternoon sun and sky, lamp on.
    Sun,
    /// Lamp only, black sky.
    Dark,
}

impl Lighting {
    fn next(self) -> Lighting {
        match self {
            Lighting::Lamp => Lighting::Sun,
            Lighting::Sun => Lighting::Dark,
            Lighting::Dark => Lighting::Lamp,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Lighting::Lamp => "desk lamp + dim room",
            Lighting::Sun => "sunlight",
            Lighting::Dark => "lamp only",
        }
    }
}

/// Card names and labels from assets/cards/templates/templates.json (a tiny line parser).
fn card_labels(root: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root.join("assets/cards/templates/templates.json")).unwrap_or_default();
    let field = |line: &str, key: &str| -> Option<String> {
        let k = format!("\"{key}\": \"");
        let i = line.find(&k)? + k.len();
        let j = line[i..].find('"')? + i;
        Some(line[i..j].to_string())
    };
    let mut out: Vec<String> = TEMPLATES
        .iter()
        .map(|t| {
            text.lines()
                .find(|l| field(l, "template").as_deref() == Some(*t))
                .map(|l| format!("{}  -  {}  ({})", field(l, "label").unwrap_or_default(), field(l, "name").unwrap_or_default(), field(l, "card").unwrap_or_default()))
                .unwrap_or_else(|| t.to_string())
        })
        .collect();
    out.push("Card back".into());
    out
}

struct App {
    args: Args,
    root: PathBuf,
    window: Option<Arc<Window>>,
    r: Option<Renderer>,
    scene: Option<Scene>,
    ui: Option<Ui>,
    labels: Vec<String>,
    cam: Camera,
    last: Instant,
    start: Instant,
    // gate turntable
    yaw: f32,
    // inspector state
    card_yaw: f32,
    card_pitch: f32,
    flipped: bool,
    spin: bool,
    dist: f32,
    lamp_az: f32,
    lamp_el: f32,
    lighting: Lighting,
    drag_left: bool,
    drag_right: bool,
    cursor: Option<(f64, f64)>,
    show_help: bool,
    shots: Vec<Shot>,
    shot_i: usize,
    shot_frames: u32,
    shot_request: Option<PathBuf>,
    // booster pack mode
    pack_mode: bool,
    rig: Option<PackRig>,
    format_i: usize,
    db: tcg::CardDb,
    press_at: Option<(f64, f64)>,
    press_moved: bool,
    test_frame: u32,
    /// Camera look-at offset from the inspected object (pack close-ups).
    look_off: Vec3,
}

/// Pack formats from tools/cardgen --packs.
const FORMATS: [&str; 4] = ["booster6", "jp5", "retail10", "premium11"];

/// Which template shows a card of this rarity (the foil shows as the holo template).
fn kit_of(r: tcg::Rarity, foil: bool) -> usize {
    let name = if foil {
        "front_holo"
    } else {
        match r {
            tcg::Rarity::Common => "front_common",
            tcg::Rarity::Uncommon => "front_uncommon",
            tcg::Rarity::Rare => "front_rare",
            tcg::Rarity::Mythic => "front_mythic",
        }
    };
    TEMPLATES.iter().position(|t| *t == name).unwrap_or(0)
}

const GATE_FRAMES: u32 = 72;

impl App {
    fn apply_shot(&mut self) {
        let s = &self.shots[self.shot_i];
        aim(&mut self.cam, s.cam, s.look);
        self.yaw = s.yaw;
        if let (Some(h), Some(sc)) = (s.hero, self.scene.as_mut()) {
            let i = TEMPLATES.iter().position(|t| *t == h).unwrap_or(4);
            sc.hero_index = i;
            sc.hero_meshes = sc.kits.meshes[i];
        }
        if let Some(r) = &mut self.r {
            r.reset_history();
        }
    }

    fn pick(&mut self, i: usize) {
        if let Some(sc) = &mut self.scene {
            let n = sc.kits.meshes.len();
            sc.hero_index = i % n;
            sc.hero_meshes = sc.kits.meshes[sc.hero_index];
        }
    }

    /// Build (or rebuild) the pack rig for FORMATS[format_i].
    fn load_pack(&mut self) -> Result<()> {
        let slug = FORMATS[self.format_i % FORMATS.len()];
        let dir = self.root.join("assets/cards/packs").join(slug);
        let spec = pack::Spec::load(&dir)?;
        let r = self.r.as_mut().unwrap();
        let mats = pack::wrapper_mats(r, &dir)?;
        let meshes = pack::build_meshes(r, &spec, &mats)?;
        let seed = self.rig.as_ref().map_or(7, |g| g.seed + 1);
        let contents = pack::roll(&self.db, spec.cards, seed, &kit_of);
        self.rig = Some(PackRig::new(spec, meshes, contents, seed));
        Ok(())
    }

    fn new_pack(&mut self) {
        if let Some(g) = self.rig.as_mut() {
            let seed = g.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 1;
            let contents = pack::roll(&self.db, g.spec.cards, seed, &kit_of);
            g.reset(contents, seed);
        }
    }

    /// Where the inspected object sits (card or pack).
    fn pose(&self) -> Affine3A {
        let yaw = self.card_yaw + if self.flipped && !self.pack_mode { std::f32::consts::PI } else { 0.0 };
        Affine3A::from_rotation_translation(Quat::from_rotation_x(self.card_pitch) * Quat::from_rotation_y(yaw), INSPECT)
    }

    /// The cursor ray hitting the pack's plane, in pack space (mm).
    fn cursor_on_pack(&self, cx: f64, cy: f64) -> Option<Vec3> {
        let win = self.window.as_ref()?;
        let sz = win.inner_size();
        let (w, h) = (sz.width.max(1) as f32, sz.height.max(1) as f32);
        let fwd = self.cam.forward();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let th = (self.cam.fov_y * 0.5).tan();
        let nx = (cx as f32 / w) * 2.0 - 1.0;
        let ny = 1.0 - (cy as f32 / h) * 2.0;
        let dir = (fwd + right * nx * th * (w / h) + up * ny * th).normalize();
        let inv = self.pose().inverse();
        let o = inv.transform_point3(self.cam.position);
        let d = inv.transform_vector3(dir);
        if d.z.abs() < 1e-6 {
            return None;
        }
        let t = -o.z / d.z;
        if t <= 0.0 {
            return None;
        }
        Some((o + d * t) / pack::PACK_MM)
    }

    /// Screen position of a pack-space point (mm).
    fn to_screen(&self, p: Vec3, w: u32, h: u32) -> Option<(f32, f32)> {
        let world = self.pose().transform_point3(p * pack::PACK_MM);
        let c = self.cam.view_proj(w as f32 / h as f32) * world.extend(1.0);
        if c.w <= 0.0 {
            return None;
        }
        Some(((c.x / c.w * 0.5 + 0.5) * w as f32, (c.y / c.w * 0.5 + 0.5) * h as f32))
    }

    fn draw_pack_ui(&mut self, w: u32, h: u32, dlss_on: bool) {
        let notches = self.rig.as_ref().map(|g| g.notches());
        let front = self.rig.as_ref().map(|g| g.front());
        let nscreen: Vec<(f32, f32)> = notches.map(|n| n.iter().filter_map(|p| self.to_screen(*p, w, h)).collect()).unwrap_or_default();
        let fscreen = front.and_then(|p| self.to_screen(p, w, h));
        let time = self.start.elapsed().as_secs_f32();
        let show_help = self.show_help;
        let Some(g) = self.rig.as_ref() else { return };
        let Some(ui) = self.ui.as_mut() else { return };
        ui.begin(w, h);
        let s = ui.scale();
        let sp = &g.spec;
        let title = format!("Booster pack  \u{00b7}  {}", sp.label);
        let sub = format!("{} cards  \u{00b7}  {:.0} x {:.0} x {:.1} mm  \u{00b7}  {} packs per box  \u{00b7}  {}", sp.cards, sp.w, sp.l, sp.d, sp.box_packs, if dlss_on { "DLSS Ray Reconstruction" } else { "TAA" });
        let (x, y) = (24.0 * s, 20.0 * s);
        let tw = ui.measure(26.0 * s, &title).max(ui.measure(17.0 * s, &sub)) + 32.0 * s;
        ui.rect(x - 12.0 * s, y - 10.0 * s, tw, 76.0 * s, Rgba(10, 10, 14, 170));
        ui.text_shadow(x, y, 26.0 * s, Rgba(245, 238, 222, 255), &title);
        ui.text_shadow(x, y + 38.0 * s, 17.0 * s, Rgba(200, 200, 210, 230), &sub);
        // what to do now
        let hint = match g.phase {
            Phase::Sealed if g.progress == 0.0 && !g.tearing => "Grab a TEAR notch at the top corner and drag across".to_string(),
            Phase::Sealed if g.tearing => String::new(),
            Phase::Sealed => "Grab the flap and keep dragging to finish the tear".to_string(),
            Phase::Opening => String::new(),
            Phase::Ready => "Click to draw the cards".to_string(),
            Phase::Reveal => format!("Card {} of {}  \u{00b7}  click for the next", g.dealt + 1, g.contents.len()),
            Phase::Done => "N  open another pack".to_string(),
        };
        if !hint.is_empty() {
            let hw = ui.measure(22.0 * s, &hint) + 40.0 * s;
            let hy = h as f32 - 190.0 * s;
            ui.rect((w as f32 - hw) * 0.5, hy - 10.0 * s, hw, 44.0 * s, Rgba(10, 10, 14, 160));
            ui.text_centered(w as f32 * 0.5, hy, 22.0 * s, Rgba(250, 244, 230, 255), &hint);
        }
        if let Some(slot) = g.revealed() {
            let (label, col) = match (slot.foil, slot.rarity) {
                (true, _) => ("FOIL", Rgba(150, 230, 255, 255)),
                (_, tcg::Rarity::Mythic) => ("MYTHIC", Rgba(255, 140, 220, 255)),
                (_, tcg::Rarity::Rare) => ("RARE", Rgba(255, 214, 120, 255)),
                (_, tcg::Rarity::Uncommon) => ("UNCOMMON", Rgba(200, 220, 235, 255)),
                (_, tcg::Rarity::Common) => ("COMMON", Rgba(200, 200, 200, 255)),
            };
            if g.phase == Phase::Reveal {
                ui.text_centered(w as f32 * 0.5, 112.0 * s, 30.0 * s, col, label);
            }
        }
        // pulsing notch markers
        if g.phase == Phase::Sealed && g.progress == 0.0 && !g.tearing {
            let pulse = 0.5 + 0.5 * (time * 4.0).sin();
            for (nx, ny) in nscreen {
                let r = (9.0 + 5.0 * pulse) * s;
                ui.outline(nx - r, ny - r, r * 2.0, r * 2.0, 2.0 * s, Rgba(120, 255, 160, (120.0 + 120.0 * pulse) as u8));
            }
        }
        if g.phase == Phase::Sealed && g.progress > 0.0 && !g.tearing {
            if let Some((fx, fy)) = fscreen {
                let r = 8.0 * s;
                ui.outline(fx - r, fy - r, r * 2.0, r * 2.0, 2.0 * s, Rgba(120, 255, 160, 200));
            }
        }
        if show_help {
            let lines = [
                "Drag from a notch   tear the strip off",
                "Click / Space       draw the next card",
                "Left drag (elsewhere)  rotate    Right drag  move the lamp    Wheel  zoom",
                "Tab  pack format    N  new pack    P  cards / packs    L  lighting    R  reset view",
                "H  hide help    F12  screenshot    Esc  quit",
            ];
            let lh = 24.0 * s;
            let top = h as f32 - (lines.len() as f32 * lh) - 28.0 * s;
            ui.rect(x - 12.0 * s, top - 12.0 * s, 720.0 * s, lines.len() as f32 * lh + 24.0 * s, Rgba(10, 10, 14, 150));
            for (i, l) in lines.iter().enumerate() {
                ui.text_shadow(x, top + i as f32 * lh, 17.0 * s, Rgba(225, 225, 232, 240), l);
            }
        }
    }

    /// --pack-test: a scripted opening, stills saved to assets/cards/packs/engine_test/.
    fn pack_script(&mut self, el: &ActiveEventLoop) -> bool {
        self.test_frame += 1;
        let f = self.test_frame;
        let out = self.root.join("assets/cards/packs/engine_test");
        let tag = FORMATS[self.format_i % FORMATS.len()];
        let shot = |name: &str| Some(out.join(format!("{tag}_{name}.png")));
        if f == 1 {
            self.card_yaw = -0.32;
            self.card_pitch = 0.06;
        }
        if f == 70 {
            self.shot_request = shot("1_sealed");
        }
        if f == 72 {
            // close on the top of the pack for the tear
            self.card_yaw = -0.18;
            self.card_pitch = 0.1;
            self.dist = 0.16;
            self.look_off = Vec3::new(0.0, 0.042, 0.0);
        }
        if f == 200 {
            self.card_yaw = 0.0;
            self.card_pitch = 0.0;
            self.dist = 0.34;
            self.look_off = Vec3::ZERO;
        }
        if (90..200).contains(&f) {
            let t = (f - 90) as f32 / 110.0;
            if let Some(g) = self.rig.as_mut() {
                // an S-curve hand speed: slow start at the notch, faster across, steady finish
                g.script_tear(1.0, pack::ease(t) * 1.02);
            }
        }
        if f == 128 {
            self.shot_request = shot("2_tear_start");
        }
        if f == 150 {
            self.shot_request = shot("3_tear_half");
        }
        if f == 178 {
            self.shot_request = shot("4_tear_late");
        }
        if f == 330 {
            self.shot_request = shot("5_open");
        }
        if f == 340 {
            if let Some(g) = self.rig.as_mut() {
                g.click();
            }
        }
        if f == 420 {
            self.shot_request = shot("6_first_card");
        }
        let n = self.rig.as_ref().map_or(6, |g| g.contents.len()) as u32;
        if f > 430 && f < 430 + n * 30 && (f - 430) % 30 == 0 {
            if let Some(g) = self.rig.as_mut() {
                g.click();
            }
        }
        let end = 430 + n * 30;
        if f == end - 2 {
            self.shot_request = shot("7_last_card");
        }
        if f == end + 6 {
            if let Some(g) = self.rig.as_mut() {
                g.click();
            }
        }
        if f == end + 110 {
            self.shot_request = shot("8_fan");
        }
        if f == end + 130 {
            self.card_yaw = 0.42;
            self.card_pitch = -0.12;
        }
        if f == end + 200 {
            self.shot_request = shot("9_fan_tilt");
        }
        if f > end + 202 {
            el.exit();
            return false;
        }
        true
    }

    fn reset_view(&mut self) {
        self.card_yaw = 0.0;
        self.card_pitch = 0.0;
        self.flipped = false;
        self.dist = if self.pack_mode { 0.34 } else { 0.26 };
        self.lamp_az = -0.55;
        self.lamp_el = 0.55;
    }

    fn lamp(&self) -> Light {
        let base = self.scene.as_ref().unwrap().lamp;
        if self.args.gate {
            return base;
        }
        let r = 0.34;
        let dir = Vec3::new(self.lamp_az.sin() * self.lamp_el.cos(), self.lamp_el.sin(), self.lamp_az.cos() * self.lamp_el.cos());
        let bright = if self.lighting == Lighting::Sun { 1.0 } else { 1.6 };
        Light { position: INSPECT + dir * r, intensity: base.intensity * bright, ..base }
    }

    fn draw_ui(&mut self, w: u32, h: u32, dlss_on: bool) {
        let Some(ui) = self.ui.as_mut() else { return };
        ui.begin(w, h);
        let s = ui.scale();
        let hero = self.scene.as_ref().map_or(0, |sc| sc.hero_index);
        let n = self.labels.len();
        let title = format!("{} / {}   {}{}", hero + 1, n, self.labels.get(hero).map(String::as_str).unwrap_or(""), if self.flipped { "   (back side)" } else { "" });
        let (x, y) = (24.0 * s, 20.0 * s);
        let tw = ui.measure(26.0 * s, &title) + 32.0 * s;
        let sw = ui.measure(17.0 * s, "light: desk lamp + dim room   ·   denoiser: DLSS Ray Reconstruction   ·   drag to rotate") + 32.0 * s;
        ui.rect(x - 12.0 * s, y - 10.0 * s, tw.max(sw), 76.0 * s, Rgba(10, 10, 14, 170));
        ui.text_shadow(x, y, 26.0 * s, Rgba(245, 238, 222, 255), &title);
        let status = format!("light: {}   ·   denoiser: {}   ·   {}", self.lighting.name(), if dlss_on { "DLSS Ray Reconstruction" } else { "TAA" }, if self.spin { "spinning" } else { "drag to rotate" });
        ui.text_shadow(x, y + 38.0 * s, 17.0 * s, Rgba(200, 200, 210, 230), &status);
        if self.show_help {
            let lines = [
                "Left drag   rotate card",
                "Right drag  move the lamp around the card",
                "Wheel       zoom",
                "← / →  or 1-9  pick card    F  flip    Space  spin",
                "L  lighting    R  reset    H  hide help    F12  screenshot    Esc  quit",
            ];
            let lh = 24.0 * s;
            let top = h as f32 - (lines.len() as f32 * lh) - 28.0 * s;
            ui.rect(x - 12.0 * s, top - 12.0 * s, 560.0 * s, lines.len() as f32 * lh + 24.0 * s, Rgba(10, 10, 14, 150));
            for (i, l) in lines.iter().enumerate() {
                ui.text_shadow(x, top + i as f32 * lh, 17.0 * s, Rgba(225, 225, 232, 240), l);
            }
        }
    }

    fn frame(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let now = Instant::now();
        // the scripted pack test runs on a fixed 60 Hz clock so its stills land on the same moments
        let dt = if self.args.pack_test { 1.0 / 60.0 } else { (now - self.last).as_secs_f32().min(0.1) };
        self.last = now;
        let inspect = !self.args.gate;
        if self.args.selftest {
            self.shot_frames += 1;
            let f = self.shot_frames;
            if f == 1 {
                self.pick(TEMPLATES.iter().position(|t| *t == "front_cosmos").unwrap_or(0));
            }
            if (30..90).contains(&f) {
                // as if left-dragging the card, then right-dragging the lamp
                self.card_yaw += 0.006;
                self.card_pitch -= 0.002;
                self.lamp_az += 0.008;
            }
            if f == 200 {
                self.shot_request = Some(self.root.join("assets/cards/templates/engine_test/inspector_selftest.png"));
            }
            if f > 201 {
                el.exit();
                return Ok(());
            }
        }
        if self.args.pack_test && !self.pack_script(el) {
            return Ok(());
        }
        if inspect && self.spin {
            self.card_yaw += dt * 0.7;
        }
        if let Some(g) = self.rig.as_mut() {
            g.update(dt);
        }
        let lamp = self.lamp();
        let scene = self.scene.as_ref().unwrap();
        let mut instances = scene.statics.clone();
        let ht = if inspect { self.pose() } else { hero_transform(self.yaw) };
        if self.pack_mode {
            if let Some(g) = &self.rig {
                g.instances(ht, &scene.kits.meshes, &mut instances);
            }
        } else {
            for m in scene.hero_meshes {
                instances.push(Instance { mesh: m, transform: ht });
            }
        }
        if inspect {
            aim(&mut self.cam, INSPECT + self.look_off + Vec3::new(0.0, 0.02, self.dist), INSPECT + self.look_off);
        }
        let lights = [lamp];
        let (sun_i, stars) = match (inspect, self.lighting, self.args.night) {
            (false, _, true) => (0.0, 1.0),
            (false, _, false) => (1.0, 0.0),
            (true, Lighting::Sun, _) => (1.0, 0.0),
            (true, Lighting::Lamp, _) => (0.06, 0.0),
            (true, Lighting::Dark, _) => (0.0, 0.0),
        };
        let sun = Vec3::new(-0.55, 0.42, 0.72).normalize();
        let capture = self.shot_request.is_some() || (self.args.gate && self.shot_frames == GATE_FRAMES);
        let (w, h) = self.r.as_ref().unwrap().extent();
        let dlss_on = self.r.as_ref().unwrap().dlss_mode().is_some();
        if inspect {
            if self.pack_mode {
                self.draw_pack_ui(w, h, dlss_on);
            } else {
                self.draw_ui(w, h, dlss_on);
            }
        }
        let desc = FrameDesc {
            camera: &self.cam,
            sun_dir: sun,
            sun_illuminance: sun_i,
            moon_dir: Vec3::new(0.3, 0.6, -0.7).normalize(),
            moon_illuminance: 0.0,
            stars,
            altitude: 40.0,
            ground_albedo: 0.2,
            haze: 1.0,
            instances: &instances,
            lights: &lights,
            time: (now - self.start).as_secs_f32(),
            dt,
            exposure_ev: -0.9,
            debug: 0,
            capture,
            ui: if inspect && (self.shot_request.is_none() || self.args.selftest || self.args.pack_test) { self.ui.as_ref() } else { None },
        };
        let r = self.r.as_mut().unwrap();
        let img = r.render(&desc)?;
        if let Some(px) = img {
            let path = if self.args.gate {
                self.root.join(format!("assets/cards/templates/engine_test/{}.png", self.shots[self.shot_i].name))
            } else {
                self.shot_request.take().unwrap()
            };
            save_png(&path, w, h, &px)?;
            println!("saved {}", path.display());
        }
        if self.args.gate {
            self.shot_frames += 1;
            if self.shot_frames > GATE_FRAMES {
                self.shot_frames = 0;
                self.shot_i += 1;
                if self.shot_i >= self.shots.len() {
                    el.exit();
                    return Ok(());
                }
                self.apply_shot();
            }
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
        let headless = self.args.gate || self.args.pack_test;
        let attrs = Window::default_attributes().with_title("Mythbeasts card inspector").with_inner_size(PhysicalSize::new(1920, 1080)).with_visible(!headless).with_active(!headless);
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let opts = RendererOptions { validation: false, vsync: !headless, dlss: if self.args.dlss { Some(1) } else { None }, render_scale: 1.0, bounces: 3 };
        let mut r = match Renderer::new(&window, opts) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("renderer: {e:#}");
                el.exit();
                return;
            }
        };
        // the inspector's instance list keeps a stable order: give moving cards, the peeling strip
        // and the dealt cards real motion vectors (no denoiser ghost trails)
        r.set_object_motion(true);
        println!("denoiser: {}", match r.dlss_mode() { Some(m) => format!("DLSS Ray Reconstruction (mode {m})"), None => "TAA fallback (no DLSS)".into() });
        match build_scene(&mut r, &self.root, &self.args.hero) {
            Ok(s) => self.scene = Some(s),
            Err(e) => {
                eprintln!("scene: {e:#}");
                el.exit();
                return;
            }
        }
        if !self.args.gate {
            self.ui = Ui::new().map_err(|e| eprintln!("ui: {e:#}")).ok();
        }
        self.r = Some(r);
        self.window = Some(window.clone());
        if self.args.pack {
            self.format_i = FORMATS.iter().position(|f| *f == self.args.format).unwrap_or(0);
            self.pack_mode = true;
            if let Err(e) = self.load_pack() {
                eprintln!("pack: {e:#}");
                el.exit();
                return;
            }
            self.reset_view();
        }
        if self.args.gate {
            self.apply_shot();
        }
        self.last = Instant::now();
        window.request_redraw();
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if (self.args.gate || self.args.pack_test) && self.r.is_some() {
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
                    MouseButton::Left => {
                        self.drag_left = down;
                        if self.pack_mode {
                            if down {
                                self.press_at = self.cursor;
                                self.press_moved = false;
                                let p = self.cursor.and_then(|(cx, cy)| self.cursor_on_pack(cx, cy));
                                if let (Some(p), Some(g)) = (p, self.rig.as_mut()) {
                                    if g.try_grab(p) {
                                        g.drag(p);
                                        self.drag_left = false; // tearing, not rotating
                                    }
                                }
                            } else {
                                let was_tearing = self.rig.as_ref().is_some_and(|g| g.tearing);
                                if let Some(g) = self.rig.as_mut() {
                                    g.release();
                                }
                                if !was_tearing && !self.press_moved && self.press_at.is_some() {
                                    if let Some(g) = self.rig.as_mut() {
                                        g.click();
                                    }
                                }
                                self.press_at = None;
                            }
                        }
                    }
                    MouseButton::Right => self.drag_right = down,
                    _ => {}
                }
                if down {
                    self.spin = false;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((sx, sy)) = self.press_at {
                    if (position.x - sx).abs() + (position.y - sy).abs() > 5.0 {
                        self.press_moved = true;
                    }
                }
                if self.rig.as_ref().is_some_and(|g| g.tearing) {
                    if let Some(p) = self.cursor_on_pack(position.x, position.y) {
                        self.rig.as_mut().unwrap().drag(p);
                    }
                }
                if let Some((px, py)) = self.cursor {
                    let (dx, dy) = ((position.x - px) as f32, (position.y - py) as f32);
                    if self.drag_left {
                        self.card_yaw += dx * 0.008;
                        self.card_pitch = (self.card_pitch + dy * 0.008).clamp(-1.45, 1.45);
                    }
                    if self.drag_right {
                        self.lamp_az += dx * 0.01;
                        self.lamp_el = (self.lamp_el - dy * 0.01).clamp(0.05, 1.5);
                    }
                }
                self.cursor = Some((position.x, position.y));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    akari::winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    akari::winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 60.0,
                };
                self.dist = (self.dist * (1.0 - d * 0.1)).clamp(0.07, 0.7);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(k) = event.physical_key else { return };
                if event.state != ElementState::Pressed || event.repeat {
                    return;
                }
                let cur = self.scene.as_ref().map_or(0, |s| s.hero_index);
                let n = self.labels.len().max(1);
                match k {
                    KeyCode::Escape => el.exit(),
                    KeyCode::Space if self.pack_mode => {
                        if let Some(g) = self.rig.as_mut() {
                            g.click();
                        }
                    }
                    KeyCode::Space => self.spin = !self.spin,
                    KeyCode::KeyP => {
                        self.pack_mode = !self.pack_mode;
                        if self.pack_mode && self.rig.is_none() {
                            if let Err(e) = self.load_pack() {
                                eprintln!("pack: {e:#}");
                                self.pack_mode = false;
                            }
                        }
                        self.reset_view();
                    }
                    KeyCode::Tab if self.pack_mode => {
                        self.format_i = (self.format_i + 1) % FORMATS.len();
                        if let Err(e) = self.load_pack() {
                            eprintln!("pack: {e:#}");
                        }
                    }
                    KeyCode::KeyN if self.pack_mode => self.new_pack(),
                    KeyCode::ArrowRight => self.pick(cur + 1),
                    KeyCode::ArrowLeft => self.pick(cur + n - 1),
                    KeyCode::KeyF => self.flipped = !self.flipped,
                    KeyCode::KeyL => self.lighting = self.lighting.next(),
                    KeyCode::KeyR => self.reset_view(),
                    KeyCode::KeyH => self.show_help = !self.show_help,
                    KeyCode::F12 => {
                        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                        self.shot_request = Some(self.root.join(format!("assets/cards/templates/engine_test/shot_{stamp}.png")));
                    }
                    _ => {
                        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
                        if let Some(i) = digits.iter().position(|d| *d == k) {
                            self.pick(i);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if self.r.is_some() && !self.args.gate && !self.args.pack_test {
                    if let Err(e) = self.frame(el) {
                        eprintln!("frame: {e:#}");
                        el.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, _event: DeviceEvent) {}
}

fn main() -> Result<()> {
    let args = parse_args();
    // Akari's texture cache and the template paths are relative to the repo root.
    // canonicalize() on Windows yields a \\?\ verbatim path, which NGX cannot search for the
    // DLSS DLL, so strip that prefix.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    let root = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\").to_string());
    std::env::set_current_dir(&root)?;
    let mut cam = Camera::default();
    cam.fov_y = 32f32.to_radians();
    cam.near = 0.01;
    aim(&mut cam, Vec3::new(0.0, 0.075, 0.27), Vec3::new(0.0, 0.045, 0.06));
    let labels = card_labels(&root);
    let event_loop = EventLoop::new().context("event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        args,
        root,
        window: None,
        r: None,
        scene: None,
        ui: None,
        labels,
        cam,
        last: Instant::now(),
        start: Instant::now(),
        yaw: 0.0,
        card_yaw: 0.0,
        card_pitch: 0.0,
        flipped: false,
        spin: false,
        dist: 0.26,
        lamp_az: -0.55,
        lamp_el: 0.55,
        lighting: Lighting::Lamp,
        drag_left: false,
        drag_right: false,
        cursor: None,
        show_help: true,
        shots: shots(),
        shot_i: 0,
        shot_frames: 0,
        shot_request: None,
        pack_mode: false,
        rig: None,
        format_i: 0,
        db: tcg::CardDb::generate(1),
        press_at: None,
        press_moved: false,
        test_frame: 0,
        look_off: Vec3::ZERO,
    };
    event_loop.run_app(&mut app)?;
    drop(app.r.take());
    Ok(())
}
