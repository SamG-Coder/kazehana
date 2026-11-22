//! Akari Phase 1 test scene: Fab / Megascans assets outdoors, a small room with a window and a
//! lamp (interior global illumination), lit by Kazehana's real sun and moon (sim::clock).
//!
//! Interactive: WASD move (Shift faster), mouse look (click to capture, Esc release),
//! [ / ] hour -/+, T hold to run time x60, B bounces 1..4, L DLSS on/off, 1-3 debug views (0 off),
//! F12 screenshot.
//!
//! `--gate` renders the fixed screenshot set to shots/phase1/ and exits.
//! Options: --time HH:MM --date YYYY-MM-DD --pos x,y,z --yaw deg --pitch deg --no-dlss
//!          --validation --bounces N

use akari::glam::{Affine3A, Quat, Vec2, Vec3};
use akari::scene::MAT_UNLIT_EMISSIVE;
use akari::textures::{file_key, TexJob, TexKind, TexSource};
use akari::winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
use akari::*;
use anyhow::{Context, Result};
use sim::clock::{self, at, GameTime};
use sim::Date;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const FAB: &str = "D:/EpicVault/VaultCache/FabLibrary";

fn fab(rel: &str) -> PathBuf {
    Path::new(FAB).join(rel)
}

/// A Megascans surface (B / N / ORM pngs next to each other) as a material.
fn surface_material(r: &mut Renderer, tex_dir: &Path, id: &str) -> Result<MaterialId> {
    let f = |suffix: &str| tex_dir.join(format!("T_{id}_2K_{suffix}.png"));
    let jobs = vec![
        TexJob { key: file_key(&f("B")), name: format!("{id} B"), kind: TexKind::Color, source: TexSource::File(f("B")) },
        TexJob { key: file_key(&f("N")), name: format!("{id} N"), kind: TexKind::Normal, source: TexSource::File(f("N")) },
        TexJob { key: file_key(&f("ORM")), name: format!("{id} ORM"), kind: TexKind::Data, source: TexSource::File(f("ORM")) },
    ];
    for j in &jobs {
        if let TexSource::File(p) = &j.source {
            anyhow::ensure!(p.exists(), "missing {}", p.display());
        }
    }
    let t = r.add_textures(&jobs)?;
    Ok(r.add_material(Material {
        base_color: [1.0; 4],
        roughness: 1.0,
        metallic: 1.0,
        base_tex: Some(t[0]),
        normal_tex: Some(t[1]),
        mr_tex: Some(t[2]),
        occlusion_tex: Some(t[2]),
        ..Default::default()
    }))
}

struct Scene {
    instances: Vec<Instance>,
    lamp_lights: Vec<Light>,
    street_lights: Vec<Light>,
}

