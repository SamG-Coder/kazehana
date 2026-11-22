//! Models as something to walk in: their solid shapes and their doors (play crate). Used for the
//! town (what the plots put down, the terrain) and for the game's levels.
//!
//! Not solid yet in the town: trees, rails, bridge parapets (the bridge decks are: they are part
//! of the ground's surface).

use crate::plots::PlotsScene;
use crate::Town;
use akari::glam::{Affine3A, Vec2, Vec3};
use akari::scene::{Instance, MeshId};
use akari::Model;
use play::{Doors, Shape, World};
use std::collections::HashMap;
use std::sync::Arc;

/// A door leaf's mesh, moved by its door.
pub struct DoorLeaf {
    pub door: usize,
    pub mesh: MeshId,
}

/// A place to walk in: what is solid, its doors, and the door leaves to draw.
#[derive(Default)]
pub struct Playable {
    pub world: World,
    pub doors: Doors,
    pub leaves: Vec<DoorLeaf>,
    /// each model's shape, built once (by model name)
    shapes: HashMap<String, Arc<Shape>>,
}

/// Is this node a door leaf (play::door names)? Such parts are drawn by their doors, not with
/// the model.
pub fn is_door(node: &str) -> bool {
    node.starts_with("kzdoor_")
}

/// Is this node cloth (a noren, a curtain: `kzcloth_*`)? Drawn, but walked through.
pub fn is_cloth(node: &str) -> bool {
    node.starts_with("kzcloth_")
}

/// The model placed at `at`, without its door leaves and its cloth (crate::cloth draws that).
pub fn static_instances(model: &Model, at: Affine3A) -> impl Iterator<Item = Instance> + '_ {
    model.parts.iter().filter(|p| !is_door(&p.node) && !(is_cloth(&p.node) && p.cloth.is_some()) && !crate::lanes::is_lane_part(&p.node)).map(move |p| Instance { mesh: p.mesh, transform: at * p.transform })
}

impl Playable {
    pub fn new() -> Self {
        Self::default()
    }

    /// A model put down: solid where its triangles are (`boxed`: its bounding box instead, for
    /// hedges and small detailed things), and its doors.
    pub fn add_model(&mut self, model: &Model, at: Affine3A, boxed: bool) {
        if boxed {
            let (s, q, t) = at.to_scale_rotation_translation();
            self.world.add(Shape::cuboid(model.min * s, model.max * s), Affine3A::from_rotation_translation(q, t));
            return;
        }
        let shape = self
            .shapes
            .entry(model.name.clone())
            .or_insert_with(|| Shape::new(model.parts.iter().filter(|p| !is_door(&p.node) && !is_cloth(&p.node)).flat_map(|p| p.triangles.iter().map(move |t| t.map(|v| p.transform.transform_point3(v))))))
            .clone();
        self.world.add(shape, at);
        // its doors: one per door node (a node may have several meshes)
        let mut by_node: HashMap<&str, Option<usize>> = HashMap::new();
        for p in model.parts.iter().filter(|p| is_door(&p.node)) {
            let door = *by_node.entry(p.node.as_str()).or_insert_with(|| {
                let tris: Vec<[Vec3; 3]> = model.parts.iter().filter(|q| q.node == p.node).flat_map(|q| q.triangles.iter().copied()).collect();
                self.doors.add(&mut self.world, &p.node, at * p.transform, tris)
            });
            if let Some(door) = door {
                self.leaves.push(DoorLeaf { door, mesh: p.mesh });
            }
        }
    }

    /// Triangles already in the world (built geometry: walls, floors).
    pub fn add_triangles(&mut self, tris: impl IntoIterator<Item = [Vec3; 3]>) {
        let s = Shape::new(tris);
        if s.triangle_count() > 0 {
            self.world.add(s, Affine3A::IDENTITY);
        }
    }

    /// A solid box (world).
    pub fn add_box(&mut self, min: Vec3, max: Vec3) {
        self.world.add(Shape::cuboid(min, max), Affine3A::IDENTITY);
    }

    /// The town: the plots' buildings, props and walls, on the terrain.
    pub fn build(town: &Town, plots: &PlotsScene) -> Self {
        let t0 = std::time::Instant::now();
        let mut p = Playable::new();
        p.world.set_ground(ground_fn(town));
        let models: HashMap<&str, &Model> = plots.models.iter().map(|m| (m.name.as_str(), m)).collect();
        for (name, at) in &plots.placed {
            if let Some(m) = models.get(name.as_str()) {
                // hedges, mailboxes and small things (stock on the shelves, cards on stands):
                // their box (their triangles are leaves and fine detail nobody walks into)
                let small = (m.max - m.min).max_element() < 0.4;
                p.add_model(m, *at, small || name.contains("hedge") || name.contains("mailbox"));
            }
        }
        p.add_triangles(plots.solid.iter().copied());
        println!("collision: {} bodies ({} shapes), {} doors, built in {:.1}s", p.world.body_count(), p.shapes.len(), p.doors.doors.len(), t0.elapsed().as_secs_f32());
        p
    }

    /// The door leaves where their doors are now.
    pub fn door_instances(&self, out: &mut Vec<Instance>) {
        out.extend(self.leaves.iter().map(|l| Instance { mesh: l.mesh, transform: self.doors.doors[l.door].transform() }));
    }
}

/// The walking surface as a function of world (x, z): the terrain, and the road decks that stand
/// above it (bridges).
pub fn ground_fn(town: &Town) -> impl Fn(f32, f32) -> f32 + Send + Sync + 'static {
    let height = town.height.clone();
    // the road segments raised above the ground: (a, b, height at a, at b, half width)
    let mut decks: Vec<(Vec2, Vec2, f32, f32, f32)> = Vec::new();
    for road in &town.plan.roads {
        let Some(h) = &road.h else { continue };
        for i in 1..road.pts.len() {
            let (a, b) = (Vec2::from(road.pts[i - 1]), Vec2::from(road.pts[i]));
            let raised = |p: Vec2, rh: f32| rh - height.plan(p.x, p.y) > 0.3;
            if raised(a, h[i - 1]) || raised(b, h[i]) || raised((a + b) * 0.5, (h[i - 1] + h[i]) * 0.5) {
                decks.push((a, b, h[i - 1] + 0.04, h[i] + 0.04, road.width / 2.0));
            }
        }
    }
    move |x: f32, z: f32| {
        let q = Vec2::new(x, -z);
        let g = height.plan(q.x, q.y);
        let mut best = g;
        for &(a, b, ha, hb, hw) in &decks {
            let ab = b - a;
            let t = ((q - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            if (a + ab * t - q).length() <= hw {
                let rh = ha + (hb - ha) * t;
                if rh > best && rh - g < 6.0 {
                    best = rh;
                }
            }
        }
        best
    }
}
