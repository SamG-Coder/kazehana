//! The whole town, ready to draw and walk in: the terrain, roads and river (crate::build), every
//! plot's outdoors and buildings (plots, from the asset registry), the vegetation, and the solid
//! shapes and doors (collide). The town viewer and the game load it the same way.

use crate::collide::Playable;
use crate::plants::Vegetation;
use crate::Town;
use akari::glam::Vec3;
use akari::{Instance, Light, Renderer};
use anyhow::Result;

/// A light of a building (assets/buildings/<id>/lights.json), in the building's Blender frame.
#[derive(serde::Deserialize)]
struct LightJ {
    at: [f32; 3],
    colour: [f32; 3],
    intensity: f32,
    radius: f32,
    range: f32,
    /// a spot shining straight down with this half angle (else a point light)
    cone_deg: Option<f32>,
}

/// What the shop sells, placed in town: (model id, where it stands in the world).
pub const STOCK: &[&str] = &["booster6_box", "booster6_pack", "single_stand_gold", "single_stand_holo", "single_stand_obsidian"];

/// Trees nearer than this are their real meshes, further ones impostors (m).
const TREE_MESH_R: f32 = 70.0;
/// Grass patches: real meshes out to this, impostors out to GRASS_FAR (m).
const GRASS_MESH_R: f32 = 45.0;
const GRASS_FAR: f32 = 170.0;
/// The plants are listed again when the camera has moved this far (m).
const RELIST: f32 = 6.0;

/// The buildings and props placed in town, by the plan's type ("modern_gable", ...): approved
/// assets, and those still in review (shown so they can be judged in place).
fn house_assets(r: &mut Renderer) -> Vec<(String, akari::Model)> {
    let reg = match assets::Registry::open(assets::REGISTRY_PATH) {
        Ok(reg) => reg,
        Err(e) => {
            eprintln!("asset registry: {e:#}");
            return Vec::new();
        }
    };
    let mut houses = Vec::new();
    // Tsukumodō, the player's home, and its fittings (in review)
    match reg.load_for_review(r, "shop_tsukumodo") {
        Ok(m) => houses.push(("tsukumodo".to_string(), m)),
        Err(e) => eprintln!("shop_tsukumodo not loaded: {e:#}"),
    }
    for id in ["shop_showcase", "shop_tower_case", "shop_counter", "shop_wall_shelf", "shop_battle_table", "shop_pipe_chair", "shop_tv"].iter().chain(STOCK) {
        match reg.load_for_review(r, id) {
            Ok(m) => houses.push((format!("prop:{id}"), m)),
            Err(e) => eprintln!("{id} not loaded: {e:#}"),
        }
    }
    // the junior high's pool and grounds (approved), its main building (in review: its doors,
    // windows and frames fixed) and its gym (in review: its doors and columns reworked); on
    // their site: crate::plots
    for (id, approved) in [("school_main", false), ("school_gym", false), ("school_pool", true), ("school_grounds", true)] {
        match if approved { reg.load_model(r, id) } else { reg.load_for_review(r, id) } {
            Ok(m) => houses.push((format!("site:{id}"), m)),
            Err(e) => eprintln!("{id} not loaded: {e:#}"),
        }
    }
    // its classrooms' furniture (approved; placed from the building's layout.json: crate::plots)
    for id in ["school_desk", "school_chair"] {
        match reg.load_model(r, id) {
            Ok(m) => houses.push((format!("prop:{id}"), m)),
            Err(e) => eprintln!("{id} not loaded: {e:#}"),
        }
    }
    // ... and its back lockers (in review)
    match reg.load_for_review(r, "school_lockers") {
        Ok(m) => houses.push(("prop:school_lockers".to_string(), m)),
        Err(e) => eprintln!("school_lockers not loaded: {e:#}"),
    }
    for ty in ["modern_gable", "modern_hip", "small_house"] {
        for v in ["", "_b", "_c", "_d"] {
            // (plan type small_house is asset family house_modern_small)
            let fam = if ty == "small_house" { "modern_small" } else { ty };
            for (key, id) in [(ty.to_string(), format!("house_{fam}{v}")), (format!("{ty}_s"), format!("house_{fam}{v}_s")), (format!("{ty}_e"), format!("house_{fam}{v}_e"))] {
                match reg.load_model(r, &id) {
                    Ok(m) => houses.push((key, m)),
                    Err(e) => eprintln!("house {id} not loaded: {e:#}"),
                }
            }
        }
    }
    houses
}