fn build_scene(r: &mut Renderer) -> Result<Scene> {
    let t0 = Instant::now();
    let mut inst = Vec::new();
    let opts = LoadOptions::default();

    // Ground: wet grass, with a mud path through it.
    let grass = surface_material(r, &fab("Wet_Grass_Ground-138dc11e/gltf/mid/wet_grass_ground_sjzkdjb_extracted/Textures"), "sjzkdjba")?;
    let mud = surface_material(r, &fab("Soil_Mud-594f71d6/gltf/mid/soil_mud_pedhi20_2k_ue_m_extracted/Textures"), "peDhI20").ok();
    let mut ground = MeshBuilder::new("ground", grass, 0.5);
    // the ground in tiles so the path can cut through
    for x in -10..10 {
        for z in -10..10 {
            let (x0, z0) = (x as f32 * 20.0, z as f32 * 20.0);
            if mud.is_some() && x == 0 && z < 0 {
                continue;
            }
            ground.ground(Vec2::new(x0, z0), Vec2::new(x0 + 20.0, z0 + 20.0), 0.0);
        }
    }
    let mut meshes = vec![ground.finish()];
    if let Some(m) = mud {
        let mut p = MeshBuilder::new("path", m, 0.5);
        p.ground(Vec2::new(0.0, -200.0), Vec2::new(20.0, 0.0), 0.0);
        meshes.push(p.finish());
    }

    // The test room: plaster walls, wooden floor, a south window (x 2..4) and an east door.
    let plaster = r.add_material(Material::color([0.82, 0.78, 0.70], 0.85));
    let floor = r.add_material(Material::color([0.45, 0.30, 0.18], 0.45));
    let dark = r.add_material(Material::color([0.12, 0.10, 0.09], 0.6));
    let metal = r.add_material(Material { base_color: [0.95, 0.93, 0.88, 1.0], roughness: 0.25, metallic: 1.0, ..Default::default() });
    let glossy = r.add_material(Material { base_color: [0.6, 0.05, 0.04, 1.0], roughness: 0.15, metallic: 0.0, ..Default::default() });
    let bulb = r.add_material(Material { base_color: [1.0; 4], emissive: [40.0, 32.0, 22.0], flags: MAT_UNLIT_EMISSIVE, ..Default::default() });
    let (rx, rz) = (6.0, -14.0); // room origin (south-west corner)
    let (w, d, h, t) = (6.0, 5.0, 2.7, 0.15);
    let mut walls = MeshBuilder::new("room walls", plaster, 1.0);
    let o = Vec3::new(rx, 0.0, rz);
    let bx = |b: &mut MeshBuilder, a: Vec3, c: Vec3| {
        b.aabb(o + a, o + c);
    };
    // north wall (z = -d)
    bx(&mut walls, Vec3::new(-t, 0.0, -d - t), Vec3::new(w + t, h, -d));
    // west wall
    bx(&mut walls, Vec3::new(-t, 0.0, -d), Vec3::new(0.0, h, 0.0));
    // east wall with a door (z -1.8..-0.8)
    bx(&mut walls, Vec3::new(w, 0.0, -d), Vec3::new(w + t, h, -1.8));
    bx(&mut walls, Vec3::new(w, 0.0, -0.8), Vec3::new(w + t, h, 0.0));
    bx(&mut walls, Vec3::new(w, 2.1, -1.8), Vec3::new(w + t, h, -0.8));
    // south wall with a window x 2..4, y 0.9..2.1
    bx(&mut walls, Vec3::new(-t, 0.0, 0.0), Vec3::new(2.0, h, t));
    bx(&mut walls, Vec3::new(4.0, 0.0, 0.0), Vec3::new(w + t, h, t));
    bx(&mut walls, Vec3::new(2.0, 0.0, 0.0), Vec3::new(4.0, 0.9, t));
    bx(&mut walls, Vec3::new(2.0, 2.1, 0.0), Vec3::new(4.0, h, t));
    // ceiling and roof slab
    bx(&mut walls, Vec3::new(-t, h, -d - t), Vec3::new(w + t, h + 0.2, t));
    meshes.push(walls.finish());
    let mut fl = MeshBuilder::new("room floor", floor, 1.0);
    fl.aabb(o + Vec3::new(0.0, 0.0, -d), o + Vec3::new(w, 0.02, 0.0));
    meshes.push(fl.finish());
    let mut furniture = MeshBuilder::new("furniture", dark, 1.0);
    furniture.aabb(o + Vec3::new(0.6, 0.0, -4.2), o + Vec3::new(2.4, 0.72, -3.2)); // table
    furniture.aabb(o + Vec3::new(4.5, 0.0, -4.9), o + Vec3::new(5.9, 1.9, -4.4)); // shelf
    furniture.cylinder(o + Vec3::new(1.5, 2.2, -3.7), 0.01, 0.5, 6); // lamp cord
    meshes.push(furniture.finish());
    let mut spheres = MeshBuilder::new("metal sphere", metal, 1.0);
    spheres.sphere(o + Vec3::new(1.0, 0.92, -3.6), Vec3::splat(0.2), 16, 24);
    meshes.push(spheres.finish());
    let mut red = MeshBuilder::new("glossy sphere", glossy, 1.0);
    red.sphere(o + Vec3::new(2.0, 0.92, -3.8), Vec3::splat(0.2), 16, 24);
    meshes.push(red.finish());
    let mut b = MeshBuilder::new("bulb", bulb, 1.0);
    b.sphere(o + Vec3::new(1.5, 2.12, -3.7), Vec3::splat(0.06), 8, 12);
    meshes.push(b.finish());
    // a street lamp post outside
    let mut post = MeshBuilder::new("lamp post", dark, 1.0);
    post.cylinder(Vec3::new(3.0, 0.0, -4.0), 0.06, 4.5, 10);
    post.aabb(Vec3::new(2.4, 4.45, -4.06), Vec3::new(3.0, 4.52, -3.94));
    meshes.push(post.finish());
    let mut lamp_head = MeshBuilder::new("lamp head", bulb, 1.0);
    lamp_head.aabb(Vec3::new(2.3, 4.38, -4.1), Vec3::new(2.6, 4.45, -3.9));
    meshes.push(lamp_head.finish());

    let ids = r.add_meshes(&meshes)?;
    for id in &ids {
        inst.push(Instance { mesh: *id, transform: Affine3A::IDENTITY });
    }

    // Fab models.
    let mut rng = 12345u32;
    let mut rnd = move || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f32) / (u32::MAX as f32)
    };
    let place = |m: &Model, p: Vec3, yaw: f32, s: f32| -> Vec<Instance> {
        m.instances(Affine3A::from_scale_rotation_translation(Vec3::splat(s), Quat::from_rotation_y(yaw), p)).collect()
    };
    let load = |r: &mut Renderer, rel: &str, o: LoadOptions| -> Option<Model> {
        match akari::gltf_load::load(r, &fab(rel), o) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("skipping {rel}: {e:#}");
                None
            }
        }
    };
    if let Some(m) = load(r, "Japanese_Cedar_Tree_Trunk-b40ccf58/gltf/mid/japanese_cedar_tree_trun_extracted/uepqbi2ga_tier_2.gltf", opts) {
        for (x, z) in [(-6.0, -10.0), (-9.0, -18.0), (-4.0, -25.0), (16.0, -24.0)] {
            inst.extend(place(&m, Vec3::new(x, 0.0, z), rnd() * 6.28, 1.0));
        }
    }
    if let Some(m) = load(r, "Mossy_Forest_Boulder-2079c412/gltf/mid/mossy_forest_boulder_wdb_extracted/wdbncbl_tier_2.gltf", opts) {
        inst.extend(place(&m, Vec3::new(-3.0, 0.0, -6.0), 0.4, 1.0));
        inst.extend(place(&m, Vec3::new(14.0, 0.0, -9.0), 2.0, 1.4));
    }
    if let Some(m) = load(r, "Nordic_Forest_Ledge_Rock_Large-ba2e0cb1/gltf/mid/nordic_forest_ledge_rock_extracted/tksucdtda_tier_2.gltf", opts) {
        inst.extend(place(&m, Vec3::new(-14.0, 0.0, -16.0), 1.2, 1.0));
    }
    if let Some(m) = load(r, "Tundra_Mossy_Boulder-0afd26b9/gltf/mid/tundra_mossy_boulder_viz_extracted/vizvactdw_tier_2.gltf", opts) {
        inst.extend(place(&m, Vec3::new(4.5, 0.0, -1.5), 0.3, 0.8));
        inst.extend(place(&m, Vec3::new(-7.0, 0.0, 2.0), 2.3, 0.6));
    }
    if let Some(m) = load(r, "Mossy_Log-3c04459d/gltf/mid/mossy_log_tjrtcibda_ue_m_extracted/tjrtcibda_tier_2.gltf", opts) {
        inst.extend(place(&m, Vec3::new(-4.0, 0.0, -4.0), 0.9, 1.0));
    }
    let mut plants: Vec<Model> = Vec::new();
    for rel in [
        "Tall_Grass-d0e4e978/gltf/mid/tall_grass_rbeki_ue_mid_extracted/standard/rbEkI_tier_2_nonUE.gltf",
        "Wild_Grass-50d9a417/gltf/mid/wild_grass_vlkhcbxia_ue__extracted/standard/vlkhcbxia_tier_2_nonUE.gltf",
        "Grass_Clumps-d22b2fc3/gltf/mid/grass_clumps_rbdoo_ue_mi_extracted/standard/rbdoo_tier_2_nonUE.gltf",
        "Leafy_Dandelions-20172125/gltf/mid/leafy_dandelions_qmbrl_u_extracted/standard/qmBrl_tier_2_nonUE.gltf",
    ] {
        if let Some(m) = load(r, rel, LoadOptions { foliage: true }) {
            // (billboard parts are for far LODs)
            plants.extend(m.split().into_iter().filter(|v| !v.name.contains("Billboard")));
        }
    }
    if !plants.is_empty() {
        let mut n = 0;
        while n < 2500 {
            let p = Vec3::new(rnd() * 70.0 - 35.0, 0.0, rnd() * -60.0 + 10.0);
            // keep the path and the room clear
            let in_room = p.x > rx - 1.0 && p.x < rx + w + 1.0 && p.z > rz - d - 1.0 && p.z < rz + 1.0;
            if ((0.5..19.5).contains(&p.x) && p.z < 0.0) || in_room {
                continue;
            }
            let v = &plants[(rnd() * plants.len() as f32) as usize % plants.len()];
            inst.extend(place(v, p, rnd() * 6.28, 0.8 + rnd() * 0.4));
            n += 1;
        }
    }

    let lamp_lights = vec![Light {
        position: o + Vec3::new(1.5, 2.12, -3.7),
        radius: 0.06,
        intensity: Vec3::new(1.0, 0.78, 0.52) * 0.012,
        range: 12.0,
        spot: None,
    }];
    let street_lights = vec![Light {
        position: Vec3::new(2.45, 4.3, -4.0),
        radius: 0.12,
        intensity: Vec3::new(0.9, 0.95, 1.0) * 0.05,
        range: 30.0,
        spot: Some((Vec3::NEG_Y, (60f32).to_radians().cos())),
    }];
    println!("Scene: {} instances in {:.1}s", inst.len(), t0.elapsed().as_secs_f32());
    Ok(Scene { instances: inst, lamp_lights, street_lights })
}

