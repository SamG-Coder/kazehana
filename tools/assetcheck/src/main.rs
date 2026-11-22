//! The asset quality gate (docs/ASSET_QUALITY.md).
//!
//!   assetcheck check <file.gltf> --id <id> --category <prop|foliage|building|character|surface>
//!   assetcheck lookdev <id>          render the look-dev shots to assets/qc/<id>/
//!       [--whole]                    show a multi-part prop assembled, not its parts side by side
//!   assetcheck approve <id> --reviewer <name> [--notes "..."]
//!   assetcheck reject <id> --reviewer <name> --notes "..."
//!   assetcheck list

use akari::glam::{Affine3A, Vec2, Vec3};
use akari::winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use akari::*;
use anyhow::{bail, Context, Result};
use assets::registry::{qc_dir, read_lookdev, read_report, write_report, Lookdev};
use assets::{Category, Level, Registry, REGISTRY_PATH};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => {
            let path = PathBuf::from(args.get(1).context("check <file.gltf>")?);
            let id = flag(&args, "--id").context("--id <id>")?;
            if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                bail!("ids are lower_snake_case");
            }
            let cat = Category::parse(&flag(&args, "--category").context("--category")?)?;
            let rep = assets::check(&path, &id, cat)?;
            write_report(&rep)?;
            rep.print();
            println!("report: {}", qc_dir(&id).join("report.json").display());
            if !rep.passed {
                std::process::exit(2);
            }
        }
        Some("lookdev") => {
            let id = args.get(1).context("lookdev <id>")?.clone();
            let rep = read_report(&id)?;
            lookdev(&id, &rep, args.iter().any(|a| a == "--whole"))?;
        }
        Some("approve") => {
            let id = args.get(1).context("approve <id>")?;
            let reviewer = flag(&args, "--reviewer").context("--reviewer <name>")?;
            let notes = flag(&args, "--notes").unwrap_or_default();
            let mut reg = Registry::open(REGISTRY_PATH)?;
            reg.approve(id, &reviewer, &notes)?;
            let e = &reg.assets[id.as_str()];
            println!("approved '{id}' ({} warnings acknowledged)", e.warnings.len());
        }
        Some("reject") => {
            let id = args.get(1).context("reject <id>")?;
            let reviewer = flag(&args, "--reviewer").context("--reviewer <name>")?;
            let notes = flag(&args, "--notes").context("--notes: say why")?;
            Registry::open(REGISTRY_PATH)?.reject(id, &reviewer, &notes)?;
            println!("rejected '{id}'");
        }
        Some("list") => {
            let reg = Registry::open(REGISTRY_PATH)?;
            for (id, e) in &reg.assets {
                let ok = reg.verify(id).map(|_| "valid").unwrap_or("INVALID");
                println!("{id:<32} {:<9?} {:<9?} {ok:<8} {}  {}", e.status, e.category, e.reviewer, e.notes);
            }
            if let Ok(rd) = std::fs::read_dir(assets::registry::QC_DIR) {
                for d in rd.flatten() {
                    let id = d.file_name().to_string_lossy().into_owned();
                    if !reg.assets.contains_key(&id) {
                        let r = read_report(&id).ok();
                        let state = match r {
                            Some(r) if !r.passed => "failed checks",
                            Some(_) if read_lookdev(&id).is_some() => "awaiting review",
                            Some(_) => "checked, needs lookdev",
                            None => "no report",
                        };
                        println!("{id:<32} pending   {state}");
                    }
                }
            }
        }
        _ => {
            eprintln!("usage: assetcheck check|lookdev|approve|reject|list  (see docs/ASSET_QUALITY.md)");
            std::process::exit(1);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Look-dev renders

struct Shot {
    name: &'static str,
    yaw: f32,
    pitch: f32,
    golden: bool,
    debug: u32,
    /// frame the asset alone (small props: next to the 1.6 m figure they are a speck)
    close: bool,
}

const SHOTS: &[Shot] = &[
    Shot { name: "front_noon", yaw: 25.0, pitch: -14.0, golden: false, debug: 0, close: false },
    Shot { name: "side_noon", yaw: 115.0, pitch: -14.0, golden: false, debug: 0, close: false },
    Shot { name: "back_high_noon", yaw: 205.0, pitch: -32.0, golden: false, debug: 0, close: false },
    Shot { name: "front_golden", yaw: 25.0, pitch: -10.0, golden: true, debug: 0, close: false },
    Shot { name: "albedo", yaw: 25.0, pitch: -14.0, golden: false, debug: 1, close: false },
    Shot { name: "normals", yaw: 25.0, pitch: -14.0, golden: false, debug: 2, close: false },
    Shot { name: "closeup_noon", yaw: 20.0, pitch: -38.0, golden: false, debug: 0, close: true },
    Shot { name: "closeup_golden", yaw: 160.0, pitch: -24.0, golden: true, debug: 0, close: true },
    Shot { name: "closeup_normals", yaw: 20.0, pitch: -38.0, golden: false, debug: 2, close: true },
];
const FRAMES: u32 = 90;

struct Look {
    id: String,
    rep: assets::Report,
    window: Option<Arc<Window>>,
    r: Option<Renderer>,
    instances: Vec<Instance>,
    center: Vec3,
    radius: f32,
    /// the asset alone (close-ups)
    asset_center: Vec3,
    asset_radius: f32,
    whole: bool,
    shot: usize,
    frame: u32,
    images: Vec<String>,
    err: Option<anyhow::Error>,
    start: Instant,
}

fn lookdev(id: &str, rep: &assets::Report, whole: bool) -> Result<()> {
    let files = assets::check::asset_files(Path::new(&rep.source))?;
    if assets::check::hash_files(&files)? != rep.hash {
        bail!("'{id}' changed since its check; run assetcheck check again");
    }
    let el = EventLoop::new()?;
    el.set_control_flow(ControlFlow::Poll);
    let mut app = Look {
        id: id.into(),
        rep: rep.clone(),
        window: None,
        r: None,
        instances: vec![],
        center: Vec3::ZERO,
        radius: 1.0,
        asset_center: Vec3::ZERO,
        asset_radius: 1.0,
        whole,
        shot: 0,
        frame: 0,
        images: vec![],
        err: None,
        start: Instant::now(),
    };
    el.run_app(&mut app)?;
    drop(app.r.take());
    if let Some(e) = app.err {
        return Err(e);
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let l = Lookdev { hash: rep.hash.clone(), images: app.images.clone(), date: now };
    std::fs::write(qc_dir(id).join("lookdev.json"), serde_json_string(&l)?)?;
    println!("look-dev renders in {} — review them against docs/ASSET_QUALITY.md, then approve or reject", qc_dir(id).display());
    let warns: Vec<_> = rep.findings.iter().filter(|f| f.level == Level::Warn).collect();
    if !warns.is_empty() {
        println!("warnings to acknowledge in the review notes:");
        for w in warns {
            println!("  {}: {}", w.check, w.message);
        }
    }
    Ok(())
}

fn serde_json_string(l: &Lookdev) -> Result<String> {
    // (the assets crate owns serde_json; format by hand to keep this tool's deps small)
    let imgs: Vec<String> = l.images.iter().map(|s| format!("\"{}\"", s.replace('\\', "/"))).collect();
    Ok(format!("{{\n  \"hash\": \"{}\",\n  \"images\": [{}],\n  \"date\": {}\n}}\n", l.hash, imgs.join(", "), l.date))
}

impl Look {
    fn setup(&mut self, r: &mut Renderer) -> Result<()> {
        let opts = LoadOptions { foliage: self.rep.category == Category::Foliage };
        let model = akari::gltf_load::load(r, Path::new(&self.rep.source), opts)?;
        // every variant side by side, along +X
        // (buildings and levels are one piece: shown whole, as they stand)
        let parts: Vec<Model> = if self.whole || matches!(self.rep.category, Category::Building | Category::Level) {
            vec![model]
        } else {
            model.split().into_iter().filter(|m| !m.name.contains("Billboard")).collect()
        };
        let mut x = 0.0;
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for m in &parts {
            let w = (m.max.x - m.min.x).max(0.05);
            let at = Vec3::new(x - m.min.x, 0.0, -(m.min.z + m.max.z) * 0.5);
            self.instances.extend(m.instances(Affine3A::from_translation(at)));
            lo = lo.min(m.min + at);
            hi = hi.max(m.max + at);
            x += w + 0.3 * w.max(0.3);
        }
        self.asset_center = (lo + hi) * 0.5;
        self.asset_radius = (hi - lo).length() * 0.5;
        // the 1.6 m reference figure, left of the asset
        let grey = r.add_material(Material::color([0.45, 0.45, 0.47], 0.7));
        let ground_mat = r.add_material(Material::color([0.46, 0.46, 0.46], 0.9)); // ~18% linear
        let fx = lo.x - 0.6;
        let mut fig = MeshBuilder::new("reference figure", grey, 1.0);
        fig.cylinder(Vec3::new(fx, 0.0, 0.0), 0.17, 1.36, 16);
        fig.sphere(Vec3::new(fx, 1.48, 0.0), Vec3::splat(0.12), 12, 16);
        let span = (hi - lo).length().max(3.0) * 6.0;
        let mut ground = MeshBuilder::new("lookdev ground", ground_mat, 1.0);
        ground.ground(Vec2::new(lo.x - span, lo.z - span), Vec2::new(hi.x + span, hi.z + span), 0.0);
        let ids = r.add_meshes(&[fig.finish(), ground.finish()])?;
        for id in ids {
            self.instances.push(Instance { mesh: id, transform: Affine3A::IDENTITY });
        }
        let lo = lo.min(Vec3::new(fx - 0.2, 0.0, -0.2));
        let hi = hi.max(Vec3::new(fx + 0.2, 1.6, 0.2));
        self.center = (lo + hi) * 0.5;
        self.radius = (hi - lo).length() * 0.5;
        Ok(())
    }

    fn frame(&mut self, el: &ActiveEventLoop) -> Result<()> {
        // (one more frame can arrive after the last shot asked to exit)
        let Some(s) = SHOTS.get(self.shot) else { return Ok(()) };
        if s.close && self.asset_radius > 0.75 {
            // close-ups are for small props only
            self.shot += 1;
            if self.shot >= SHOTS.len() {
                el.exit();
            }
            return Ok(());
        }
        let mut cam = Camera { fov_y: 40f32.to_radians(), ..Default::default() };
        cam.yaw = s.yaw.to_radians();
        cam.pitch = s.pitch.to_radians();
        let (center, radius) = if s.close { (self.asset_center, self.asset_radius) } else { (self.center, self.radius) };
        let dist = radius / (cam.fov_y * 0.5).sin() * 1.15;
        cam.position = center - cam.forward() * dist;
        cam.near = cam.near.min(dist * 0.05);
        let sun = if s.golden { Vec3::new(-0.8, 0.1, 0.55).normalize() } else { Vec3::new(0.35, 0.85, 0.4).normalize() };
        let capture = self.frame == FRAMES;
        let desc = FrameDesc {
            camera: &cam,
            sun_dir: sun,
            sun_illuminance: 1.0,
            moon_dir: -sun,
            moon_illuminance: 0.0,
            stars: 0.0,
            altitude: 100.0,
            ground_albedo: 0.18,
            haze: 0.5,
            instances: &self.instances,
            lights: &[],
            time: self.start.elapsed().as_secs_f32(),
            dt: 1.0 / 60.0,
            exposure_ev: 0.0,
            debug: s.debug,
            capture,
            ui: None,
        };
        let r = self.r.as_mut().unwrap();
        if self.frame == 0 {
            r.reset_history();
        }
        if let Some(px) = r.render(&desc)? {
            let (w, h) = r.extent();
            let path = qc_dir(&self.id).join(format!("lookdev_{}.png", s.name));
            save_png(&path, w, h, &px)?;
            println!("  {}", path.display());
            self.images.push(path.display().to_string());
        }
        self.frame += 1;
        if self.frame > FRAMES {
            self.frame = 0;
            self.shot += 1;
            if self.shot == SHOTS.len() {
                el.exit();
            }
        }
        if let Some(w) = &self.window {
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
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

impl Look {
    fn headless_frame(&mut self, el: &ActiveEventLoop) {
        if let Err(e) = self.frame(el) {
            self.err = Some(e);
            el.exit();
        }
    }
}

impl ApplicationHandler for Look {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // headless: hidden and never focused
        let attrs = Window::default_attributes().with_title(format!("Look-dev: {}", self.id)).with_inner_size(PhysicalSize::new(1600, 1000)).with_visible(false).with_active(false);
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let res = Renderer::new(&window, RendererOptions { vsync: false, ..Default::default() }).and_then(|mut r| {
            self.setup(&mut r)?;
            Ok(r)
        });
        match res {
            Ok(r) => self.r = Some(r),
            Err(e) => {
                self.err = Some(e);
                el.exit();
                return;
            }
        }
        self.window = Some(window.clone());
        window.request_redraw();
    }

    /// Headless runs: a hidden window gets no redraw events, so frames are driven from here.
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if true && self.r.is_some() {
            self.headless_frame(el);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested if self.r.is_some() => {
                if let Err(e) = self.frame(el) {
                    self.err = Some(e);
                    el.exit();
                }
            }
            _ => {}
        }
    }
}