pub struct TownWorld {
    /// everything but the plants and the door leaves
    pub base: Vec<Instance>,
    /// the buildings' own lights (their fittings)
    pub lights: Vec<Light>,
    /// the stock on the shelves: (model id, its middle in the world)
    pub stock: Vec<(String, Vec3)>,
    /// the cloth (the noren): step it with the people in it before each frame
    pub cloth: crate::cloth::Cloths,
    /// Tsukumodō's frame (its building's model space, glTF axes, to the world)
    pub home: Option<akari::glam::Affine3A>,
    /// your desk in 1-A (its prop frame to the world)
    pub your_desk: Option<akari::glam::Affine3A>,
    /// places for people in the buildings: (room, spot, frame in the world) (crate::plots)
    pub spots: Vec<(String, String, akari::glam::Affine3A)>,
    pub veg: Option<Vegetation>,
    /// the pool's lane ropes, afloat (drawn where the water has them each frame)
    pub lanes: Option<crate::lanes::LaneRopes>,
    /// Tsukumodō's open / closed sign in the window by the door (in review), and where it hangs:
    /// turned to 営業中 while `shop_open`, to 準備中 when not (the game sets it)
    shop_sign: Option<(akari::Model, akari::glam::Affine3A)>,
    pub shop_open: bool,
    /// the plants listed for the camera's last place (plan x, y)
    plants: Vec<Instance>,
    plants_at: Option<[f32; 2]>,
    /// the door leaves' transforms last frame (their motion vectors)
    doors_prev: Vec<akari::glam::Affine3A>,
}