struct Shot {
    name: &'static str,
    time: (u32, u32),
    pos: Vec3,
    yaw: f32,
    pitch: f32,
}

const GATE: &[Shot] = &[
    Shot { name: "outdoor_morning", time: (7, 30), pos: Vec3::new(10.0, 1.45, 6.0), yaw: -20.0, pitch: -3.0 },
    Shot { name: "outdoor_noon", time: (12, 30), pos: Vec3::new(10.0, 1.45, 6.0), yaw: -20.0, pitch: -3.0 },
    Shot { name: "outdoor_dusk", time: (18, 5), pos: Vec3::new(-2.0, 1.45, 4.0), yaw: 25.0, pitch: 0.0 },
    Shot { name: "outdoor_night", time: (22, 30), pos: Vec3::new(10.0, 1.45, 6.0), yaw: -20.0, pitch: -3.0 },
    Shot { name: "room_morning", time: (9, 0), pos: Vec3::new(11.2, 1.4, -17.8), yaw: -120.0, pitch: -8.0 },
    Shot { name: "room_afternoon", time: (15, 30), pos: Vec3::new(11.2, 1.4, -17.8), yaw: -120.0, pitch: -8.0 },
    Shot { name: "room_night_lamp", time: (21, 0), pos: Vec3::new(11.2, 1.4, -17.8), yaw: -120.0, pitch: -8.0 },
    Shot { name: "grass_closeup", time: (16, 0), pos: Vec3::new(-8.0, 0.9, 0.0), yaw: -35.0, pitch: -10.0 },
];

struct Args {
    gate: bool,
    date: Date,
    minute: u32,
    pos: Option<Vec3>,
    yaw: f32,
    pitch: f32,
    dlss: bool,
    validation: bool,
    bounces: u32,
}

fn parse_args() -> Args {
    let mut a = Args { gate: false, date: Date::new(2026, 4, 8), minute: at::hm(10, 0), pos: None, yaw: -20.0, pitch: 0.0, dlss: true, validation: false, bounces: 2 };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        let next = v.get(i + 1).cloned().unwrap_or_default();
        match v[i].as_str() {
            "--gate" => a.gate = true,
            "--no-dlss" => a.dlss = false,
            "--validation" => a.validation = true,
            "--time" => {
                let p: Vec<u32> = next.split(':').filter_map(|x| x.parse().ok()).collect();
                a.minute = at::hm(p.first().copied().unwrap_or(10), p.get(1).copied().unwrap_or(0));
                i += 1;
            }
            "--date" => {
                let p: Vec<i32> = next.split('-').filter_map(|x| x.parse().ok()).collect();
                if p.len() == 3 {
                    a.date = Date::new(p[0], p[1] as u32, p[2] as u32);
                }
                i += 1;
            }
            "--pos" => {
                let p: Vec<f32> = next.split(',').filter_map(|x| x.parse().ok()).collect();
                if p.len() == 3 {
                    a.pos = Some(Vec3::new(p[0], p[1], p[2]));
                }
                i += 1;
            }
            "--yaw" => { a.yaw = next.parse().unwrap_or(0.0); i += 1; }
            "--pitch" => { a.pitch = next.parse().unwrap_or(0.0); i += 1; }
            "--bounces" => { a.bounces = next.parse().unwrap_or(2); i += 1; }
            _ => {}
        }
        i += 1;
    }
    a
}