impl TownWorld {
    /// Load the town into the renderer; `plants`: with its vegetation. Returns it and the town
    /// as somewhere to walk.
    pub fn load(r: &mut Renderer, town: &Town, plants: bool) -> Result<(Self, Playable)> {
        // the lawn (akari::lawn): baked once (cached), drawn on the gardens' lawns and on turf
        // (KZ_NO_LAWN=1: without it, to compare)
        if std::env::var_os("KZ_NO_LAWN").is_some() {
        } else if let Err(e) = r.bake_lawn(&akari::lawn::LawnParams::default()) {
            eprintln!("lawn not baked: {e:#}");
        }
        let scene = crate::build(r, town)?;
        let mut base = scene.instances;
        let reg = assets::Registry::open(assets::REGISTRY_PATH);
        let model = |r: &mut Renderer, id: &str| reg.as_ref().map_err(|e| anyhow::anyhow!("{e:#}")).and_then(|g| g.load_model(r, id)).map_err(|e| eprintln!("{id} not loaded: {e:#}")).ok();
        let mailbox = model(r, "japanese_wooden_mailbox");
        let hedge = model(r, "boxwood_hedge");
        let houses = house_assets(r);
        let home = town.plan.houses[town.plan.home].gate;
        let plots = crate::plots::build(r, town, home, None, mailbox, hedge, houses)?;
        base.extend(plots.instances.iter().cloned());
        let playable = Playable::build(town, &plots);
        // the open / closed sign on Tsukumodō's display window, just right of the door (the
        // building's frame: x 1.6 along the front, 0.04 in from the window frames' face at the
        // shopfront line y 3.8, its plate's foot 1.40 m up; Blender to glTF axes), its OPEN face
        // to the street
        let shop_sign = plots.home.and_then(|home| {
            let reg = assets::Registry::open(assets::REGISTRY_PATH).ok()?;
            let m = reg.load_for_review(r, "shop_open_sign").map_err(|e| eprintln!("shop_open_sign not loaded: {e:#}")).ok()?;
            let local = akari::glam::Affine3A::from_rotation_translation(akari::glam::Quat::from_rotation_y(std::f32::consts::PI), Vec3::new(1.6, 1.40, -(3.8 - 0.04)));
            let at = home * local;
            Some((m, at))
        });
        // the pool's lane ropes: its floats and rope, strung where lanes.json says
        let lanes = plots.placed.iter().find(|(n, _)| n.contains("school_pool")).and_then(|(n, at)| {
            let m = plots.models.iter().find(|m| &m.name == n)?;
            let j = std::fs::read("assets/buildings/school_pool/lanes.json").map_err(|e| eprintln!("pool lanes: {e}")).ok()?;
            crate::lanes::LaneRopes::new(m, *at, &j).map_err(|e| eprintln!("pool lanes: {e:#}")).ok().flatten()
        });
        let cloth = crate::cloth::Cloths::build(r, &plots, &playable.world)?;
        // the buildings' lights, and what stands on the shop's shelves
        let mut lights = Vec::new();
        let mut stock = Vec::new();
        let mut light_files: std::collections::HashMap<String, Vec<LightJ>> = Default::default();
        for (name, at) in &plots.placed {
            if STOCK.contains(&name.as_str()) {
                stock.push((name.clone(), at.transform_point3(Vec3::new(0.0, 0.09, 0.0))));
                continue;
            }
            let ls = light_files.entry(name.clone()).or_insert_with(|| {
                std::fs::read(format!("assets/buildings/{name}/lights.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
            });
            for l in ls.iter() {
                // (Blender x, y, z -> the model's x, z, -y)
                let p = at.transform_point3(Vec3::new(l.at[0], l.at[2], -l.at[1]));
                let spot = l.cone_deg.map(|c| (at.transform_vector3(Vec3::NEG_Y).normalize(), c.to_radians().cos()));
                lights.push(Light { position: p, radius: l.radius, intensity: Vec3::from(l.colour) * l.intensity, range: l.range, spot });
            }
        }
        let veg = if plants {
            let lu = crate::load_landuse(&town.dir.join(&town.plan.terrain.landuse))?;
            match Vegetation::load(r, &town.plan, &town.height, lu) {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("plants not loaded: {e:#}");
                    None
                }
            }
        } else {
            None
        };
        Ok((TownWorld { base, lights, stock, cloth, lanes, shop_sign, shop_open: true, home: plots.home, your_desk: plots.your_desk, spots: plots.spots.clone(), veg, plants: Vec::new(), plants_at: None, doors_prev: Vec::new() }, playable))
    }

    /// The plants are listed again at the next frame (after a cut).
    pub fn relist(&mut self) {
        self.plants_at = None;
    }

    /// This frame's instances into `out`: the town, the plants round the camera (listed again
    /// when it has moved a few metres), the doors where they are now, the cloth as it hangs
    /// now (step `cloth` first). Returns how many plant
    /// instances were listed afresh (0: none this frame).
    /// `time`: the renderer's animation clock (FrameDesc::time), which moves the water.
    pub fn frame(&mut self, r: &mut Renderer, town: &Town, camera: Vec3, doors: Option<&Playable>, time: f32, out: &mut Vec<Instance>) -> Result<usize> {
        let here = [camera.x, -camera.z];
        let mut listed = 0;
        if let Some(v) = &self.veg {
            if self.plants_at.is_none_or(|p| (p[0] - here[0]).hypot(p[1] - here[1]) > RELIST) {
                self.plants = v.instances(&town.height, here, TREE_MESH_R, GRASS_MESH_R);
                r.set_plant_state(&v.mesh_state(here, TREE_MESH_R))?;
                r.set_plant_group(1, &v.grass_impostors(&town.height, here, GRASS_MESH_R, GRASS_FAR))?;
                self.plants_at = Some(here);
                listed = self.plants.len();
            }
        }
        out.clear();
        out.extend_from_slice(&self.base);
        // the door leaves (before the plants: the motion buffer covers instances up to the last
        // one that moved), with where each was last frame, so the denoiser follows a door that
        // swings or slides instead of leaving a ghost of it
        if let Some(p) = doors {
            let at = out.len();
            p.door_instances(out);
            let moved: Vec<(usize, akari::glam::Affine3A)> = if self.doors_prev.len() == out.len() - at {
                out[at..].iter().zip(&self.doors_prev).enumerate().filter(|(_, (i, t))| i.transform != **t).map(|(k, (_, t))| (at + k, *t)).collect()
            } else {
                Vec::new()
            };
            r.set_moved(&moved);
            self.doors_prev.clear();
            self.doors_prev.extend(out[at..].iter().map(|i| i.transform));
        }
        out.extend_from_slice(&self.plants);
        if let Some(l) = &self.lanes {
            l.instances(time, out);
        }
        if let Some((m, at)) = &self.shop_sign {
            let turn = if self.shop_open { akari::glam::Affine3A::IDENTITY } else { akari::glam::Affine3A::from_rotation_y(std::f32::consts::PI) };
            out.extend(m.instances(*at * turn));
        }
        self.cloth.upload(r)?;
        self.cloth.instances(out);
        Ok(listed)
    }
}

/// Where the player starts: inside Tsukumodō, by its door, looking into the shop. (feet, yaw)
pub fn home_start(town: &Town) -> (Vec3, f32) {
    let h = &town.plan.houses[town.plan.home];
    let n = h.house.len() as f32;
    let c = h.house.iter().fold([0.0f32; 2], |a, q| [a[0] + q[0] / n, a[1] + q[1] / n]);
    let (cx, cy) = (c[0], c[1]);
    let (dx, dy) = (h.gate[0] - cx, h.gate[1] - cy);
    let l = dx.hypot(dy).max(1e-3);
    let (ox, oy) = (dx / l, dy / l);
    // 2.6 m in from the house's middle towards the street: just inside the doors
    let at = [cx + ox * 2.6, cy + oy * 2.6];
    // facing back into the shop (plan (x, y) -> world (x, -z); yaw 0 looks north)
    let yaw = (-ox).atan2(-oy);
    (Vec3::new(at[0], h.ground + 0.15, -at[1]), yaw)
}