/// Sun / moon / sky parameters for a moment in Kazehana.
fn sky_frame(t: &GameTime) -> (Vec3, f32, Vec3, f32, f32) {
    let s = clock::sky(t);
    let sun = Vec3::from(s.sun_dir).normalize();
    let moon = Vec3::from(s.moon_dir).normalize();
    // the sun keeps lighting the sky (twilight) until it is 18 degrees down
    let sun_i = if s.sun_elevation.to_degrees() > -18.0 { 1.0 } else { 0.0 };
    let moon_up = ((s.moon_elevation.to_degrees() + 1.0) / 6.0).clamp(0.0, 1.0);
    let moon_i = 2.5e-6 * s.moon_light * moon_up;
    let stars = (1.0 - ((s.sun_elevation.to_degrees() + 4.0) / 8.0).clamp(0.0, 1.0)).powi(2);
    (sun, sun_i, moon, moon_i, stars)
}

struct App {
    args: Args,
    window: Option<Arc<Window>>,
    r: Option<Renderer>,
    scene: Option<Scene>,
    cam: Camera,
    time: GameTime,
    keys: HashSet<KeyCode>,
    captured: bool,
    last: Instant,
    start: Instant,
    minute_acc: f32,
    gate_index: usize,
    gate_frames: u32,
    shot_request: Option<PathBuf>,
    debug: u32,
    bounces: u32,
}

impl App {
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

    fn apply_shot(&mut self, i: usize) {
        let s = &GATE[i];
        self.time = GameTime::new(self.args.date, at::hm(s.time.0, s.time.1));
        self.cam.position = s.pos;
        self.cam.yaw = s.yaw.to_radians();
        self.cam.pitch = s.pitch.to_radians();
        if let Some(r) = &mut self.r {
            r.reset_history();
        }
    }

    fn frame(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        if !self.args.gate {
            let speed = if self.keys.contains(&KeyCode::ShiftLeft) { 12.0 } else { 3.0 };
            let mut wish = Vec3::ZERO;
            let (f, rt) = (self.cam.forward_flat(), self.cam.right_flat());
            for (k, v) in [(KeyCode::KeyW, f), (KeyCode::KeyS, -f), (KeyCode::KeyD, rt), (KeyCode::KeyA, -rt), (KeyCode::KeyE, Vec3::Y), (KeyCode::KeyQ, -Vec3::Y)] {
                if self.keys.contains(&k) {
                    wish += v;
                }
            }
            self.cam.position += wish.normalize_or_zero() * speed * dt;
            if self.keys.contains(&KeyCode::KeyT) {
                self.minute_acc += dt * 60.0;
                let whole = self.minute_acc.floor();
                self.minute_acc -= whole;
                self.time.advance(whole as u32);
            }
        }
        let (sun, sun_i, moon, moon_i, stars) = sky_frame(&self.time);
        let scene = self.scene.as_ref().unwrap();
        let night = sun.y < 0.02;
        let mut lights = Vec::new();
        if night || self.time.minute < at::hm(7, 0) || self.time.minute > at::hm(17, 0) {
            lights.extend(scene.lamp_lights.iter().copied());
        }
        if sun.y < 0.05 {
            lights.extend(scene.street_lights.iter().copied());
        }
        let capture = self.shot_request.is_some() || (self.args.gate && self.gate_frames == 90);
        let desc = FrameDesc {
            camera: &self.cam,
            sun_dir: sun,
            sun_illuminance: sun_i,
            moon_dir: moon,
            moon_illuminance: moon_i,
            stars,
            altitude: 120.0,
            ground_albedo: 0.15,
            haze: 1.0,
            instances: &scene.instances,
            lights: &lights,
            time: (now - self.start).as_secs_f32(),
            dt,
            exposure_ev: 0.0,
            debug: self.debug,
            capture,
            ui: None,
        };
        let r = self.r.as_mut().unwrap();
        r.set_bounces(self.bounces);
        let img = r.render(&desc)?;
        let (w, h) = r.extent();
        let (lum, dlss_on) = (r.scene_luminance, r.dlss_mode().is_some());
        if let Some(px) = img {
            let path = if self.args.gate { PathBuf::from(format!("shots/phase1/{}.png", GATE[self.gate_index].name)) } else { self.shot_request.take().unwrap() };
            save_png(&path, w, h, &px)?;
            println!("saved {}  (scene luminance {:.2e})", path.display(), lum);
        }
        if self.args.gate {
            self.gate_frames += 1;
            if self.gate_frames > 90 {
                self.gate_frames = 0;
                self.gate_index += 1;
                if self.gate_index >= GATE.len() {
                    el.exit();
                    return Ok(());
                }
                self.apply_shot(self.gate_index);
            }
        }
        if let Some(w) = &self.window {
            let t = self.time;
            w.set_title(&format!(
                "Akari test — {} {:02}:{:02} — bounces {} — {} — lum {:.1e}",
                t.date,
                t.minute / 60,
                t.minute % 60,
                self.bounces,
                if dlss_on { "DLSS-RR" } else { "TAA" },
                lum
            ));
            w.request_redraw();
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
    // the swapchain-order image is BGRA when the surface format was B8G8R8A8; the renderer's
    // output image is RGBA8, which is what is copied back
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
        let headless = self.args.gate;
        let attrs = Window::default_attributes().with_title("Akari test").with_inner_size(PhysicalSize::new(1920, 1080)).with_visible(!headless).with_active(!headless);
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let opts = RendererOptions { validation: self.args.validation, vsync: !self.args.gate, dlss: if self.args.dlss { Some(1) } else { None }, render_scale: 1.0, bounces: self.args.bounces };
        let mut r = match Renderer::new(&window, opts) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("renderer: {e:#}");
                el.exit();
                return;
            }
        };
        match build_scene(&mut r) {
            Ok(s) => self.scene = Some(s),
            Err(e) => {
                eprintln!("scene: {e:#}");
                el.exit();
                return;
            }
        }
        self.r = Some(r);
        self.window = Some(window.clone());
        if self.args.gate {
            self.apply_shot(0);
        }
        self.last = Instant::now();
        window.request_redraw();
    }

    /// Headless runs: a hidden window gets no redraw events, so frames are driven from here.
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.args.gate && self.r.is_some() {
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
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => self.set_capture(true),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(k) = event.physical_key else { return };
                if event.state == ElementState::Pressed {
                    if !event.repeat {
                        match k {
                            KeyCode::Escape => {
                                if self.captured { self.set_capture(false) } else { el.exit() }
                            }
                            KeyCode::BracketLeft => self.time.minute = (self.time.minute + 1440 - 60) % 1440,
                            KeyCode::BracketRight => self.time.minute = (self.time.minute + 60) % 1440,
                            KeyCode::KeyB => self.bounces = self.bounces % 4 + 1,
                            KeyCode::KeyL => {
                                if let Some(r) = &mut self.r {
                                    let m = if r.dlss_mode().is_some() { None } else { Some(1) };
                                    let _ = r.set_dlss(m);
                                }
                            }
                            KeyCode::Digit0 => self.debug = 0,
                            KeyCode::Digit1 => self.debug = 1,
                            KeyCode::Digit2 => self.debug = 2,
                            KeyCode::F12 => {
                                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                                self.shot_request = Some(PathBuf::from(format!("shots/akari_{stamp}.png")));
                            }
                            _ => {}
                        }
                    }
                    self.keys.insert(k);
                } else {
                    self.keys.remove(&k);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.r.is_some() {
                    if let Err(e) = self.frame(el) {
                        eprintln!("frame: {e:#}");
                        el.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.captured {
                self.cam.yaw += delta.0 as f32 * 0.0022;
                self.cam.pitch = (self.cam.pitch - delta.1 as f32 * 0.0022).clamp(-1.5, 1.5);
            }
        }
    }
}

fn main() -> Result<()> {
    let args = parse_args();
    let mut cam = Camera::default();
    cam.position = args.pos.unwrap_or(Vec3::new(10.0, 1.45, 6.0));
    cam.yaw = args.yaw.to_radians();
    cam.pitch = args.pitch.to_radians();
    cam.fov_y = 60f32.to_radians();
    let time = GameTime::new(args.date, args.minute);
    let bounces = args.bounces;
    let event_loop = EventLoop::new().context("event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        args,
        window: None,
        r: None,
        scene: None,
        cam,
        time,
        keys: HashSet::new(),
        captured: false,
        last: Instant::now(),
        start: Instant::now(),
        minute_acc: 0.0,
        gate_index: 0,
        gate_frames: 0,
        shot_request: None,
        debug: 0,
        bounces,
    };
    event_loop.run_app(&mut app)?;
    // drop the renderer (and its device) before the window
    drop(app.r.take());
    Ok(())
}
